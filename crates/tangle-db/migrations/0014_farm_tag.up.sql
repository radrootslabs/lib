CREATE TABLE IF NOT EXISTS farm_tag (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    farm_id CHAR(36) NOT NULL,
    tag TEXT NOT NULL,
    FOREIGN KEY (farm_id) REFERENCES farm(id) ON DELETE CASCADE,
    UNIQUE (farm_id, tag)
);
