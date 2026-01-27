use crate::models::message::Message;

pub fn format_message_list(account: &str, folder: &str, messages: &[Message]) -> String {
    let mut output = String::new();

    output.push_str(&format!("# Messages in {}/{}\n\n", account, folder));
    output.push_str(&format!("Found {} message(s)\n\n", messages.len()));

    if messages.is_empty() {
        output.push_str("No messages found.\n");
        return output;
    }

    for message in messages {
        output.push_str("---\n\n");

        // Display shadow UID as ID (primary identifier), fall back to IMAP UID if not assigned
        let id_display = message
            .shadow_uid
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("~{}", message.uid));
        output.push_str(&format!("**ID:** {}\n\n", id_display));

        if let Some(ref subject) = message.subject {
            output.push_str(&format!("**Subject:** {}\n\n", subject));
        }

        if let Some(ref from) = message.from {
            output.push_str(&format!("**From:** {}\n\n", from.format()));
        }

        if let Some(ref date) = message.date {
            output.push_str(&format!(
                "**Date:** {}\n\n",
                date.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        // Flags
        let mut flags = Vec::new();
        if message.flags.seen {
            flags.push("Seen");
        }
        if message.flags.flagged {
            flags.push("Starred");
        }
        if message.flags.answered {
            flags.push("Answered");
        }
        if !flags.is_empty() {
            output.push_str(&format!("**Flags:** {}\n\n", flags.join(", ")));
        }

        if let Some(ref preview) = message.preview {
            output.push_str(&format!("**Preview:** {}\n\n", preview));
        }
    }

    output
}

pub fn print_message_list(account: &str, folder: &str, messages: &[Message]) {
    println!("{}", format_message_list(account, folder, messages));
}

fn basic_html_to_text(html: &str) -> String {
    // Simple HTML tag removal for terminal display
    html.replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .split('<')
        .map(|s| s.split_once('>').map(|(_, rest)| rest).unwrap_or(s))
        .collect::<String>()
}

pub fn print_message(message: &Message) {
    println!("# Message");
    println!();

    if let Some(subject) = &message.subject {
        println!("**Subject:** {}", subject);
    }

    if let Some(from) = &message.from {
        println!("**From:** {}", from.format());
    }

    if !message.to.is_empty() {
        let to_list: Vec<_> = message.to.iter().map(|e| e.format()).collect();
        println!("**To:** {}", to_list.join(", "));
    }

    if !message.cc.is_empty() {
        let cc_list: Vec<_> = message.cc.iter().map(|e| e.format()).collect();
        println!("**CC:** {}", cc_list.join(", "));
    }

    if !message.bcc.is_empty() {
        let bcc_list: Vec<_> = message.bcc.iter().map(|e| e.format()).collect();
        println!("**BCC:** {}", bcc_list.join(", "));
    }

    if let Some(reply_to) = &message.reply_to {
        println!("**Reply-To:** {}", reply_to.format());
    }

    if let Some(date) = &message.date {
        println!("**Date:** {}", date.format("%Y-%m-%d %H:%M:%S %Z"));
    }

    // Display shadow UID as ID (primary identifier), fall back to IMAP UID if not assigned
    let id_display = message
        .shadow_uid
        .map(|id| id.to_string())
        .unwrap_or_else(|| format!("~{}", message.uid));
    println!("**ID:** {}", id_display);

    if let Some(msg_id) = &message.message_id {
        println!("**Message-ID:** {}", msg_id);
    }

    // Flags
    let mut flags = Vec::new();
    if message.flags.seen {
        flags.push("Seen");
    }
    if message.flags.flagged {
        flags.push("Starred");
    }
    if message.flags.answered {
        flags.push("Answered");
    }
    if message.flags.draft {
        flags.push("Draft");
    }
    if !flags.is_empty() {
        println!("**Flags:** {}", flags.join(", "));
    }

    println!();
    println!("---");
    println!();

    // Show body
    if let Some(body_text) = &message.body_text {
        println!("{}", body_text);
    } else if let Some(body_html) = &message.body_html {
        // Basic HTML stripping for display
        println!("(HTML email - showing with tags stripped)");
        println!();
        let text = basic_html_to_text(body_html);
        println!("{}", text);
    } else {
        println!("(No body content)");
    }
}
