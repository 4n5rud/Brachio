-- The Codex conversation an extra Codex tab started, found from the tab's note in the conversation.
ALTER TABLE task_tabs ADD COLUMN conversation_id TEXT;
