use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SecurityType {
    StartTls,
    Ssl,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: SecurityType,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: SecurityType,
    #[serde(default)]
    pub default: bool,
}

impl Account {
    pub fn new_protonmail_bridge(email: String) -> Self {
        Self {
            email,
            imap_host: "127.0.0.1".to_string(),
            imap_port: 1143,
            imap_security: SecurityType::StartTls,
            smtp_host: "127.0.0.1".to_string(),
            smtp_port: 1025,
            smtp_security: SecurityType::Ssl,
            default: false,
        }
    }
}
