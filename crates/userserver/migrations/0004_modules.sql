-- Out-of-process modules + the account links they mediate.

CREATE TABLE modules (
    name       TEXT PRIMARY KEY,
    base_url   TEXT NOT NULL,
    secret     TEXT NOT NULL,
    manifest   TEXT,               -- JSON, refreshed from {base_url}/.well-known/bscp-module
    enabled    INTEGER NOT NULL DEFAULT 1,
    created_at REAL NOT NULL
);

CREATE TABLE account_links (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    module       TEXT NOT NULL,
    provider     TEXT NOT NULL,
    external_id  TEXT,
    display_name TEXT,
    profile_url  TEXT,
    avatar_url   TEXT,
    created_at   REAL NOT NULL,
    UNIQUE (user_id, module, provider)
);
CREATE INDEX idx_account_links_user ON account_links (user_id);
