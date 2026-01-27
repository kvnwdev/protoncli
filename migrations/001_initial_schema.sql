-- Initial schema for ProtonCLI state database
-- This tracks message metadata and agent-read status

-- Migrations tracking table
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Check if we've already applied migration 001
-- If not, create the messages table with the NEW schema (message_id as primary key)
-- This handles both fresh installs and upgrades

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account TEXT NOT NULL,
    message_id TEXT NOT NULL,
    folder TEXT,
    uid INTEGER,
    subject TEXT,
    from_address TEXT,
    date_sent TIMESTAMP,
    agent_read BOOLEAN DEFAULT FALSE,
    first_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(account, message_id)
);

CREATE INDEX IF NOT EXISTS idx_messages_account ON messages(account);
CREATE INDEX IF NOT EXISTS idx_messages_message_id ON messages(account, message_id);
CREATE INDEX IF NOT EXISTS idx_messages_folder ON messages(account, folder);
CREATE INDEX IF NOT EXISTS idx_messages_agent_read ON messages(agent_read);
CREATE INDEX IF NOT EXISTS idx_messages_date_sent ON messages(date_sent);
CREATE INDEX IF NOT EXISTS idx_messages_uid ON messages(account, folder, uid);

CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account TEXT NOT NULL,
    folder_path TEXT NOT NULL,
    folder_type TEXT,
    last_synced TIMESTAMP,
    UNIQUE(account, folder_path)
);

CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account);

-- Mark migration as applied
INSERT OR IGNORE INTO schema_migrations (version) VALUES (1);
