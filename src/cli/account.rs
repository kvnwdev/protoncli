use crate::core::auth::KeychainManager;
use crate::core::imap::ImapClient;
use crate::models::account::Account;
use crate::models::config::Config;
use anyhow::{anyhow, Context, Result};

pub async fn test_account(email: &str) -> Result<()> {
    let config = Config::load()?;

    let account = config
        .get_account(email)
        .ok_or_else(|| anyhow!("Account {} not found in config", email))?;

    println!("Testing connection to ProtonMail Bridge...");
    println!("  Email: {}", account.email);
    println!("  IMAP: {}:{}", account.imap_host, account.imap_port);
    println!("  SMTP: {}:{}", account.smtp_host, account.smtp_port);
    println!();

    match ImapClient::test_connection(account).await {
        Ok(message) => {
            println!("✓ {}", message);
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ Connection failed: {}", e);
            eprintln!();
            eprintln!("Troubleshooting:");
            eprintln!("  1. Ensure ProtonMail Bridge is running");
            eprintln!("  2. Verify the password is stored in keychain");
            eprintln!("  3. Check Bridge settings for correct ports");
            Err(e)
        }
    }
}

pub async fn add_account(email: &str) -> Result<()> {
    let mut config = Config::load()?;

    // Check if account already exists
    if config.get_account(email).is_some() {
        return Err(anyhow!("Account {} already exists", email));
    }

    // Prompt for password (read from stdin without echo)
    println!("Enter password for {}: ", email);
    use std::io::{self, Write};
    io::stdout().flush()?;
    let password = rpassword::read_password().context("Failed to read password")?;

    if password.is_empty() {
        return Err(anyhow!("Password cannot be empty"));
    }

    // Store password in keychain
    KeychainManager::set_password(email, &password)?;

    // Create account with ProtonMail Bridge defaults
    let account = Account::new_protonmail_bridge(email.to_string());

    // Add to config
    config.add_account(account);
    config.save()?;

    println!("✓ Account {} added successfully", email);
    println!("  IMAP: {}:{}", "127.0.0.1", 1143);
    println!("  SMTP: {}:{}", "127.0.0.1", 1025);
    println!();
    println!("Test connection with: protoncli account test {}", email);

    Ok(())
}

pub fn list_accounts() -> Result<()> {
    let config = Config::load()?;

    if config.accounts.is_empty() {
        println!("No accounts configured.");
        println!("Add an account with: protoncli account add <email>");
        return Ok(());
    }

    println!("Configured accounts:\n");

    for account in &config.accounts {
        let default_marker = if account.default { " (default)" } else { "" };
        let password_status = if KeychainManager::password_exists(&account.email) {
            "✓ password stored"
        } else {
            "✗ password missing"
        };

        println!("  {} {}", account.email, default_marker);
        println!(
            "    IMAP: {}:{} ({})",
            account.imap_host,
            account.imap_port,
            format!("{:?}", account.imap_security).to_lowercase()
        );
        println!(
            "    SMTP: {}:{} ({})",
            account.smtp_host,
            account.smtp_port,
            format!("{:?}", account.smtp_security).to_lowercase()
        );
        println!("    Status: {}", password_status);
        println!();
    }

    Ok(())
}

pub fn set_default_account(email: &str) -> Result<()> {
    let mut config = Config::load()?;

    if !config.set_default_account(email) {
        return Err(anyhow!("Account {} not found", email));
    }

    config.save()?;

    println!("✓ Set {} as default account", email);

    Ok(())
}

pub fn remove_account(email: &str) -> Result<()> {
    let mut config = Config::load()?;

    // Check if account exists
    if config.get_account(email).is_none() {
        return Err(anyhow!("Account {} not found", email));
    }

    // Confirm deletion
    println!(
        "Remove account {}? This will delete the account configuration and password from keychain.",
        email
    );
    println!("Type 'yes' to confirm: ");

    use std::io::{self, BufRead};
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    if line.trim().to_lowercase() != "yes" {
        println!("Cancelled");
        return Ok(());
    }

    // Remove from config
    if !config.remove_account(email) {
        return Err(anyhow!("Failed to remove account from config"));
    }

    // Delete password from keychain
    if let Err(e) = KeychainManager::delete_password(email) {
        eprintln!("Warning: Failed to delete password from keychain: {}", e);
    }

    config.save()?;

    println!("✓ Account {} removed", email);

    Ok(())
}
