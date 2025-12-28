CREATE TABLE IF NOT EXISTS nostr_event_state (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    key TEXT NOT NULL UNIQUE,
    kind INTEGER NOT NULL,
    pubkey CHAR(64) NOT NULL CHECK(length(pubkey) = 64),
    d_tag TEXT NOT NULL,
    last_event_id CHAR(64) NOT NULL CHECK(length(last_event_id) = 64),
    last_created_at INTEGER NOT NULL,
    content_hash TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS nostr_event_state_kind_idx ON nostr_event_state(kind);
