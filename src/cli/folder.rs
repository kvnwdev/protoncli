use crate::core::imap::ImapClient;
use crate::models::config::Config;
use crate::output::json;
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize)]
struct FoldersOutput {
    account: String,
    folders: Vec<crate::models::folder::Folder>,
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
