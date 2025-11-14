CREATE TABLE IF NOT EXISTS nostr_profile (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    public_key CHAR(64) NOT NULL CHECK(length(public_key) = 64),
    name TEXT,
    display_name TEXT,
    about TEXT,
    website TEXT,
    picture TEXT,
    banner TEXT,
    nip05 TEXT,
    lud06 TEXT,
    lud16 TEXT
);