-- Token usage read from the agents' local session logs, kept as daily aggregates so charts do not
-- reread the logs. usage_files remembers where each log was read up to.
CREATE TABLE usage_files (
    path TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    state TEXT NOT NULL
);

CREATE TABLE usage_daily (
    provider TEXT NOT NULL,
    day TEXT NOT NULL,
    project TEXT NOT NULL,
    session TEXT NOT NULL,
    input INTEGER NOT NULL DEFAULT 0,
    output INTEGER NOT NULL DEFAULT 0,
    cache_read INTEGER NOT NULL DEFAULT 0,
    cache_write INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (provider, day, project, session)
);
CREATE INDEX usage_daily_day ON usage_daily(day);

-- Plan limit windows as the agents reported them, so a history line can be drawn.
CREATE TABLE usage_limit_snapshots (
    provider TEXT NOT NULL,
    window TEXT NOT NULL,
    at_ms INTEGER NOT NULL,
    used_percent REAL NOT NULL,
    PRIMARY KEY (provider, window, at_ms)
);
