-- Tasks made before the agent was recorded had none and were treated as Claude Code. Store that
-- id so every task names one of the agent catalog's ids. Unknown ids are left as they are.
UPDATE tasks SET agent_kind = 'claude_code' WHERE agent_kind IS NULL;
