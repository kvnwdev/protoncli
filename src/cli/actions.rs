use crate::core::imap::ImapClient;
use crate::core::state::{ActionType, Draft, FlagParams, StateManager};
use crate::models::config::Config;
use crate::output::json;
use crate::utils::batch::{chunk_uids, DEFAULT_BATCH_SIZE};
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

/// Resolve UIDs based on provided arguments, selection, or existing draft
async fn resolve_uids(
    provided_uids: Vec<u32>,
    use_selection: bool,
    from_folder: &str,
    account_email: &str,
    state: &StateManager,
) -> Result<(Vec<u32>, String)> {
    if use_selection {
        // Get UIDs from selection for the specified folder
        let selection = state.get_selection_for_folder(account_email, from_folder).await?;
        if selection.is_empty() {
            return Err(anyhow!(
                "Selection is empty for folder '{}'. Use 'protoncli select add' or 'protoncli query --select' first.",
                from_folder
            ));
        }
        let uids: Vec<u32> = selection.iter().map(|e| e.uid as u32).collect();
        Ok((uids, from_folder.to_string()))
    } else if !provided_uids.is_empty() {
        Ok((provided_uids, from_folder.to_string()))
    } else {
        Err(anyhow!(
            "No UIDs provided. Either provide UIDs directly or use --selection flag."
        ))
    }
}

/// After action completion, optionally clear selection
async fn post_action_cleanup(
    account_email: &str,
    folder: &str,
    keep_selection: bool,
    state: &StateManager,
) -> Result<bool> {
    if keep_selection {
        Ok(false)
    } else {
        let cleared = state.clear_selection_for_folder(account_email, folder).await?;
        Ok(cleared > 0)
    }
}

/// Move messages from one folder to another
pub async fn move_messages(
    uids: Vec<u32>,
    from: &str,
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
    let source_folder = resolve_folder_path(from);
    let dest_folder = resolve_folder_path(to);

    // Resolve UIDs from selection or provided list
    let (resolved_uids, folder) = resolve_uids(uids, use_selection, &source_folder, &account.email, &state).await?;

    // If draft mode, save draft and return
    if create_draft {
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Move,
            folder: folder.clone(),
            uids: resolved_uids.clone(),
            flag_params: None,
            dest_folder: Some(dest_folder.clone()),
            permanent: false,
        };
        state.save_draft(&draft).await?;

        let output = ActionOutput {
            action: "move".to_string(),
            account: account.email.clone(),
            source_folder: folder,
            dest_folder: Some(dest_folder.clone()),
            uids: resolved_uids.clone(),
            success_count: 0,
            failed_uids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: Move {} message(s) from {} → {}. Run 'protoncli move --to {}' to execute.",
                resolved_uids.len(), source_folder, dest_folder, to
            ),
        }
        return Ok(());
    }

    let mut client = ImapClient::connect(account).await?;

    // Validate destination folder exists
    if !client.folder_exists(&dest_folder).await? {
        return Err(anyhow!("Destination folder '{}' does not exist", dest_folder));
    }

    // Select source folder and move messages in batches
    client.select_folder(&folder).await?;
    
    for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
        client.move_messages(&chunk, &dest_folder).await?;
    }

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, &folder, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "move".to_string(),
        account: account.email.clone(),
        source_folder: folder.clone(),
        dest_folder: Some(dest_folder.clone()),
        uids: resolved_uids.clone(),
        success_count: resolved_uids.len(),
        failed_uids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!(
            "✓ Moved {} message(s) from {} → {}",
            resolved_uids.len(),
            folder,
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
    let source_folder = resolve_folder_path(from);
    let dest_folder = resolve_folder_path(to);

    // Resolve UIDs from selection or provided list
    let (resolved_uids, folder) = resolve_uids(uids, use_selection, &source_folder, &account.email, &state).await?;

    // If draft mode, save draft and return
    if create_draft {
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Copy,
            folder: folder.clone(),
            uids: resolved_uids.clone(),
            flag_params: None,
            dest_folder: Some(dest_folder.clone()),
            permanent: false,
        };
        state.save_draft(&draft).await?;

        let output = ActionOutput {
            action: "copy".to_string(),
            account: account.email.clone(),
            source_folder: folder,
            dest_folder: Some(dest_folder.clone()),
            uids: resolved_uids.clone(),
            success_count: 0,
            failed_uids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: Copy {} message(s) from {} → {}. Run 'protoncli copy --to {}' to execute.",
                resolved_uids.len(), source_folder, dest_folder, to
            ),
        }
        return Ok(());
    }

    let mut client = ImapClient::connect(account).await?;

    // Validate destination folder exists
    if !client.folder_exists(&dest_folder).await? {
        return Err(anyhow!("Destination folder '{}' does not exist", dest_folder));
    }

    // Select source folder and copy messages in batches
    client.select_folder(&folder).await?;
    
    for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
        client.copy_messages(&chunk, &dest_folder).await?;
    }

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, &folder, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "copy".to_string(),
        account: account.email.clone(),
        source_folder: folder.clone(),
        dest_folder: Some(dest_folder.clone()),
        uids: resolved_uids.clone(),
        success_count: resolved_uids.len(),
        failed_uids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!(
            "✓ Copied {} message(s) from {} → {}",
            resolved_uids.len(),
            folder,
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
    let source_folder = resolve_folder_path(from);

    // Resolve UIDs from selection or provided list
    let (resolved_uids, folder) = resolve_uids(uids, use_selection, &source_folder, &account.email, &state).await?;

    // If draft mode, save draft and return
    if create_draft {
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Delete,
            folder: folder.clone(),
            uids: resolved_uids.clone(),
            flag_params: None,
            dest_folder: None,
            permanent,
        };
        state.save_draft(&draft).await?;

        let action_desc = if permanent { "Permanently delete" } else { "Move to Trash" };
        let output = ActionOutput {
            action: "delete".to_string(),
            account: account.email.clone(),
            source_folder: folder,
            dest_folder: if permanent { None } else { Some("Trash".to_string()) },
            uids: resolved_uids.clone(),
            success_count: 0,
            failed_uids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: {} {} message(s). Run 'protoncli delete' to execute.",
                action_desc, resolved_uids.len()
            ),
        }
        return Ok(());
    }

    // Confirm permanent delete if not already confirmed
    if permanent && !yes {
        print!(
            "Permanently delete {} message(s)? This cannot be undone. [y/N] ",
            resolved_uids.len()
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
    client.select_folder(&folder).await?;

    if permanent {
        // Permanent delete: mark as deleted and expunge (in batches)
        for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
            client.mark_messages_deleted(&chunk).await?;
        }
        client.expunge().await?;
    } else {
        // Soft delete: move to Trash (in batches)
        let trash_folder = resolve_folder_path("trash");
        if !client.folder_exists(&trash_folder).await? {
            return Err(anyhow!("Trash folder not found"));
        }
        for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
            client.move_messages(&chunk, &trash_folder).await?;
        }
    }

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, &folder, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "delete".to_string(),
        account: account.email.clone(),
        source_folder: folder.clone(),
        dest_folder: if permanent {
            None
        } else {
            Some("Trash".to_string())
        },
        uids: resolved_uids.clone(),
        success_count: resolved_uids.len(),
        failed_uids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => {
            if permanent {
                println!("✓ Permanently deleted {} message(s)", resolved_uids.len());
            } else {
                println!("✓ Moved {} message(s) to Trash", resolved_uids.len());
            }
        }
    }

    Ok(())
}

/// Archive messages (shortcut for move to Archive)
pub async fn archive_messages(
    uids: Vec<u32>,
    from: &str,
    use_selection: bool,
    create_draft: bool,
    keep_selection: bool,
    output_format: Option<&str>,
) -> Result<()> {
    // Archive is just a move to Archive folder, but we handle draft specially
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;
    let source_folder = resolve_folder_path(from);
    let dest_folder = "Archive".to_string();

    // Resolve UIDs from selection or provided list
    let (resolved_uids, folder) = resolve_uids(uids, use_selection, &source_folder, &account.email, &state).await?;

    // If draft mode, save draft and return
    if create_draft {
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Archive,
            folder: folder.clone(),
            uids: resolved_uids.clone(),
            flag_params: None,
            dest_folder: Some(dest_folder.clone()),
            permanent: false,
        };
        state.save_draft(&draft).await?;

        let output = ActionOutput {
            action: "archive".to_string(),
            account: account.email.clone(),
            source_folder: folder,
            dest_folder: Some(dest_folder),
            uids: resolved_uids.clone(),
            success_count: 0,
            failed_uids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: Archive {} message(s). Run 'protoncli archive' to execute.",
                resolved_uids.len()
            ),
        }
        return Ok(());
    }

    let mut client = ImapClient::connect(account).await?;

    // Validate Archive folder exists
    if !client.folder_exists(&dest_folder).await? {
        return Err(anyhow!("Archive folder does not exist"));
    }

    // Select source folder and move messages in batches
    client.select_folder(&folder).await?;
    
    for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
        client.move_messages(&chunk, &dest_folder).await?;
    }

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, &folder, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "archive".to_string(),
        account: account.email.clone(),
        source_folder: folder.clone(),
        dest_folder: Some(dest_folder.clone()),
        uids: resolved_uids.clone(),
        success_count: resolved_uids.len(),
        failed_uids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!(
            "✓ Archived {} message(s) from {} → Archive",
            resolved_uids.len(),
            folder
        ),
    }

    Ok(())
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
    let source_folder = resolve_folder_path(from);

    // Build flag params from provided flags
    let flag_params = FlagParams {
        read: if read { Some(true) } else if unread { Some(false) } else { None },
        starred: if starred { Some(true) } else if unstarred { Some(false) } else { None },
        labels: labels.clone(),
        unlabels: unlabels.clone(),
        move_to: move_to.clone(),
    };

    // Check if this is executing an existing draft
    let existing_draft = state.get_draft(&account.email).await?;
    let (resolved_uids, folder, final_flag_params) = if let Some(draft) = existing_draft {
        // We have an existing draft - merge params if this is a flag action
        if draft.action_type == ActionType::Flag {
            let mut merged_params = draft.flag_params.clone().unwrap_or_default();
            merged_params.merge(&flag_params);
            
            // If no new flags are provided and not using selection, execute the draft
            if !use_selection && uids.is_empty() && !flag_params.has_any_action() {
                // Execute draft as-is
                state.clear_draft(&account.email).await?;
                (draft.uids.clone(), draft.folder.clone(), merged_params)
            } else if use_selection || !uids.is_empty() {
                // New UIDs provided - resolve them normally but use merged params
                let (resolved, folder) = resolve_uids(uids, use_selection, &source_folder, &account.email, &state).await?;
                (resolved, folder, merged_params)
            } else {
                // Execute draft with merged params
                state.clear_draft(&account.email).await?;
                (draft.uids.clone(), draft.folder.clone(), merged_params)
            }
        } else {
            // Draft is for different action type, resolve UIDs normally
            let (resolved, folder) = resolve_uids(uids, use_selection, &source_folder, &account.email, &state).await?;
            (resolved, folder, flag_params)
        }
    } else {
        // No existing draft - must have UIDs or selection
        let (resolved, folder) = resolve_uids(uids, use_selection, &source_folder, &account.email, &state).await?;
        (resolved, folder, flag_params)
    };

    // Require at least one action
    if !final_flag_params.has_any_action() {
        return Err(anyhow!(
            "At least one action required: --read, --unread, --starred, --unstarred, --label, --unlabel, or --move"
        ));
    }

    // If draft mode, save draft and return
    if create_draft {
        let draft = Draft {
            account: account.email.clone(),
            action_type: ActionType::Flag,
            folder: folder.clone(),
            uids: resolved_uids.clone(),
            flag_params: Some(final_flag_params.clone()),
            dest_folder: final_flag_params.move_to.clone().map(|d| resolve_folder_path(&d)),
            permanent: false,
        };
        state.save_draft(&draft).await?;

        let mut actions = Vec::new();
        if final_flag_params.read == Some(true) { actions.push("mark as read"); }
        if final_flag_params.read == Some(false) { actions.push("mark as unread"); }
        if final_flag_params.starred == Some(true) { actions.push("star"); }
        if final_flag_params.starred == Some(false) { actions.push("unstar"); }
        if !final_flag_params.labels.is_empty() { actions.push("add labels"); }
        if !final_flag_params.unlabels.is_empty() { actions.push("remove labels"); }
        if final_flag_params.move_to.is_some() { actions.push("move"); }

        let output = ActionOutput {
            action: "flag".to_string(),
            account: account.email.clone(),
            source_folder: folder,
            dest_folder: final_flag_params.move_to.clone().map(|d| resolve_folder_path(&d)),
            uids: resolved_uids.clone(),
            success_count: 0,
            failed_uids: vec![],
            draft_staged: Some(true),
            selection_cleared: None,
        };

        match output_format.unwrap_or("text") {
            "json" => json::print_json(&output)?,
            _ => println!(
                "Draft staged: {} on {} message(s). Run 'protoncli flag' to execute.",
                actions.join(", "), resolved_uids.len()
            ),
        }
        return Ok(());
    }

    let mut client = ImapClient::connect(account).await?;
    client.select_folder(&folder).await?;

    let mut actions_performed = Vec::new();

    // Apply read/unread flags in batches
    if final_flag_params.read == Some(true) {
        for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
            client.mark_messages_read(&chunk).await?;
        }
        actions_performed.push("marked read");
    }
    if final_flag_params.read == Some(false) {
        for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
            client.mark_messages_unread(&chunk).await?;
        }
        actions_performed.push("marked unread");
    }

    // Apply starred/unstarred flags in batches
    if final_flag_params.starred == Some(true) {
        for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
            client.star_messages(&chunk).await?;
        }
        actions_performed.push("starred");
    }
    if final_flag_params.starred == Some(false) {
        for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
            client.unstar_messages(&chunk).await?;
        }
        actions_performed.push("unstarred");
    }

    // Add labels (using IMAP keywords)
    for label in &final_flag_params.labels {
        for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
            client.modify_flags(&chunk, label, true).await?;
        }
        actions_performed.push("labeled");
    }

    // Remove labels
    for unlabel in &final_flag_params.unlabels {
        for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
            client.modify_flags(&chunk, unlabel, false).await?;
        }
        actions_performed.push("unlabeled");
    }

    // Move to destination folder if specified
    let dest_folder = if let Some(ref dest) = final_flag_params.move_to {
        let dest_resolved = resolve_folder_path(dest);
        if !client.folder_exists(&dest_resolved).await? {
            return Err(anyhow!("Destination folder '{}' does not exist", dest_resolved));
        }
        for chunk in chunk_uids(&resolved_uids, DEFAULT_BATCH_SIZE) {
            client.move_messages(&chunk, &dest_resolved).await?;
        }
        actions_performed.push("moved");
        Some(dest_resolved)
    } else {
        None
    };

    // Clear selection unless --keep
    let selection_cleared = if use_selection {
        Some(post_action_cleanup(&account.email, &folder, keep_selection, &state).await?)
    } else {
        None
    };

    let output = ActionOutput {
        action: "flag".to_string(),
        account: account.email.clone(),
        source_folder: folder.clone(),
        dest_folder,
        uids: resolved_uids.clone(),
        success_count: resolved_uids.len(),
        failed_uids: vec![],
        draft_staged: None,
        selection_cleared,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => {
            let actions_str = actions_performed.join(", ");
            println!(
                "✓ Updated {} message(s): {}",
                resolved_uids.len(),
                actions_str
            );
        }
    }

    Ok(())
}
