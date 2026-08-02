DROP INDEX radroots_runtime_event_provenance_observed_idx;

ALTER TABLE radroots_runtime_event_provenance
RENAME TO radroots_runtime_event_provenance_v1;

CREATE TABLE radroots_runtime_event_provenance (
  event_id BLOB NOT NULL REFERENCES radroots_runtime_events(event_id),
  transport_id TEXT NOT NULL CHECK (length(transport_id) BETWEEN 1 AND 64),
  target_fingerprint TEXT NOT NULL CHECK (length(target_fingerprint) = 64),
  observed_at_unix_ms INTEGER NOT NULL CHECK (observed_at_unix_ms > 0),
  cursor TEXT NOT NULL CHECK (length(cursor) <= 2048),
  PRIMARY KEY (event_id, transport_id, target_fingerprint, observed_at_unix_ms, cursor)
) STRICT, WITHOUT ROWID;

INSERT OR IGNORE INTO radroots_runtime_event_provenance (
  event_id,
  transport_id,
  target_fingerprint,
  observed_at_unix_ms,
  cursor
)
SELECT
  event_id,
  transport_kind,
  CAST(endpoint_fingerprint AS TEXT),
  last_observed_at_unix_ms,
  ''
FROM radroots_runtime_event_provenance_v1;

DROP TABLE radroots_runtime_event_provenance_v1;

CREATE INDEX radroots_runtime_event_provenance_observed_idx
ON radroots_runtime_event_provenance(observed_at_unix_ms, event_id);

CREATE UNIQUE INDEX radroots_runtime_source_generations_active_idx
ON radroots_runtime_source_generations(state)
WHERE state = 'active';

CREATE TRIGGER radroots_runtime_source_generations_sequence_guard
BEFORE UPDATE OF sequence_head ON radroots_runtime_source_generations
WHEN NEW.sequence_head < OLD.sequence_head
BEGIN
  SELECT RAISE(ABORT, 'runtime source sequence cannot regress');
END;
