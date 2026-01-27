use crate::core::imap::ImapClient;
use crate::core::state::{validate_shadow_uids, StateManager};
use crate::models::config::Config;
use crate::models::filter::MessageFilter;
use crate::models::message::Message;
use crate::output::{json, markdown, table};
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

    // Initialize state manager for shadow UID assignment
    let state = StateManager::new().await?;

    // If agent_unread filter is set, check the database
    if agent_unread {
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
    }

    // Assign shadow UIDs to all messages and update state database
    for message in &mut messages {
        message.folder = Some("INBOX".to_string());

        // Get or create shadow UID for this message
        if let Some(ref msg_id) = message.message_id {
            let shadow_uid = state
                .get_or_create_shadow_uid(
                    &account.email,
                    "INBOX",
                    message.uid,
                    Some(msg_id),
                    message.subject.as_deref(),
                    message.from.as_ref().map(|f| f.address.as_str()),
                    message.date,
                )
                .await?;
            message.shadow_uid = Some(shadow_uid);
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
        "table" => {
            table::print_message_table(&account.email, "INBOX", &messages);
        }
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}

pub async fn read_message(
    shadow_uid: i64,
    folder_override: Option<&str>,
    output_format: Option<&str>,
    mark_read: bool,
    show_raw: bool,
) -> Result<()> {
    // Validate shadow UID
    validate_shadow_uids(&[shadow_uid])?;

    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;

    // Resolve shadow UID to current IMAP location
    let resolved = state
        .resolve_shadow_uids(&account.email, &[shadow_uid])
        .await?;
    let msg_info = resolved
        .first()
        .ok_or_else(|| anyhow!("Message {} not found", shadow_uid))?;

    // Use folder override if provided, otherwise use resolved folder
    let folder_name = folder_override.unwrap_or(&msg_info.folder);

    // Connect and fetch message
    let mut client = ImapClient::connect(account).await?;
    let mut message = client
        .fetch_message_by_uid(msg_info.imap_uid, folder_name, show_raw)
        .await?;

    // Set shadow_uid on the message
    message.shadow_uid = Some(shadow_uid);
    message.folder = Some(folder_name.to_string());

    // Mark as read in IMAP if requested
    if mark_read {
        client
            .mark_message_read(msg_info.imap_uid, folder_name)
            .await?;
    }

    // Always mark as agent-read in local state (using message_id as stable identifier)
    if let Some(ref msg_id) = message.message_id {
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
