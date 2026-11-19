CREATE TABLE task_pr_status (
    task_id TEXT PRIMARY KEY NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    snapshot_json TEXT NOT NULL,
    checked_at_ms INTEGER NOT NULL
);
