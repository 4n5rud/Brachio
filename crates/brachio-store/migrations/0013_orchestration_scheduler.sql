ALTER TABLE task_orchestration ADD COLUMN reason TEXT;

CREATE TABLE lead_orchestration (
    task_id TEXT PRIMARY KEY NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    scheduler_enabled INTEGER NOT NULL DEFAULT 0 CHECK (scheduler_enabled IN (0, 1))
);
