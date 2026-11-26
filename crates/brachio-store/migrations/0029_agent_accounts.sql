-- Extra accounts for an agent. The login the CLI already has on this computer is not stored
-- here: it is the built-in account, and a task without a row in task_agent_account uses it.
CREATE TABLE agent_accounts (
    id TEXT PRIMARY KEY NOT NULL,
    agent TEXT NOT NULL,
    name TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at_ms INTEGER NOT NULL
);
CREATE UNIQUE INDEX agent_accounts_name ON agent_accounts (agent, name COLLATE NOCASE);
CREATE UNIQUE INDEX agent_accounts_one_default ON agent_accounts (agent) WHERE is_default = 1;

-- The account a task was created with. No foreign key to agent_accounts: a removed account
-- must stop the task's launch instead of silently switching it to another account.
CREATE TABLE task_agent_account (
    task_id TEXT PRIMARY KEY NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL
);
