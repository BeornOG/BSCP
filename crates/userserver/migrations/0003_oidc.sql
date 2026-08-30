-- OIDC provider: this server is its own issuer; it only vouches for local users.

CREATE TABLE oauth_clients (
    client_id                     TEXT PRIMARY KEY,
    client_secret_hash            TEXT,
    name                          TEXT NOT NULL DEFAULT '',
    redirect_uris                 TEXT NOT NULL DEFAULT '[]',   -- JSON array
    grant_types                   TEXT NOT NULL DEFAULT 'authorization_code refresh_token',
    scope                         TEXT NOT NULL DEFAULT 'openid profile email',
    token_endpoint_auth_method    TEXT NOT NULL DEFAULT 'client_secret_basic',
    registration_access_token_hash TEXT,
    created_at                    REAL NOT NULL,
    disabled                      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE oauth_codes (
    code                  TEXT PRIMARY KEY,
    client_id             TEXT NOT NULL,
    user_id               TEXT NOT NULL,
    redirect_uri          TEXT NOT NULL,
    scope                 TEXT NOT NULL,
    nonce                 TEXT,
    code_challenge        TEXT,
    code_challenge_method TEXT,
    auth_time             REAL NOT NULL,
    expires_at            REAL NOT NULL,
    used                  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE oauth_tokens (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,               -- 'access' | 'refresh'
    token_hash TEXT NOT NULL UNIQUE,
    client_id  TEXT NOT NULL,
    user_id    TEXT NOT NULL,
    scope      TEXT NOT NULL,
    created_at REAL NOT NULL,
    expires_at REAL NOT NULL,
    revoked    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_oauth_tokens_user ON oauth_tokens (user_id);

CREATE TABLE oauth_consents (
    user_id    TEXT NOT NULL,
    client_id  TEXT NOT NULL,
    scope      TEXT NOT NULL,
    created_at REAL NOT NULL,
    PRIMARY KEY (user_id, client_id)
);

ALTER TABLE server_config ADD COLUMN oidc_enabled INTEGER NOT NULL DEFAULT 1;
