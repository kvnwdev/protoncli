use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "protoncli";

pub struct KeychainManager;

impl KeychainManager {
    pub fn set_password(email: &str, password: &str) -> Result<()> {
        let entry = Entry::new(SERVICE_NAME, email)
            .context("Failed to create keychain entry")?;

        entry
            .set_password(password)
            .context("Failed to store password in keychain")?;

        Ok(())
    }

    pub fn get_password(email: &str) -> Result<String> {
        let entry = Entry::new(SERVICE_NAME, email)
            .context("Failed to create keychain entry")?;

        let password = entry
            .get_password()
            .context("Failed to retrieve password from keychain")?;

        Ok(password)
    }

    pub fn delete_password(email: &str) -> Result<()> {
        let entry = Entry::new(SERVICE_NAME, email)
            .context("Failed to create keychain entry")?;

        entry
            .delete_password()
            .context("Failed to delete password from keychain")?;

        Ok(())
    }

    pub fn password_exists(email: &str) -> bool {
        if let Ok(entry) = Entry::new(SERVICE_NAME, email) {
            entry.get_password().is_ok()
        } else {
            false
        }
    }
}
