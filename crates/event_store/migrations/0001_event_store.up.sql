CREATE TABLE IF NOT EXISTS nostr_events (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE,
  pubkey TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  kind INTEGER NOT NULL,
  tags_json TEXT NOT NULL,
  content TEXT NOT NULL,
  sig TEXT NOT NULL,
  raw_json TEXT NOT NULL,
  verification_status TEXT NOT NULL,
  contract_status TEXT NOT NULL,
  contract_id TEXT,
  event_class TEXT,
  projection_eligible INTEGER NOT NULL,
  inserted_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS nostr_event_kind_created_idx ON nostr_events(kind, created_at, event_id);
CREATE INDEX IF NOT EXISTS nostr_event_contract_idx ON nostr_events(contract_id, seq);
CREATE INDEX IF NOT EXISTS nostr_event_projection_idx ON nostr_events(projection_eligible, seq);
CREATE INDEX IF NOT EXISTS nostr_event_verification_contract_idx
ON nostr_events(verification_status, contract_status, seq);

CREATE TABLE IF NOT EXISTS nostr_event_tags (
  event_id TEXT NOT NULL REFERENCES nostr_events(event_id) ON DELETE CASCADE,
  tag_index INTEGER NOT NULL,
  tag_name TEXT NOT NULL,
  tag_value TEXT,
  tag_json TEXT NOT NULL,
  contract_semantic TEXT,
  contract_value_type TEXT,
  relay_indexed INTEGER NOT NULL,
  PRIMARY KEY (event_id, tag_index)
);

CREATE INDEX IF NOT EXISTS nostr_event_tag_lookup_idx ON nostr_event_tags(tag_name, tag_value, event_id);
CREATE INDEX IF NOT EXISTS nostr_event_tag_relay_idx ON nostr_event_tags(relay_indexed, tag_name, tag_value, event_id);

CREATE TABLE IF NOT EXISTS relay_event_seen (
  event_id TEXT NOT NULL REFERENCES nostr_events(event_id) ON DELETE CASCADE,
  relay_url TEXT NOT NULL,
  observation_type TEXT NOT NULL,
  first_seen_at_ms INTEGER NOT NULL,
  last_seen_at_ms INTEGER NOT NULL,
  observation_count INTEGER NOT NULL,
  last_message TEXT,
  PRIMARY KEY (event_id, relay_url, observation_type)
);

CREATE INDEX IF NOT EXISTS relay_event_seen_relay_idx ON relay_event_seen(relay_url, last_seen_at_ms, event_id);

CREATE TABLE IF NOT EXISTS nostr_event_head (
  coordinate_type TEXT NOT NULL,
  kind INTEGER NOT NULL,
  pubkey TEXT NOT NULL,
  d_tag TEXT,
  event_id TEXT NOT NULL REFERENCES nostr_events(event_id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  CHECK (
    (coordinate_type = 'replaceable' AND d_tag IS NULL)
    OR (coordinate_type = 'addressable' AND d_tag IS NOT NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS nostr_event_head_replaceable_idx
ON nostr_event_head(kind, pubkey)
WHERE coordinate_type = 'replaceable';

CREATE UNIQUE INDEX IF NOT EXISTS nostr_event_head_addressable_idx
ON nostr_event_head(kind, pubkey, d_tag)
WHERE coordinate_type = 'addressable';

CREATE INDEX IF NOT EXISTS nostr_event_head_event_idx ON nostr_event_head(event_id);

CREATE TABLE IF NOT EXISTS projection_cursor (
  projection_id TEXT PRIMARY KEY NOT NULL,
  projection_version INTEGER NOT NULL DEFAULT 1,
  last_event_seq INTEGER NOT NULL DEFAULT 0,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS listing_projection (
  listing_addr TEXT PRIMARY KEY NOT NULL,
  listing_event_id TEXT NOT NULL REFERENCES nostr_events(event_id) ON DELETE CASCADE,
  seller_pubkey TEXT NOT NULL,
  farm_pubkey TEXT NOT NULL,
  farm_d_tag TEXT NOT NULL,
  listing_d_tag TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  product_type TEXT NOT NULL,
  primary_bin_id TEXT NOT NULL,
  quantity_amount TEXT NOT NULL,
  quantity_unit TEXT NOT NULL,
  price_amount TEXT NOT NULL,
  price_currency TEXT NOT NULL,
  inventory_available TEXT NOT NULL,
  availability_status TEXT NOT NULL,
  delivery_method TEXT NOT NULL,
  locality_primary TEXT NOT NULL,
  locality_city TEXT,
  locality_region TEXT,
  locality_country TEXT,
  geohash5 TEXT NOT NULL,
  listing_json TEXT NOT NULL,
  source_event_seq INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS listing_projection_seller_idx
ON listing_projection(seller_pubkey, updated_at_ms, listing_addr);

CREATE INDEX IF NOT EXISTS listing_projection_geohash_idx
ON listing_projection(geohash5, updated_at_ms, listing_addr);

CREATE VIRTUAL TABLE IF NOT EXISTS listing_search_fts USING fts5(
  listing_addr UNINDEXED,
  title,
  description,
  product_type,
  locality,
  seller_pubkey UNINDEXED,
  tokenize = 'unicode61'
);

CREATE TABLE IF NOT EXISTS trade_projection (
  order_id TEXT PRIMARY KEY NOT NULL,
  status TEXT NOT NULL,
  lifecycle_terminal INTEGER NOT NULL,
  rhi_state TEXT NOT NULL,
  listing_addr TEXT,
  buyer_pubkey TEXT,
  seller_pubkey TEXT,
  request_event_id TEXT REFERENCES nostr_events(event_id) ON DELETE SET NULL,
  decision_event_id TEXT REFERENCES nostr_events(event_id) ON DELETE SET NULL,
  agreement_event_id TEXT REFERENCES nostr_events(event_id) ON DELETE SET NULL,
  pending_revision_event_id TEXT REFERENCES nostr_events(event_id) ON DELETE SET NULL,
  cancellation_event_id TEXT REFERENCES nostr_events(event_id) ON DELETE SET NULL,
  validation_receipt_event_id TEXT REFERENCES nostr_events(event_id) ON DELETE SET NULL,
  last_event_id TEXT REFERENCES nostr_events(event_id) ON DELETE SET NULL,
  expected_listing_event_id TEXT,
  current_listing_event_id TEXT REFERENCES nostr_events(event_id) ON DELETE SET NULL,
  economics_json TEXT,
  pending_inventory_json TEXT NOT NULL,
  committed_inventory_json TEXT NOT NULL,
  issues_json TEXT NOT NULL,
  issue_count INTEGER NOT NULL,
  source_event_count INTEGER NOT NULL,
  relay_observation_count INTEGER NOT NULL,
  last_source_event_seq INTEGER,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS trade_projection_status_idx
ON trade_projection(status, updated_at_ms, order_id);

CREATE INDEX IF NOT EXISTS trade_projection_listing_idx
ON trade_projection(listing_addr, updated_at_ms, order_id);

CREATE INDEX IF NOT EXISTS trade_projection_actor_idx
ON trade_projection(buyer_pubkey, seller_pubkey, updated_at_ms, order_id);
