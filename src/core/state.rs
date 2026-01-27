#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool, FromRow, Sqlite};
use std::path::PathBuf;

/// A message in the selection
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct SelectionEntry {
    pub account: String,
    pub folder: String,
    pub uid: i64,
    pub message_id: Option<String>,
    pub subject: Option<String>,
}

/// A query history result entry
#[derive(Debug, Clone, FromRow)]
pub struct QueryResultEntry {
    pub account: String,
    pub folder: String,
    pub uid: i64,
    pub message_id: Option<String>,
    pub subject: Option<String>,
}

/// Action types for drafts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ActionType {
    Flag,
    Move,
    Copy,
    Delete,
    Archive,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionType::Flag => "flag",
            ActionType::Move => "move",
            ActionType::Copy => "copy",
            ActionType::Delete => "delete",
            ActionType::Archive => "archive",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "flag" => Some(ActionType::Flag),
            "move" => Some(ActionType::Move),
            "copy" => Some(ActionType::Copy),
            "delete" => Some(ActionType::Delete),
            "archive" => Some(ActionType::Archive),
            _ => None,
        }
    }
}

/// Flag parameters for draft operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlagParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starred: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unlabels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub move_to: Option<String>,
}

impl FlagParams {
    /// Merge another FlagParams into this one (OR logic for booleans, concatenate arrays)
    pub fn merge(&mut self, other: &FlagParams) {
        // Boolean: true wins (OR logic)
        if other.read == Some(true) {
            self.read = Some(true);
        } else if self.read.is_none() {
            self.read = other.read;
        }

        if other.starred == Some(true) {
            self.starred = Some(true);
        } else if self.starred.is_none() {
            self.starred = other.starred;
        }

        // Arrays: concatenate
        for label in &other.labels {
            if !self.labels.contains(label) {
                self.labels.push(label.clone());
            }
        }
        for unlabel in &other.unlabels {
            if !self.unlabels.contains(unlabel) {
                self.unlabels.push(unlabel.clone());
            }
        }

        // move_to: execution-time value wins
        if other.move_to.is_some() {
            self.move_to = other.move_to.clone();
        }
    }

    /// Check if any flag action is specified
    pub fn has_any_action(&self) -> bool {
        self.read.is_some()
            || self.starred.is_some()
            || !self.labels.is_empty()
            || !self.unlabels.is_empty()
            || self.move_to.is_some()
    }
}

/// A draft (staged) operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub account: String,
    pub action_type: ActionType,
    pub folder: String,
    pub uids: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag_params: Option<FlagParams>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_folder: Option<String>,
    #[serde(default)]
    pub permanent: bool,
}

pub struct StateManager {
    pool: SqlitePool,
}

impl StateManager {
    fn db_path() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Failed to get local data directory")?
            .join("protoncli");

        std::fs::create_dir_all(&data_dir)
            .context("Failed to create data directory")?;

        // Set restrictive permissions on the data directory (0700 on Unix)
        #[cfg(unix)]
        {
            let permissions = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(&data_dir, permissions)
                .context("Failed to set directory permissions")?;
        }

        Ok(data_dir.join("state.db"))
    }

    pub async fn new() -> Result<Self> {
        let db_path = Self::db_path()?;
        let db_url = format!("sqlite://{}", db_path.display());

        // Create database if it doesn't exist
        if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
            Sqlite::create_database(&db_url)
                .await
                .context("Failed to create database")?;
        }

        // Connect to database
        let pool = SqlitePool::connect(&db_url)
            .await
            .context("Failed to connect to database")?;

        // Check if we need to migrate from old schema
        let needs_migration = Self::check_needs_schema_migration(&pool).await?;

        if needs_migration {
            // Drop old tables and let the new schema be created
            sqlx::query("DROP TABLE IF EXISTS messages")
                .execute(&pool)
                .await
                .context("Failed to drop old messages table")?;
        }

        // Run migrations
        let migration_001 = include_str!("../../migrations/001_initial_schema.sql");
        sqlx::query(migration_001)
            .execute(&pool)
            .await
            .context("Failed to run migration 001")?;

        let migration_002 = include_str!("../../migrations/002_message_id_primary.sql");
        sqlx::query(migration_002)
            .execute(&pool)
            .await
            .context("Failed to run migration 002")?;

        let migration_003 = include_str!("../../migrations/003_batch_operations.sql");
        sqlx::query(migration_003)
            .execute(&pool)
            .await
            .context("Failed to run migration 003")?;

        Ok(Self { pool })
    }

    /// Check if the database has the old schema (folder/uid unique) that needs migration
    async fn check_needs_schema_migration(pool: &SqlitePool) -> Result<bool> {
        // Check if messages table exists
        let table_exists: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='messages'"
        )
        .fetch_optional(pool)
        .await
        .context("Failed to check for messages table")?;

        if table_exists.is_none() {
            return Ok(false); // Fresh install, no migration needed
        }

        // Check if schema_migrations table exists (new schema indicator)
        let migrations_exists: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'"
        )
        .fetch_optional(pool)
        .await
        .context("Failed to check for schema_migrations table")?;

        // If messages exists but schema_migrations doesn't, we have old schema
        Ok(migrations_exists.is_none())
    }

    /// Upsert a message using message_id as the stable identifier.
    /// folder/uid represent the current location and are updated on conflict.
    pub async fn upsert_message(
        &self,
        account: &str,
        folder: &str,
        uid: u32,
        message_id: Option<&str>,
        subject: Option<&str>,
        from_address: Option<&str>,
        date_sent: Option<DateTime<Utc>>,
    ) -> Result<()> {
        // Skip if no message_id - we can't track without a stable identifier
        let Some(msg_id) = message_id else {
            return Ok(());
        };

        let date_sent_str = date_sent.map(|d| d.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO messages (account, message_id, folder, uid, subject, from_address, date_sent)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(account, message_id) DO UPDATE SET
                folder = ?3,
                uid = ?4,
                subject = COALESCE(?5, subject),
                from_address = COALESCE(?6, from_address),
                date_sent = COALESCE(?7, date_sent)
            "#,
        )
        .bind(account)
        .bind(msg_id)
        .bind(folder)
        .bind(uid)
        .bind(subject)
        .bind(from_address)
        .bind(date_sent_str)
        .execute(&self.pool)
        .await
        .context("Failed to upsert message")?;

        Ok(())
    }

    /// Mark a message as read by the agent using message_id
    pub async fn mark_agent_read(&self, account: &str, message_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE messages
            SET agent_read = TRUE
            WHERE account = ?1 AND message_id = ?2
            "#,
        )
        .bind(account)
        .bind(message_id)
        .execute(&self.pool)
        .await
        .context("Failed to mark message as agent-read")?;

        Ok(())
    }

    /// Check if a message has been read by the agent using message_id
    pub async fn is_agent_read(&self, account: &str, message_id: &str) -> Result<bool> {
        let result: Option<(bool,)> = sqlx::query_as(
            r#"
            SELECT agent_read
            FROM messages
            WHERE account = ?1 AND message_id = ?2
            "#,
        )
        .bind(account)
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to check agent-read status")?;

        Ok(result.map(|(read,)| read).unwrap_or(false))
    }

    /// Update the location (folder/uid) of a message after it's been moved
    pub async fn update_message_location(
        &self,
        account: &str,
        message_id: &str,
        new_folder: &str,
        new_uid: Option<u32>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE messages
            SET folder = ?3, uid = ?4
            WHERE account = ?1 AND message_id = ?2
            "#,
        )
        .bind(account)
        .bind(message_id)
        .bind(new_folder)
        .bind(new_uid)
        .execute(&self.pool)
        .await
        .context("Failed to update message location")?;

        Ok(())
    }

    /// Clear the location when a message is deleted (but keep the record for agent_read history)
    pub async fn clear_message_location(&self, account: &str, message_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE messages
            SET folder = NULL, uid = NULL
            WHERE account = ?1 AND message_id = ?2
            "#,
        )
        .bind(account)
        .bind(message_id)
        .execute(&self.pool)
        .await
        .context("Failed to clear message location")?;

        Ok(())
    }

    // ============================================================
    // Selection methods
    // ============================================================

    /// Add messages to the selection
    pub async fn add_to_selection(
        &self,
        account: &str,
        folder: &str,
        entries: &[(u32, Option<&str>, Option<&str>)], // (uid, message_id, subject)
    ) -> Result<usize> {
        let mut count = 0;
        for (uid, message_id, subject) in entries {
            let result = sqlx::query(
                r#"
                INSERT INTO selections (account, folder, uid, message_id, subject)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(account, folder, uid) DO UPDATE SET
                    message_id = COALESCE(?4, message_id),
                    subject = COALESCE(?5, subject)
                "#,
            )
            .bind(account)
            .bind(folder)
            .bind(*uid as i64)
            .bind(*message_id)
            .bind(*subject)
            .execute(&self.pool)
            .await
            .context("Failed to add to selection")?;

            if result.rows_affected() > 0 {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Remove specific UIDs from the selection
    pub async fn remove_from_selection(
        &self,
        account: &str,
        folder: &str,
        uids: &[u32],
    ) -> Result<usize> {
        if uids.is_empty() {
            return Ok(0);
        }

        let placeholders: Vec<String> = (0..uids.len()).map(|i| format!("?{}", i + 3)).collect();
        let query = format!(
            "DELETE FROM selections WHERE account = ?1 AND folder = ?2 AND uid IN ({})",
            placeholders.join(", ")
        );

        let mut q = sqlx::query(&query).bind(account).bind(folder);
        for uid in uids {
            q = q.bind(*uid as i64);
        }

        let result = q
            .execute(&self.pool)
            .await
            .context("Failed to remove from selection")?;

        Ok(result.rows_affected() as usize)
    }

    /// Get all messages in the selection for an account
    pub async fn get_selection(&self, account: &str) -> Result<Vec<SelectionEntry>> {
        let entries: Vec<SelectionEntry> = sqlx::query_as(
            r#"
            SELECT account, folder, uid, message_id, subject
            FROM selections
            WHERE account = ?1
            ORDER BY added_at ASC
            "#,
        )
        .bind(account)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get selection")?;

        Ok(entries)
    }

    /// Get selection for a specific folder
    pub async fn get_selection_for_folder(
        &self,
        account: &str,
        folder: &str,
    ) -> Result<Vec<SelectionEntry>> {
        let entries: Vec<SelectionEntry> = sqlx::query_as(
            r#"
            SELECT account, folder, uid, message_id, subject
            FROM selections
            WHERE account = ?1 AND folder = ?2
            ORDER BY added_at ASC
            "#,
        )
        .bind(account)
        .bind(folder)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get selection for folder")?;

        Ok(entries)
    }

    /// Clear all selections for an account
    pub async fn clear_selection(&self, account: &str) -> Result<usize> {
        let result = sqlx::query("DELETE FROM selections WHERE account = ?1")
            .bind(account)
            .execute(&self.pool)
            .await
            .context("Failed to clear selection")?;

        Ok(result.rows_affected() as usize)
    }

    /// Clear selection for a specific folder
    pub async fn clear_selection_for_folder(&self, account: &str, folder: &str) -> Result<usize> {
        let result =
            sqlx::query("DELETE FROM selections WHERE account = ?1 AND folder = ?2")
                .bind(account)
                .bind(folder)
                .execute(&self.pool)
                .await
                .context("Failed to clear selection for folder")?;

        Ok(result.rows_affected() as usize)
    }

    /// Count messages in the selection
    pub async fn selection_count(&self, account: &str) -> Result<usize> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM selections WHERE account = ?1")
            .bind(account)
            .fetch_one(&self.pool)
            .await
            .context("Failed to count selection")?;

        Ok(count.0 as usize)
    }

    // ============================================================
    // Query history methods
    // ============================================================

    /// Save query results (replaces previous results for account/folder)
    pub async fn save_query_results(
        &self,
        account: &str,
        folder: &str,
        query_string: &str,
        results: &[(u32, Option<&str>, Option<&str>)], // (uid, message_id, subject)
    ) -> Result<()> {
        // Update or insert query history
        sqlx::query(
            r#"
            INSERT INTO query_history (account, folder, query_string, executed_at)
            VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
            ON CONFLICT(account, folder) DO UPDATE SET
                query_string = ?3,
                executed_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(account)
        .bind(folder)
        .bind(query_string)
        .execute(&self.pool)
        .await
        .context("Failed to save query history")?;

        // Clear previous results for this account/folder
        sqlx::query("DELETE FROM query_history_results WHERE account = ?1 AND folder = ?2")
            .bind(account)
            .bind(folder)
            .execute(&self.pool)
            .await
            .context("Failed to clear old query results")?;

        // Insert new results
        for (uid, message_id, subject) in results {
            sqlx::query(
                r#"
                INSERT INTO query_history_results (account, folder, uid, message_id, subject)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(account)
            .bind(folder)
            .bind(*uid as i64)
            .bind(*message_id)
            .bind(*subject)
            .execute(&self.pool)
            .await
            .context("Failed to save query result")?;
        }

        Ok(())
    }

    /// Get the last query results for an account/folder
    pub async fn get_last_query_results(
        &self,
        account: &str,
        folder: &str,
    ) -> Result<Vec<QueryResultEntry>> {
        let entries: Vec<QueryResultEntry> = sqlx::query_as(
            r#"
            SELECT account, folder, uid, message_id, subject
            FROM query_history_results
            WHERE account = ?1 AND folder = ?2
            ORDER BY id ASC
            "#,
        )
        .bind(account)
        .bind(folder)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get last query results")?;

        Ok(entries)
    }

    /// Get the last query string for an account/folder
    pub async fn get_last_query_string(
        &self,
        account: &str,
        folder: &str,
    ) -> Result<Option<String>> {
        let result: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT query_string
            FROM query_history
            WHERE account = ?1 AND folder = ?2
            "#,
        )
        .bind(account)
        .bind(folder)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get last query string")?;

        Ok(result.map(|(s,)| s))
    }

    // ============================================================
    // Draft methods
    // ============================================================

    /// Save a draft operation (replaces any existing draft for the account)
    pub async fn save_draft(&self, draft: &Draft) -> Result<()> {
        let uids_json = serde_json::to_string(&draft.uids)?;
        let flag_params_json = draft
            .flag_params
            .as_ref()
            .map(|p| serde_json::to_string(p))
            .transpose()?;

        sqlx::query(
            r#"
            INSERT INTO drafts (account, action_type, folder, uids_json, flag_params_json, dest_folder, permanent)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(account) DO UPDATE SET
                action_type = ?2,
                folder = ?3,
                uids_json = ?4,
                flag_params_json = ?5,
                dest_folder = ?6,
                permanent = ?7,
                created_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&draft.account)
        .bind(draft.action_type.as_str())
        .bind(&draft.folder)
        .bind(&uids_json)
        .bind(&flag_params_json)
        .bind(&draft.dest_folder)
        .bind(draft.permanent)
        .execute(&self.pool)
        .await
        .context("Failed to save draft")?;

        Ok(())
    }

    /// Get the current draft for an account
    pub async fn get_draft(&self, account: &str) -> Result<Option<Draft>> {
        let row: Option<(String, String, String, String, Option<String>, Option<String>, bool)> =
            sqlx::query_as(
                r#"
            SELECT account, action_type, folder, uids_json, flag_params_json, dest_folder, permanent
            FROM drafts
            WHERE account = ?1
            "#,
            )
            .bind(account)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to get draft")?;

        match row {
            Some((account, action_type, folder, uids_json, flag_params_json, dest_folder, permanent)) => {
                let action_type = ActionType::from_str(&action_type)
                    .ok_or_else(|| anyhow::anyhow!("Invalid action type in draft: {}", action_type))?;
                let uids: Vec<u32> = serde_json::from_str(&uids_json)?;
                let flag_params: Option<FlagParams> = flag_params_json
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?;

                Ok(Some(Draft {
                    account,
                    action_type,
                    folder,
                    uids,
                    flag_params,
                    dest_folder,
                    permanent,
                }))
            }
            None => Ok(None),
        }
    }

    /// Clear the draft for an account
    pub async fn clear_draft(&self, account: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM drafts WHERE account = ?1")
            .bind(account)
            .execute(&self.pool)
            .await
            .context("Failed to clear draft")?;

        Ok(result.rows_affected() > 0)
    }

    /// Check if a draft exists for an account
    pub async fn has_draft(&self, account: &str) -> Result<bool> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM drafts WHERE account = ?1")
            .bind(account)
            .fetch_one(&self.pool)
            .await
            .context("Failed to check for draft")?;

        Ok(count.0 > 0)
    }
}
