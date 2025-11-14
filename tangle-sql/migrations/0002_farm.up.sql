CREATE TABLE IF NOT EXISTS farm (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    name TEXT NOT NULL,
    area TEXT,
    area_unit TEXT,
    title TEXT,
    description TEXT
);