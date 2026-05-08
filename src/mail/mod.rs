pub mod error;
pub mod imap;
pub mod session;
pub mod smtp;
pub mod types;

use std::sync::Arc;

pub use error::MailError;
pub use types::{AgentMessage, MailConfig};

use crate::store::database::EmailStore;

pub struct PanoramaMail {
    pub config: MailConfig,
    store: Option<Arc<dyn EmailStore>>,
}

impl PanoramaMail {
    pub fn new(config: MailConfig) -> Self {
        Self {
            config,
            store: None,
        }
    }

    /// Attach a storage backend for write-through caching.
    pub fn with_store(mut self, store: Arc<dyn EmailStore>) -> Self {
        self.store = Some(store);
        self
    }

    pub async fn get_by_uid(&self, uid: u32) -> Result<Option<AgentMessage>, MailError> {
        let msg = imap::get_by_uid(&self.config, uid).await?;
        if let (Some(store), Some(m)) = (&self.store, &msg) {
            store.upsert(m).await.ok();
        }
        Ok(msg)
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
        let messages = imap::fetch_unread(&self.config).await?;
        if let Some(store) = &self.store {
            for m in &messages {
                store.upsert(m).await.ok();
            }
        }
        Ok(messages)
    }

    async fn search(&self, query: &str) -> Result<Vec<AgentMessage>, MailError> {
        let messages = imap::search(&self.config, query).await?;
        if let Some(store) = &self.store {
            for m in &messages {
                store.upsert(m).await.ok();
            }
        }
        Ok(messages)
    }

    async fn mark_read(&self, uid: u32) -> Result<(), MailError> {
        imap::mark_read(&self.config, uid).await?;
        if let Some(store) = &self.store {
            store.mark_read(uid).await.ok();
        }
        Ok(())
    }
}
