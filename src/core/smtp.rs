use crate::core::auth::KeychainManager;
use crate::models::account::{Account, SecurityType};
use anyhow::{anyhow, Context, Result};
use lettre::{
    message::Message,
    transport::smtp::{authentication::Credentials, response::Response},
    SmtpTransport, Transport,
};

pub struct SmtpClient {
    transport: SmtpTransport,
}

impl SmtpClient {
    pub fn connect(account: &Account) -> Result<Self> {
        // Get password from keychain
        let password = KeychainManager::get_password(&account.email)
            .context("Failed to retrieve password from keychain")?;

        // Build SMTP transport based on security type
        let transport = match account.smtp_security {
            SecurityType::StartTls => SmtpTransport::starttls_relay(&account.smtp_host)?
                .port(account.smtp_port)
                .credentials(Credentials::new(account.email.clone(), password))
                .build(),
            SecurityType::Ssl => SmtpTransport::relay(&account.smtp_host)?
                .port(account.smtp_port)
                .credentials(Credentials::new(account.email.clone(), password))
                .build(),
            SecurityType::None => {
                return Err(anyhow!("Insecure SMTP connections are not supported"));
            }
        };

        Ok(Self { transport })
    }

    pub fn send_message(&mut self, message: Message) -> Result<Response> {
        self.transport
            .send(&message)
            .context("Failed to send email via SMTP")
    }
}
