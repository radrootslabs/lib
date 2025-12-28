CREATE TABLE IF NOT EXISTS farm_member (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    farm_id CHAR(36) NOT NULL,
    member_pubkey CHAR(64) NOT NULL CHECK(length(member_pubkey) = 64),
    role TEXT NOT NULL,
    FOREIGN KEY (farm_id) REFERENCES farm(id) ON DELETE CASCADE,
    UNIQUE (farm_id, member_pubkey, role)
);
