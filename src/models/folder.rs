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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub path: String,
    pub folder_type: FolderType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderStatus {
    pub path: String,
    pub messages: u32,
    pub unseen: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid_next: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid_validity: Option<u32>,
}

/// Resolve the names shown in Proton Mail to their IMAP mailbox paths.
pub fn resolve_folder_reference(reference: &str, folders: &[Folder]) -> Option<String> {
    if let Some(folder) = folders.iter().find(|folder| folder.path == reference) {
        return Some(folder.path.clone());
    }

    if reference.eq_ignore_ascii_case("inbox") {
        return folders
            .iter()
            .find(|folder| folder.folder_type == FolderType::Inbox)
            .map(|folder| folder.path.clone());
    }

    let custom_path = format!("Folders/{reference}");
    folders
        .iter()
        .find(|folder| folder.path == custom_path)
        .map(|folder| folder.path.clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn folders() -> Vec<Folder> {
        vec![
            Folder::new("INBOX".to_string(), Some("/".to_string())),
            Folder::new("Folders/Professional".to_string(), Some("/".to_string())),
            Folder::new("Folders/Promo".to_string(), Some("/".to_string())),
        ]
    }

    #[test]
    fn resolves_exact_imap_path() {
        assert_eq!(
            resolve_folder_reference("Folders/Professional", &folders()),
            Some("Folders/Professional".to_string())
        );
    }

    #[test]
    fn resolves_visible_custom_folder_name() {
        assert_eq!(
            resolve_folder_reference("Professional", &folders()),
            Some("Folders/Professional".to_string())
        );
    }

    #[test]
    fn resolves_inbox_case_insensitively() {
        assert_eq!(
            resolve_folder_reference("Inbox", &folders()),
            Some("INBOX".to_string())
        );
    }
}
