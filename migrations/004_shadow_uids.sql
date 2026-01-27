-- Migration 004: Shadow UIDs support
-- Uses messages.id as stable shadow UID that persists across folder moves
-- Adds shadow_uid references to selections, query_history_results, and drafts

-- Add shadow_uid column to selections (references messages.id)
-- NULL until message is registered in messages table
ALTER TABLE selections ADD COLUMN shadow_uid INTEGER REFERENCES messages(id);

-- Add shadow_uid column to query_history_results
ALTER TABLE query_history_results ADD COLUMN shadow_uid INTEGER REFERENCES messages(id);

-- Create indexes for efficient shadow_uid lookups
CREATE INDEX IF NOT EXISTS idx_messages_id ON messages(id);
CREATE INDEX IF NOT EXISTS idx_selections_shadow_uid ON selections(shadow_uid);
CREATE INDEX IF NOT EXISTS idx_query_history_results_shadow_uid ON query_history_results(shadow_uid);

-- Mark migration as applied
INSERT OR IGNORE INTO schema_migrations (version) VALUES (4);
