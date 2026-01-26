-- Initial schema for ProtonCLI state database
-- This tracks message metadata and agent-read status

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account TEXT NOT NULL,
    folder TEXT NOT NULL,
    uid INTEGER NOT NULL,
    message_id TEXT,
    subject TEXT,
    from_address TEXT,
    date_sent TIMESTAMP,
    agent_read BOOLEAN DEFAULT FALSE,
    first_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(account, folder, uid)
);

CREATE INDEX IF NOT EXISTS idx_messages_account ON messages(account);
CREATE INDEX IF NOT EXISTS idx_messages_folder ON messages(account, folder);
CREATE INDEX IF NOT EXISTS idx_messages_agent_read ON messages(agent_read);
CREATE INDEX IF NOT EXISTS idx_messages_date_sent ON messages(date_sent);

CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account TEXT NOT NULL,
    folder_path TEXT NOT NULL,
    folder_type TEXT,
    last_synced TIMESTAMP,
    UNIQUE(account, folder_path)
);

CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account);
