pub mod error;
pub mod imap;
pub mod session;
pub mod smtp;
pub mod types;

pub use error::MailError;
pub use types::{AgentMessage, MailConfig};

pub struct PanoramaMail {
    pub config: MailConfig,
}

impl PanoramaMail {
    pub fn new(config: MailConfig) -> Self {
        Self { config }
    }

    pub async fn get_by_uid(&self, uid: u32) -> Result<Option<AgentMessage>, MailError> {
        imap::get_by_uid(&self.config, uid).await
    }

    pub async fn list_mailboxes(&self) -> Result<Vec<String>, MailError> {
        imap::list_mailboxes(&self.config).await
    }
}

#[allow(async_fn_in_trait)]
pub trait AgentMailTransport {
    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), MailError>;
    async fn fetch_unread(&self) -> Result<Vec<AgentMessage>, MailError>;
    async fn search(&self, query: &str) -> Result<Vec<AgentMessage>, MailError>;
    async fn mark_read(&self, uid: u32) -> Result<(), MailError>;
}

impl AgentMailTransport for PanoramaMail {
    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), MailError> {
        smtp::send(&self.config, to, subject, body).await
    }

    async fn fetch_unread(&self) -> Result<Vec<AgentMessage>, MailError> {
        imap::fetch_unread(&self.config).await
    }

    async fn search(&self, query: &str) -> Result<Vec<AgentMessage>, MailError> {
        imap::search(&self.config, query).await
    }

    async fn mark_read(&self, uid: u32) -> Result<(), MailError> {
        imap::mark_read(&self.config, uid).await
    }
}
