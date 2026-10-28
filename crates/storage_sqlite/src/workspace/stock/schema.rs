//! Exact stock catalogs. Enrollment-only version1 is retained without upgrade.
use super::WorkspaceError;
use sqlx::SqliteConnection;
pub(super) const CURRENT: i64 = 17;
const AUTHORITY: &str = "CREATE TABLE stock_authority (singleton INTEGER PRIMARY KEY CHECK(singleton=1), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT";
const GRANTS: &str = "CREATE TABLE stock_grants (grant_id BLOB PRIMARY KEY CHECK(length(grant_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID";
const POOLS: &str = "CREATE TABLE stock_pools (pool_id BLOB PRIMARY KEY CHECK(length(pool_id)=32), revision INTEGER NOT NULL CHECK(revision>0), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID";
const OPERATIONS: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB NOT NULL REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(actor,grant_id,operation_id)) STRICT, WITHOUT ROWID";
const TX: &str =
    "CREATE UNIQUE INDEX stock_operation_transaction_identity ON stock_operations(transaction_id)";
const REVISION: &str =
    "CREATE UNIQUE INDEX stock_operation_revision_identity ON stock_operations(revision)";
const HEAD: &str =
    "CREATE UNIQUE INDEX stock_operation_head_identity ON stock_operations(chain_head)";
const OPERATIONS_V7: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), family INTEGER NOT NULL DEFAULT 0 CHECK(family BETWEEN 0 AND 2), order_id BLOB CHECK(order_id IS NULL OR length(order_id)=32), related_operation BLOB CHECK(related_operation IS NULL OR length(related_operation)=32), subject_grant BLOB REFERENCES stock_grants(grant_id) CHECK(subject_grant IS NULL OR length(subject_grant)=32), PRIMARY KEY(actor,grant_id,operation_id), CHECK((family=0 AND pool_id IS NOT NULL AND order_id IS NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=1 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NOT NULL) OR (family=2 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NULL AND subject_grant IS NULL))) STRICT, WITHOUT ROWID";
const OPERATIONS_V10: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), family INTEGER NOT NULL DEFAULT 0 CHECK(family BETWEEN 0 AND 3), order_id BLOB CHECK(order_id IS NULL OR length(order_id)=32), related_operation BLOB CHECK(related_operation IS NULL OR length(related_operation)=32), subject_grant BLOB REFERENCES stock_grants(grant_id) CHECK(subject_grant IS NULL OR length(subject_grant)=32), PRIMARY KEY(actor,grant_id,operation_id), CHECK((family=0 AND pool_id IS NOT NULL AND order_id IS NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=1 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NOT NULL) OR (family=2 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=3 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NULL))) STRICT, WITHOUT ROWID";
const OPERATIONS_V11: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), family INTEGER NOT NULL DEFAULT 0 CHECK(family BETWEEN 0 AND 5), order_id BLOB CHECK(order_id IS NULL OR length(order_id)=32), related_operation BLOB CHECK(related_operation IS NULL OR length(related_operation)=32), subject_grant BLOB REFERENCES stock_grants(grant_id) CHECK(subject_grant IS NULL OR length(subject_grant)=32), PRIMARY KEY(actor,grant_id,operation_id), CHECK((family=0 AND pool_id IS NOT NULL AND order_id IS NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=1 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NOT NULL) OR (family=2 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family BETWEEN 3 AND 5 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NULL))) STRICT, WITHOUT ROWID";
const OPERATIONS_V12: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), family INTEGER NOT NULL DEFAULT 0 CHECK(family BETWEEN 0 AND 6), order_id BLOB CHECK(order_id IS NULL OR length(order_id)=32), related_operation BLOB CHECK(related_operation IS NULL OR length(related_operation)=32), subject_grant BLOB REFERENCES stock_grants(grant_id) CHECK(subject_grant IS NULL OR length(subject_grant)=32), PRIMARY KEY(actor,grant_id,operation_id), CHECK((family=0 AND pool_id IS NOT NULL AND order_id IS NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=1 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NOT NULL) OR (family=2 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family BETWEEN 3 AND 6 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NULL))) STRICT, WITHOUT ROWID";
const OPERATIONS_V13: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), family INTEGER NOT NULL DEFAULT 0 CHECK(family BETWEEN 0 AND 7), order_id BLOB CHECK(order_id IS NULL OR length(order_id)=32), related_operation BLOB CHECK(related_operation IS NULL OR length(related_operation)=32), subject_grant BLOB REFERENCES stock_grants(grant_id) CHECK(subject_grant IS NULL OR length(subject_grant)=32), PRIMARY KEY(actor,grant_id,operation_id), CHECK((family=0 AND pool_id IS NOT NULL AND order_id IS NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=1 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NOT NULL) OR (family=2 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family BETWEEN 3 AND 7 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NULL))) STRICT, WITHOUT ROWID";
const OPERATIONS_V14: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), family INTEGER NOT NULL DEFAULT 0 CHECK(family BETWEEN 0 AND 8), order_id BLOB CHECK(order_id IS NULL OR length(order_id)=32), related_operation BLOB CHECK(related_operation IS NULL OR length(related_operation)=32), subject_grant BLOB REFERENCES stock_grants(grant_id) CHECK(subject_grant IS NULL OR length(subject_grant)=32), PRIMARY KEY(actor,grant_id,operation_id), CHECK((family=0 AND pool_id IS NOT NULL AND order_id IS NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=1 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NOT NULL) OR (family=2 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family BETWEEN 3 AND 8 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NULL))) STRICT, WITHOUT ROWID";
const OPERATIONS_V15: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), family INTEGER NOT NULL DEFAULT 0 CHECK(family BETWEEN 0 AND 9), order_id BLOB CHECK(order_id IS NULL OR length(order_id)=32), related_operation BLOB CHECK(related_operation IS NULL OR length(related_operation)=32), subject_grant BLOB REFERENCES stock_grants(grant_id) CHECK(subject_grant IS NULL OR length(subject_grant)=32), PRIMARY KEY(actor,grant_id,operation_id), CHECK((family=0 AND pool_id IS NOT NULL AND order_id IS NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=1 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NOT NULL) OR (family=2 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family BETWEEN 3 AND 9 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NULL))) STRICT, WITHOUT ROWID";
const OPERATIONS_V16: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), family INTEGER NOT NULL DEFAULT 0 CHECK(family BETWEEN 0 AND 10), order_id BLOB CHECK(order_id IS NULL OR length(order_id)=32), related_operation BLOB CHECK(related_operation IS NULL OR length(related_operation)=32), subject_grant BLOB REFERENCES stock_grants(grant_id) CHECK(subject_grant IS NULL OR length(subject_grant)=32), PRIMARY KEY(actor,grant_id,operation_id), CHECK((family=0 AND pool_id IS NOT NULL AND order_id IS NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=1 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NOT NULL) OR (family=2 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family BETWEEN 3 AND 10 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NULL))) STRICT, WITHOUT ROWID";
const OPERATIONS_V17: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), family INTEGER NOT NULL DEFAULT 0 CHECK(family BETWEEN 0 AND 12), order_id BLOB CHECK(order_id IS NULL OR length(order_id)=32), related_operation BLOB CHECK(related_operation IS NULL OR length(related_operation)=32), subject_grant BLOB REFERENCES stock_grants(grant_id) CHECK(subject_grant IS NULL OR length(subject_grant)=32), PRIMARY KEY(actor,grant_id,operation_id), CHECK((family=0 AND pool_id IS NOT NULL AND order_id IS NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family=1 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NOT NULL) OR (family=2 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NULL AND subject_grant IS NULL) OR (family BETWEEN 3 AND 12 AND pool_id IS NULL AND order_id IS NOT NULL AND related_operation IS NOT NULL AND subject_grant IS NULL))) STRICT, WITHOUT ROWID";
const MOVEMENTS: &str = "CREATE TABLE stock_movements (movement_id BLOB PRIMARY KEY CHECK(length(movement_id)=32), actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL CHECK(length(grant_id)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), pool_id BLOB NOT NULL REFERENCES stock_pools(pool_id), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), FOREIGN KEY(actor,grant_id,operation_id) REFERENCES stock_operations(actor,grant_id,operation_id)) STRICT, WITHOUT ROWID";
pub(super) fn objects(
    version: i64,
) -> Result<Vec<(&'static str, &'static str, &'static str, &'static str)>, WorkspaceError> {
    let mut objects = vec![
        ("table", "stock_authority", "stock_authority", AUTHORITY),
        ("table", "stock_grants", "stock_grants", GRANTS),
    ];
    match version {
        1 => {}
        2..=17 => objects.extend([
            ("table", "stock_pools", "stock_pools", POOLS),
            (
                "table",
                "stock_operations",
                "stock_operations",
                if version >= 17 {
                    OPERATIONS_V17
                } else if version >= 16 {
                    OPERATIONS_V16
                } else if version >= 15 {
                    OPERATIONS_V15
                } else if version >= 14 {
                    OPERATIONS_V14
                } else if version >= 13 {
                    OPERATIONS_V13
                } else if version >= 12 {
                    OPERATIONS_V12
                } else if version >= 11 {
                    OPERATIONS_V11
                } else if version >= 10 {
                    OPERATIONS_V10
                } else if version >= 7 {
                    OPERATIONS_V7
                } else {
                    OPERATIONS
                },
            ),
            (
                "index",
                "stock_operation_transaction_identity",
                "stock_operations",
                TX,
            ),
            (
                "index",
                "stock_operation_revision_identity",
                "stock_operations",
                REVISION,
            ),
            (
                "index",
                "stock_operation_head_identity",
                "stock_operations",
                HEAD,
            ),
            ("table", "stock_movements", "stock_movements", MOVEMENTS),
        ]),
        _ => return Err(WorkspaceError::RecoveryRequired),
    }
    if version >= 4 {
        objects.push(("table", "stock_grant_changes", "stock_grant_changes", "CREATE TABLE stock_grant_changes (transaction_id BLOB PRIMARY KEY CHECK(length(transaction_id)=32), revision INTEGER NOT NULL CHECK(revision>1), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID"));
        objects.push((
            "index",
            "stock_grant_change_revision",
            "stock_grant_changes",
            "CREATE UNIQUE INDEX stock_grant_change_revision ON stock_grant_changes(revision)",
        ));
        objects.push(("index", "stock_actor_operation_identity", "stock_operations", "CREATE UNIQUE INDEX stock_actor_operation_identity ON stock_operations(actor,operation_id)"));
    }
    if version >= 5 {
        objects.push(("table","stock_mappings","stock_mappings","CREATE TABLE stock_mappings (listing_author BLOB NOT NULL CHECK(length(listing_author)=32), listing_d BLOB NOT NULL CHECK(length(listing_d)=16), option_id BLOB NOT NULL CHECK(length(option_id)=32), pool_id BLOB NOT NULL REFERENCES stock_pools(pool_id), revision INTEGER NOT NULL CHECK(revision>0), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(listing_author,listing_d,option_id)) STRICT, WITHOUT ROWID"));
    }
    if (6..=17).contains(&version) {
        objects.push(("table","stock_source_records","stock_source_records","CREATE TABLE stock_source_records (record_id BLOB PRIMARY KEY CHECK(length(record_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID"));
        objects.push(("table","stock_source_evidence","stock_source_evidence","CREATE TABLE stock_source_evidence (envelope_id BLOB PRIMARY KEY CHECK(length(envelope_id)=32), record_id BLOB REFERENCES stock_source_records(record_id), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID"));
        objects.push(("index","stock_source_transaction_identity","stock_source_evidence","CREATE UNIQUE INDEX stock_source_transaction_identity ON stock_source_evidence(transaction_id)"));
        objects.push((
            "index",
            "stock_source_revision_identity",
            "stock_source_evidence",
            "CREATE UNIQUE INDEX stock_source_revision_identity ON stock_source_evidence(revision)",
        ));
        objects.push((
            "index",
            "stock_source_head_identity",
            "stock_source_evidence",
            "CREATE UNIQUE INDEX stock_source_head_identity ON stock_source_evidence(chain_head)",
        ));
        objects.push(("table","stock_source_parts","stock_source_parts","CREATE TABLE stock_source_parts (envelope_id BLOB NOT NULL REFERENCES stock_source_evidence(envelope_id), role INTEGER NOT NULL CHECK(role BETWEEN 0 AND 2), part INTEGER NOT NULL CHECK(part BETWEEN 0 AND 3), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(envelope_id,role,part)) STRICT, WITHOUT ROWID"));
    }
    if version >= 7 {
        objects.push(("table","stock_operation_parts","stock_operation_parts","CREATE TABLE stock_operation_parts (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL CHECK(length(grant_id)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), part INTEGER NOT NULL CHECK(part BETWEEN 0 AND 15), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(actor,operation_id,part), FOREIGN KEY(actor,grant_id,operation_id) REFERENCES stock_operations(actor,grant_id,operation_id)) STRICT, WITHOUT ROWID"));
        objects.push(("index","stock_consent_target_identity","stock_operations","CREATE UNIQUE INDEX stock_consent_target_identity ON stock_operations(actor,related_operation,subject_grant) WHERE family=1"));
        objects.push(("index","stock_order_operation_position","stock_operations","CREATE INDEX stock_order_operation_position ON stock_operations(order_id,revision) WHERE order_id IS NOT NULL"));
        if version >= 13 {
            objects.push(("table","stock_orders","stock_orders","CREATE TABLE stock_orders (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), order_id BLOB NOT NULL CHECK(length(order_id)=32), revision INTEGER NOT NULL CHECK(revision=1), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(actor,operation_id), FOREIGN KEY(actor,operation_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
            objects.push(("table","stock_order_states","stock_order_states","CREATE TABLE stock_order_states (order_id BLOB PRIMARY KEY CHECK(length(order_id)=32), actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), revision INTEGER NOT NULL CHECK(revision>0), phase INTEGER NOT NULL CHECK(phase BETWEEN 0 AND 2), FOREIGN KEY(actor,operation_id) REFERENCES stock_orders(actor,operation_id), FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
            objects.push(("table","stock_aborts","stock_aborts","CREATE TABLE stock_aborts (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), PRIMARY KEY(actor,operation_id), FOREIGN KEY(actor,operation_id) REFERENCES stock_orders(actor,operation_id), FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
            objects.push((
                "index",
                "stock_abort_entry",
                "stock_aborts",
                "CREATE UNIQUE INDEX stock_abort_entry ON stock_aborts(actor,entry_id)",
            ));
        } else {
            objects.push(("table","stock_orders","stock_orders","CREATE TABLE stock_orders (order_id BLOB PRIMARY KEY CHECK(length(order_id)=32), revision INTEGER NOT NULL CHECK(revision>0), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID"));
        }
    }
    if matches!(version, 8..=17) {
        objects.push(("table","stock_public_events","stock_public_events","CREATE TABLE stock_public_events (event_id BLOB PRIMARY KEY CHECK(length(event_id)=32), listing_author BLOB NOT NULL CHECK(length(listing_author)=32), listing_d BLOB NOT NULL CHECK(length(listing_d)=16), created_at BLOB NOT NULL CHECK(length(created_at)=8), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID"));
        objects.push(("index","stock_public_transaction_identity","stock_public_events","CREATE UNIQUE INDEX stock_public_transaction_identity ON stock_public_events(transaction_id)"));
        objects.push((
            "index",
            "stock_public_revision_identity",
            "stock_public_events",
            "CREATE UNIQUE INDEX stock_public_revision_identity ON stock_public_events(revision)",
        ));
        objects.push((
            "index",
            "stock_public_head_identity",
            "stock_public_events",
            "CREATE UNIQUE INDEX stock_public_head_identity ON stock_public_events(chain_head)",
        ));
        objects.push(("table","stock_public_parts","stock_public_parts","CREATE TABLE stock_public_parts (event_id BLOB NOT NULL REFERENCES stock_public_events(event_id), part INTEGER NOT NULL CHECK(part BETWEEN 0 AND 3), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(event_id,part)) STRICT, WITHOUT ROWID"));
        objects.push(("table","stock_public_heads","stock_public_heads","CREATE TABLE stock_public_heads (listing_author BLOB NOT NULL CHECK(length(listing_author)=32), listing_d BLOB NOT NULL CHECK(length(listing_d)=16), event_id BLOB NOT NULL REFERENCES stock_public_events(event_id), revision INTEGER NOT NULL CHECK(revision>0), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(listing_author,listing_d)) STRICT, WITHOUT ROWID"));
    }
    if version >= 10 {
        objects.push(("index", "stock_signing_plan_identity", "stock_operations", "CREATE UNIQUE INDEX stock_signing_plan_identity ON stock_operations(actor,related_operation) WHERE family=3"));
    }
    if version >= 11 {
        objects.push(("table","stock_signing_claims","stock_signing_claims","CREATE TABLE stock_signing_claims (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), recipient BLOB NOT NULL CHECK(length(recipient)=32), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), PRIMARY KEY(actor,operation_id,recipient),  FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id), FOREIGN KEY(actor,operation_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
        objects.push((
            "index",
            "stock_signing_claim_entry",
            "stock_signing_claims",
            "CREATE UNIQUE INDEX stock_signing_claim_entry ON stock_signing_claims(actor,entry_id)",
        ));
        objects.push(("table","stock_signing_evidence","stock_signing_evidence","CREATE TABLE stock_signing_evidence (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), recipient BLOB NOT NULL CHECK(length(recipient)=32), evidence_digest BLOB NOT NULL CHECK(length(evidence_digest)=32), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), PRIMARY KEY(actor,operation_id,recipient,evidence_digest),  FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id), FOREIGN KEY(actor,operation_id,recipient) REFERENCES stock_signing_claims(actor,operation_id,recipient)) STRICT, WITHOUT ROWID"));
        objects.push(("index","stock_signing_evidence_entry","stock_signing_evidence","CREATE UNIQUE INDEX stock_signing_evidence_entry ON stock_signing_evidence(actor,entry_id)"));
    }
    if version >= 12 {
        objects.push(("table","stock_agreements","stock_agreements","CREATE TABLE stock_agreements (order_id BLOB NOT NULL CHECK(length(order_id)=32), commit_sequence INTEGER NOT NULL CHECK(commit_sequence>0), actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), record_id BLOB NOT NULL CHECK(length(record_id)=32), PRIMARY KEY(order_id,commit_sequence), FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id), FOREIGN KEY(actor,operation_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
        objects.push((
            "index",
            "stock_agreement_operation",
            "stock_agreements",
            "CREATE UNIQUE INDEX stock_agreement_operation ON stock_agreements(actor,operation_id)",
        ));
        objects.push((
            "index",
            "stock_agreement_entry",
            "stock_agreements",
            "CREATE UNIQUE INDEX stock_agreement_entry ON stock_agreements(actor,entry_id)",
        ));
        objects.push((
            "index",
            "stock_agreement_record",
            "stock_agreements",
            "CREATE UNIQUE INDEX stock_agreement_record ON stock_agreements(record_id)",
        ));
        objects.push(("table","stock_publication_work","stock_publication_work","CREATE TABLE stock_publication_work (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), recipient BLOB NOT NULL CHECK(length(recipient)=32), installation_entry BLOB NOT NULL CHECK(length(installation_entry)=32), PRIMARY KEY(actor,operation_id,recipient), FOREIGN KEY(actor,installation_entry) REFERENCES stock_operations(actor,operation_id), FOREIGN KEY(actor,operation_id) REFERENCES stock_agreements(actor,operation_id)) STRICT, WITHOUT ROWID"));
    }
    if version >= 14 {
        objects.push(("table","stock_original_unissued","stock_original_unissued","CREATE TABLE stock_original_unissued (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), recipient BLOB NOT NULL CHECK(length(recipient)=32), reason INTEGER NOT NULL CHECK(reason BETWEEN 1 AND 9), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), PRIMARY KEY(actor,operation_id,recipient), FOREIGN KEY(actor,operation_id,recipient) REFERENCES stock_signing_claims(actor,operation_id,recipient), FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
        objects.push(("index","stock_original_unissued_entry","stock_original_unissued","CREATE UNIQUE INDEX stock_original_unissued_entry ON stock_original_unissued(actor,entry_id)"));
    }
    if version >= 15 {
        objects.push(("table","stock_replay_admissions","stock_replay_admissions","CREATE TABLE stock_replay_admissions (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), recipient BLOB NOT NULL CHECK(length(recipient)=32), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), PRIMARY KEY(actor,operation_id,recipient), FOREIGN KEY(actor,operation_id,recipient) REFERENCES stock_signing_claims(actor,operation_id,recipient), FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
        objects.push(("index","stock_replay_admission_entry","stock_replay_admissions","CREATE UNIQUE INDEX stock_replay_admission_entry ON stock_replay_admissions(actor,entry_id)"));
    }

    if version >= 16 {
        objects.push(("table","stock_envelopes","stock_envelopes","CREATE TABLE stock_envelopes (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), recipient BLOB NOT NULL CHECK(length(recipient)=32), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), PRIMARY KEY(actor,operation_id,recipient), FOREIGN KEY(actor,operation_id,recipient) REFERENCES stock_publication_work(actor,operation_id,recipient), FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
        objects.push((
            "index",
            "stock_envelope_entry",
            "stock_envelopes",
            "CREATE UNIQUE INDEX stock_envelope_entry ON stock_envelopes(actor,entry_id)",
        ));
    }
    if version >= 17 {
        objects.push(("table","stock_delivery_attempts","stock_delivery_attempts","CREATE TABLE stock_delivery_attempts (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), recipient BLOB NOT NULL CHECK(length(recipient)=32), request_key BLOB NOT NULL CHECK(length(request_key)=32), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), PRIMARY KEY(actor,operation_id,recipient,request_key), FOREIGN KEY(actor,operation_id,recipient) REFERENCES stock_envelopes(actor,operation_id,recipient), FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
        objects.push(("index","stock_delivery_attempt_entry","stock_delivery_attempts","CREATE UNIQUE INDEX stock_delivery_attempt_entry ON stock_delivery_attempts(actor,entry_id)"));
        objects.push(("table","stock_delivery_observations","stock_delivery_observations","CREATE TABLE stock_delivery_observations (actor BLOB NOT NULL CHECK(length(actor)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), recipient BLOB NOT NULL CHECK(length(recipient)=32), request_key BLOB NOT NULL CHECK(length(request_key)=32), target_key BLOB NOT NULL CHECK(length(target_key)=32), kind INTEGER NOT NULL CHECK(kind BETWEEN 1 AND 6), entry_id BLOB NOT NULL CHECK(length(entry_id)=32), PRIMARY KEY(actor,operation_id,recipient,request_key,target_key,kind), FOREIGN KEY(actor,operation_id,recipient,request_key) REFERENCES stock_delivery_attempts(actor,operation_id,recipient,request_key), FOREIGN KEY(actor,entry_id) REFERENCES stock_operations(actor,operation_id)) STRICT, WITHOUT ROWID"));
        objects.push(("index","stock_delivery_observation_entry","stock_delivery_observations","CREATE UNIQUE INDEX stock_delivery_observation_entry ON stock_delivery_observations(actor,entry_id)"));
    }
    Ok(objects)
}
pub(super) async fn admit(connection: &mut SqliteConnection) -> Result<i64, WorkspaceError> {
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let objects = objects(version)?;
    let mut sql = String::from("SELECT CASE WHEN count(*)=? AND sum(CASE");
    for _ in &objects {
        sql.push_str(" WHEN type=? AND name=? AND tbl_name=? AND sql=? THEN 1");
    }
    sql.push_str(" ELSE 0 END)=? THEN 1 ELSE 0 END FROM sqlite_schema");
    let mut query = sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).bind(objects.len() as i64);
    for (kind, name, owner, definition) in &objects {
        query = query.bind(*kind).bind(*name).bind(*owner).bind(*definition);
    }
    let valid: i64 = query
        .bind(objects.len() as i64)
        .fetch_one(connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if valid != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(version)
}
pub(super) async fn integrity(connection: &mut SqliteConnection) -> Result<(), WorkspaceError> {
    let check: String = sqlx::query_scalar("PRAGMA quick_check(1)")
        .fetch_one(connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if check != "ok" {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(())
}
