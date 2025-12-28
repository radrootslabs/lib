CREATE TABLE IF NOT EXISTS farm_member_claim (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    member_pubkey CHAR(64) NOT NULL CHECK(length(member_pubkey) = 64),
    farm_pubkey CHAR(64) NOT NULL CHECK(length(farm_pubkey) = 64),
    UNIQUE (member_pubkey, farm_pubkey)
);
