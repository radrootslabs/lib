CREATE TABLE IF NOT EXISTS event_envelopes (
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

CREATE INDEX IF NOT EXISTS event_envelope_kind_created_idx ON event_envelopes(kind, created_at, event_id);
CREATE INDEX IF NOT EXISTS event_envelope_contract_idx ON event_envelopes(contract_id, seq);
CREATE INDEX IF NOT EXISTS event_envelope_projection_idx ON event_envelopes(projection_eligible, seq);
CREATE INDEX IF NOT EXISTS event_envelope_verification_contract_idx
ON event_envelopes(verification_status, contract_status, seq);

CREATE TABLE IF NOT EXISTS event_envelope_tags (
  event_id TEXT NOT NULL REFERENCES event_envelopes(event_id) ON DELETE CASCADE,
  tag_index INTEGER NOT NULL,
  tag_name TEXT NOT NULL,
  tag_value TEXT,
  tag_json TEXT NOT NULL,
  contract_semantic TEXT,
  contract_value_type TEXT,
  relay_indexed INTEGER NOT NULL,
  PRIMARY KEY (event_id, tag_index)
);

CREATE INDEX IF NOT EXISTS event_envelope_tag_lookup_idx ON event_envelope_tags(tag_name, tag_value, event_id);
CREATE INDEX IF NOT EXISTS event_envelope_tag_relay_idx ON event_envelope_tags(relay_indexed, tag_name, tag_value, event_id);

CREATE TABLE IF NOT EXISTS event_transport_observation (
  event_id TEXT NOT NULL REFERENCES event_envelopes(event_id) ON DELETE CASCADE,
  transport_kind TEXT NOT NULL,
  endpoint_uri TEXT NOT NULL,
  endpoint_fingerprint TEXT NOT NULL,
  observation_type TEXT NOT NULL,
  first_observed_at_ms INTEGER NOT NULL,
  last_observed_at_ms INTEGER NOT NULL,
  observation_count INTEGER NOT NULL,
  redacted_message TEXT,
  PRIMARY KEY (event_id, transport_kind, endpoint_fingerprint, observation_type)
);

CREATE INDEX IF NOT EXISTS event_transport_observation_endpoint_idx
ON event_transport_observation(transport_kind, endpoint_fingerprint, last_observed_at_ms, event_id);

CREATE TABLE IF NOT EXISTS event_envelope_head (
  coordinate_type TEXT NOT NULL,
  kind INTEGER NOT NULL,
  pubkey TEXT NOT NULL,
  d_tag TEXT,
  event_id TEXT NOT NULL REFERENCES event_envelopes(event_id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  CHECK (
    (coordinate_type = 'replaceable' AND d_tag IS NULL)
    OR (coordinate_type = 'addressable' AND d_tag IS NOT NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS event_envelope_head_replaceable_idx
ON event_envelope_head(kind, pubkey)
WHERE coordinate_type = 'replaceable';

CREATE UNIQUE INDEX IF NOT EXISTS event_envelope_head_addressable_idx
ON event_envelope_head(kind, pubkey, d_tag)
WHERE coordinate_type = 'addressable';

CREATE INDEX IF NOT EXISTS event_envelope_head_event_idx ON event_envelope_head(event_id);

CREATE TABLE IF NOT EXISTS projection_cursor (
  projection_id TEXT PRIMARY KEY NOT NULL,
  projection_version INTEGER NOT NULL DEFAULT 1,
  last_event_seq INTEGER NOT NULL DEFAULT 0,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS listing_projection (
  listing_addr TEXT PRIMARY KEY NOT NULL,
  listing_event_id TEXT NOT NULL REFERENCES event_envelopes(event_id) ON DELETE CASCADE,
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

CREATE TABLE IF NOT EXISTS trade_mutation (
  mutation_id TEXT PRIMARY KEY NOT NULL,
  trade_id TEXT NOT NULL,
  root_mutation_id TEXT,
  contract_id TEXT NOT NULL,
  mutation_kind TEXT NOT NULL CHECK (mutation_kind IN ('proposal', 'decision', 'revision_proposal', 'revision_decision', 'cancellation')),
  schema_version INTEGER NOT NULL,
  candidate_id TEXT,
  proposal_mutation_id TEXT,
  target_claim_mutation_id TEXT,
  author_pubkey TEXT NOT NULL,
  counterparty_pubkey TEXT NOT NULL,
  buyer_pubkey TEXT NOT NULL,
  seller_pubkey TEXT NOT NULL,
  farm_id TEXT NOT NULL,
  authored_at_unix_s INTEGER NOT NULL,
  canonical_payload_bytes BLOB NOT NULL,
  payload_sha256 TEXT NOT NULL CHECK (length(payload_sha256) = 64),
  first_event_seq INTEGER NOT NULL REFERENCES event_envelopes(seq) ON DELETE RESTRICT,
  first_transport_event_id TEXT NOT NULL REFERENCES event_envelopes(event_id) ON DELETE RESTRICT,
  inserted_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS trade_mutation_trade_idx
ON trade_mutation(trade_id, authored_at_unix_s, mutation_id);

CREATE INDEX IF NOT EXISTS trade_mutation_candidate_idx
ON trade_mutation(trade_id, candidate_id, mutation_id)
WHERE candidate_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS trade_mutation_actor_idx
ON trade_mutation(buyer_pubkey, seller_pubkey, authored_at_unix_s, mutation_id);

CREATE TABLE IF NOT EXISTS trade_mutation_parent (
  mutation_id TEXT NOT NULL REFERENCES trade_mutation(mutation_id) ON DELETE CASCADE,
  parent_mutation_id TEXT NOT NULL,
  parent_index INTEGER NOT NULL,
  PRIMARY KEY(mutation_id, parent_mutation_id)
) STRICT;

CREATE INDEX IF NOT EXISTS trade_mutation_parent_lookup_idx
ON trade_mutation_parent(parent_mutation_id, mutation_id);

CREATE TABLE IF NOT EXISTS trade_missing_parent (
  trade_id TEXT NOT NULL,
  mutation_id TEXT NOT NULL REFERENCES trade_mutation(mutation_id) ON DELETE CASCADE,
  missing_parent_mutation_id TEXT NOT NULL,
  first_transport_event_id TEXT NOT NULL REFERENCES event_envelopes(event_id) ON DELETE CASCADE,
  first_seen_at_ms INTEGER NOT NULL,
  PRIMARY KEY(trade_id, mutation_id, missing_parent_mutation_id)
) STRICT;

CREATE INDEX IF NOT EXISTS trade_missing_parent_lookup_idx
ON trade_missing_parent(missing_parent_mutation_id, trade_id, mutation_id);

CREATE TABLE IF NOT EXISTS trade_transport_envelope (
  transport_event_id TEXT PRIMARY KEY NOT NULL REFERENCES event_envelopes(event_id) ON DELETE CASCADE,
  mutation_id TEXT NOT NULL REFERENCES trade_mutation(mutation_id) ON DELETE CASCADE,
  trade_id TEXT NOT NULL,
  transport_kind TEXT NOT NULL,
  pubkey TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  event_seq INTEGER NOT NULL REFERENCES event_envelopes(seq) ON DELETE CASCADE,
  payload_sha256 TEXT NOT NULL CHECK (length(payload_sha256) = 64),
  observed_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS trade_transport_envelope_mutation_idx
ON trade_transport_envelope(mutation_id, observed_at_ms, transport_event_id);

CREATE INDEX IF NOT EXISTS trade_transport_envelope_trade_idx
ON trade_transport_envelope(trade_id, event_seq, transport_event_id);

CREATE TABLE IF NOT EXISTS seller_inventory_reservation (
  reservation_id TEXT PRIMARY KEY NOT NULL,
  trade_id TEXT NOT NULL,
  candidate_id TEXT NOT NULL,
  claim_mutation_id TEXT NOT NULL REFERENCES trade_mutation(mutation_id) ON DELETE CASCADE,
  inventory_authority_pubkey TEXT NOT NULL,
  inventory_epoch INTEGER NOT NULL,
  assertion_commitment TEXT NOT NULL CHECK (length(assertion_commitment) = 64),
  reservation_expires_at_unix_s INTEGER NOT NULL,
  reservation_json TEXT NOT NULL,
  inserted_at_ms INTEGER NOT NULL,
  UNIQUE(candidate_id, assertion_commitment)
) STRICT;

CREATE INDEX IF NOT EXISTS seller_inventory_reservation_trade_idx
ON seller_inventory_reservation(trade_id, candidate_id, reservation_expires_at_unix_s);

CREATE INDEX IF NOT EXISTS seller_inventory_reservation_authority_idx
ON seller_inventory_reservation(inventory_authority_pubkey, inventory_epoch, reservation_id);

CREATE TABLE IF NOT EXISTS seller_inventory_reservation_line (
  reservation_id TEXT NOT NULL REFERENCES seller_inventory_reservation(reservation_id) ON DELETE CASCADE,
  line_id TEXT NOT NULL,
  bin_id TEXT NOT NULL,
  quantity_mantissa TEXT NOT NULL,
  quantity_scale INTEGER NOT NULL,
  unit_code TEXT NOT NULL,
  line_index INTEGER NOT NULL,
  PRIMARY KEY(reservation_id, line_id)
) STRICT;

CREATE INDEX IF NOT EXISTS seller_inventory_reservation_line_bin_idx
ON seller_inventory_reservation_line(bin_id, reservation_id, line_id);

CREATE TABLE IF NOT EXISTS trade_projection_checkpoint (
  trade_id TEXT PRIMARY KEY NOT NULL,
  reducer_contract_id TEXT NOT NULL,
  reducer_version INTEGER NOT NULL,
  projection_digest TEXT NOT NULL CHECK (length(projection_digest) = 64),
  root_mutation_id TEXT,
  negotiation_state TEXT NOT NULL,
  agreement_state TEXT NOT NULL,
  evidence_state TEXT NOT NULL,
  conflict_state TEXT NOT NULL,
  private_terms_state TEXT NOT NULL,
  attestation_state TEXT NOT NULL,
  fulfillment_state TEXT NOT NULL,
  payment_state TEXT NOT NULL,
  projection_json TEXT NOT NULL,
  last_mutation_id TEXT,
  last_transport_event_seq INTEGER,
  updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS trade_projection_checkpoint_agreement_idx
ON trade_projection_checkpoint(agreement_state, updated_at_ms, trade_id);

CREATE INDEX IF NOT EXISTS trade_projection_checkpoint_actor_idx
ON trade_projection_checkpoint(root_mutation_id, updated_at_ms, trade_id);

CREATE TABLE IF NOT EXISTS trade_projection_quarantine (
  quarantine_id INTEGER PRIMARY KEY AUTOINCREMENT,
  trade_id TEXT,
  mutation_id TEXT,
  transport_event_id TEXT REFERENCES event_envelopes(event_id) ON DELETE CASCADE,
  reason TEXT NOT NULL,
  observed_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS trade_projection_quarantine_trade_idx
ON trade_projection_quarantine(trade_id, observed_at_ms, quarantine_id);

CREATE INDEX IF NOT EXISTS trade_projection_quarantine_mutation_idx
ON trade_projection_quarantine(mutation_id, observed_at_ms, quarantine_id);
