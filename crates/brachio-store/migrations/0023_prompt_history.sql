-- Prompts sent through Brachio, recalled with Up/Down in the new-task field.
CREATE TABLE prompt_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    task_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('prompt', 'cue', 'draft')),
    body TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX prompt_history_repo ON prompt_history(repo_id, id);
