CREATE TABLE IF NOT EXISTS farm (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    d_tag TEXT NOT NULL,
    pubkey TEXT NOT NULL,
    name TEXT NOT NULL,
    about TEXT,
    website TEXT,
    picture TEXT,
    banner TEXT,
    location_primary TEXT,
    location_city TEXT,
    location_region TEXT,
    location_country TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS farm_pubkey_d_tag_idx ON farm(pubkey, d_tag);
