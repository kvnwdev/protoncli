use crate::core::auth::KeychainManager;
use crate::models::account::{Account, SecurityType};
use anyhow::{Context, Result};
use lettre::{
    message::Message,
    transport::smtp::{authentication::Credentials, response::Response},
    SmtpTransport, Transport,
};
use secrecy::ExposeSecret;

pub struct SmtpClient {
    transport: SmtpTransport,
}

impl SmtpClient {
    pub fn connect(account: &Account) -> Result<Self> {
        // Get password from keychain (returns SecretString for secure handling)
        let password = KeychainManager::get_password(&account.email)
            .context("Failed to retrieve password from keychain")?;

        // Build SMTP transport based on security type
        // SECURITY NOTE: SMTP connections go to localhost ProtonMail Bridge which uses
        // self-signed certificates. The lettre library handles TLS validation, and we
        // rely on the default account settings pointing to 127.0.0.1.
        let transport = match account.smtp_security {
            SecurityType::StartTls => SmtpTransport::starttls_relay(&account.smtp_host)?
                .port(account.smtp_port)
                .credentials(Credentials::new(
                    account.email.clone(),
                    password.expose_secret().to_string(),
                ))
                .build(),
            SecurityType::Ssl => SmtpTransport::relay(&account.smtp_host)?
                .port(account.smtp_port)
                .credentials(Credentials::new(
                    account.email.clone(),
                    password.expose_secret().to_string(),
                ))
                .build(),
        };

        Ok(Self { transport })
    }

    pub fn send_message(&mut self, message: Message) -> Result<Response> {
        self.transport
            .send(&message)
            .context("Failed to send email via SMTP")
    }
}
