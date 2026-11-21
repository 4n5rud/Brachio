-- One row per open extra terminal tab, so a task can run several of the same agent or shell.
CREATE TABLE task_tabs (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    tab_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    shell_id TEXT,
    PRIMARY KEY (task_id, tab_id)
);
INSERT INTO task_tabs (task_id, tab_id, kind)
SELECT s.task_id, s.tab_id, CASE WHEN t.agent_kind = 'codex' THEN 'claude_code' ELSE 'codex' END
FROM task_secondary_tabs s JOIN tasks t ON t.id = s.task_id
WHERE s.tab_id IS NOT NULL;
