-- Reusable prompts and shell commands a project offers as quick actions.
CREATE TABLE project_cues (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('prompt', 'shell')),
    label TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX project_cues_repo ON project_cues(repo_id, created_at_ms);
