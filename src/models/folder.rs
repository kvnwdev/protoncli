use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FolderType {
    Inbox,
    Sent,
    Drafts,
    Archive,
    Spam,
    Trash,
    Starred,
    AllMail,
    Label,
    Custom,
    Other,
}

impl FolderType {
    pub fn from_path(path: &str) -> Self {
        match path {
            "INBOX" | "Inbox" => FolderType::Inbox,
            "Sent" => FolderType::Sent,
            "Drafts" => FolderType::Drafts,
            "Archive" => FolderType::Archive,
            "Spam" => FolderType::Spam,
            "Trash" => FolderType::Trash,
            "Starred" => FolderType::Starred,
            "All Mail" => FolderType::AllMail,
            p if p.starts_with("Labels/") => FolderType::Label,
            p if p.starts_with("Folders/") => FolderType::Custom,
            _ => FolderType::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub path: String,
    pub folder_type: FolderType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
}

impl Folder {
    pub fn new(path: String, delimiter: Option<String>) -> Self {
        let folder_type = FolderType::from_path(&path);
        Self {
            path,
            folder_type,
            delimiter,
        }
    }
}
