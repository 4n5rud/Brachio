-- The agent conversation a task resumes after a restart.
ALTER TABLE tasks ADD COLUMN agent_session_id TEXT;
