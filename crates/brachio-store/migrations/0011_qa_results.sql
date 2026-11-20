CREATE TABLE qa_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    command TEXT NOT NULL,
    cwd TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN
        ('passed', 'failed', 'timedOut', 'outputLimit', 'processError')),
    exit_code INTEGER,
    duration_ms INTEGER NOT NULL,
    commit_sha TEXT NOT NULL,
    verified_by_brachio INTEGER NOT NULL CHECK (verified_by_brachio IN (0, 1)),
    stdout TEXT NOT NULL,
    stderr TEXT NOT NULL,
    truncated INTEGER NOT NULL CHECK (truncated IN (0, 1)),
    retry_allowed INTEGER NOT NULL CHECK (retry_allowed IN (0, 1)),
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX qa_results_task ON qa_results (task_id, id);

ALTER TABLE task_handoffs ADD COLUMN verified_qa_result_id INTEGER
    REFERENCES qa_results(id) ON DELETE SET NULL;
