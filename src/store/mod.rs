// This module provides access to various types of stores for emails.
// Currently backed by SQLite + Tantivy; designed for multi-store fanout in the future.

pub mod database;
pub mod sqlite_and_tantivy;

pub use database::{EmailStore, StoredEmail, StoreError};
pub use sqlite_and_tantivy::SqliteTantivyStore;
