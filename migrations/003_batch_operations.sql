-- Migration 003: Batch operations support
-- Adds tables for message selection, query history, and draft operations

-- Selection: persists message selections across invocations
CREATE TABLE IF NOT EXISTS selections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account TEXT NOT NULL,
    folder TEXT NOT NULL,
    uid INTEGER NOT NULL,
    message_id TEXT,
    subject TEXT,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(account, folder, uid)
);

CREATE INDEX IF NOT EXISTS idx_selections_account ON selections(account);
CREATE INDEX IF NOT EXISTS idx_selections_account_folder ON selections(account, folder);

-- Query history: stores only the last query per account/folder
CREATE TABLE IF NOT EXISTS query_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account TEXT NOT NULL,
    folder TEXT NOT NULL,
    query_string TEXT NOT NULL,
    executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(account, folder)
);

-- Query history results: UIDs from the last query for `select last` command
CREATE TABLE IF NOT EXISTS query_history_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account TEXT NOT NULL,
    folder TEXT NOT NULL,
    uid INTEGER NOT NULL,
    message_id TEXT,
    subject TEXT,
    UNIQUE(account, folder, uid)
);

CREATE INDEX IF NOT EXISTS idx_query_history_results_account ON query_history_results(account);
CREATE INDEX IF NOT EXISTS idx_query_history_results_account_folder ON query_history_results(account, folder);

-- Draft: one pending operation per account
CREATE TABLE IF NOT EXISTS drafts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account TEXT NOT NULL UNIQUE,
    action_type TEXT NOT NULL,       -- 'flag', 'move', 'copy', 'delete', 'archive'
    folder TEXT NOT NULL,
    uids_json TEXT NOT NULL,         -- JSON array of UIDs
    flag_params_json TEXT,           -- {"read": true, "starred": false, ...}
    dest_folder TEXT,
    permanent BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Mark migration as applied
INSERT OR IGNORE INTO schema_migrations (version) VALUES (3);
