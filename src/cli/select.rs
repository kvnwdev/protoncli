use crate::core::state::StateManager;
use crate::models::config::Config;
use crate::output::json;
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize)]
pub struct SelectionOutput {
    pub account: String,
    pub total_count: usize,
    pub messages: Vec<SelectionMessage>,
}

#[derive(Serialize)]
pub struct SelectionMessage {
    pub id: i64,
    pub folder: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

#[derive(Serialize)]
pub struct SelectActionOutput {
    pub action: String,
    pub account: String,
    pub count: usize,
}

/// Add shadow UIDs to the selection
pub async fn add_to_selection(ids: Vec<i64>, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    if ids.is_empty() {
        return Err(anyhow!("No message IDs specified"));
    }

    let state = StateManager::new().await?;

    // Resolve shadow UIDs to get their info
    let resolved = state.resolve_shadow_uids(&account.email, &ids).await?;

    // Add to selection with shadow_uid info
    let mut count = 0;
    for msg in &resolved {
        // Add the resolved message to selection with its shadow_uid
        let entries: Vec<(u32, Option<&str>, Option<&str>)> =
            vec![(msg.imap_uid, msg.message_id.as_deref(), None)];
        count += state
            .add_to_selection(&account.email, &msg.folder, &entries)
            .await?;
    }

    let output = SelectActionOutput {
        action: "add".to_string(),
        account: account.email.clone(),
        count,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("✓ Added {} message(s) to selection", count),
    }

    Ok(())
}

/// Add last query results to the selection
pub async fn add_last_query_to_selection(folder: &str, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    let state = StateManager::new().await?;

    // Get last query results
    let results = state.get_last_query_results(&account.email, folder).await?;

    if results.is_empty() {
        return Err(anyhow!(
            "No previous query results for folder '{}'. Run a query first.",
            folder
        ));
    }

    // Convert to entries
    let entries: Vec<(u32, Option<&str>, Option<&str>)> = results
        .iter()
        .map(|r| (r.uid as u32, r.message_id.as_deref(), r.subject.as_deref()))
        .collect();

    let count = state
        .add_to_selection(&account.email, folder, &entries)
        .await?;

    let output = SelectActionOutput {
        action: "add_last".to_string(),
        account: account.email.clone(),
        count,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("✓ Added {} message(s) from last query to selection", count),
    }

    Ok(())
}

/// Remove shadow UIDs from the selection
pub async fn remove_from_selection(ids: Vec<i64>, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    if ids.is_empty() {
        return Err(anyhow!("No message IDs specified"));
    }

    let state = StateManager::new().await?;

    // Resolve shadow UIDs to get their info
    let resolved = state.resolve_shadow_uids(&account.email, &ids).await?;

    // Remove from selection by folder
    let mut count = 0;
    for msg in &resolved {
        let removed = state
            .remove_from_selection(&account.email, &msg.folder, &[msg.imap_uid])
            .await?;
        count += removed;
    }

    let output = SelectActionOutput {
        action: "remove".to_string(),
        account: account.email.clone(),
        count,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        _ => println!("✓ Removed {} message(s) from selection", count),
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
                messages: vec![],
            })?,
            _ => println!("Selection is empty"),
        }
        return Ok(());
    }

    // Convert entries to messages with shadow UIDs
    let messages: Vec<SelectionMessage> = entries
        .iter()
        .filter_map(|entry| {
            entry.shadow_uid.map(|id| SelectionMessage {
                id,
                folder: entry.folder.clone(),
                subject: entry.subject.clone(),
            })
        })
        .collect();

    let output = SelectionOutput {
        account: account.email.clone(),
        total_count: messages.len(),
        messages,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        "table" => {
            print_selection_table(&output);
        }
        _ => {
            println!("Selection for {}:", output.account);
            println!("Total: {} message(s)", output.total_count);
            println!();
            for msg in &output.messages {
                let subject_str = msg.subject.as_deref().unwrap_or("(no subject)");
                println!("  ID {}: {} - {}", msg.id, msg.folder, subject_str);
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

/// Print selection as a formatted table
fn print_selection_table(output: &SelectionOutput) {
    println!(
        "Selection for {} ({} messages)",
        output.account, output.total_count
    );

    if output.messages.is_empty() {
        println!("No messages in selection.");
        return;
    }

    // Column widths
    let id_width = 8;
    let folder_width = 20;
    let subject_width = 50;

    // Header
    println!(
        "{:>id_w$}  {:folder_w$}  SUBJECT",
        "ID",
        "FOLDER",
        id_w = id_width,
        folder_w = folder_width,
    );

    // Separator
    println!(
        "{:->id_w$}  {:->folder_w$}  {:->subj_w$}",
        "",
        "",
        "",
        id_w = id_width,
        folder_w = folder_width,
        subj_w = subject_width,
    );

    // Rows
    for msg in &output.messages {
        let subject = msg.subject.as_deref().unwrap_or("(no subject)");
        let subject_truncated = if subject.len() > subject_width {
            format!("{}...", &subject[..subject_width - 3])
        } else {
            subject.to_string()
        };

        let folder_truncated = if msg.folder.len() > folder_width {
            format!("{}...", &msg.folder[..folder_width - 3])
        } else {
            msg.folder.clone()
        };

        println!(
            "{:>id_w$}  {:folder_w$}  {}",
            msg.id,
            folder_truncated,
            subject_truncated,
            id_w = id_width,
            folder_w = folder_width,
        );
    }
}
