use crate::core::imap::ImapClient;
use crate::models::config::Config;
use crate::models::folder::Folder;
use crate::output::json;
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize)]
struct LabelsOutput {
    account: String,
    labels: Vec<LabelInfo>,
}

#[derive(Serialize)]
struct LabelInfo {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct LabelActionOutput {
    success: bool,
    action: String,
    label: String,
    message: String,
}

#[derive(Serialize)]
struct LabelRenameOutput {
    success: bool,
    action: String,
    from: String,
    to: String,
    message: String,
}

/// Convert folder to label info if it's a label folder
fn folder_to_label(folder: &Folder) -> Option<LabelInfo> {
    if folder.path.starts_with("Labels/") {
        let name = folder.path.strip_prefix("Labels/").unwrap_or(&folder.path);
        Some(LabelInfo {
            name: name.to_string(),
            path: folder.path.clone(),
        })
    } else {
        None
    }
}

pub async fn list_labels(output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    let mut client = ImapClient::connect(account).await?;
    let folders = client.list_folders().await?;

    // Filter to only labels (folders starting with "Labels/")
    let labels: Vec<LabelInfo> = folders
        .iter()
        .filter_map(folder_to_label)
        .collect();

    let output = LabelsOutput {
        account: account.email.clone(),
        labels,
    };

    match output_format.unwrap_or("json") {
        "json" => json::print_json(&output)?,
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}

pub async fn create_label(name: &str, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    // Prepend "Labels/" prefix for ProtonMail labels
    let label_path = format!("Labels/{}", name);

    let mut client = ImapClient::connect(account).await?;

    // Check if label already exists
    if client.folder_exists(&label_path).await? {
        return Err(anyhow!("Label '{}' already exists", name));
    }

    client.create_folder(&label_path).await?;

    let output = LabelActionOutput {
        success: true,
        action: "create".to_string(),
        label: name.to_string(),
        message: format!("Label '{}' created successfully", name),
    };

    match output_format.unwrap_or("json") {
        "json" => json::print_json(&output)?,
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}

pub async fn delete_label(name: &str, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    // Prepend "Labels/" prefix for ProtonMail labels
    let label_path = format!("Labels/{}", name);

    let mut client = ImapClient::connect(account).await?;

    // Check if label exists
    if !client.folder_exists(&label_path).await? {
        return Err(anyhow!("Label '{}' does not exist", name));
    }

    client.delete_folder(&label_path).await?;

    let output = LabelActionOutput {
        success: true,
        action: "delete".to_string(),
        label: name.to_string(),
        message: format!("Label '{}' deleted successfully", name),
    };

    match output_format.unwrap_or("json") {
        "json" => json::print_json(&output)?,
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}

pub async fn rename_label(from: &str, to: &str, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    // Prepend "Labels/" prefix for ProtonMail labels
    let from_path = format!("Labels/{}", from);
    let to_path = format!("Labels/{}", to);

    let mut client = ImapClient::connect(account).await?;

    // Check if source label exists
    if !client.folder_exists(&from_path).await? {
        return Err(anyhow!("Label '{}' does not exist", from));
    }

    // Check if destination label already exists
    if client.folder_exists(&to_path).await? {
        return Err(anyhow!("Label '{}' already exists", to));
    }

    client.rename_folder(&from_path, &to_path).await?;

    let output = LabelRenameOutput {
        success: true,
        action: "rename".to_string(),
        from: from.to_string(),
        to: to.to_string(),
        message: format!("Label renamed from '{}' to '{}'", from, to),
    };

    match output_format.unwrap_or("json") {
        "json" => json::print_json(&output)?,
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}
