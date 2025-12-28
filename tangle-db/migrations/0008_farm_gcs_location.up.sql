CREATE TABLE IF NOT EXISTS farm_gcs_location (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    farm_id CHAR(36) NOT NULL,
    gcs_location_id CHAR(36) NOT NULL,
    role TEXT NOT NULL,
    FOREIGN KEY (farm_id) REFERENCES farm(id) ON DELETE CASCADE,
    FOREIGN KEY (gcs_location_id) REFERENCES gcs_location(id) ON DELETE CASCADE,
    UNIQUE (farm_id, gcs_location_id, role)
);
