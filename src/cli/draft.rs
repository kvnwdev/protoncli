use crate::core::state::{ActionType, StateManager};
use crate::models::config::Config;
use crate::output::json;
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize)]
pub struct DraftOutput {
    pub account: String,
    pub has_draft: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uids: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_folder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Show the current draft
pub async fn show_draft(output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;
    let draft = state.get_draft(&account.email).await?;

    let output = match draft {
        Some(d) => {
            let description = format_draft_description(&d);
            DraftOutput {
                account: account.email.clone(),
                has_draft: true,
                action_type: Some(d.action_type.as_str().to_string()),
                folder: Some(d.folder),
                message_count: Some(d.uids.len()),
                uids: Some(d.uids),
                dest_folder: d.dest_folder,
                description: Some(description),
            }
        }
        None => DraftOutput {
            account: account.email.clone(),
            has_draft: false,
            action_type: None,
            folder: None,
            message_count: None,
            uids: None,
            dest_folder: None,
            description: None,
        },
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => {
            if output.has_draft {
                println!("Draft staged for {}:", output.account);
                println!();
                println!("  Action: {}", output.action_type.as_ref().unwrap());
                println!("  Folder: {}", output.folder.as_ref().unwrap());
                println!("  Messages: {}", output.message_count.unwrap());
                if let Some(dest) = &output.dest_folder {
                    println!("  Destination: {}", dest);
                }
                println!();
                println!("{}", output.description.as_ref().unwrap());
                println!();
                println!("Run the action command without --draft to execute.");
            } else {
                println!("No draft staged for {}", output.account);
            }
        }
    }

    Ok(())
}

/// Clear the current draft
pub async fn clear_draft(output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;
    let had_draft = state.clear_draft(&account.email).await?;

    #[derive(Serialize)]
    struct ClearOutput {
        account: String,
        cleared: bool,
    }

    let output = ClearOutput {
        account: account.email.clone(),
        cleared: had_draft,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => {
            if had_draft {
                println!("✓ Draft cleared");
            } else {
                println!("No draft to clear");
            }
        }
    }

    Ok(())
}

/// Format a human-readable description of what the draft will do
fn format_draft_description(draft: &crate::core::state::Draft) -> String {
    let count = draft.uids.len();
    let msgs = if count == 1 { "message" } else { "messages" };

    match &draft.action_type {
        ActionType::Flag => {
            let mut actions = Vec::new();

            if let Some(params) = &draft.flag_params {
                if params.read == Some(true) {
                    actions.push("mark as read".to_string());
                } else if params.read == Some(false) {
                    actions.push("mark as unread".to_string());
                }

                if params.starred == Some(true) {
                    actions.push("star".to_string());
                } else if params.starred == Some(false) {
                    actions.push("unstar".to_string());
                }

                for label in &params.labels {
                    actions.push(format!("add label '{}'", label));
                }

                for label in &params.unlabels {
                    actions.push(format!("remove label '{}'", label));
                }

                if let Some(dest) = &params.move_to {
                    actions.push(format!("move to '{}'", dest));
                }
            }

            if actions.is_empty() {
                format!("Flag {} {} (no flag changes specified)", count, msgs)
            } else {
                format!("{} on {} {}", actions.join(", "), count, msgs)
            }
        }
        ActionType::Move => {
            let dest = draft.dest_folder.as_deref().unwrap_or("?");
            format!(
                "Move {} {} from '{}' to '{}'",
                count, msgs, draft.folder, dest
            )
        }
        ActionType::Copy => {
            let dest = draft.dest_folder.as_deref().unwrap_or("?");
            format!(
                "Copy {} {} from '{}' to '{}'",
                count, msgs, draft.folder, dest
            )
        }
        ActionType::Delete => {
            if draft.permanent {
                format!(
                    "Permanently delete {} {} from '{}'",
                    count, msgs, draft.folder
                )
            } else {
                format!("Move {} {} from '{}' to Trash", count, msgs, draft.folder)
            }
        }
        ActionType::Archive => {
            format!(
                "Archive {} {} from '{}' to 'Archive'",
                count, msgs, draft.folder
            )
        }
    }
}
