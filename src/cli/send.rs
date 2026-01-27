use crate::core::smtp::SmtpClient;
use crate::models::config::Config;
use anyhow::{anyhow, Context, Result};
use lettre::message::{header::ContentType, Mailbox, Message, MultiPart, SinglePart};
use std::fs;
use std::path::Path;

pub struct EmailBuilder {
    from: Option<Mailbox>,
    to: Vec<Mailbox>,
    cc: Vec<Mailbox>,
    bcc: Vec<Mailbox>,
    subject: Option<String>,
    body: Option<String>,
    attachments: Vec<String>,
}

impl EmailBuilder {
    pub fn new() -> Self {
        Self {
            from: None,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: None,
            body: None,
            attachments: Vec::new(),
        }
    }

    pub fn from(mut self, email: &str) -> Self {
        self.from = email.parse().ok();
        self
    }

    pub fn to(mut self, email: &str) -> Result<Self> {
        let mailbox = email
            .parse()
            .context(format!("Invalid email address: {}", email))?;
        self.to.push(mailbox);
        Ok(self)
    }

    pub fn cc(mut self, email: &str) -> Result<Self> {
        let mailbox = email
            .parse()
            .context(format!("Invalid email address: {}", email))?;
        self.cc.push(mailbox);
        Ok(self)
    }

    pub fn bcc(mut self, email: &str) -> Result<Self> {
        let mailbox = email
            .parse()
            .context(format!("Invalid email address: {}", email))?;
        self.bcc.push(mailbox);
        Ok(self)
    }

    pub fn subject(mut self, subject: String) -> Self {
        self.subject = Some(subject);
        self
    }

    pub fn body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

    pub fn attach(mut self, file_path: String) -> Self {
        self.attachments.push(file_path);
        self
    }

    pub fn build(self) -> Result<Message> {
        let from = self.from.ok_or_else(|| anyhow!("From address required"))?;

        if self.to.is_empty() {
            return Err(anyhow!("At least one recipient required"));
        }

        let subject = self.subject.unwrap_or_else(|| "(No subject)".to_string());
        let body_text = self.body.unwrap_or_default();

        // Start building message
        let mut message_builder = Message::builder().from(from);

        // Add recipients
        for to_addr in &self.to {
            message_builder = message_builder.to(to_addr.clone());
        }

        for cc_addr in &self.cc {
            message_builder = message_builder.cc(cc_addr.clone());
        }

        for bcc_addr in &self.bcc {
            message_builder = message_builder.bcc(bcc_addr.clone());
        }

        message_builder = message_builder.subject(subject);

        // Build message with or without attachments
        if self.attachments.is_empty() {
            // Simple text message
            message_builder
                .body(body_text)
                .context("Failed to build message")
        } else {
            // Multipart message with attachments
            let mut multipart = MultiPart::mixed().singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(body_text),
            );

            // Add each attachment
            for attachment_path in &self.attachments {
                let file_content = fs::read(attachment_path)
                    .context(format!("Failed to read attachment: {}", attachment_path))?;

                let filename = Path::new(attachment_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("attachment");

                multipart = multipart.singlepart(
                    lettre::message::Attachment::new(filename.to_string()).body(
                        file_content,
                        ContentType::parse("application/octet-stream").unwrap(),
                    ),
                );
            }

            message_builder
                .multipart(multipart)
                .context("Failed to build message with attachments")
        }
    }
}

pub async fn send_email(
    from: Option<String>,
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    subject: Option<String>,
    body: Option<String>,
    body_file: Option<String>,
    attachments: Vec<String>,
) -> Result<()> {
    let config = Config::load()?;

    // Determine which account to use for SMTP credentials and which email for From header
    let (smtp_account, from_email) = if let Some(from_addr) = &from {
        // Check if --from matches a configured account
        if let Some(account) = config.get_account(from_addr) {
            // Use the matched account for SMTP and From
            (account, account.email.clone())
        } else {
            // Use default account for SMTP, but specified email for From (alias support)
            let default = config
                .get_default_account()
                .ok_or_else(|| anyhow!("No default account configured. Configure an account with 'protoncli account add' or specify a configured account with --from"))?;
            (default, from_addr.clone())
        }
    } else {
        // No --from specified, use default account for both
        let default = config
            .get_default_account()
            .ok_or_else(|| anyhow!("No default account configured. Use --from to specify an account or set a default with 'protoncli account set-default'"))?;
        (default, default.email.clone())
    };

    // Read body from file if specified
    let body_text = if let Some(file_path) = body_file {
        fs::read_to_string(&file_path)
            .context(format!("Failed to read body file: {}", file_path))?
    } else {
        body.unwrap_or_default()
    };

    // Build email
    let mut builder = EmailBuilder::new()
        .from(&from_email)
        .subject(subject.unwrap_or_else(|| "(No subject)".to_string()))
        .body(body_text);

    // Add recipients
    for to_addr in to {
        builder = builder.to(&to_addr)?;
    }

    for cc_addr in cc {
        builder = builder.cc(&cc_addr)?;
    }

    for bcc_addr in bcc {
        builder = builder.bcc(&bcc_addr)?;
    }

    // Add attachments
    for attachment in attachments {
        builder = builder.attach(attachment);
    }

    let message = builder.build()?;

    // Connect to SMTP and send
    println!("Connecting to SMTP server...");
    let mut smtp_client = SmtpClient::connect(smtp_account)?;

    println!("Sending email...");
    let response = smtp_client.send_message(message)?;

    println!("✓ Email sent successfully!");
    println!("  Response: {:?}", response);

    Ok(())
}
