#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool, Sqlite};
use std::path::PathBuf;

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
}
