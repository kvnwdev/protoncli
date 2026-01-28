use crate::core::auth::KeychainManager;
use crate::models::account::{Account, SecurityType};
use crate::models::filter::MessageFilter;
use crate::models::folder::Folder;
use crate::models::message::{EmailAddress, Message, MessageFlags};
use crate::utils::batch::{chunk_uids, FETCH_BATCH_SIZE};
use anyhow::{anyhow, Context, Result};
use async_imap::Session;
use async_native_tls::{TlsConnector, TlsStream};
use chrono::{DateTime, Utc};
use futures::stream::StreamExt;
use secrecy::ExposeSecret;
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

/// Statistics about a fetch operation (useful for diagnostics)
#[derive(Debug, Clone, Default)]
pub struct FetchStats {
    /// Number of UIDs returned by IMAP search
    pub search_count: usize,
    /// Number of UIDs requested for fetch (after limit applied)
    pub fetch_count: usize,
    /// Number of messages successfully parsed
    pub parsed_count: usize,
    /// Number of messages that failed to parse
    pub skipped_count: usize,
}

pub struct ImapClient {
    session: Session<TlsStream<Compat<TcpStream>>>,
}

impl ImapClient {
    pub async fn connect(account: &Account) -> Result<Self> {
        let password = KeychainManager::get_password(&account.email)
            .context("Password not found in keychain. Please add the account first.")?;

        let addr = format!("{}:{}", account.imap_host, account.imap_port);

        let client = match account.imap_security {
            SecurityType::StartTls => {
                // Step 1: Plain TCP connection
                let tcp_stream = TcpStream::connect(&addr)
                    .await
                    .context(format!("Failed to connect to IMAP server at {}", addr))?;

                // Step 2: Convert to Compat for futures compatibility
                let tcp_stream = tcp_stream.compat();

                // Step 3: Create plain IMAP client
                let mut client = async_imap::Client::new(tcp_stream);

                // Step 4: Send STARTTLS command
                client
                    .run_command_and_check_ok("STARTTLS", None)
                    .await
                    .context("Failed to execute STARTTLS command")?;

                // Step 5: Extract underlying stream
                let tcp_stream = client.into_inner();

                // Step 6: Upgrade to TLS
                // SECURITY NOTE: We accept invalid certificates here because ProtonMail Bridge
                // runs on localhost (127.0.0.1) and uses a self-signed certificate. This is
                // intentional and safe because:
                //   1. This client is designed exclusively for local ProtonMail Bridge connections
                //   2. The default imap_host is hardcoded to 127.0.0.1 (see Account::new_protonmail_bridge)
                //   3. Traffic stays on the local machine and cannot be intercepted
                // Do NOT use this pattern for connections to external servers.
                let tls_connector = TlsConnector::new().danger_accept_invalid_certs(true);

                let tls_stream = tls_connector
                    .connect(&account.imap_host, tcp_stream)
                    .await
                    .context("Failed to upgrade connection to TLS")?;

                // Step 7: Create new client with TLS stream
                async_imap::Client::new(tls_stream)
            }
            SecurityType::Ssl => {
                // Direct TLS connection (e.g., IMAPS on port 993)
                let tcp_stream = TcpStream::connect(&addr)
                    .await
                    .context(format!("Failed to connect to IMAP server at {}", addr))?;

                let tcp_stream = tcp_stream.compat();

                // SECURITY NOTE: See comment above about why we accept invalid certificates.
                // This is safe for localhost ProtonMail Bridge connections only.
                let tls_connector = TlsConnector::new().danger_accept_invalid_certs(true);

                let tls_stream = tls_connector
                    .connect(&account.imap_host, tcp_stream)
                    .await
                    .context("Failed to establish TLS connection")?;

                async_imap::Client::new(tls_stream)
            }
        };

        // Step 8: Authenticate
        let session = client
            .login(&account.email, password.expose_secret())
            .await
            .map_err(|e| anyhow!("Authentication failed: {}", e.0))?;

        Ok(Self { session })
    }

    pub async fn test_connection(account: &Account) -> Result<String> {
        let mut client = Self::connect(account).await?;

        // Try to list folders to verify full functionality
        let _mailboxes = client
            .session
            .list(None, Some("*"))
            .await
            .context("Failed to list mailboxes")?;

        Ok(format!(
            "Successfully connected to ProtonMail Bridge for {}",
            account.email
        ))
    }

    pub async fn list_folders(&mut self) -> Result<Vec<Folder>> {
        let mut mailboxes_stream = self
            .session
            .list(None, Some("*"))
            .await
            .context("Failed to list mailboxes")?;

        let mut folders = Vec::new();

        while let Some(mailbox_result) = mailboxes_stream.next().await {
            let mailbox = mailbox_result.context("Failed to read mailbox from stream")?;
            let name = mailbox.name().to_string();
            let delimiter = mailbox.delimiter().map(|d| d.to_string());

            folders.push(Folder::new(name, delimiter));
        }

        // Sort folders by path for consistent output
        folders.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(folders)
    }

    pub async fn select_folder(&mut self, folder: &str) -> Result<()> {
        self.session
            .select(folder)
            .await
            .context(format!("Failed to select folder: {}", folder))?;

        Ok(())
    }

    pub async fn fetch_messages(
        &mut self,
        filter: &MessageFilter,
    ) -> Result<(Vec<Message>, FetchStats)> {
        let mut stats = FetchStats::default();

        // Build and execute search query
        let search_query = filter.build_imap_search_query()?;
        let uids_set = self
            .session
            .uid_search(&search_query)
            .await
            .context("Failed to search messages")?;

        let mut uids: Vec<u32> = uids_set.into_iter().collect();
        uids.sort(); // Ensure deterministic ordering
        stats.search_count = uids.len();

        if uids.is_empty() {
            return Ok((vec![], stats));
        }

        // Apply limit if specified
        let uids_to_fetch: Vec<_> = if let Some(limit) = filter.limit {
            uids.into_iter().rev().take(limit).collect()
        } else {
            uids.into_iter().rev().collect()
        };
        stats.fetch_count = uids_to_fetch.len();

        // Fetch headers using BODY.PEEK[HEADER] and parse with mail-parser
        // This avoids async-imap's ENVELOPE parsing which has Unicode issues
        // For preview, fetch full RFC822 to properly parse MIME content
        let fetch_query = if filter.preview {
            "(UID FLAGS RFC822)"
        } else {
            "(UID FLAGS BODY.PEEK[HEADER])"
        };

        let mut messages = Vec::new();

        // Fetch in batches to work around ProtonMail Bridge issues with large requests
        let batches = chunk_uids(&uids_to_fetch, FETCH_BATCH_SIZE);

        for batch in batches.iter() {
            let uid_set = batch
                .iter()
                .map(|u| u.to_string())
                .collect::<Vec<_>>()
                .join(",");

            let mut messages_stream = self
                .session
                .uid_fetch(&uid_set, fetch_query)
                .await
                .context("Failed to fetch messages")?;

            while let Some(fetch_result) = messages_stream.next().await {
                let fetch = match fetch_result {
                    Ok(f) => f,
                    Err(_) => {
                        // Skip messages that fail to parse (often due to Unicode issues in headers)
                        stats.skipped_count += 1;
                        continue;
                    }
                };

                if let Some(uid) = fetch.uid {
                    let mut message = Message::new(uid);

                    // Parse flags
                    let flags: Vec<_> = fetch.flags().collect();
                    message.flags = MessageFlags::from_imap_flags(&flags);

                    // Get the raw bytes - either from body() for RFC822 or header() for BODY.PEEK[HEADER]
                    let mail_bytes = fetch.body().or_else(|| fetch.header());

                    // Parse using mail-parser (more robust Unicode handling than ENVELOPE)
                    if let Some(bytes) = mail_bytes {
                        if let Some(parsed_mail) =
                            mail_parser::MessageParser::default().parse(bytes)
                        {
                            message.subject = parsed_mail.subject().map(String::from);
                            message.message_id = parsed_mail.message_id().map(String::from);

                            // Parse from address
                            if let Some(from_addr) =
                                parsed_mail.from().and_then(|addrs| addrs.first())
                            {
                                if let Some(email) = from_addr.address() {
                                    message.from = Some(EmailAddress::new(
                                        email.to_string(),
                                        from_addr.name().map(String::from),
                                    ));
                                }
                            }

                            // Parse to addresses
                            if let Some(to_addrs) = parsed_mail.to() {
                                for addr in to_addrs.iter() {
                                    if let Some(email) = addr.address() {
                                        message.to.push(EmailAddress::new(
                                            email.to_string(),
                                            addr.name().map(String::from),
                                        ));
                                    }
                                }
                            }

                            // Parse date
                            if let Some(date) = parsed_mail.date() {
                                message.date = Some(
                                    DateTime::from_timestamp(date.to_timestamp(), 0)
                                        .unwrap_or_else(Utc::now),
                                );
                            }

                            // Extract preview from body (only when we have full RFC822)
                            if filter.preview {
                                // Try to get plain text body first
                                if let Some(body_text) = parsed_mail.body_text(0) {
                                    let preview: String = body_text.chars().take(200).collect();
                                    if !preview.trim().is_empty() {
                                        message.preview = Some(preview.trim().to_string());
                                    }
                                }
                                // If no text part, try HTML and strip tags
                                else if let Some(body_html) = parsed_mail.body_html(0) {
                                    // Basic HTML stripping - just remove tags for preview
                                    let text =
                                        body_html.replace("<br>", "\n").replace("</p>", "\n");
                                    let preview: String = text
                                        .split('<')
                                        .map(|s| {
                                            s.split_once('>').map(|(_, rest)| rest).unwrap_or(s)
                                        })
                                        .collect::<String>()
                                        .chars()
                                        .take(200)
                                        .collect();
                                    if !preview.trim().is_empty() {
                                        message.preview = Some(preview.trim().to_string());
                                    }
                                }
                            }
                        }
                    }

                    messages.push(message);
                }
            }
        }

        stats.parsed_count = messages.len();

        if stats.skipped_count > 0 {
            eprintln!(
                "\nNote: Skipped {} message(s) due to parsing errors.",
                stats.skipped_count
            );
        }

        // Warn if Bridge returned significantly fewer messages than requested
        let expected = stats.fetch_count.saturating_sub(stats.skipped_count);
        if stats.parsed_count < expected && expected > 0 {
            let missing = expected - stats.parsed_count;
            eprintln!(
                "\nWarning: ProtonMail Bridge returned {} fewer message(s) than expected.",
                missing
            );
            eprintln!("This is a known Bridge issue with large folders. Try using --days to filter by date.");
        }

        // Sort by UID descending (newest first) for deterministic ordering
        messages.sort_by(|a, b| b.uid.cmp(&a.uid));

        Ok((messages, stats))
    }

    pub async fn fetch_message_by_uid(
        &mut self,
        uid: u32,
        folder: &str,
        include_raw: bool,
    ) -> Result<Message> {
        // Select folder
        self.select_folder(folder).await?;

        // Fetch full message with RFC822 using UID
        let fetch_query = "RFC822";
        let mut messages_stream = self
            .session
            .uid_fetch(&uid.to_string(), fetch_query)
            .await
            .context("Failed to fetch message")?;

        // Get first (and only) result
        if let Some(fetch_result) = messages_stream.next().await {
            let fetch = fetch_result.context("Failed to read message")?;

            if let Some(body_bytes) = fetch.body() {
                // Parse with mail-parser
                if let Some(parsed_mail) = mail_parser::MessageParser::default().parse(body_bytes) {
                    let mut message = Message::new(uid);

                    // Extract subject
                    message.subject = parsed_mail.subject().map(String::from);
                    message.message_id = parsed_mail.message_id().map(String::from);

                    // Extract from address
                    if let Some(from_addr) = parsed_mail.from().and_then(|addrs| addrs.first()) {
                        if let Some(email) = from_addr.address() {
                            message.from = Some(EmailAddress::new(
                                email.to_string(),
                                from_addr.name().map(String::from),
                            ));
                        }
                    }

                    // Extract to addresses
                    if let Some(to_addrs) = parsed_mail.to() {
                        if let Some(addrs) = to_addrs.as_list() {
                            for addr in addrs {
                                if let Some(email) = addr.address() {
                                    message.to.push(EmailAddress::new(
                                        email.to_string(),
                                        addr.name().map(String::from),
                                    ));
                                }
                            }
                        }
                    }

                    // Extract cc addresses
                    if let Some(cc_addrs) = parsed_mail.cc() {
                        if let Some(addrs) = cc_addrs.as_list() {
                            for addr in addrs {
                                if let Some(email) = addr.address() {
                                    message.cc.push(EmailAddress::new(
                                        email.to_string(),
                                        addr.name().map(String::from),
                                    ));
                                }
                            }
                        }
                    }

                    // Extract bcc addresses
                    if let Some(bcc_addrs) = parsed_mail.bcc() {
                        if let Some(addrs) = bcc_addrs.as_list() {
                            for addr in addrs {
                                if let Some(email) = addr.address() {
                                    message.bcc.push(EmailAddress::new(
                                        email.to_string(),
                                        addr.name().map(String::from),
                                    ));
                                }
                            }
                        }
                    }

                    // Extract reply-to
                    if let Some(reply_to_addrs) = parsed_mail.reply_to() {
                        if let Some(addrs) = reply_to_addrs.as_list() {
                            if let Some(addr) = addrs.first() {
                                if let Some(email) = addr.address() {
                                    message.reply_to = Some(EmailAddress::new(
                                        email.to_string(),
                                        addr.name().map(String::from),
                                    ));
                                }
                            }
                        }
                    }

                    // Extract date
                    if let Some(date) = parsed_mail.date() {
                        // Convert mail_parser::DateTime to chrono::DateTime<Utc>
                        message.date = Some(
                            DateTime::from_timestamp(date.to_timestamp(), 0)
                                .unwrap_or_else(Utc::now),
                        );
                    }

                    // Extract body text and HTML
                    message.body_text = parsed_mail.body_text(0).map(String::from);
                    message.body_html = parsed_mail.body_html(0).map(String::from);

                    // Extract headers
                    for header in parsed_mail.headers() {
                        let name = header.name().to_string();
                        if let Some(value) = header.value().as_text() {
                            message.headers.insert(name, value.to_string());
                        }
                    }

                    // Store raw if requested
                    if include_raw {
                        message.raw_message = Some(body_bytes.to_vec());
                    }

                    // Parse flags
                    let flags: Vec<_> = fetch.flags().collect();
                    message.flags = MessageFlags::from_imap_flags(&flags);

                    return Ok(message);
                }
            }
        }

        Err(anyhow!(
            "Message with UID {} not found in folder {}",
            uid,
            folder
        ))
    }

    pub async fn mark_message_read(&mut self, uid: u32, folder: &str) -> Result<()> {
        self.select_folder(folder).await?;

        let mut store_stream = self
            .session
            .uid_store(&uid.to_string(), "+FLAGS (\\Seen)")
            .await
            .context("Failed to mark message as read")?;

        // Consume the stream to complete the operation
        while store_stream.next().await.is_some() {}

        Ok(())
    }

    /// Copy messages to a destination folder
    pub async fn copy_messages(&mut self, uids: &[u32], dest_folder: &str) -> Result<()> {
        if uids.is_empty() {
            return Ok(());
        }

        let uid_set = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        self.session
            .uid_copy(&uid_set, dest_folder)
            .await
            .context(format!(
                "Failed to copy messages to folder: {}",
                dest_folder
            ))?;

        Ok(())
    }

    /// Move messages to a destination folder (COPY + DELETE + EXPUNGE)
    pub async fn move_messages(&mut self, uids: &[u32], dest_folder: &str) -> Result<()> {
        if uids.is_empty() {
            return Ok(());
        }

        // Copy to destination
        self.copy_messages(uids, dest_folder).await?;

        // Mark as deleted in source
        self.mark_messages_deleted(uids).await?;

        // Expunge deleted messages
        self.expunge().await?;

        Ok(())
    }

    /// Mark messages with \Deleted flag
    pub async fn mark_messages_deleted(&mut self, uids: &[u32]) -> Result<()> {
        if uids.is_empty() {
            return Ok(());
        }

        let uid_set = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let mut store_stream = self
            .session
            .uid_store(&uid_set, "+FLAGS (\\Deleted)")
            .await
            .context("Failed to mark messages as deleted")?;

        // Consume the stream to complete the operation
        while store_stream.next().await.is_some() {}

        Ok(())
    }

    /// Expunge deleted messages from the current folder
    pub async fn expunge(&mut self) -> Result<()> {
        let expunge_stream = self
            .session
            .expunge()
            .await
            .context("Failed to expunge deleted messages")?;

        // Collect the stream to consume it and complete the operation
        let _: Vec<_> = expunge_stream.collect().await;

        Ok(())
    }

    /// Modify flags on messages (add or remove)
    pub async fn modify_flags(&mut self, uids: &[u32], flags: &str, add: bool) -> Result<()> {
        if uids.is_empty() {
            return Ok(());
        }

        let uid_set = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let flag_command = if add {
            format!("+FLAGS ({})", flags)
        } else {
            format!("-FLAGS ({})", flags)
        };

        let mut store_stream = self
            .session
            .uid_store(&uid_set, &flag_command)
            .await
            .context(format!("Failed to modify flags: {}", flags))?;

        // Consume the stream to complete the operation
        while store_stream.next().await.is_some() {}

        Ok(())
    }

    /// Mark multiple messages as read
    pub async fn mark_messages_read(&mut self, uids: &[u32]) -> Result<()> {
        self.modify_flags(uids, "\\Seen", true).await
    }

    /// Mark multiple messages as unread
    pub async fn mark_messages_unread(&mut self, uids: &[u32]) -> Result<()> {
        self.modify_flags(uids, "\\Seen", false).await
    }

    /// Star messages (add \Flagged)
    pub async fn star_messages(&mut self, uids: &[u32]) -> Result<()> {
        self.modify_flags(uids, "\\Flagged", true).await
    }

    /// Unstar messages (remove \Flagged)
    pub async fn unstar_messages(&mut self, uids: &[u32]) -> Result<()> {
        self.modify_flags(uids, "\\Flagged", false).await
    }

    /// Check if a folder exists
    pub async fn folder_exists(&mut self, folder: &str) -> Result<bool> {
        let mut mailboxes_stream = self
            .session
            .list(None, Some(folder))
            .await
            .context("Failed to check folder existence")?;

        // Check if the folder is in the results
        while let Some(mailbox_result) = mailboxes_stream.next().await {
            if let Ok(mailbox) = mailbox_result {
                if mailbox.name() == folder {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Create a new folder/mailbox
    pub async fn create_folder(&mut self, folder: &str) -> Result<()> {
        self.session
            .create(folder)
            .await
            .context(format!("Failed to create folder: {}", folder))?;

        Ok(())
    }

    /// Delete a folder/mailbox (must be empty)
    pub async fn delete_folder(&mut self, folder: &str) -> Result<()> {
        self.session
            .delete(folder)
            .await
            .context(format!("Failed to delete folder: {}", folder))?;

        Ok(())
    }

    /// Rename a folder/mailbox
    pub async fn rename_folder(&mut self, from: &str, to: &str) -> Result<()> {
        self.session.rename(from, to).await.context(format!(
            "Failed to rename folder from '{}' to '{}'",
            from, to
        ))?;

        Ok(())
    }
}
