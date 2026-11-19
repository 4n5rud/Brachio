-- A project can be a plain folder. Its tasks run in the folder itself, with no branch or worktree.
ALTER TABLE repos ADD COLUMN is_git INTEGER NOT NULL DEFAULT 1;
