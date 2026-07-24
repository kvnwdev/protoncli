use crate::core::imap::ImapClient;
use crate::models::{config::Config, folder::resolve_folder_reference};
use crate::output::json;
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize)]
struct FoldersOutput {
    account: String,
    folders: Vec<crate::models::folder::Folder>,
}

#[derive(Serialize)]
struct FolderActionOutput {
    success: bool,
    action: String,
    folder: String,
    message: String,
}

#[derive(Serialize)]
struct FolderRenameOutput {
    success: bool,
    action: String,
    from: String,
    to: String,
    message: String,
}

fn custom_folder_path(name: &str) -> String {
    if name.starts_with("Folders/") {
        name.to_string()
    } else {
        format!("Folders/{name}")
    }
}

pub async fn list_folders(output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    let mut client = ImapClient::connect(account).await?;
    let folders = client.list_folders().await?;

    let output = FoldersOutput {
        account: account.email.clone(),
        folders,
    };

    match output_format.unwrap_or("json") {
        "json" => json::print_json(&output)?,
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}

pub async fn create_folder(name: &str, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    let folder_path = custom_folder_path(name);

    let mut client = ImapClient::connect(account).await?;

    // Check if folder already exists
    if client.folder_exists(&folder_path).await? {
        return Err(anyhow!("Folder '{}' already exists", name));
    }

    client.create_folder(&folder_path).await?;

    let output = FolderActionOutput {
        success: true,
        action: "create".to_string(),
        folder: name.to_string(),
        message: format!("Folder '{}' created successfully", name),
    };

    match output_format.unwrap_or("json") {
        "json" => json::print_json(&output)?,
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}

pub async fn delete_folder(name: &str, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    let folder_path = custom_folder_path(name);

    let mut client = ImapClient::connect(account).await?;

    // Check if folder exists
    if !client.folder_exists(&folder_path).await? {
        return Err(anyhow!("Folder '{}' does not exist", name));
    }

    client.delete_folder(&folder_path).await?;

    let output = FolderActionOutput {
        success: true,
        action: "delete".to_string(),
        folder: name.to_string(),
        message: format!("Folder '{}' deleted successfully", name),
    };

    match output_format.unwrap_or("json") {
        "json" => json::print_json(&output)?,
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}

pub async fn rename_folder(from: &str, to: &str, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    let from_path = custom_folder_path(from);
    let to_path = custom_folder_path(to);

    let mut client = ImapClient::connect(account).await?;

    // Check if source folder exists
    if !client.folder_exists(&from_path).await? {
        return Err(anyhow!("Folder '{}' does not exist", from));
    }

    // Check if destination folder already exists
    if client.folder_exists(&to_path).await? {
        return Err(anyhow!("Folder '{}' already exists", to));
    }

    client.rename_folder(&from_path, &to_path).await?;

    let output = FolderRenameOutput {
        success: true,
        action: "rename".to_string(),
        from: from.to_string(),
        to: to.to_string(),
        message: format!("Folder renamed from '{}' to '{}'", from, to),
    };

    match output_format.unwrap_or("json") {
        "json" => json::print_json(&output)?,
        _ => {
            return Err(anyhow!("Unsupported output format"));
        }
    }

    Ok(())
}

pub async fn folder_status(name: &str, output_format: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured. Please add an account first."))?;

    let mut client = ImapClient::connect(account).await?;
    let folders = client.list_folders().await?;
    let folder_path = resolve_folder_reference(name, &folders)
        .ok_or_else(|| anyhow!("Folder '{name}' does not exist"))?;
    let status = client.folder_status(&folder_path).await?;

    match output_format.unwrap_or("json") {
        "json" => json::print_json(&status)?,
        _ => return Err(anyhow!("Unsupported output format")),
    }

    Ok(())
}
