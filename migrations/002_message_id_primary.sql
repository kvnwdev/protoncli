-- Migration 002: Upgrade from old schema (folder/uid unique) to new schema (message_id unique)
-- This migration handles existing databases that have the old schema

-- This migration is now a no-op since we consolidated into 001_initial_schema.sql
-- Keeping for backward compatibility tracking

INSERT OR IGNORE INTO schema_migrations (version) VALUES (2);
