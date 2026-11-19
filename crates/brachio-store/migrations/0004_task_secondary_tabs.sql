CREATE TABLE task_secondary_tabs (
    task_id TEXT PRIMARY KEY NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    tab_id TEXT
);
