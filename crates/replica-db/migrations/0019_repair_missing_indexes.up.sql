CREATE UNIQUE INDEX IF NOT EXISTS farm_pubkey_d_tag_idx ON farm(pubkey, d_tag);
CREATE INDEX IF NOT EXISTS gcs_location_geohash_idx ON gcs_location(geohash);
CREATE UNIQUE INDEX IF NOT EXISTS plot_farm_d_tag_idx ON plot(farm_id, d_tag);
CREATE INDEX IF NOT EXISTS nostr_event_state_kind_idx ON nostr_event_state(kind);
