CREATE TABLE task_pins (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    pinned_at_ms INTEGER NOT NULL
);
