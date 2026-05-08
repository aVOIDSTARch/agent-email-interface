// This file define the database using sqlx + SQLite and implements Tantivy for full-text search.
// It provides functions to store and retrieve emails, as well as to perform full-text search on
// email content. The module implements the necessary database schema, connection management, and
// search indexing as defined in database.rs so that it is compatible with the rest of the agent's
// storage interface. It also includes functions to initialize the database and search index, as
// well as to perform search queries efficiently.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sqlx::Row as _;
use sqlx::SqlitePool;
use tantivy::{
    collector::TopDocs,
    query::QueryParser,
    schema::{Field, Schema, Value, FAST, STORED, TEXT},
    Index, IndexReader, IndexWriter, TantivyDocument, Term,
};

use crate::mail::types::AgentMessage;

use super::database::{EmailStore, StoredEmail, StoreError};

// ── Internal tantivy state (held behind Arc for spawn_blocking) ───────────────

struct TantivyInner {
    reader: IndexReader,
    writer: Mutex<IndexWriter>,
    f_uid: Field,
    f_subject: Field,
    f_from: Field,
    f_to: Field,
    f_body: Field,
    f_date: Field,
}

// ── Public store type ─────────────────────────────────────────────────────────

pub struct SqliteTantivyStore {
    pool: SqlitePool,
    tantivy: Arc<TantivyInner>,
}

/// Acquire a Tantivy IndexWriter, automatically removing a stale lock file on first failure.
///
/// Tantivy leaves `.tantivy-writer.lock` behind when a process crashes. On `LockBusy` we
/// delete the stale file (safe because it is empty — the actual data is in segments) and
/// retry once. If another live instance holds the lock the retry will also fail.
fn acquire_writer(index: &Index, index_path: &std::path::Path) -> Result<IndexWriter, StoreError> {
    match index.writer(50_000_000) {
        Ok(w) => Ok(w),
        Err(e) if e.to_string().contains("LockBusy") => {
            let lock = index_path.join(".tantivy-writer.lock");
            if lock.exists() {
                std::fs::remove_file(&lock)?;
            }
            index.writer(50_000_000).map_err(|e2| {
                StoreError::Search(format!(
                    "Tantivy index is locked at '{}'. \
                     Stop all other panorama-mail processes and try again. ({e2})",
                    index_path.display()
                ))
            })
        }
        Err(e) => Err(StoreError::Search(e.to_string())),
    }
}

impl SqliteTantivyStore {
    /// Open (or create) a store at the given base directory.
    /// Creates `<base>/panorama.db` (SQLite) and `<base>/tantivy/` (search index).
    pub async fn open(base_path: &str) -> Result<Self, StoreError> {
        let base = PathBuf::from(base_path);
        std::fs::create_dir_all(&base)?;

        // ── SQLite ────────────────────────────────────────────────────────────
        let db_path = base.join("panorama.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&db_url)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS emails (
                uid        INTEGER PRIMARY KEY NOT NULL,
                subject    TEXT,
                from_addr  TEXT,
                to_addrs   TEXT    NOT NULL DEFAULT '[]',
                body       TEXT,
                html_body  TEXT,
                date       TEXT,
                is_read    INTEGER NOT NULL DEFAULT 0,
                mailbox    TEXT,
                stored_at  INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .map_err(|e| StoreError::Database(e.to_string()))?;

        // ── Tantivy ───────────────────────────────────────────────────────────
        let mut sb = Schema::builder();
        let f_uid = sb.add_u64_field("uid", FAST | STORED);
        let f_subject = sb.add_text_field("subject", TEXT | STORED);
        let f_from = sb.add_text_field("from_addr", TEXT | STORED);
        let f_to = sb.add_text_field("to_addrs", TEXT | STORED);
        let f_body = sb.add_text_field("body", TEXT);
        let f_date = sb.add_text_field("date_str", TEXT | STORED);
        let schema = sb.build();

        let index_path = base.join("tantivy");
        std::fs::create_dir_all(&index_path)?;
        let index = if index_path.join("meta.json").exists() {
            Index::open_in_dir(&index_path).map_err(|e| StoreError::Search(e.to_string()))?
        } else {
            Index::create_in_dir(&index_path, schema)
                .map_err(|e| StoreError::Search(e.to_string()))?
        };

        let reader = index
            .reader()
            .map_err(|e| StoreError::Search(e.to_string()))?;
        let writer = acquire_writer(&index, &index_path)?;

        Ok(Self {
            pool,
            tantivy: Arc::new(TantivyInner {
                reader,
                writer: Mutex::new(writer),
                f_uid,
                f_subject,
                f_from,
                f_to,
                f_body,
                f_date,
            }),
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn row_to_stored(row: sqlx::sqlite::SqliteRow) -> Result<StoredEmail, StoreError> {
    let uid: i64 = row
        .try_get("uid")
        .map_err(|e| StoreError::Database(e.to_string()))?;
    let to_json: String = row.try_get("to_addrs").unwrap_or_default();
    let to: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
    let is_read: i64 = row.try_get("is_read").unwrap_or(0);

    Ok(StoredEmail {
        message: AgentMessage {
            uid: uid as u32,
            subject: row.try_get("subject").unwrap_or(None),
            from: row.try_get("from_addr").unwrap_or(None),
            to,
            date: row.try_get("date").unwrap_or(None),
            body: row.try_get("body").unwrap_or(None),
            html_body: row.try_get("html_body").unwrap_or(None),
            attachments: vec![],
        },
        is_read: is_read != 0,
        mailbox: row.try_get("mailbox").unwrap_or(None),
        stored_at: row.try_get("stored_at").unwrap_or(0),
    })
}

// ── EmailStore implementation ─────────────────────────────────────────────────

#[async_trait]
impl EmailStore for SqliteTantivyStore {
    async fn init(&self) -> Result<(), StoreError> {
        // Initialization happens in open(); this is a no-op.
        Ok(())
    }

    async fn upsert(&self, msg: &AgentMessage) -> Result<(), StoreError> {
        let to_json = serde_json::to_string(&msg.to).unwrap_or_else(|_| "[]".to_string());
        let stored_at = now_secs();

        // SQLite — preserve is_read on conflict.
        sqlx::query(
            "INSERT INTO emails
                (uid, subject, from_addr, to_addrs, body, html_body, date, is_read, stored_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)
             ON CONFLICT(uid) DO UPDATE SET
                subject   = excluded.subject,
                from_addr = excluded.from_addr,
                to_addrs  = excluded.to_addrs,
                body      = excluded.body,
                html_body = excluded.html_body,
                date      = excluded.date,
                stored_at = excluded.stored_at",
        )
        .bind(msg.uid as i64)
        .bind(&msg.subject)
        .bind(&msg.from)
        .bind(&to_json)
        .bind(&msg.body)
        .bind(&msg.html_body)
        .bind(&msg.date)
        .bind(stored_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Database(e.to_string()))?;

        // Tantivy — delete existing doc then add updated one.
        let t = Arc::clone(&self.tantivy);
        let uid = msg.uid;
        let subject = msg.subject.clone().unwrap_or_default();
        let from = msg.from.clone().unwrap_or_default();
        let to = msg.to.join(" ");
        let body = msg.body.clone().unwrap_or_default();
        let date = msg.date.clone().unwrap_or_default();

        tokio::task::spawn_blocking(move || -> Result<(), StoreError> {
            let mut w = t
                .writer
                .lock()
                .map_err(|e| StoreError::Search(format!("writer lock poisoned: {e}")))?;
            w.delete_term(Term::from_field_u64(t.f_uid, uid as u64));
            let mut doc = TantivyDocument::default();
            doc.add_u64(t.f_uid, uid as u64);
            doc.add_text(t.f_subject, &subject);
            doc.add_text(t.f_from, &from);
            doc.add_text(t.f_to, &to);
            doc.add_text(t.f_body, &body);
            doc.add_text(t.f_date, &date);
            w.add_document(doc)
                .map_err(|e| StoreError::Search(e.to_string()))?;
            w.commit()
                .map_err(|e| StoreError::Search(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Search(format!("spawn_blocking failed: {e}")))??;

        self.tantivy
            .reader
            .reload()
            .map_err(|e| StoreError::Search(e.to_string()))?;

        Ok(())
    }

    async fn get_by_uid(&self, uid: u32) -> Result<Option<StoredEmail>, StoreError> {
        let row = sqlx::query(
            "SELECT uid, subject, from_addr, to_addrs, body, html_body, date, is_read, mailbox, stored_at
             FROM emails WHERE uid = ?",
        )
        .bind(uid as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StoreError::Database(e.to_string()))?;

        row.map(row_to_stored).transpose()
    }

    async fn list_all(&self) -> Result<Vec<StoredEmail>, StoreError> {
        let rows = sqlx::query(
            "SELECT uid, subject, from_addr, to_addrs, body, html_body, date, is_read, mailbox, stored_at
             FROM emails ORDER BY stored_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Database(e.to_string()))?;

        rows.into_iter().map(row_to_stored).collect()
    }

    async fn list_unread(&self) -> Result<Vec<StoredEmail>, StoreError> {
        let rows = sqlx::query(
            "SELECT uid, subject, from_addr, to_addrs, body, html_body, date, is_read, mailbox, stored_at
             FROM emails WHERE is_read = 0 ORDER BY stored_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Database(e.to_string()))?;

        rows.into_iter().map(row_to_stored).collect()
    }

    async fn search(&self, query: &str) -> Result<Vec<StoredEmail>, StoreError> {
        let t = Arc::clone(&self.tantivy);
        let q = query.to_string();

        let uids: Vec<u32> = tokio::task::spawn_blocking(move || -> Result<Vec<u32>, StoreError> {
            let searcher = t.reader.searcher();
            let mut parser = QueryParser::for_index(
                searcher.index(),
                vec![t.f_subject, t.f_from, t.f_to, t.f_body, t.f_date],
            );
            parser.set_conjunction_by_default();
            let query = parser
                .parse_query(&q)
                .map_err(|e| StoreError::Search(e.to_string()))?;
            let top = searcher
                .search(&query, &TopDocs::with_limit(100))
                .map_err(|e| StoreError::Search(e.to_string()))?;

            let mut uids = Vec::new();
            for (_score, addr) in top {
                let doc: TantivyDocument = searcher
                    .doc(addr)
                    .map_err(|e| StoreError::Search(e.to_string()))?;
                if let Some(v) = doc.get_first(t.f_uid)
                    && let Some(uid) = v.as_u64()
                {
                    uids.push(uid as u32);
                }
            }
            Ok(uids)
        })
        .await
        .map_err(|e| StoreError::Search(format!("spawn_blocking failed: {e}")))??;

        if uids.is_empty() {
            return Ok(vec![]);
        }

        // Fetch full rows from SQLite using parameterised IN list built from trusted u32 values.
        let placeholders = uids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT uid, subject, from_addr, to_addrs, body, html_body, date, is_read, mailbox, stored_at
             FROM emails WHERE uid IN ({placeholders})"
        );
        let mut q = sqlx::query(&sql);
        for uid in &uids {
            q = q.bind(*uid as i64);
        }
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        rows.into_iter().map(row_to_stored).collect()
    }

    async fn mark_read(&self, uid: u32) -> Result<(), StoreError> {
        let result = sqlx::query("UPDATE emails SET is_read = 1 WHERE uid = ?")
            .bind(uid as i64)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    // ── Optional overrides ────────────────────────────────────────────────────

    async fn delete(&self, uid: u32) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM emails WHERE uid = ?")
            .bind(uid as i64)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let t = Arc::clone(&self.tantivy);
        tokio::task::spawn_blocking(move || -> Result<(), StoreError> {
            let mut w = t
                .writer
                .lock()
                .map_err(|e| StoreError::Search(format!("lock poisoned: {e}")))?;
            w.delete_term(Term::from_field_u64(t.f_uid, uid as u64));
            w.commit()
                .map_err(|e| StoreError::Search(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Search(format!("spawn_blocking: {e}")))??;

        Ok(())
    }

    async fn list_mailboxes(&self) -> Result<Vec<String>, StoreError> {
        let rows = sqlx::query(
            "SELECT DISTINCT mailbox FROM emails WHERE mailbox IS NOT NULL ORDER BY mailbox",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| r.get::<String, _>("mailbox"))
            .collect())
    }

    async fn count(&self) -> Result<u64, StoreError> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM emails")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;

        Ok(row.get::<i64, _>("cnt") as u64)
    }

    async fn list_by_mailbox(&self, mailbox: &str) -> Result<Vec<StoredEmail>, StoreError> {
        let rows = sqlx::query(
            "SELECT uid, subject, from_addr, to_addrs, body, html_body, date, is_read, mailbox, stored_at
             FROM emails WHERE mailbox = ? ORDER BY stored_at DESC",
        )
        .bind(mailbox)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Database(e.to_string()))?;

        rows.into_iter().map(row_to_stored).collect()
    }

    async fn list_by_date_range(&self, from: i64, to: i64) -> Result<Vec<StoredEmail>, StoreError> {
        let rows = sqlx::query(
            "SELECT uid, subject, from_addr, to_addrs, body, html_body, date, is_read, mailbox, stored_at
             FROM emails WHERE stored_at BETWEEN ? AND ? ORDER BY stored_at DESC",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Database(e.to_string()))?;

        rows.into_iter().map(row_to_stored).collect()
    }
}
