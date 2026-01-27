use crate::core::imap::ImapClient;
use crate::core::state::StateManager;
use crate::models::config::Config;
use crate::models::filter::MessageFilter;
use crate::models::message::Message;
use crate::output::{json, markdown};
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize)]
struct InboxOutput {
    account: String,
    folder: String,
    count: usize,
    messages: Vec<Message>,
}

pub async fn list_inbox(
    days: Option<u32>,
    unread_only: bool,
    agent_unread: bool,
    limit: Option<usize>,
    output_format: Option<&str>,
    query: Option<String>,
    preview: bool,
) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    // Build filter
    let mut filter = MessageFilter::new();
    if let Some(d) = days {
        filter = filter.with_days(d);
    }
    if unread_only {
        filter = filter.with_unread_only(true);
    }
    if agent_unread {
        filter = filter.with_agent_unread(true);
    }
    if let Some(l) = limit {
        filter = filter.with_limit(l);
    }
    if let Some(q) = query {
        filter = filter.with_query(q);
    }
    if preview {
        filter = filter.with_preview(true);
    }

    // Connect and fetch messages
    let mut client = ImapClient::connect(account).await?;
    client.select_folder("INBOX").await?;
    let mut messages = client.fetch_messages(&filter).await?;

    // If agent_unread filter is set, check the database
    if agent_unread {
        let state = StateManager::new().await?;
        let mut filtered_messages = Vec::new();

        for message in messages {
            // Use message_id for agent_read lookup
            if let Some(ref msg_id) = message.message_id {
                let is_read = state.is_agent_read(&account.email, msg_id).await?;
                if !is_read {
                    filtered_messages.push(message);
                }
            } else {
                // No message_id means we haven't seen it before
                filtered_messages.push(message);
            }
        }

        messages = filtered_messages;
    } else {
        // Update state database with message metadata
        let state = StateManager::new().await?;
        for message in &messages {
            state
                .upsert_message(
                    &account.email,
                    "INBOX",
                    message.uid,
                    message.message_id.as_deref(),
                    message.subject.as_deref(),
                    message.from.as_ref().map(|f| f.address.as_str()),
                    message.date,
                )
                .await?;
        }
    }

    match output_format.unwrap_or("json") {
        "json" => {
            let output = InboxOutput {
                account: account.email.clone(),
                folder: "INBOX".to_string(),
                count: messages.len(),
                messages,
            };
            json::print_json(&output)?;
        }
        "markdown" => {
            markdown::print_message_list(&account.email, "INBOX", &messages);
        }
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}

pub async fn read_message(
    uid: u32,
    folder: Option<&str>,
    output_format: Option<&str>,
    mark_read: bool,
    show_raw: bool,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let folder_name = folder.unwrap_or("INBOX");

    // Connect and fetch message
    let mut client = ImapClient::connect(account).await?;
    let message = client
        .fetch_message_by_uid(uid, folder_name, show_raw)
        .await?;

    // Mark as read in IMAP if requested
    if mark_read {
        client.mark_message_read(uid, folder_name).await?;
    }

    // Always mark as agent-read in local state (using message_id as stable identifier)
    if let Some(ref msg_id) = message.message_id {
        let state = StateManager::new().await?;
        // First upsert to ensure the message exists in our database
        state
            .upsert_message(
                &account.email,
                folder_name,
                uid,
                Some(msg_id),
                message.subject.as_deref(),
                message.from.as_ref().map(|f| f.address.as_str()),
                message.date,
            )
            .await?;
        // Then mark as agent-read
        state.mark_agent_read(&account.email, msg_id).await?;
    }

    // Output based on format
    match output_format.unwrap_or("markdown") {
        "json" => {
            json::print_json(&message)?;
        }
        "markdown" => {
            markdown::print_message(&message);
        }
        "raw" => {
            if let Some(raw) = message.raw_message {
                println!("{}", String::from_utf8_lossy(&raw));
            } else {
                return Err(anyhow!("Raw message not available. Use --raw flag."));
            }
        }
        _ => return Err(anyhow!("Invalid output format")),
    }

    Ok(())
}
