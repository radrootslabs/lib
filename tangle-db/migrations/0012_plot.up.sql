CREATE TABLE IF NOT EXISTS plot (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    d_tag TEXT NOT NULL,
    farm_id CHAR(36) NOT NULL,
    name TEXT NOT NULL,
    about TEXT,
    location_primary TEXT,
    location_city TEXT,
    location_region TEXT,
    location_country TEXT,
    FOREIGN KEY (farm_id) REFERENCES farm(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS plot_farm_d_tag_idx ON plot(farm_id, d_tag);
