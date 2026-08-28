CREATE TABLE users (
    id                TEXT PRIMARY KEY,
    username          TEXT NOT NULL UNIQUE,
    password_hash     TEXT NOT NULL,
    email             TEXT,
    otp_secret        TEXT NOT NULL,
    is_2fa_enabled    INTEGER NOT NULL DEFAULT 0,
    is_admin          INTEGER NOT NULL DEFAULT 0,
    is_primary_admin  INTEGER NOT NULL DEFAULT 0,
    is_deleted        INTEGER NOT NULL DEFAULT 0,
    storage_limit_mb  INTEGER NOT NULL DEFAULT 500,
    display_name      TEXT,
    theme             TEXT NOT NULL DEFAULT 'dark',
    accent_color      TEXT NOT NULL DEFAULT '#7eafff',
    bio               TEXT,
    profile_pic       TEXT,
    status_text       TEXT,
    status_type       INTEGER NOT NULL DEFAULT 1,
    created_at        REAL NOT NULL
);

CREATE TABLE user_sessions (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token        TEXT NOT NULL UNIQUE,
    device_info  TEXT,
    last_active  REAL NOT NULL,
    expires_at   REAL NOT NULL
);
CREATE INDEX idx_user_sessions_user ON user_sessions (user_id);

CREATE TABLE messages (
    id              TEXT PRIMARY KEY,
    sender          TEXT NOT NULL,
    receiver        TEXT NOT NULL,
    text            TEXT NOT NULL,
    validation_key  TEXT,
    timestamp       REAL NOT NULL,
    is_read         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_messages_sender_receiver ON messages (sender, receiver);
CREATE INDEX idx_messages_ts ON messages (timestamp);

CREATE TABLE push_subscriptions (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint    TEXT NOT NULL UNIQUE,
    p256dh      TEXT NOT NULL,
    auth        TEXT NOT NULL,
    created_at  REAL NOT NULL,
    updated_at  REAL NOT NULL
);

CREATE TABLE uploads (
    id           TEXT PRIMARY KEY,
    filename     TEXT NOT NULL,
    mimetype     TEXT NOT NULL,
    size_bytes   INTEGER NOT NULL DEFAULT 0,
    uploaded_by  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at   REAL NOT NULL
);

CREATE TABLE server_config (
    id                INTEGER PRIMARY KEY CHECK (id = 1),
    storage_limit_mb  INTEGER NOT NULL DEFAULT 500,
    updated_at        REAL NOT NULL
);

CREATE TABLE invite_codes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    code        TEXT NOT NULL UNIQUE,
    created_by  TEXT NOT NULL,
    used_by     TEXT,
    created_at  REAL NOT NULL,
    used_at     REAL,
    expires_at  REAL
);

CREATE TABLE webhooks (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id   TEXT,
    name         TEXT NOT NULL,
    token        TEXT NOT NULL UNIQUE,
    profile_pic  TEXT,
    created_at   REAL NOT NULL,
    last_used    REAL
);
