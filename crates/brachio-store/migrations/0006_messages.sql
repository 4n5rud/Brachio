CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL,
    reply_to TEXT,
    from_key TEXT NOT NULL,
    to_key TEXT NOT NULL,
    kind TEXT NOT NULL,
    priority TEXT NOT NULL DEFAULT 'normal',
    subject TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    payload_json TEXT,
    request_id TEXT,
    created_at_ms INTEGER NOT NULL,
    read_at_ms INTEGER
);

CREATE UNIQUE INDEX messages_retry ON messages (from_key, request_id) WHERE request_id IS NOT NULL;
CREATE INDEX messages_unread ON messages (to_key, read_at_ms, created_at_ms);
