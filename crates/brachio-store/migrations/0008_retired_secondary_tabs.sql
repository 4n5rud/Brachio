CREATE TABLE task_retired_secondary_tabs (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    tab_id TEXT NOT NULL,
    PRIMARY KEY (task_id, tab_id)
);
