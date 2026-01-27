use crate::core::state::StateManager;
use crate::models::config::Config;
use crate::output::json;
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize)]
pub struct SelectionOutput {
    pub account: String,
    pub total_count: usize,
    pub folders: Vec<FolderSelection>,
}

#[derive(Serialize)]
pub struct FolderSelection {
    pub folder: String,
    pub count: usize,
    pub uids: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
}

#[derive(Serialize)]
pub struct SelectActionOutput {
    pub action: String,
    pub account: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

/// Add UIDs to the selection
pub async fn add_to_selection(
    uids: Vec<u32>,
    folder: &str,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    if uids.is_empty() {
        return Err(anyhow!("No UIDs specified"));
    }

    let state = StateManager::new().await?;

    // Create entries without message_id or subject (those would need to be fetched from IMAP)
    let entries: Vec<(u32, Option<&str>, Option<&str>)> =
        uids.iter().map(|&uid| (uid, None, None)).collect();

    let count = state.add_to_selection(&account.email, folder, &entries).await?;

    let output = SelectActionOutput {
        action: "add".to_string(),
        account: account.email.clone(),
        count,
        folder: Some(folder.to_string()),
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("✓ Added {} UID(s) to selection (folder: {})", count, folder),
    }

    Ok(())
}

/// Add last query results to the selection
pub async fn add_last_query_to_selection(
    folder: &str,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;

    // Get last query results
    let results = state.get_last_query_results(&account.email, folder).await?;

    if results.is_empty() {
        return Err(anyhow!(
            "No previous query results for folder '{}'. Run a query with --select first.",
            folder
        ));
    }

    // Convert to entries
    let entries: Vec<(u32, Option<&str>, Option<&str>)> = results
        .iter()
        .map(|r| {
            (
                r.uid as u32,
                r.message_id.as_deref(),
                r.subject.as_deref(),
            )
        })
        .collect();

    let count = state.add_to_selection(&account.email, folder, &entries).await?;

    let output = SelectActionOutput {
        action: "add_last".to_string(),
        account: account.email.clone(),
        count,
        folder: Some(folder.to_string()),
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!(
            "✓ Added {} message(s) from last query to selection (folder: {})",
            count, folder
        ),
    }

    Ok(())
}

/// Remove UIDs from the selection
pub async fn remove_from_selection(
    uids: Vec<u32>,
    folder: &str,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    if uids.is_empty() {
        return Err(anyhow!("No UIDs specified"));
    }

    let state = StateManager::new().await?;
    let count = state.remove_from_selection(&account.email, folder, &uids).await?;

    let output = SelectActionOutput {
        action: "remove".to_string(),
        account: account.email.clone(),
        count,
        folder: Some(folder.to_string()),
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("✓ Removed {} UID(s) from selection", count),
    }

    Ok(())
}

/// Show the current selection
pub async fn show_selection(output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;
    let entries = state.get_selection(&account.email).await?;

    if entries.is_empty() {
        match output_format.unwrap_or("text") {
            "json" => json::print_json(&SelectionOutput {
                account: account.email.clone(),
                total_count: 0,
                folders: vec![],
            })?,
            _ => println!("Selection is empty"),
        }
        return Ok(());
    }

    // Group by folder
    let mut folder_map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for entry in &entries {
        folder_map
            .entry(entry.folder.clone())
            .or_default()
            .push(entry);
    }

    let folders: Vec<FolderSelection> = folder_map
        .into_iter()
        .map(|(folder, entries)| {
            let uids: Vec<u32> = entries.iter().map(|e| e.uid as u32).collect();
            let subjects: Vec<String> = entries
                .iter()
                .filter_map(|e| e.subject.clone())
                .collect();
            FolderSelection {
                folder,
                count: uids.len(),
                uids,
                subjects,
            }
        })
        .collect();

    let output = SelectionOutput {
        account: account.email.clone(),
        total_count: entries.len(),
        folders,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => {
            println!("Selection for {}:", output.account);
            println!("Total: {} message(s)", output.total_count);
            println!();
            for folder in &output.folders {
                println!("  {} ({} messages):", folder.folder, folder.count);
                for (i, uid) in folder.uids.iter().enumerate() {
                    let subject = folder.subjects.get(i).map(|s| s.as_str()).unwrap_or("");
                    if subject.is_empty() {
                        println!("    - UID {}", uid);
                    } else {
                        println!("    - UID {}: {}", uid, subject);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Clear the selection
pub async fn clear_selection(output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;
    let count = state.clear_selection(&account.email).await?;

    let output = SelectActionOutput {
        action: "clear".to_string(),
        account: account.email.clone(),
        count,
        folder: None,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("✓ Cleared {} message(s) from selection", count),
    }

    Ok(())
}

/// Count messages in the selection
pub async fn count_selection(output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;
    let count = state.selection_count(&account.email).await?;

    #[derive(Serialize)]
    struct CountOutput {
        account: String,
        count: usize,
    }

    let output = CountOutput {
        account: account.email.clone(),
        count,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("{}", count),
    }

    Ok(())
}
