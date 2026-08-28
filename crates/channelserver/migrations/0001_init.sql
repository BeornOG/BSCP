CREATE TABLE channel_messages (
    id           TEXT PRIMARY KEY,
    channel_path TEXT NOT NULL,
    sender       TEXT,
    text         TEXT,
    timestamp    REAL NOT NULL
);
CREATE INDEX idx_channel_messages_path ON channel_messages (channel_path);
CREATE INDEX idx_channel_messages_ts ON channel_messages (timestamp);

CREATE TABLE channel_webhooks (
    id           TEXT PRIMARY KEY,
    channel_path TEXT NOT NULL,
    name         TEXT NOT NULL,
    token        TEXT NOT NULL UNIQUE,
    profile_pic  TEXT,
    created_at   REAL NOT NULL,
    last_used    REAL
);
