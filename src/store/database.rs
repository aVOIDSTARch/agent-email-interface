// This file defines the database access layer for the email agent. It provides functions to store
// and retrieve emails from any configured database backend. The actual database implementation is
// abstracted away, allowing for flexibility in choosing the storage solution (e.g., SQLite,
// PostgreSQL, etc.). The module includes functions to save emails, fetch emails by various
// criteria, search, and manage email metadata.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::mail::types::AgentMessage;

/// An email persisted in the local store, plus store-level metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEmail {
    #[serde(flatten)]
    pub message: AgentMessage,
    pub is_read: bool,
    pub mailbox: Option<String>,
    /// Unix timestamp (seconds) when this email was first written to the store.
    pub stored_at: i64,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(String),
    #[error("search index error: {0}")]
    Search(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found")]
    NotFound,
    #[error("operation not supported by this store: {0}")]
    NotSupported(&'static str),
}

/// Common interface for all email storage backends.
///
/// **Required** methods must be implemented by every backend.
/// **Optional** methods return [`StoreError::NotSupported`] by default and can be overridden
/// when the backend supports them, keeping the trait open for future implementations.
#[async_trait]
pub trait EmailStore: Send + Sync {
    // ── Required ─────────────────────────────────────────────────────────────

    /// Initialize the store (create tables, indexes, directories).
    /// Must be called once before any other operation.
    async fn init(&self) -> Result<(), StoreError>;

    /// Persist an email, inserting a new row or updating if the UID already exists.
    /// The `is_read` flag is preserved on conflict — upsert never resets a read email to unread.
    async fn upsert(&self, msg: &AgentMessage) -> Result<(), StoreError>;

    /// Retrieve a single stored email by IMAP UID.
    async fn get_by_uid(&self, uid: u32) -> Result<Option<StoredEmail>, StoreError>;

    /// Return all stored emails, newest first.
    async fn list_all(&self) -> Result<Vec<StoredEmail>, StoreError>;

    /// Return stored emails that have not been marked read, newest first.
    async fn list_unread(&self) -> Result<Vec<StoredEmail>, StoreError>;

    /// Full-text search across stored emails (subject, from, to, body, date).
    async fn search(&self, query: &str) -> Result<Vec<StoredEmail>, StoreError>;

    /// Mark a stored email as read. Returns [`StoreError::NotFound`] if the UID is unknown.
    async fn mark_read(&self, uid: u32) -> Result<(), StoreError>;

    // ── Optional ─────────────────────────────────────────────────────────────

    /// Delete a stored email by IMAP UID.
    async fn delete(&self, _uid: u32) -> Result<(), StoreError> {
        Err(StoreError::NotSupported("delete"))
    }

    /// Return distinct mailbox names present in the store.
    async fn list_mailboxes(&self) -> Result<Vec<String>, StoreError> {
        Err(StoreError::NotSupported("list_mailboxes"))
    }

    /// Count all stored emails.
    async fn count(&self) -> Result<u64, StoreError> {
        Err(StoreError::NotSupported("count"))
    }

    /// Return all stored emails for a specific mailbox, newest first.
    async fn list_by_mailbox(&self, _mailbox: &str) -> Result<Vec<StoredEmail>, StoreError> {
        Err(StoreError::NotSupported("list_by_mailbox"))
    }

    /// Return emails whose `stored_at` Unix timestamp falls within `[from, to]`.
    async fn list_by_date_range(
        &self,
        _from: i64,
        _to: i64,
    ) -> Result<Vec<StoredEmail>, StoreError> {
        Err(StoreError::NotSupported("list_by_date_range"))
    }
}
