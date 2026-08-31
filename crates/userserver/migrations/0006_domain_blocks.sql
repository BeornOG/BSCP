-- Domains an admin has blocked from federating into this server. Inbound
-- messages (DMs and channel deliveries) whose sender lives on a blocked domain
-- are rejected, and local users can't send to one.
CREATE TABLE blocked_domains (
    domain     TEXT PRIMARY KEY,
    reason     TEXT,
    blocked_by TEXT,
    created_at REAL NOT NULL
);
