use crate::models::message::Message;

/// Truncate a string to fit within max_width, adding "..." if truncated
fn truncate(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else if max_width <= 3 {
        s.chars().take(max_width).collect()
    } else {
        format!("{}...", &s[..max_width - 3])
    }
}

/// Format sender for display (prefer name, fallback to email)
fn format_sender(message: &Message) -> String {
    match &message.from {
        Some(addr) => addr.name.as_deref().unwrap_or(&addr.address).to_string(),
        None => String::new(),
    }
}

/// Format flags as compact indicators
fn format_flags(message: &Message) -> String {
    let mut flags = String::new();
    if !message.flags.seen {
        flags.push('●'); // Unread indicator
    }
    if message.flags.flagged {
        flags.push('★'); // Starred
    }
    if message.flags.answered {
        flags.push('↩'); // Replied
    }
    flags
}

/// Format date compactly
fn format_date(message: &Message) -> String {
    match &message.date {
        Some(date) => date.format("%m/%d %H:%M").to_string(),
        None => String::new(),
    }
}

pub fn format_message_table(account: &str, folder: &str, messages: &[Message]) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "{}/{} ({} messages)\n",
        account,
        folder,
        messages.len()
    ));

    if messages.is_empty() {
        output.push_str("No messages found.\n");
        return output;
    }

    // Column widths
    let id_width = 8;
    let flags_width = 3;
    let date_width = 11;
    let from_width = 20;
    let subject_width = 45;

    // Table header
    output.push_str(&format!(
        "{:>id_w$}  {:flags_w$}  {:date_w$}  {:from_w$}  {}\n",
        "ID",
        "",
        "DATE",
        "FROM",
        "SUBJECT",
        id_w = id_width,
        flags_w = flags_width,
        date_w = date_width,
        from_w = from_width,
    ));

    // Separator
    output.push_str(&format!(
        "{:->id_w$}  {:->flags_w$}  {:->date_w$}  {:->from_w$}  {:->subj_w$}\n",
        "",
        "",
        "",
        "",
        "",
        id_w = id_width,
        flags_w = flags_width,
        date_w = date_width,
        from_w = from_width,
        subj_w = subject_width,
    ));

    // Rows
    for message in messages {
        let flags = format_flags(message);
        let date = format_date(message);
        let from = truncate(&format_sender(message), from_width);
        let subject = truncate(
            message.subject.as_deref().unwrap_or("(no subject)"),
            subject_width,
        );

        // Display shadow UID as ID (primary identifier), fall back to IMAP UID if not assigned
        let id_display = message
            .shadow_uid
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("~{}", message.uid));

        output.push_str(&format!(
            "{:>id_w$}  {:flags_w$}  {:date_w$}  {:from_w$}  {}\n",
            id_display,
            flags,
            date,
            from,
            subject,
            id_w = id_width,
            flags_w = flags_width,
            date_w = date_width,
            from_w = from_width,
        ));
    }

    output
}

pub fn print_message_table(account: &str, folder: &str, messages: &[Message]) {
    print!("{}", format_message_table(account, folder, messages));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::message::{EmailAddress, MessageFlags};
    use chrono::TimeZone;

    fn make_test_message(
        shadow_uid: i64,
        uid: u32,
        subject: &str,
        from_name: &str,
        seen: bool,
    ) -> Message {
        let mut msg = Message::new(uid);
        msg.shadow_uid = Some(shadow_uid);
        msg.subject = Some(subject.to_string());
        msg.from = Some(EmailAddress {
            name: Some(from_name.to_string()),
            address: "test@example.com".to_string(),
        });
        msg.date = Some(
            chrono::Utc
                .with_ymd_and_hms(2024, 1, 15, 10, 30, 0)
                .unwrap(),
        );
        msg.flags = MessageFlags {
            seen,
            flagged: false,
            answered: false,
            deleted: false,
            draft: false,
        };
        msg
    }

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_format_flags_unread() {
        let msg = make_test_message(1, 100, "Test", "Sender", false);
        assert!(format_flags(&msg).contains('●'));
    }

    #[test]
    fn test_format_flags_read() {
        let msg = make_test_message(1, 100, "Test", "Sender", true);
        assert!(!format_flags(&msg).contains('●'));
    }

    #[test]
    fn test_format_message_table_empty() {
        let output = format_message_table("test@example.com", "INBOX", &[]);
        assert!(output.contains("0 messages"));
        assert!(output.contains("No messages found"));
    }

    #[test]
    fn test_format_message_table_with_messages() {
        let messages = vec![
            make_test_message(42, 123, "Test Subject", "John Doe", false),
            make_test_message(43, 124, "Another Subject", "Jane Smith", true),
        ];
        let output = format_message_table("test@example.com", "INBOX", &messages);
        assert!(output.contains("2 messages"));
        // Should show shadow UIDs (42, 43), not IMAP UIDs (123, 124)
        assert!(output.contains("42"));
        assert!(output.contains("43"));
        assert!(output.contains("Test Subject"));
        assert!(output.contains("John Doe"));
        // Verify header says "ID" not "UID"
        assert!(output.contains("ID"));
        assert!(!output.contains("UID"));
    }
}
