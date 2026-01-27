use crate::models::account::Account;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default = "default_output_format")]
    pub default_output: String,
    #[serde(default = "default_date_filter_days")]
    pub date_filter_days: u32,
    #[serde(default = "default_cache_enabled")]
    pub cache_enabled: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_output_format() -> String {
    "json".to_string()
}

fn default_date_filter_days() -> u32 {
    3
}

fn default_cache_enabled() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_output: default_output_format(),
            date_filter_days: default_date_filter_days(),
            cache_enabled: default_cache_enabled(),
            log_level: default_log_level(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub preferences: Preferences,
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config directory")?
            .join("protoncli");

        fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        Ok(config_dir.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self {
                accounts: vec![],
                preferences: Preferences::default(),
            });
        }

        let contents = fs::read_to_string(&config_path).context("Failed to read config file")?;

        let config: Config = toml::from_str(&contents).context("Failed to parse config file")?;

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let toml_str = toml::to_string_pretty(self).context("Failed to serialize config")?;

        // Create file with restrictive permissions (0600 on Unix)
        #[cfg(unix)]
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&config_path)
                .context("Failed to create config file")?;
            file.write_all(toml_str.as_bytes())
                .context("Failed to write config file")?;
        }

        #[cfg(not(unix))]
        {
            fs::write(&config_path, toml_str).context("Failed to write config file")?;
        }

        Ok(())
    }

    pub fn get_account(&self, email: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.email == email)
    }

    pub fn get_default_account(&self) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|a| a.default)
            .or_else(|| self.accounts.first())
    }

    pub fn add_account(&mut self, account: Account) {
        // If this is the first account or marked as default, make it default
        if self.accounts.is_empty() || account.default {
            // Unset all other defaults
            for acc in &mut self.accounts {
                acc.default = false;
            }
        }

        // Remove existing account with same email if present
        self.accounts.retain(|a| a.email != account.email);
        self.accounts.push(account);
    }

    pub fn remove_account(&mut self, email: &str) -> bool {
        let original_len = self.accounts.len();
        self.accounts.retain(|a| a.email != email);

        // If we removed the default account and there are still accounts left,
        // make the first one default
        if self.accounts.len() < original_len
            && !self.accounts.is_empty()
            && !self.accounts.iter().any(|a| a.default)
        {
            self.accounts[0].default = true;
        }

        self.accounts.len() < original_len
    }

    pub fn set_default_account(&mut self, email: &str) -> bool {
        let mut found = false;
        for account in &mut self.accounts {
            if account.email == email {
                account.default = true;
                found = true;
            } else {
                account.default = false;
            }
        }
        found
    }
}
