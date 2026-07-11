CREATE TABLE IF NOT EXISTS gcs_location (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    d_tag TEXT NOT NULL,
    lat REAL NOT NULL,
    lng REAL NOT NULL,
    geohash TEXT NOT NULL,
    point TEXT NOT NULL,
    polygon TEXT NOT NULL,
    accuracy REAL,
    altitude REAL,
    tag_0 TEXT,
    label TEXT,
    area REAL,
    elevation INTEGER,
    soil TEXT,
    climate TEXT,
    gc_id TEXT,
    gc_name TEXT,
    gc_admin1_id TEXT,
    gc_admin1_name TEXT,
    gc_country_id TEXT,
    gc_country_name TEXT
);

CREATE INDEX IF NOT EXISTS gcs_location_geohash_idx ON gcs_location(geohash);
