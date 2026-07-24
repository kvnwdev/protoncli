#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool, FromRow, Sqlite};
use std::path::PathBuf;

/// Type alias for selection/query entry tuple: (uid, folder, message_id, subject, shadow_uid)
pub type SelectionEntryTuple<'a> = (u32, &'a str, Option<&'a str>, Option<&'a str>, Option<i64>);

/// A message in the selection
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct SelectionEntry {
    pub account: String,
    pub folder: String,
    pub uid: i64,
    pub message_id: Option<String>,
    pub subject: Option<String>,
    pub shadow_uid: Option<i64>,
}

/// A query history result entry
#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)] // Fields used via FromRow derive for database queries
pub struct QueryResultEntry {
    pub account: String,
    pub folder: String,
    pub uid: i64,
    pub message_id: Option<String>,
    pub subject: Option<String>,
    pub shadow_uid: Option<i64>,
}

/// A message record from the messages table (includes shadow_uid which is the row id)
#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)] // Fields populated via FromRow derive for database queries
pub struct MessageRecord {
    pub id: i64, // This IS the shadow_uid
    pub account: String,
    pub message_id: String,
    pub folder: String,
    pub uid: i64, // IMAP UID (current location)
    pub subject: Option<String>,
    pub from_address: Option<String>,
    pub date_sent: Option<String>,
    pub agent_read: bool,
}

#[derive(Debug, Clone, FromRow)]
struct MessageLocation {
    folder: String,
    uid: i64,
}

/// Resolved message location for operations
#[derive(Debug, Clone)]
#[allow(dead_code)] // shadow_uid included for debugging/logging purposes
pub struct ResolvedMessage {
    pub shadow_uid: i64,
    pub folder: String,
    pub imap_uid: u32,
    pub message_id: Option<String>,
}

/// Validate shadow UIDs from a vector of i64
pub fn validate_shadow_uids(ids: &[i64]) -> Result<()> {
    for id in ids {
        if *id <= 0 {
            return Err(anyhow::anyhow!(
                "Invalid message ID '{}'. Message IDs must be positive integers.",
                id
            ));
        }
    }
    Ok(())
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

        std::fs::create_dir_all(&data_dir).context("Failed to create data directory")?;

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

        Self::ensure_message_schema(&pool).await?;
        Self::ensure_shadow_uid_schema(&pool).await?;
        Self::ensure_location_schema(&pool).await?;

        Ok(Self { pool })
    }

    /// Repair and complete the shadow UID schema even if an earlier build
    /// recorded migration 004 before all of its ALTER TABLE statements ran.
    async fn ensure_shadow_uid_schema(pool: &SqlitePool) -> Result<()> {
        Self::ensure_column(
            pool,
            "selections",
            "shadow_uid",
            "INTEGER REFERENCES messages(id)",
        )
        .await?;
        Self::ensure_column(
            pool,
            "query_history_results",
            "shadow_uid",
            "INTEGER REFERENCES messages(id)",
        )
        .await?;

        for (index, table) in [
            ("idx_messages_id", "messages(id)"),
            ("idx_selections_shadow_uid", "selections(shadow_uid)"),
            (
                "idx_query_history_results_shadow_uid",
                "query_history_results(shadow_uid)",
            ),
        ] {
            sqlx::query(&format!("CREATE INDEX IF NOT EXISTS {index} ON {table}"))
                .execute(pool)
                .await
                .with_context(|| format!("Failed to create {index}"))?;
        }

        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version) VALUES (4)")
            .execute(pool)
            .await
            .context("Failed to mark migration 004 as applied")?;

        Ok(())
    }

    /// Keep old local state intact. Older builds can have a messages table that
    /// lacks modern metadata columns, but it must never be dropped on startup.
    async fn ensure_message_schema(pool: &SqlitePool) -> Result<()> {
        for (column, definition) in [
            ("message_id", "TEXT"),
            ("folder", "TEXT"),
            ("uid", "INTEGER"),
            ("subject", "TEXT"),
            ("from_address", "TEXT"),
            ("date_sent", "TIMESTAMP"),
            ("agent_read", "BOOLEAN DEFAULT FALSE"),
        ] {
            Self::ensure_column(pool, "messages", column, definition).await?;
        }
        Ok(())
    }

    async fn ensure_location_schema(pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS message_locations (
                message_shadow_uid INTEGER NOT NULL REFERENCES messages(id),
                folder TEXT NOT NULL,
                uid INTEGER NOT NULL,
                PRIMARY KEY (message_shadow_uid, folder, uid)
            )
            "#,
        )
        .execute(pool)
        .await
        .context("Failed to create message location table")?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_message_locations_folder ON message_locations(folder, uid)",
        )
        .execute(pool)
        .await
        .context("Failed to index message locations")?;
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO message_locations (message_shadow_uid, folder, uid)
            SELECT id, folder, uid FROM messages
            WHERE folder IS NOT NULL AND uid IS NOT NULL AND uid > 0
            "#,
        )
        .execute(pool)
        .await
        .context("Failed to backfill message locations")?;
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version) VALUES (5)")
            .execute(pool)
            .await
            .context("Failed to mark migration 005 as applied")?;
        Ok(())
    }

    async fn ensure_column(
        pool: &SqlitePool,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<()> {
        let columns: Vec<(String,)> =
            sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .fetch_all(pool)
                .await
                .with_context(|| format!("Failed to inspect {table} schema"))?;

        if columns.iter().any(|(name,)| name == column) {
            return Ok(());
        }

        sqlx::query(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))
        .execute(pool)
        .await
        .with_context(|| format!("Failed to add {table}.{column}"))?;

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

    // ============================================================
    // Selection methods
    // ============================================================

    /// Add messages to the selection (folder is per-entry since IMAP UIDs are folder-scoped)
    pub async fn add_to_selection(
        &self,
        account: &str,
        entries: &[SelectionEntryTuple<'_>],
    ) -> Result<usize> {
        let mut count = 0;
        for (uid, folder, message_id, subject, shadow_uid) in entries {
            let result = sqlx::query(
                r#"
                INSERT INTO selections (account, folder, uid, message_id, subject, shadow_uid)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(account, folder, uid) DO UPDATE SET
                    message_id = COALESCE(?4, message_id),
                    subject = COALESCE(?5, subject),
                    shadow_uid = COALESCE(?6, shadow_uid)
                "#,
            )
            .bind(account)
            .bind(*folder)
            .bind(*uid as i64)
            .bind(*message_id)
            .bind(*subject)
            .bind(*shadow_uid)
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
            SELECT account, folder, uid, message_id, subject, shadow_uid
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

    /// Clear all selections for an account
    pub async fn clear_selection(&self, account: &str) -> Result<usize> {
        let result = sqlx::query("DELETE FROM selections WHERE account = ?1")
            .bind(account)
            .execute(&self.pool)
            .await
            .context("Failed to clear selection")?;

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

    /// Save query results (replaces previous results for the folders involved)
    /// Each entry includes its folder since IMAP UIDs are folder-scoped.
    pub async fn save_query_results(
        &self,
        account: &str,
        query_string: &str,
        results: &[SelectionEntryTuple<'_>],
    ) -> Result<()> {
        use std::collections::HashSet;

        // Collect unique folders from results
        let folders: HashSet<&str> = results.iter().map(|(_, folder, _, _, _)| *folder).collect();

        // Update query_history and clear old results for each folder
        for folder in &folders {
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
            .bind(*folder)
            .bind(query_string)
            .execute(&self.pool)
            .await
            .context("Failed to save query history")?;

            // Clear previous results for this account/folder
            sqlx::query("DELETE FROM query_history_results WHERE account = ?1 AND folder = ?2")
                .bind(account)
                .bind(*folder)
                .execute(&self.pool)
                .await
                .context("Failed to clear old query results")?;
        }

        // Insert new results with their actual folders
        for (uid, folder, message_id, subject, shadow_uid) in results {
            sqlx::query(
                r#"
                INSERT INTO query_history_results (account, folder, uid, message_id, subject, shadow_uid)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
            )
            .bind(account)
            .bind(*folder)
            .bind(*uid as i64)
            .bind(*message_id)
            .bind(*subject)
            .bind(*shadow_uid)
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

    // ============================================================
    // Draft methods
    // ============================================================

    /// Save a draft operation (replaces any existing draft for the account)
    pub async fn save_draft(&self, draft: &Draft) -> Result<()> {
        let uids_json = serde_json::to_string(&draft.uids)?;
        let flag_params_json = draft
            .flag_params
            .as_ref()
            .map(serde_json::to_string)
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
    #[allow(clippy::type_complexity)]
    pub async fn get_draft(&self, account: &str) -> Result<Option<Draft>> {
        let row: Option<(
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            bool,
        )> = sqlx::query_as(
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
            Some((
                account,
                action_type,
                folder,
                uids_json,
                flag_params_json,
                dest_folder,
                permanent,
            )) => {
                let action_type = ActionType::from_str(&action_type).ok_or_else(|| {
                    anyhow::anyhow!("Invalid action type in draft: {}", action_type)
                })?;
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

    // ============================================================
    // Shadow UID methods
    // ============================================================

    /// Get or create a shadow UID for a message.
    /// Returns the shadow_uid (messages.id) for the message.
    /// If the message doesn't exist, it's created and the new id is returned.
    #[allow(clippy::too_many_arguments)]
    pub async fn get_or_create_shadow_uid(
        &self,
        account: &str,
        folder: &str,
        imap_uid: u32,
        message_id: Option<&str>,
        subject: Option<&str>,
        from_address: Option<&str>,
        date_sent: Option<DateTime<Utc>>,
    ) -> Result<i64> {
        // We need a message_id to track messages - if none, we can't create a shadow UID
        let Some(msg_id) = message_id else {
            return Err(anyhow::anyhow!(
                "Cannot create shadow UID: message has no Message-ID header"
            ));
        };

        let date_sent_str = date_sent.map(|d| d.to_rfc3339());

        // Try to insert or update, then get the id
        sqlx::query(
            r#"
            INSERT INTO messages (account, message_id, folder, uid, subject, from_address, date_sent)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(account, message_id) DO UPDATE SET
                subject = COALESCE(?5, subject),
                from_address = COALESCE(?6, from_address),
                date_sent = COALESCE(?7, date_sent)
            "#,
        )
        .bind(account)
        .bind(msg_id)
        .bind(folder)
        .bind(imap_uid)
        .bind(subject)
        .bind(from_address)
        .bind(date_sent_str)
        .execute(&self.pool)
        .await
        .context("Failed to upsert message for shadow UID")?;

        // Now fetch the id
        let result: (i64,) =
            sqlx::query_as("SELECT id FROM messages WHERE account = ?1 AND message_id = ?2")
                .bind(account)
                .bind(msg_id)
                .fetch_one(&self.pool)
                .await
                .context("Failed to get shadow UID after upsert")?;

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO message_locations (message_shadow_uid, folder, uid)
            VALUES (?1, ?2, ?3)
            "#,
        )
        .bind(result.0)
        .bind(folder)
        .bind(imap_uid as i64)
        .execute(&self.pool)
        .await
        .context("Failed to record message location")?;

        Ok(result.0)
    }

    /// Get a message record by shadow UID
    pub async fn get_message_by_shadow_uid(
        &self,
        account: &str,
        shadow_uid: i64,
    ) -> Result<Option<MessageRecord>> {
        let record: Option<MessageRecord> = sqlx::query_as(
            r#"
            SELECT id, account, message_id, folder, uid, subject, from_address, date_sent, agent_read
            FROM messages
            WHERE account = ?1 AND id = ?2
            "#,
        )
        .bind(account)
        .bind(shadow_uid)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get message by shadow UID")?;

        Ok(record)
    }

    /// Resolve shadow UIDs to their current IMAP locations
    /// Returns a list of resolved messages with their current folder and IMAP UID
    pub async fn resolve_shadow_uids(
        &self,
        account: &str,
        shadow_uids: &[i64],
    ) -> Result<Vec<ResolvedMessage>> {
        if shadow_uids.is_empty() {
            return Ok(vec![]);
        }

        let mut resolved = Vec::with_capacity(shadow_uids.len());

        for &shadow_uid in shadow_uids {
            let record = self.get_message_by_shadow_uid(account, shadow_uid).await?;
            let Some(msg) = record else {
                return Err(anyhow::anyhow!(
                    "Message {} not found. Run 'inbox' or 'query' first to discover messages.",
                    shadow_uid
                ));
            };
            let locations: Vec<MessageLocation> = sqlx::query_as(
                "SELECT folder, uid FROM message_locations WHERE message_shadow_uid = ?1 ORDER BY folder, uid",
            )
            .bind(shadow_uid)
            .fetch_all(&self.pool)
            .await
            .context("Failed to resolve message locations")?;
            if locations.len() != 1 {
                return Err(anyhow::anyhow!(
                    "Message {} has {} known folder locations. Use a selection from the target folder before mutating it.",
                    shadow_uid,
                    locations.len()
                ));
            }
            let location = &locations[0];
            resolved.push(ResolvedMessage {
                shadow_uid: msg.id,
                folder: location.folder.clone(),
                imap_uid: location.uid as u32,
                message_id: Some(msg.message_id),
            });
        }

        Ok(resolved)
    }

    /// Resolve a saved selection at the exact folder and UID that produced it.
    pub async fn resolve_selection_entries(
        &self,
        account: &str,
        entries: &[SelectionEntry],
    ) -> Result<Vec<ResolvedMessage>> {
        let mut resolved = Vec::with_capacity(entries.len());
        for entry in entries {
            let shadow_uid = entry.shadow_uid.ok_or_else(|| {
                anyhow::anyhow!(
                    "Selection contains a message without a stable ID. Re-run the query before mutating it."
                )
            })?;
            let message_id = entry.message_id.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "Selection contains a message without a Message-ID. Re-run the query before mutating it."
                )
            })?;
            let exists: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM messages WHERE account = ?1 AND id = ?2 AND message_id = ?3",
            )
            .bind(account)
            .bind(shadow_uid)
            .bind(&message_id)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to validate selected message")?;
            if exists.is_none() {
                return Err(anyhow::anyhow!(
                    "Selected message {} is stale. Re-run the query before mutating it.",
                    shadow_uid
                ));
            }
            resolved.push(ResolvedMessage {
                shadow_uid,
                folder: entry.folder.clone(),
                imap_uid: entry.uid as u32,
                message_id: Some(message_id),
            });
        }
        Ok(resolved)
    }

    pub async fn remove_message_location(
        &self,
        account: &str,
        message_id: &str,
        folder: &str,
        uid: u32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM message_locations
            WHERE message_shadow_uid = (
                SELECT id FROM messages WHERE account = ?1 AND message_id = ?2
            ) AND folder = ?3 AND uid = ?4
            "#,
        )
        .bind(account)
        .bind(message_id)
        .bind(folder)
        .bind(uid as i64)
        .execute(&self.pool)
        .await
        .context("Failed to remove stale message location")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::StateManager;
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    async fn column_exists(pool: &SqlitePool, table: &str, column: &str) -> bool {
        let columns: Vec<(String,)> =
            sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .fetch_all(pool)
                .await
                .unwrap();
        columns.iter().any(|(name,)| name == column)
    }

    #[tokio::test]
    async fn shadow_uid_repair_completes_partially_migrated_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        for statement in [
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY)",
            "CREATE TABLE messages (id INTEGER PRIMARY KEY)",
            "CREATE TABLE selections (id INTEGER PRIMARY KEY)",
            "CREATE TABLE query_history_results (id INTEGER PRIMARY KEY)",
            "INSERT INTO schema_migrations (version) VALUES (4)",
        ] {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }

        StateManager::ensure_shadow_uid_schema(&pool).await.unwrap();
        StateManager::ensure_shadow_uid_schema(&pool).await.unwrap();

        assert!(column_exists(&pool, "selections", "shadow_uid").await);
        assert!(column_exists(&pool, "query_history_results", "shadow_uid").await);
    }
}
