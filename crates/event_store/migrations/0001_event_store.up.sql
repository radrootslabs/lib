CREATE TABLE nostr_event (
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

CREATE INDEX nostr_event_kind_created_idx ON nostr_event(kind, created_at, event_id);
CREATE INDEX nostr_event_contract_idx ON nostr_event(contract_id, seq);
CREATE INDEX nostr_event_projection_idx ON nostr_event(projection_eligible, seq);
CREATE INDEX nostr_event_verification_contract_idx
ON nostr_event(verification_status, contract_status, seq);

CREATE TABLE nostr_event_tag (
  event_id TEXT NOT NULL REFERENCES nostr_event(event_id) ON DELETE CASCADE,
  tag_index INTEGER NOT NULL,
  tag_name TEXT NOT NULL,
  tag_value TEXT,
  tag_json TEXT NOT NULL,
  contract_semantic TEXT,
  contract_value_type TEXT,
  relay_indexed INTEGER NOT NULL,
  PRIMARY KEY (event_id, tag_index)
);

CREATE INDEX nostr_event_tag_lookup_idx ON nostr_event_tag(tag_name, tag_value, event_id);
CREATE INDEX nostr_event_tag_relay_idx ON nostr_event_tag(relay_indexed, tag_name, tag_value, event_id);

CREATE TABLE relay_event_seen (
  event_id TEXT NOT NULL REFERENCES nostr_event(event_id) ON DELETE CASCADE,
  relay_url TEXT NOT NULL,
  observation_type TEXT NOT NULL,
  first_seen_at_ms INTEGER NOT NULL,
  last_seen_at_ms INTEGER NOT NULL,
  observation_count INTEGER NOT NULL,
  last_message TEXT,
  PRIMARY KEY (event_id, relay_url, observation_type)
);

CREATE INDEX relay_event_seen_relay_idx ON relay_event_seen(relay_url, last_seen_at_ms, event_id);

CREATE TABLE nostr_event_head (
  coordinate_type TEXT NOT NULL,
  kind INTEGER NOT NULL,
  pubkey TEXT NOT NULL,
  d_tag TEXT,
  event_id TEXT NOT NULL REFERENCES nostr_event(event_id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  CHECK (
    (coordinate_type = 'replaceable' AND d_tag IS NULL)
    OR (coordinate_type = 'addressable' AND d_tag IS NOT NULL)
  )
);

CREATE UNIQUE INDEX nostr_event_head_replaceable_idx
ON nostr_event_head(kind, pubkey)
WHERE coordinate_type = 'replaceable';

CREATE UNIQUE INDEX nostr_event_head_addressable_idx
ON nostr_event_head(kind, pubkey, d_tag)
WHERE coordinate_type = 'addressable';

CREATE INDEX nostr_event_head_event_idx ON nostr_event_head(event_id);

CREATE TABLE projection_cursor (
  projection_id TEXT PRIMARY KEY NOT NULL,
  projection_version INTEGER NOT NULL DEFAULT 1,
  last_event_seq INTEGER NOT NULL DEFAULT 0,
  updated_at_ms INTEGER NOT NULL
);
