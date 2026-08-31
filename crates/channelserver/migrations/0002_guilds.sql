-- ── operator (channel-server infra owner) ──────────────────────────────
CREATE TABLE operator_config (
    id           INTEGER PRIMARY KEY CHECK (id = 1),
    operator_sub TEXT,                 -- claimed on the first "Sign in with BSCP"
    created_at   REAL NOT NULL
);
INSERT INTO operator_config (id, operator_sub, created_at) VALUES (1, NULL, 0);

CREATE TABLE idp_clients (
    idp           TEXT PRIMARY KEY,    -- issuer origin
    client_id     TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    registered_at REAL NOT NULL
);

CREATE TABLE operator_sessions (
    id         TEXT PRIMARY KEY,
    sub        TEXT NOT NULL,
    expires_at REAL NOT NULL
);

-- pending OIDC auth states (client-console sign-in)
CREATE TABLE oidc_states (
    state         TEXT PRIMARY KEY,
    idp           TEXT NOT NULL,
    code_verifier TEXT NOT NULL,
    created_at    REAL NOT NULL
);

CREATE TABLE guild_creators (
    user_id TEXT PRIMARY KEY          -- user@domain allowed to create guilds
);

-- ── guilds ─────────────────────────────────────────────────────────────
CREATE TABLE guilds (
    id         TEXT PRIMARY KEY,      -- uuid
    name       TEXT NOT NULL,
    icon       TEXT,
    owner      TEXT NOT NULL,         -- user@domain
    created_at REAL NOT NULL
);

CREATE TABLE guild_members (
    guild_id  TEXT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    user_id   TEXT NOT NULL,
    nickname  TEXT,
    joined_at REAL NOT NULL,
    PRIMARY KEY (guild_id, user_id)
);

CREATE TABLE roles (
    id          TEXT PRIMARY KEY,
    guild_id    TEXT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    color       TEXT,
    position    INTEGER NOT NULL DEFAULT 0,
    permissions INTEGER NOT NULL DEFAULT 0,
    is_everyone INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE member_roles (
    guild_id TEXT NOT NULL,
    user_id  TEXT NOT NULL,
    role_id  TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    PRIMARY KEY (guild_id, user_id, role_id)
);

CREATE TABLE channels (
    id        TEXT PRIMARY KEY,       -- uuid
    guild_id  TEXT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES channels(id) ON DELETE CASCADE,
    name      TEXT NOT NULL,
    kind      TEXT NOT NULL DEFAULT 'text',   -- text | voice | category
    topic     TEXT,
    position  INTEGER NOT NULL DEFAULT 0,
    path      TEXT NOT NULL UNIQUE            -- domain#<guild>#<channel>#<sub>…
);
CREATE INDEX idx_channels_guild ON channels (guild_id);

CREATE TABLE channel_overrides (
    channel_id  TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    target_type TEXT NOT NULL,        -- role | member
    target_id   TEXT NOT NULL,        -- role_id | user@domain
    allow       INTEGER NOT NULL DEFAULT 0,
    deny        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (channel_id, target_type, target_id)
);

CREATE TABLE guild_invites (
    code       TEXT PRIMARY KEY,
    guild_id   TEXT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    created_by TEXT NOT NULL,
    uses       INTEGER NOT NULL DEFAULT 0,
    max_uses   INTEGER,
    expires_at REAL,
    created_at REAL NOT NULL
);

-- ── messages: bind existing rows to the new channel model ───────────────
ALTER TABLE channel_messages ADD COLUMN channel_id TEXT;
ALTER TABLE channel_messages ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;
