use crate::core::imap::ImapClient;
use crate::models::config::Config;
use crate::output::json;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::io::{self, Write};

#[derive(Serialize)]
pub struct ActionOutput {
    pub action: String,
    pub account: String,
    pub source_folder: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_folder: Option<String>,
    pub uids: Vec<u32>,
    pub success_count: usize,
    pub failed_uids: Vec<u32>,
}

/// Resolve common folder name aliases to their actual IMAP names
fn resolve_folder_path(folder: &str) -> String {
    match folder.to_lowercase().as_str() {
        "inbox" => "INBOX".to_string(),
        "archive" => "Archive".to_string(),
        "trash" => "Trash".to_string(),
        "sent" => "Sent".to_string(),
        "drafts" => "Drafts".to_string(),
        "spam" | "junk" => "Spam".to_string(),
        "all" | "all mail" => "All Mail".to_string(),
        _ => folder.to_string(),
    }
}

/// Move messages from one folder to another
pub async fn move_messages(
    uids: Vec<u32>,
    from: &str,
    to: &str,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let source_folder = resolve_folder_path(from);
    let dest_folder = resolve_folder_path(to);

    let mut client = ImapClient::connect(account).await?;

    // Validate destination folder exists
    if !client.folder_exists(&dest_folder).await? {
        return Err(anyhow!("Destination folder '{}' does not exist", dest_folder));
    }

    // Select source folder and move messages
    client.select_folder(&source_folder).await?;
    client.move_messages(&uids, &dest_folder).await?;

    // Note: State tracking uses message_id as stable identifier.
    // Location gets updated automatically when message is seen in new folder.

    let output = ActionOutput {
        action: "move".to_string(),
        account: account.email.clone(),
        source_folder: source_folder.clone(),
        dest_folder: Some(dest_folder.clone()),
        uids: uids.clone(),
        success_count: uids.len(),
        failed_uids: vec![],
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!(
            "\u{2713} Moved {} message(s) from {} \u{2192} {}",
            uids.len(),
            source_folder,
            dest_folder
        ),
    }

    Ok(())
}

/// Copy messages from one folder to another
pub async fn copy_messages(
    uids: Vec<u32>,
    from: &str,
    to: &str,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let source_folder = resolve_folder_path(from);
    let dest_folder = resolve_folder_path(to);

    let mut client = ImapClient::connect(account).await?;

    // Validate destination folder exists
    if !client.folder_exists(&dest_folder).await? {
        return Err(anyhow!("Destination folder '{}' does not exist", dest_folder));
    }

    // Select source folder and copy messages
    client.select_folder(&source_folder).await?;
    client.copy_messages(&uids, &dest_folder).await?;

    let output = ActionOutput {
        action: "copy".to_string(),
        account: account.email.clone(),
        source_folder: source_folder.clone(),
        dest_folder: Some(dest_folder.clone()),
        uids: uids.clone(),
        success_count: uids.len(),
        failed_uids: vec![],
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!(
            "\u{2713} Copied {} message(s) from {} \u{2192} {}",
            uids.len(),
            source_folder,
            dest_folder
        ),
    }

    Ok(())
}

/// Delete messages (move to Trash or permanent delete)
pub async fn delete_messages(
    uids: Vec<u32>,
    from: &str,
    permanent: bool,
    yes: bool,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let source_folder = resolve_folder_path(from);

    // Confirm permanent delete if not already confirmed
    if permanent && !yes {
        print!(
            "Permanently delete {} message(s)? This cannot be undone. [y/N] ",
            uids.len()
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let mut client = ImapClient::connect(account).await?;
    client.select_folder(&source_folder).await?;

    if permanent {
        // Permanent delete: mark as deleted and expunge
        client.mark_messages_deleted(&uids).await?;
        client.expunge().await?;
    } else {
        // Soft delete: move to Trash
        let trash_folder = resolve_folder_path("trash");
        if !client.folder_exists(&trash_folder).await? {
            return Err(anyhow!("Trash folder not found"));
        }
        client.move_messages(&uids, &trash_folder).await?;
    }

    // Note: State tracking uses message_id as stable identifier.
    // Location gets updated automatically when message is seen in new folder.

    let output = ActionOutput {
        action: "delete".to_string(),
        account: account.email.clone(),
        source_folder: source_folder.clone(),
        dest_folder: if permanent {
            None
        } else {
            Some("Trash".to_string())
        },
        uids: uids.clone(),
        success_count: uids.len(),
        failed_uids: vec![],
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => {
            if permanent {
                println!("\u{2713} Permanently deleted {} message(s)", uids.len());
            } else {
                println!(
                    "\u{2713} Moved {} message(s) to Trash",
                    uids.len()
                );
            }
        }
    }

    Ok(())
}

/// Archive messages (shortcut for move to Archive)
pub async fn archive_messages(
    uids: Vec<u32>,
    from: &str,
    output_format: Option<&str>,
) -> Result<()> {
    move_messages(uids, from, "Archive", output_format).await
}

/// Modify message flags (read/unread, starred, labels) and optionally move
pub async fn modify_flags(
    uids: Vec<u32>,
    from: &str,
    read: bool,
    unread: bool,
    starred: bool,
    unstarred: bool,
    labels: Vec<String>,
    unlabels: Vec<String>,
    move_to: Option<String>,
    output_format: Option<&str>,
) -> Result<()> {
    // Validate conflicting flags
    if read && unread {
        return Err(anyhow!("Cannot use both --read and --unread"));
    }
    if starred && unstarred {
        return Err(anyhow!("Cannot use both --starred and --unstarred"));
    }

    // Require at least one action
    if !read && !unread && !starred && !unstarred && labels.is_empty() && unlabels.is_empty() && move_to.is_none() {
        return Err(anyhow!(
            "At least one action required: --read, --unread, --starred, --unstarred, --label, --unlabel, or --move"
        ));
    }

    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let source_folder = resolve_folder_path(from);

    let mut client = ImapClient::connect(account).await?;
    client.select_folder(&source_folder).await?;

    let mut actions_performed = Vec::new();

    // Apply read/unread flags
    if read {
        client.mark_messages_read(&uids).await?;
        actions_performed.push("marked read");
    }
    if unread {
        client.mark_messages_unread(&uids).await?;
        actions_performed.push("marked unread");
    }

    // Apply starred/unstarred flags
    if starred {
        client.star_messages(&uids).await?;
        actions_performed.push("starred");
    }
    if unstarred {
        client.unstar_messages(&uids).await?;
        actions_performed.push("unstarred");
    }

    // Add labels (using IMAP keywords)
    for label in &labels {
        client.modify_flags(&uids, label, true).await?;
        actions_performed.push("labeled");
    }

    // Remove labels
    for unlabel in &unlabels {
        client.modify_flags(&uids, unlabel, false).await?;
        actions_performed.push("unlabeled");
    }

    // Move to destination folder if specified
    let dest_folder = if let Some(ref dest) = move_to {
        let dest_resolved = resolve_folder_path(dest);
        if !client.folder_exists(&dest_resolved).await? {
            return Err(anyhow!("Destination folder '{}' does not exist", dest_resolved));
        }
        client.move_messages(&uids, &dest_resolved).await?;
        actions_performed.push("moved");

        // Note: State tracking uses message_id as stable identifier.
        // Location gets updated automatically when message is seen in new folder.

        Some(dest_resolved)
    } else {
        None
    };

    let output = ActionOutput {
        action: "flag".to_string(),
        account: account.email.clone(),
        source_folder: source_folder.clone(),
        dest_folder,
        uids: uids.clone(),
        success_count: uids.len(),
        failed_uids: vec![],
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => {
            let actions_str = actions_performed.join(", ");
            println!(
                "\u{2713} Updated {} message(s): {}",
                uids.len(),
                actions_str
            );
        }
    }

    Ok(())
}
