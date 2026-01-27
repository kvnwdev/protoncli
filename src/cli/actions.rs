use crate::core::imap::ImapClient;
use crate::core::state::{ActionType, Draft, FlagParams, ResolvedMessage, StateManager};
use crate::models::config::Config;
use crate::output::json;
use crate::utils::batch::{chunk_uids, DEFAULT_BATCH_SIZE};
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{self, Write};

#[derive(Serialize)]
pub struct ActionOutput {
    pub action: String,
    pub account: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_folder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_folder: Option<String>,
    pub ids: Vec<i64>,
    pub success_count: usize,
    pub failed_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_staged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_cleared: Option<bool>,
}

/// Resolve common folder name aliases to their actual IMAP names
pub fn resolve_folder_path(folder: &str) -> String {
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

/// Resolve shadow UIDs to their current IMAP locations, grouped by folder
async fn resolve_shadow_uids_by_folder(
    provided_ids: Vec<i64>,
    use_selection: bool,
    account_email: &str,
    state: &StateManager,
) -> Result<(Vec<i64>, HashMap<String, Vec<ResolvedMessage>>)> {
    let shadow_uids = if use_selection {
        // Get shadow UIDs from selection
        let selection = state.get_selection(account_email).await?;
        if selection.is_empty() {
            return Err(anyhow!(
                "Selection is empty. Use 'protoncli select add' or 'protoncli query --select' first."
            ));
        }
        // Get shadow UIDs from selection entries
        let mut uids = Vec::new();
        for entry in selection {
            if let Some(shadow_uid) = entry.shadow_uid {
                uids.push(shadow_uid);
            }
        }
        if uids.is_empty() {
            return Err(anyhow!(
                "Selection contains no messages with shadow UIDs. Please re-run 'inbox' or 'query' to assign IDs."
            ));
        }
        uids
    } else if !provided_ids.is_empty() {
        provided_ids
    } else {
        return Err(anyhow!(
            "No message IDs provided. Either provide IDs directly or use --selection flag."
        ));
    };

    // Resolve all shadow UIDs to their current locations
    let resolved = state
        .resolve_shadow_uids(account_email, &shadow_uids)
        .await?;

    // Group by folder
    let mut by_folder: HashMap<String, Vec<ResolvedMessage>> = HashMap::new();
    for msg in resolved {
        by_folder.entry(msg.folder.clone()).or_default().push(msg);
    }

    Ok((shadow_uids, by_folder))
}

/// After action completion, optionally clear selection
async fn post_action_cleanup(
    account_email: &str,
    keep_selection: bool,
    state: &StateManager,
) -> Result<bool> {
    if keep_selection {
        Ok(false)
    } else {
        let cleared = state.clear_selection(account_email).await?;
        Ok(cleared > 0)
    }
}

/// Move messages to another folder
pub async fn move_messages(
    ids: Vec<i64>,
    to: &str,
    use_selection: bool,
    create_draft: bool,
    keep_selection: bool,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;
    let dest_folder = resolve_folder_path(to);

    // Resolve shadow UIDs to current locations
    let (shadow_uids, by_folder) =
        resolve_shadow_uids_by_folder(ids, use_selection, &account.email, &state).await?;

    // If draft mode, save draft and return
    if create_draft {
        // For drafts, we store shadow UIDs (they persist across moves)
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Move,
            folder: "".to_string(), // Not used with shadow UIDs
            uids: shadow_uids.iter().map(|&id| id as u32).collect(), // Store as u32 for backwards compat
            flag_params: None,
            dest_folder: Some(dest_folder.clone()),
            permanent: false,
        };
        state.save_draft(&draft).await?;

        let output = ActionOutput {
            action: "move".to_string(),
            account: account.email.clone(),
            source_folder: None,
            dest_folder: Some(dest_folder.clone()),
            ids: shadow_uids.clone(),
            success_count: 0,
            failed_ids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: Move {} message(s) → {}. Run 'protoncli move --to {}' to execute.",
                shadow_uids.len(),
                dest_folder,
                to
            ),
        }
        return Ok(());
    }

    let mut client = ImapClient::connect(account).await?;

    // Validate destination folder exists
    if !client.folder_exists(&dest_folder).await? {
        return Err(anyhow!(
            "Destination folder '{}' does not exist",
            dest_folder
        ));
    }

    // Process each source folder
    let mut moved_count = 0;
    for (folder, messages) in &by_folder {
        client.select_folder(folder).await?;

        let imap_uids: Vec<u32> = messages.iter().map(|m| m.imap_uid).collect();
        for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
            client.move_messages(&chunk, &dest_folder).await?;
        }

        // Update message locations in database
        // After move, we need to find the new UIDs. For now, we mark location as dest_folder
        // with UID 0 (will be resolved on next fetch)
        for msg in messages {
            if let Some(ref msg_id) = msg.message_id {
                // Try to find the new UID by searching in dest folder
                // For simplicity, just update the folder - UID will be resolved on next access
                state
                    .update_message_location_by_message_id(
                        &account.email,
                        msg_id,
                        &dest_folder,
                        0, // UID will be resolved on next access
                    )
                    .await?;
            }
        }

        moved_count += messages.len();
    }

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "move".to_string(),
        account: account.email.clone(),
        source_folder: by_folder.keys().next().cloned(),
        dest_folder: Some(dest_folder.clone()),
        ids: shadow_uids.clone(),
        success_count: moved_count,
        failed_ids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("✓ Moved {} message(s) → {}", moved_count, dest_folder),
    }

    Ok(())
}

/// Copy messages to another folder
pub async fn copy_messages(
    ids: Vec<i64>,
    to: &str,
    use_selection: bool,
    create_draft: bool,
    keep_selection: bool,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;
    let dest_folder = resolve_folder_path(to);

    // Resolve shadow UIDs to current locations
    let (shadow_uids, by_folder) =
        resolve_shadow_uids_by_folder(ids, use_selection, &account.email, &state).await?;

    // If draft mode, save draft and return
    if create_draft {
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Copy,
            folder: "".to_string(),
            uids: shadow_uids.iter().map(|&id| id as u32).collect(),
            flag_params: None,
            dest_folder: Some(dest_folder.clone()),
            permanent: false,
        };
        state.save_draft(&draft).await?;

        let output = ActionOutput {
            action: "copy".to_string(),
            account: account.email.clone(),
            source_folder: None,
            dest_folder: Some(dest_folder.clone()),
            ids: shadow_uids.clone(),
            success_count: 0,
            failed_ids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: Copy {} message(s) → {}. Run 'protoncli copy --to {}' to execute.",
                shadow_uids.len(),
                dest_folder,
                to
            ),
        }
        return Ok(());
    }

    let mut client = ImapClient::connect(account).await?;

    // Validate destination folder exists
    if !client.folder_exists(&dest_folder).await? {
        return Err(anyhow!(
            "Destination folder '{}' does not exist",
            dest_folder
        ));
    }

    // Process each source folder
    let mut copied_count = 0;
    for (folder, messages) in &by_folder {
        client.select_folder(folder).await?;

        let imap_uids: Vec<u32> = messages.iter().map(|m| m.imap_uid).collect();
        for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
            client.copy_messages(&chunk, &dest_folder).await?;
        }

        copied_count += messages.len();
    }

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "copy".to_string(),
        account: account.email.clone(),
        source_folder: by_folder.keys().next().cloned(),
        dest_folder: Some(dest_folder.clone()),
        ids: shadow_uids.clone(),
        success_count: copied_count,
        failed_ids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("✓ Copied {} message(s) → {}", copied_count, dest_folder),
    }

    Ok(())
}

/// Delete messages (move to Trash or permanent delete)
#[allow(clippy::too_many_arguments)]
pub async fn delete_messages(
    ids: Vec<i64>,
    permanent: bool,
    yes: bool,
    use_selection: bool,
    create_draft: bool,
    keep_selection: bool,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;

    // Resolve shadow UIDs to current locations
    let (shadow_uids, by_folder) =
        resolve_shadow_uids_by_folder(ids, use_selection, &account.email, &state).await?;

    // If draft mode, save draft and return
    if create_draft {
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Delete,
            folder: "".to_string(),
            uids: shadow_uids.iter().map(|&id| id as u32).collect(),
            flag_params: None,
            dest_folder: None,
            permanent,
        };
        state.save_draft(&draft).await?;

        let action_desc = if permanent {
            "Permanently delete"
        } else {
            "Move to Trash"
        };
        let output = ActionOutput {
            action: "delete".to_string(),
            account: account.email.clone(),
            source_folder: None,
            dest_folder: if permanent {
                None
            } else {
                Some("Trash".to_string())
            },
            ids: shadow_uids.clone(),
            success_count: 0,
            failed_ids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: {} {} message(s). Run 'protoncli delete' to execute.",
                action_desc,
                shadow_uids.len()
            ),
        }
        return Ok(());
    }

    // Confirm permanent delete if not already confirmed
    if permanent && !yes {
        print!(
            "Permanently delete {} message(s)? This cannot be undone. [y/N] ",
            shadow_uids.len()
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
    let trash_folder = resolve_folder_path("trash");

    let mut deleted_count = 0;
    for (folder, messages) in &by_folder {
        client.select_folder(folder).await?;
        let imap_uids: Vec<u32> = messages.iter().map(|m| m.imap_uid).collect();

        if permanent {
            // Permanent delete: mark as deleted and expunge
            for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
                client.mark_messages_deleted(&chunk).await?;
            }
            client.expunge().await?;
        } else {
            // Soft delete: move to Trash
            if !client.folder_exists(&trash_folder).await? {
                return Err(anyhow!("Trash folder not found"));
            }
            for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
                client.move_messages(&chunk, &trash_folder).await?;
            }

            // Update message locations
            for msg in messages {
                if let Some(ref msg_id) = msg.message_id {
                    state
                        .update_message_location_by_message_id(
                            &account.email,
                            msg_id,
                            &trash_folder,
                            0,
                        )
                        .await?;
                }
            }
        }

        deleted_count += messages.len();
    }

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "delete".to_string(),
        account: account.email.clone(),
        source_folder: by_folder.keys().next().cloned(),
        dest_folder: if permanent {
            None
        } else {
            Some("Trash".to_string())
        },
        ids: shadow_uids.clone(),
        success_count: deleted_count,
        failed_ids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => {
            if permanent {
                println!("✓ Permanently deleted {} message(s)", deleted_count);
            } else {
                println!("✓ Moved {} message(s) to Trash", deleted_count);
            }
        }
    }

    Ok(())
}

/// Archive messages (shortcut for move to Archive)
pub async fn archive_messages(
    ids: Vec<i64>,
    use_selection: bool,
    create_draft: bool,
    keep_selection: bool,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;
    let dest_folder = "Archive".to_string();

    // Resolve shadow UIDs to current locations
    let (shadow_uids, by_folder) =
        resolve_shadow_uids_by_folder(ids, use_selection, &account.email, &state).await?;

    // If draft mode, save draft and return
    if create_draft {
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Archive,
            folder: "".to_string(),
            uids: shadow_uids.iter().map(|&id| id as u32).collect(),
            flag_params: None,
            dest_folder: Some(dest_folder.clone()),
            permanent: false,
        };
        state.save_draft(&draft).await?;

        let output = ActionOutput {
            action: "archive".to_string(),
            account: account.email.clone(),
            source_folder: None,
            dest_folder: Some(dest_folder),
            ids: shadow_uids.clone(),
            success_count: 0,
            failed_ids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: Archive {} message(s). Run 'protoncli archive' to execute.",
                shadow_uids.len()
            ),
        }
        return Ok(());
    }

    let mut client = ImapClient::connect(account).await?;

    // Validate Archive folder exists
    if !client.folder_exists(&dest_folder).await? {
        return Err(anyhow!("Archive folder does not exist"));
    }

    // Process each source folder
    let mut archived_count = 0;
    for (folder, messages) in &by_folder {
        client.select_folder(folder).await?;

        let imap_uids: Vec<u32> = messages.iter().map(|m| m.imap_uid).collect();
        for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
            client.move_messages(&chunk, &dest_folder).await?;
        }

        // Update message locations
        for msg in messages {
            if let Some(ref msg_id) = msg.message_id {
                state
                    .update_message_location_by_message_id(&account.email, msg_id, &dest_folder, 0)
                    .await?;
            }
        }

        archived_count += messages.len();
    }

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "archive".to_string(),
        account: account.email.clone(),
        source_folder: by_folder.keys().next().cloned(),
        dest_folder: Some(dest_folder.clone()),
        ids: shadow_uids.clone(),
        success_count: archived_count,
        failed_ids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("✓ Archived {} message(s) → Archive", archived_count),
    }

    Ok(())
}

/// Modify message flags (read/unread, starred, labels) and optionally move
#[allow(clippy::too_many_arguments)]
pub async fn modify_flags(
    ids: Vec<i64>,
    read: bool,
    unread: bool,
    starred: bool,
    unstarred: bool,
    labels: Vec<String>,
    unlabels: Vec<String>,
    move_to: Option<String>,
    use_selection: bool,
    create_draft: bool,
    keep_selection: bool,
    output_format: Option<&str>,
) -> Result<()> {
    // Validate conflicting flags
    if read && unread {
        return Err(anyhow!("Cannot use both --read and --unread"));
    }
    if starred && unstarred {
        return Err(anyhow!("Cannot use both --starred and --unstarred"));
    }

    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;

    // Build flag params from provided flags
    let flag_params = FlagParams {
        read: if read {
            Some(true)
        } else if unread {
            Some(false)
        } else {
            None
        },
        starred: if starred {
            Some(true)
        } else if unstarred {
            Some(false)
        } else {
            None
        },
        labels: labels.clone(),
        unlabels: unlabels.clone(),
        move_to: move_to.clone(),
    };

    // Resolve shadow UIDs to current locations
    let (shadow_uids, by_folder) =
        resolve_shadow_uids_by_folder(ids, use_selection, &account.email, &state).await?;

    // Require at least one action
    if !flag_params.has_any_action() {
        return Err(anyhow!(
            "At least one action required: --read, --unread, --starred, --unstarred, --label, --unlabel, or --move"
        ));
    }

    // If draft mode, save draft and return
    if create_draft {
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Flag,
            folder: "".to_string(),
            uids: shadow_uids.iter().map(|&id| id as u32).collect(),
            flag_params: Some(flag_params.clone()),
            dest_folder: flag_params.move_to.clone().map(|d| resolve_folder_path(&d)),
            permanent: false,
        };
        state.save_draft(&draft).await?;

        let mut actions = Vec::new();
        if flag_params.read == Some(true) {
            actions.push("mark as read");
        }
        if flag_params.read == Some(false) {
            actions.push("mark as unread");
        }
        if flag_params.starred == Some(true) {
            actions.push("star");
        }
        if flag_params.starred == Some(false) {
            actions.push("unstar");
        }
        if !flag_params.labels.is_empty() {
            actions.push("add labels");
        }
        if !flag_params.unlabels.is_empty() {
            actions.push("remove labels");
        }
        if flag_params.move_to.is_some() {
            actions.push("move");
        }

        let output = ActionOutput {
            action: "flag".to_string(),
            account: account.email.clone(),
            source_folder: None,
            dest_folder: flag_params.move_to.clone().map(|d| resolve_folder_path(&d)),
            ids: shadow_uids.clone(),
            success_count: 0,
            failed_ids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: {} on {} message(s). Run 'protoncli flag' to execute.",
                actions.join(", "),
                shadow_uids.len()
            ),
        }
        return Ok(());
    }

    let mut client = ImapClient::connect(account).await?;
    let mut actions_performed = Vec::new();
    let mut updated_count = 0;

    // Process each folder
    for (folder, messages) in &by_folder {
        client.select_folder(folder).await?;
        let imap_uids: Vec<u32> = messages.iter().map(|m| m.imap_uid).collect();

        // Apply read/unread flags
        if flag_params.read == Some(true) {
            for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
                client.mark_messages_read(&chunk).await?;
            }
            if !actions_performed.contains(&"marked read") {
                actions_performed.push("marked read");
            }
        }
        if flag_params.read == Some(false) {
            for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
                client.mark_messages_unread(&chunk).await?;
            }
            if !actions_performed.contains(&"marked unread") {
                actions_performed.push("marked unread");
            }
        }

        // Apply starred/unstarred flags
        if flag_params.starred == Some(true) {
            for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
                client.star_messages(&chunk).await?;
            }
            if !actions_performed.contains(&"starred") {
                actions_performed.push("starred");
            }
        }
        if flag_params.starred == Some(false) {
            for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
                client.unstar_messages(&chunk).await?;
            }
            if !actions_performed.contains(&"unstarred") {
                actions_performed.push("unstarred");
            }
        }

        // Add labels
        for label in &flag_params.labels {
            for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
                client.modify_flags(&chunk, label, true).await?;
            }
        }
        if !flag_params.labels.is_empty() && !actions_performed.contains(&"labeled") {
            actions_performed.push("labeled");
        }

        // Remove labels
        for unlabel in &flag_params.unlabels {
            for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
                client.modify_flags(&chunk, unlabel, false).await?;
            }
        }
        if !flag_params.unlabels.is_empty() && !actions_performed.contains(&"unlabeled") {
            actions_performed.push("unlabeled");
        }

        // Move to destination folder if specified
        if let Some(ref dest) = flag_params.move_to {
            let dest_resolved = resolve_folder_path(dest);
            if !client.folder_exists(&dest_resolved).await? {
                return Err(anyhow!(
                    "Destination folder '{}' does not exist",
                    dest_resolved
                ));
            }
            for chunk in chunk_uids(&imap_uids, DEFAULT_BATCH_SIZE) {
                client.move_messages(&chunk, &dest_resolved).await?;
            }

            // Update message locations
            for msg in messages {
                if let Some(ref msg_id) = msg.message_id {
                    state
                        .update_message_location_by_message_id(
                            &account.email,
                            msg_id,
                            &dest_resolved,
                            0,
                        )
                        .await?;
                }
            }

            if !actions_performed.contains(&"moved") {
                actions_performed.push("moved");
            }
        }

        updated_count += messages.len();
    }

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "flag".to_string(),
        account: account.email.clone(),
        source_folder: by_folder.keys().next().cloned(),
        dest_folder: flag_params.move_to.map(|d| resolve_folder_path(&d)),
        ids: shadow_uids.clone(),
        success_count: updated_count,
        failed_ids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => {
            let actions_str = actions_performed.join(", ");
            println!("✓ Updated {} message(s): {}", updated_count, actions_str);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_folder_path_inbox() {
        assert_eq!(resolve_folder_path("inbox"), "INBOX");
        assert_eq!(resolve_folder_path("INBOX"), "INBOX");
        assert_eq!(resolve_folder_path("InBox"), "INBOX");
    }

    #[test]
    fn test_resolve_folder_path_common_aliases() {
        assert_eq!(resolve_folder_path("archive"), "Archive");
        assert_eq!(resolve_folder_path("ARCHIVE"), "Archive");
        assert_eq!(resolve_folder_path("trash"), "Trash");
        assert_eq!(resolve_folder_path("TRASH"), "Trash");
        assert_eq!(resolve_folder_path("sent"), "Sent");
        assert_eq!(resolve_folder_path("SENT"), "Sent");
        assert_eq!(resolve_folder_path("drafts"), "Drafts");
        assert_eq!(resolve_folder_path("DRAFTS"), "Drafts");
    }

    #[test]
    fn test_resolve_folder_path_spam_aliases() {
        assert_eq!(resolve_folder_path("spam"), "Spam");
        assert_eq!(resolve_folder_path("SPAM"), "Spam");
        assert_eq!(resolve_folder_path("junk"), "Spam");
        assert_eq!(resolve_folder_path("JUNK"), "Spam");
    }

    #[test]
    fn test_resolve_folder_path_all_mail() {
        assert_eq!(resolve_folder_path("all"), "All Mail");
        assert_eq!(resolve_folder_path("all mail"), "All Mail");
    }

    #[test]
    fn test_resolve_folder_path_passthrough() {
        // Custom folders should pass through unchanged
        assert_eq!(resolve_folder_path("Custom/Folder"), "Custom/Folder");
        assert_eq!(resolve_folder_path("Folders/Crypto"), "Folders/Crypto");
        assert_eq!(resolve_folder_path("Labels/Important"), "Labels/Important");
        assert_eq!(resolve_folder_path("SomeOtherFolder"), "SomeOtherFolder");
    }

    #[test]
    fn test_resolve_folder_path_preserves_case_on_passthrough() {
        // Unknown folders should preserve their original case
        assert_eq!(resolve_folder_path("MyFolder"), "MyFolder");
        assert_eq!(resolve_folder_path("myfolder"), "myfolder");
        assert_eq!(resolve_folder_path("MYFOLDER"), "MYFOLDER");
    }
}
