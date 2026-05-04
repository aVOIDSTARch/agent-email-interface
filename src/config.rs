use std::error::Error;

use crate::mail::MailConfig;

pub struct AppConfig {
    pub mail: MailConfig,
    pub http_port: u16,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            mail: MailConfig {
                imap_host: std::env::var("IMAP_HOST")
                    .unwrap_or_else(|_| "127.0.0.1".to_string()),
                imap_port: std::env::var("IMAP_PORT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1143),
                smtp_host: std::env::var("SMTP_HOST")
                    .unwrap_or_else(|_| "127.0.0.1".to_string()),
                smtp_port: std::env::var("SMTP_PORT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1025),
                username: std::env::var("PROTON_USERNAME")
                    .map_err(|_| "PROTON_USERNAME is required")?,
                password: std::env::var("PROTON_PASSWORD")
                    .map_err(|_| "PROTON_PASSWORD is required")?,
                mailbox: std::env::var("MAILBOX").unwrap_or_else(|_| "INBOX".to_string()),
            },
            http_port: std::env::var("HTTP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8420),
        })
    }
}
