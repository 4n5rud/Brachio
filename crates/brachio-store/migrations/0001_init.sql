-- Never edit a shipped migration; add a new numbered file instead.
CREATE TABLE repos (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    default_branch TEXT NOT NULL,
    config_hash TEXT
);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL REFERENCES repos(id),
    parent_task_id TEXT REFERENCES tasks(id),
    title TEXT NOT NULL,
    prompt TEXT NOT NULL,
    branch TEXT NOT NULL,
    worktree_path TEXT NOT NULL,
    port_base INTEGER,
    state TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    agent_kind TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('pty', 'structured')),
    agent_session_id TEXT,
    pid INTEGER,
    status TEXT NOT NULL,
    exit_code INTEGER,
    started_at_ms INTEGER NOT NULL
);

CREATE TABLE status_events (
    session_id TEXT NOT NULL REFERENCES sessions(id),
    source TEXT NOT NULL,
    state TEXT NOT NULL,
    detail_json TEXT,
    observed_at_ms INTEGER NOT NULL
);
CREATE INDEX status_events_by_session ON status_events(session_id, observed_at_ms);

CREATE TABLE qa_runs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    mode TEXT NOT NULL CHECK (mode IN ('scripted', 'exploratory')),
    diff_hash TEXT NOT NULL,
    result TEXT,
    started_at_ms INTEGER NOT NULL,
    finished_at_ms INTEGER
);

CREATE TABLE qa_findings (
    qa_run_id TEXT NOT NULL REFERENCES qa_runs(id),
    test_id TEXT NOT NULL,
    signature TEXT NOT NULL,
    message TEXT NOT NULL,
    artifacts_json TEXT
);

CREATE TABLE feedback_attempts (
    task_id TEXT NOT NULL REFERENCES tasks(id),
    signature TEXT NOT NULL,
    attempt INTEGER NOT NULL,
    sent_at_ms INTEGER NOT NULL,
    PRIMARY KEY (task_id, attempt)
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);
