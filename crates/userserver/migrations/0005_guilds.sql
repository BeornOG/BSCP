-- Federation assertions minted for channel servers, so the issuer callback can
-- confirm them.
CREATE TABLE issued_assertions (
    jti      TEXT PRIMARY KEY,
    user_id  TEXT NOT NULL,
    sub      TEXT NOT NULL,
    aud      TEXT NOT NULL,
    exp      REAL NOT NULL
);
CREATE INDEX idx_issued_assertions_exp ON issued_assertions (exp);

-- Local cache of guilds a user has joined, so the SPA can list them without
-- fanning out to every channel server.
CREATE TABLE guild_memberships (
    user_id        TEXT NOT NULL,
    channel_server TEXT NOT NULL,
    guild_id       TEXT NOT NULL,
    name           TEXT,
    icon           TEXT,
    joined_at      REAL NOT NULL,
    PRIMARY KEY (user_id, channel_server, guild_id)
);
