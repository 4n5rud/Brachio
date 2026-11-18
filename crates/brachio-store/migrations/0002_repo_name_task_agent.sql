-- Display name for repos, the agent a task was created for, and soft
-- archiving so finished tasks leave the sidebar without losing history.
ALTER TABLE repos ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE repos ADD COLUMN added_at_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tasks ADD COLUMN agent_kind TEXT;
ALTER TABLE tasks ADD COLUMN archived_at_ms INTEGER;
CREATE INDEX tasks_by_repo ON tasks(repo_id, created_at_ms);
