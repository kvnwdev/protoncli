use crate::core::imap::ImapClient;
use crate::core::state::StateManager;
use crate::models::config::Config;
use crate::models::filter::MessageFilter;
use crate::output::{json, table};
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize)]
pub struct QueryOutput {
    pub account: String,
    pub folder: String,
    pub query: String,
    pub count: usize,
    pub messages: Vec<QueryMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_to_selection: Option<usize>,
}

#[derive(Serialize)]
pub struct QueryMessage {
    pub uid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

/// Available fields for --fields option
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryField {
    Uid,
    MessageId,
    Subject,
    From,
    Date,
    Flags,
}

impl QueryField {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "uid" => Some(QueryField::Uid),
            "message_id" | "messageid" | "id" => Some(QueryField::MessageId),
            "subject" => Some(QueryField::Subject),
            "from" => Some(QueryField::From),
            "date" => Some(QueryField::Date),
            "flags" => Some(QueryField::Flags),
            _ => None,
        }
    }
}

/// Parse fields string into a set of QueryFields
pub fn parse_fields(fields_str: &str) -> Vec<QueryField> {
    fields_str
        .split(',')
        .filter_map(|s| QueryField::from_str(s.trim()))
        .collect()
}

/// Execute a query and optionally add results to selection
pub async fn execute_query(
    query_str: &str,
    folder: &str,
    fields: Option<&str>,
    limit: Option<usize>,
    preview: bool,
    select: bool,
    output_format: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let account = config
        .get_default_account()
        .ok_or_else(|| anyhow!("No default account configured"))?;

    // Build the message filter with query
    let mut filter = MessageFilter::new().with_query(query_str.to_string());
    if let Some(l) = limit {
        filter = filter.with_limit(l);
    }
    if preview {
        filter = filter.with_preview(true);
    }

    // Check if query contains `in:folder` - that takes precedence
    let effective_folder =
        MessageFilter::extract_folder_from_query(query_str).unwrap_or_else(|| folder.to_string());

    // Connect to IMAP and fetch messages
    let mut client = ImapClient::connect(account).await?;
    client.select_folder(&effective_folder).await?;

    // Fetch messages using the filter (IMAP does the filtering)
    let mut messages = client.fetch_messages(&filter).await?;

    // Initialize state manager for shadow UID assignment
    let state = StateManager::new().await?;

    // Assign shadow UIDs to all messages
    for message in &mut messages {
        message.folder = Some(effective_folder.clone());

        // Get or create shadow UID for this message
        if let Some(ref msg_id) = message.message_id {
            let shadow_uid = state
                .get_or_create_shadow_uid(
                    &account.email,
                    &effective_folder,
                    message.uid,
                    Some(msg_id),
                    message.subject.as_deref(),
                    message.from.as_ref().map(|f| f.address.as_str()),
                    message.date,
                )
                .await?;
            message.shadow_uid = Some(shadow_uid);
        }
    }

    // Parse which fields to include
    let requested_fields = fields.map(parse_fields);
    let show_all_fields = requested_fields.is_none();

    // Convert to output format
    let query_messages: Vec<QueryMessage> = messages
        .iter()
        .map(|msg| {
            let include = |field: QueryField| -> bool {
                show_all_fields
                    || requested_fields
                        .as_ref()
                        .is_some_and(|f| f.contains(&field))
            };

            QueryMessage {
                uid: msg.uid,
                message_id: if include(QueryField::MessageId) {
                    msg.message_id.clone()
                } else {
                    None
                },
                subject: if include(QueryField::Subject) {
                    msg.subject.clone()
                } else {
                    None
                },
                from: if include(QueryField::From) {
                    msg.from.as_ref().map(|f| f.format())
                } else {
                    None
                },
                date: if include(QueryField::Date) {
                    msg.date.map(|d| d.to_rfc3339())
                } else {
                    None
                },
                flags: if include(QueryField::Flags) {
                    let mut flags = Vec::new();
                    if msg.flags.seen {
                        flags.push("seen".to_string());
                    }
                    if msg.flags.answered {
                        flags.push("answered".to_string());
                    }
                    if msg.flags.flagged {
                        flags.push("flagged".to_string());
                    }
                    if msg.flags.deleted {
                        flags.push("deleted".to_string());
                    }
                    if msg.flags.draft {
                        flags.push("draft".to_string());
                    }
                    Some(flags)
                } else {
                    None
                },
                preview: if preview { msg.preview.clone() } else { None },
            }
        })
        .collect();

    // Save query results for potential `select last`
    let result_entries: Vec<(u32, Option<&str>, Option<&str>)> = messages
        .iter()
        .map(|msg| (msg.uid, msg.message_id.as_deref(), msg.subject.as_deref()))
        .collect();

    state
        .save_query_results(
            &account.email,
            &effective_folder,
            query_str,
            &result_entries,
        )
        .await?;

    // Optionally add to selection
    let added_to_selection = if select {
        let count = state
            .add_to_selection(&account.email, &effective_folder, &result_entries)
            .await?;
        Some(count)
    } else {
        None
    };

    let output = QueryOutput {
        account: account.email.clone(),
        folder: effective_folder.clone(),
        query: query_str.to_string(),
        count: query_messages.len(),
        messages: query_messages,
        added_to_selection,
    };

    match output_format.unwrap_or("text") {
        "json" => json::print_json(&output)?,
        "markdown" => print_markdown(&output)?,
        "table" => {
            table::print_message_table(&output.account, &output.folder, &messages);
            if let Some(count) = output.added_to_selection {
                println!();
                println!("✓ Added {} message(s) to selection", count);
            }
        }
        _ => print_text(&output)?,
    }

    Ok(())
}

fn print_text(output: &QueryOutput) -> Result<()> {
    println!(
        "Query '{}' in {}/{}: {} result(s)",
        output.query, output.account, output.folder, output.count
    );

    if output.count == 0 {
        return Ok(());
    }

    println!();

    for msg in &output.messages {
        let mut parts = vec![format!("UID {}", msg.uid)];

        if let Some(from) = &msg.from {
            parts.push(format!("from: {}", from));
        }
        if let Some(subject) = &msg.subject {
            parts.push(format!("\"{}\"", subject));
        }
        if let Some(date) = &msg.date {
            parts.push(format!("[{}]", date));
        }
        if let Some(flags) = &msg.flags {
            if !flags.is_empty() {
                parts.push(format!("({})", flags.join(", ")));
            }
        }

        println!("  {}", parts.join(" | "));

        if let Some(preview) = &msg.preview {
            let preview_short: String = preview.chars().take(100).collect();
            println!("    {}", preview_short);
        }
    }

    if let Some(count) = output.added_to_selection {
        println!();
        println!("✓ Added {} message(s) to selection", count);
    }

    Ok(())
}

fn print_markdown(output: &QueryOutput) -> Result<()> {
    println!("## Query Results");
    println!();
    println!(
        "**Query:** `{}`  \n**Folder:** {}  \n**Results:** {}",
        output.query, output.folder, output.count
    );
    println!();

    if output.count == 0 {
        println!("*No messages found*");
        return Ok(());
    }

    println!("| UID | From | Subject | Date |");
    println!("|-----|------|---------|------|");

    for msg in &output.messages {
        let from = msg.from.as_deref().unwrap_or("-");
        let subject = msg.subject.as_deref().unwrap_or("-");
        let date = msg.date.as_deref().unwrap_or("-");
        println!("| {} | {} | {} | {} |", msg.uid, from, subject, date);
    }

    if let Some(count) = output.added_to_selection {
        println!();
        println!("*Added {} message(s) to selection*", count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_field_from_str_basic() {
        assert_eq!(QueryField::from_str("uid"), Some(QueryField::Uid));
        assert_eq!(QueryField::from_str("subject"), Some(QueryField::Subject));
        assert_eq!(QueryField::from_str("from"), Some(QueryField::From));
        assert_eq!(QueryField::from_str("date"), Some(QueryField::Date));
        assert_eq!(QueryField::from_str("flags"), Some(QueryField::Flags));
    }

    #[test]
    fn test_query_field_from_str_message_id_variants() {
        assert_eq!(
            QueryField::from_str("message_id"),
            Some(QueryField::MessageId)
        );
        assert_eq!(
            QueryField::from_str("messageid"),
            Some(QueryField::MessageId)
        );
        assert_eq!(QueryField::from_str("id"), Some(QueryField::MessageId));
    }

    #[test]
    fn test_query_field_from_str_case_insensitive() {
        assert_eq!(QueryField::from_str("UID"), Some(QueryField::Uid));
        assert_eq!(QueryField::from_str("SUBJECT"), Some(QueryField::Subject));
        assert_eq!(QueryField::from_str("FROM"), Some(QueryField::From));
        assert_eq!(
            QueryField::from_str("Message_Id"),
            Some(QueryField::MessageId)
        );
    }

    #[test]
    fn test_query_field_from_str_invalid() {
        assert_eq!(QueryField::from_str("invalid"), None);
        assert_eq!(QueryField::from_str("body"), None);
        assert_eq!(QueryField::from_str(""), None);
        assert_eq!(QueryField::from_str("subject "), None);
    }

    #[test]
    fn test_parse_fields_simple() {
        let fields = parse_fields("uid,subject,from");
        assert_eq!(fields.len(), 3);
        assert!(fields.contains(&QueryField::Uid));
        assert!(fields.contains(&QueryField::Subject));
        assert!(fields.contains(&QueryField::From));
    }

    #[test]
    fn test_parse_fields_with_spaces() {
        let fields = parse_fields("uid, subject , from");
        assert_eq!(fields.len(), 3);
        assert!(fields.contains(&QueryField::Uid));
        assert!(fields.contains(&QueryField::Subject));
        assert!(fields.contains(&QueryField::From));
    }

    #[test]
    fn test_parse_fields_ignores_invalid() {
        let fields = parse_fields("uid,invalid,subject,body");
        assert_eq!(fields.len(), 2);
        assert!(fields.contains(&QueryField::Uid));
        assert!(fields.contains(&QueryField::Subject));
    }

    #[test]
    fn test_parse_fields_empty_string() {
        let fields = parse_fields("");
        assert!(fields.is_empty());
    }

    #[test]
    fn test_parse_fields_all_invalid() {
        let fields = parse_fields("invalid,body,foo");
        assert!(fields.is_empty());
    }

    #[test]
    fn test_parse_fields_single_field() {
        let fields = parse_fields("uid");
        assert_eq!(fields.len(), 1);
        assert!(fields.contains(&QueryField::Uid));
    }
}
