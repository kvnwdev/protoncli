use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAddress {
    pub name: Option<String>,
    pub address: String,
}

impl EmailAddress {
    pub fn new(address: String, name: Option<String>) -> Self {
        Self { name, address }
    }

    pub fn format(&self) -> String {
        if let Some(ref name) = self.name {
            format!("{} <{}>", name, self.address)
        } else {
            self.address.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFlags {
    pub seen: bool,
    pub answered: bool,
    pub flagged: bool,
    pub deleted: bool,
    pub draft: bool,
}

impl MessageFlags {
    pub fn from_imap_flags(flags: &[async_imap::types::Flag]) -> Self {
        use async_imap::types::Flag;

        Self {
            seen: flags.iter().any(|f| matches!(f, Flag::Seen)),
            answered: flags.iter().any(|f| matches!(f, Flag::Answered)),
            flagged: flags.iter().any(|f| matches!(f, Flag::Flagged)),
            deleted: flags.iter().any(|f| matches!(f, Flag::Deleted)),
            draft: flags.iter().any(|f| matches!(f, Flag::Draft)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub uid: u32,
    pub message_id: Option<String>,
    pub subject: Option<String>,
    pub from: Option<EmailAddress>,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub date: Option<DateTime<Utc>>,
    pub flags: MessageFlags,
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_read: Option<bool>,

    // Full message fields (for read command)
    pub bcc: Vec<EmailAddress>,
    pub reply_to: Option<EmailAddress>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_message: Option<Vec<u8>>,
}

impl Message {
    pub fn new(uid: u32) -> Self {
        Self {
            uid,
            message_id: None,
            subject: None,
            from: None,
            to: vec![],
            cc: vec![],
            date: None,
            flags: MessageFlags {
                seen: false,
                answered: false,
                flagged: false,
                deleted: false,
                draft: false,
            },
            preview: None,
            agent_read: None,
            bcc: vec![],
            reply_to: None,
            body_text: None,
            body_html: None,
            headers: HashMap::new(),
            raw_message: None,
        }
    }
}
