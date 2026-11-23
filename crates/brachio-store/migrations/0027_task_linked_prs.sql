CREATE TABLE task_linked_prs (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    repository TEXT NOT NULL,
    number INTEGER NOT NULL,
    added_at_ms INTEGER NOT NULL,
    PRIMARY KEY (task_id, repository, number)
);
