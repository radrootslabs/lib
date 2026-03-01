CREATE TABLE IF NOT EXISTS nostr_relay (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    url TEXT NOT NULL UNIQUE,
    relay_id TEXT,
    name TEXT,
    description TEXT,
    pubkey TEXT,
    contact TEXT,
    supported_nips TEXT,
    software TEXT,
    version TEXT,
    data TEXT
);