-- Domains the operator has blocked. Federated users whose home server is on a
-- blocked domain can't authenticate, post, join, or reach voice here, and
-- inbound legacy channel deliveries from a blocked domain are rejected.
CREATE TABLE blocked_domains (
    domain     TEXT PRIMARY KEY,
    reason     TEXT,
    created_at REAL NOT NULL
);
