use anyhow::{Context, Result};
use keyring::Entry;
use secrecy::{ExposeSecret, SecretString};

const SERVICE_NAME: &str = "protoncli";

pub struct KeychainManager;

impl KeychainManager {
    /// Store a password securely in the system keychain.
    /// The password is accepted as a SecretString to encourage secure handling throughout.
    pub fn set_password(email: &str, password: &SecretString) -> Result<()> {
        let entry = Entry::new(SERVICE_NAME, email).context("Failed to create keychain entry")?;

        entry
            .set_password(password.expose_secret())
            .context("Failed to store password in keychain")?;

        Ok(())
    }

    /// Retrieve a password from the system keychain.
    /// Returns a SecretString that will be zeroed on drop to prevent password leakage.
    pub fn get_password(email: &str) -> Result<SecretString> {
        let entry = Entry::new(SERVICE_NAME, email).context("Failed to create keychain entry")?;

        let password = entry
            .get_password()
            .context("Failed to retrieve password from keychain")?;

        // Wrap in SecretString immediately to ensure it's zeroed on drop
        Ok(SecretString::from(password))
    }

    pub fn delete_password(email: &str) -> Result<()> {
        let entry = Entry::new(SERVICE_NAME, email).context("Failed to create keychain entry")?;

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
