CREATE TABLE task_orchestration (
    task_id TEXT PRIMARY KEY NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK (state IN
        ('queued', 'ready', 'running', 'review_required', 'completed', 'blocked', 'failed', 'cancelled')),
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE task_dependencies (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, depends_on),
    CHECK (task_id <> depends_on)
);

CREATE INDEX task_dependencies_depends_on ON task_dependencies (depends_on);

CREATE TABLE task_handoffs (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    commit_sha TEXT NOT NULL,
    base_sha TEXT NOT NULL,
    summary TEXT NOT NULL,
    changed_paths TEXT NOT NULL CHECK (json_valid(changed_paths)),
    reported_tests TEXT NOT NULL CHECK (json_valid(reported_tests)),
    source TEXT NOT NULL CHECK (source IN ('reported_by_agent', 'verified_by_brachio')),
    created_at_ms INTEGER NOT NULL,
    reviewed_at_ms INTEGER,
    review TEXT CHECK (review IN ('approved', 'rejected')),
    CHECK ((review IS NULL) = (reviewed_at_ms IS NULL))
);

CREATE INDEX task_handoffs_task ON task_handoffs (task_id);
