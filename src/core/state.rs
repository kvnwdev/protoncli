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

        // Run migrations
        let migration_sql = include_str!("../../migrations/001_initial_schema.sql");
        sqlx::query(migration_sql)
            .execute(&pool)
            .await
            .context("Failed to run migrations")?;

        Ok(Self { pool })
    }

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
        // Convert DateTime to string for SQLite storage
        let date_sent_str = date_sent.map(|d| d.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO messages (account, folder, uid, message_id, subject, from_address, date_sent)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(account, folder, uid) DO UPDATE SET
                message_id = ?4,
                subject = ?5,
                from_address = ?6,
                date_sent = ?7
            "#,
        )
        .bind(account)
        .bind(folder)
        .bind(uid)
        .bind(message_id)
        .bind(subject)
        .bind(from_address)
        .bind(date_sent_str)
        .execute(&self.pool)
        .await
        .context("Failed to upsert message")?;

        Ok(())
    }

    pub async fn mark_agent_read(&self, account: &str, folder: &str, uid: u32) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE messages
            SET agent_read = TRUE
            WHERE account = ?1 AND folder = ?2 AND uid = ?3
            "#,
        )
        .bind(account)
        .bind(folder)
        .bind(uid)
        .execute(&self.pool)
        .await
        .context("Failed to mark message as agent-read")?;

        Ok(())
    }

    pub async fn is_agent_read(&self, account: &str, folder: &str, uid: u32) -> Result<bool> {
        let result: Option<(bool,)> = sqlx::query_as(
            r#"
            SELECT agent_read
            FROM messages
            WHERE account = ?1 AND folder = ?2 AND uid = ?3
            "#,
        )
        .bind(account)
        .bind(folder)
        .bind(uid)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to check agent-read status")?;

        Ok(result.map(|(read,)| read).unwrap_or(false))
    }
}
