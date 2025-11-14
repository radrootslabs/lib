CREATE TABLE IF NOT EXISTS farm_location (
    tb_farm CHAR(36),
    tb_lg CHAR(36),
    FOREIGN KEY (tb_farm) REFERENCES farm(id) ON DELETE CASCADE,
    FOREIGN KEY (tb_lg) REFERENCES location_gcs(id) ON DELETE CASCADE,
    PRIMARY KEY (tb_farm, tb_lg)
);