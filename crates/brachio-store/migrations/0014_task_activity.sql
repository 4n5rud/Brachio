CREATE TABLE task_activity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    detail TEXT,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX task_activity_task ON task_activity (task_id, id);
