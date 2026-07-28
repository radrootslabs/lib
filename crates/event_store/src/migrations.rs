use crate::RadrootsEventStoreError;
use crate::generated::food_availability_projection_manifest as food_manifest;
use crate::generated::nip09_reconciliation_manifest as nip09_manifest;
use crate::generated::source_maintenance_manifest;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) const EVENT_STORE_LEDGER_NAME: &str = "radroots_event_store_schema_migrations";
pub(crate) const EVENT_STORE_RESERVED_PREFIX: &str = "radroots_event_store_";

pub const RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN: u32 = 1;
pub const RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT: u32 = 4;

pub(crate) const EVENT_STORE_LEDGER_DDL: &str = "CREATE TABLE radroots_event_store_schema_migrations (
  version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0),
  name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
  up_sha256 TEXT NOT NULL CHECK (length(up_sha256) = 64 AND up_sha256 NOT GLOB '*[^0-9a-f]*'),
  down_sha256 TEXT NOT NULL CHECK (length(down_sha256) = 64 AND down_sha256 NOT GLOB '*[^0-9a-f]*'),
  schema_sha256 TEXT NOT NULL CHECK (length(schema_sha256) = 64 AND schema_sha256 NOT GLOB '*[^0-9a-f]*')
) STRICT, WITHOUT ROWID";
pub(crate) const EVENT_STORE_LEDGER_CREATE_DDL: &str =
    "CREATE TABLE main.radroots_event_store_schema_migrations (
  version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0),
  name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
  up_sha256 TEXT NOT NULL CHECK (length(up_sha256) = 64 AND up_sha256 NOT GLOB '*[^0-9a-f]*'),
  down_sha256 TEXT NOT NULL CHECK (length(down_sha256) = 64 AND down_sha256 NOT GLOB '*[^0-9a-f]*'),
  schema_sha256 TEXT NOT NULL CHECK (length(schema_sha256) = 64 AND schema_sha256 NOT GLOB '*[^0-9a-f]*')
) STRICT, WITHOUT ROWID";

pub(crate) const EVENT_STORE_BASELINE_OBJECT_NAMES: &[&str] = &[
    "event_envelope_contract_idx",
    "event_envelope_head",
    "event_envelope_head_addressable_idx",
    "event_envelope_head_event_idx",
    "event_envelope_head_replaceable_idx",
    "event_envelope_kind_created_idx",
    "event_envelope_projection_idx",
    "event_envelope_tag_lookup_idx",
    "event_envelope_tag_relay_idx",
    "event_envelope_tags",
    "event_envelope_verification_contract_idx",
    "event_envelopes",
    "event_transport_observation",
    "event_transport_observation_endpoint_idx",
    "listing_projection",
    "listing_projection_geohash_idx",
    "listing_projection_seller_idx",
    "listing_search_fts",
    "listing_search_fts_config",
    "listing_search_fts_content",
    "listing_search_fts_data",
    "listing_search_fts_docsize",
    "listing_search_fts_idx",
    "projection_cursor",
    "seller_inventory_reservation",
    "seller_inventory_reservation_authority_idx",
    "seller_inventory_reservation_line",
    "seller_inventory_reservation_line_bin_idx",
    "seller_inventory_reservation_trade_idx",
    "trade_missing_parent",
    "trade_missing_parent_lookup_idx",
    "trade_mutation",
    "trade_mutation_actor_idx",
    "trade_mutation_candidate_idx",
    "trade_mutation_parent",
    "trade_mutation_parent_lookup_idx",
    "trade_mutation_trade_idx",
    "trade_projection_checkpoint",
    "trade_projection_checkpoint_actor_idx",
    "trade_projection_checkpoint_agreement_idx",
    "trade_projection_quarantine",
    "trade_projection_quarantine_mutation_idx",
    "trade_projection_quarantine_trade_idx",
    "trade_transport_envelope",
    "trade_transport_envelope_mutation_idx",
    "trade_transport_envelope_trade_idx",
];

pub(crate) const EVENT_STORE_BASELINE_TABLE_NAMES: &[&str] = &[
    "event_envelope_head",
    "event_envelope_tags",
    "event_envelopes",
    "event_transport_observation",
    "listing_projection",
    "listing_search_fts",
    "listing_search_fts_config",
    "listing_search_fts_content",
    "listing_search_fts_data",
    "listing_search_fts_docsize",
    "listing_search_fts_idx",
    "projection_cursor",
    "seller_inventory_reservation",
    "seller_inventory_reservation_line",
    "trade_missing_parent",
    "trade_mutation",
    "trade_mutation_parent",
    "trade_projection_checkpoint",
    "trade_projection_quarantine",
    "trade_transport_envelope",
];

pub(crate) const EVENT_STORE_BASELINE_FTS5_TABLE_NAMES: &[&str] = &["listing_search_fts"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventStoreMigrationHook {
    None,
    Nip09ReconciliationV1,
    FoodAvailabilityProjectionV1,
    SourceMaintenanceV1,
}

impl EventStoreMigrationHook {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Nip09ReconciliationV1 => nip09_manifest::NIP09_RECONCILIATION_HOOK_ID,
            Self::FoodAvailabilityProjectionV1 => {
                food_manifest::FOOD_AVAILABILITY_PROJECTION_HOOK_ID
            }
            Self::SourceMaintenanceV1 => source_maintenance_manifest::SOURCE_MAINTENANCE_HOOK_ID,
        }
    }

    pub(crate) const fn manifest_sha256(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Nip09ReconciliationV1 => {
                Some(nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SHA256)
            }
            Self::FoodAvailabilityProjectionV1 => {
                Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256)
            }
            Self::SourceMaintenanceV1 => {
                Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256)
            }
        }
    }
}

pub(crate) const EVENT_STORE_SOURCE_MAINTENANCE_OBJECT_NAMES: &[&str] = &[
    "radroots_event_store_source_capacity_delete_guard",
    "radroots_event_store_source_capacity_insert_guard",
    "radroots_event_store_source_capacity_marker_close_guard",
    "radroots_event_store_source_capacity_update_guard",
    "radroots_event_store_source_capacity_v1",
    "radroots_event_store_source_generation_capacity_advance",
    "radroots_event_store_source_generation_capacity_guard",
];

pub(crate) const EVENT_STORE_SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES: &[&str] = &[
    "radroots_event_store_food_availability_image_delete_guard",
    "radroots_event_store_food_availability_projection_delete_guard",
    "radroots_event_store_source_rebuild_marker_insert_guard",
];

pub(crate) const EVENT_STORE_SOURCE_MAINTENANCE_TABLE_NAMES: &[&str] =
    &["radroots_event_store_source_capacity_v1"];

pub(crate) const EVENT_STORE_FOOD_AVAILABILITY_OBJECT_NAMES: &[&str] = &[
    "radroots_event_store_addressable_feed_generation_insert",
    "radroots_event_store_addressable_feed_integrity_v1",
    "radroots_event_store_addressable_feed_transition_insert",
    "radroots_event_store_addressable_transition_coordinate_idx",
    "radroots_event_store_current_visibility_head_lookup_idx",
    "radroots_event_store_current_visibility_v1",
    "radroots_event_store_food_availability_author_idx",
    "radroots_event_store_food_availability_cursor",
    "radroots_event_store_food_availability_cursor_delete_guard",
    "radroots_event_store_food_availability_cursor_insert_guard",
    "radroots_event_store_food_availability_cursor_update_guard",
    "radroots_event_store_food_availability_image",
    "radroots_event_store_food_availability_image_delete_guard",
    "radroots_event_store_food_availability_image_insert_guard",
    "radroots_event_store_food_availability_image_update_guard",
    "radroots_event_store_food_availability_projection",
    "radroots_event_store_food_availability_projection_delete_guard",
    "radroots_event_store_food_availability_projection_insert_guard",
    "radroots_event_store_food_availability_projection_update_guard",
    "radroots_event_store_food_availability_read_v1",
    "radroots_event_store_food_availability_recent_idx",
    "radroots_event_store_food_availability_search_delete",
    "radroots_event_store_food_availability_search_fts",
    "radroots_event_store_food_availability_search_fts_config",
    "radroots_event_store_food_availability_search_fts_content",
    "radroots_event_store_food_availability_search_fts_data",
    "radroots_event_store_food_availability_search_fts_docsize",
    "radroots_event_store_food_availability_search_fts_idx",
    "radroots_event_store_food_availability_search_insert",
    "radroots_event_store_food_availability_status_idx",
    "radroots_event_store_nip09_address_target_visibility_lookup_idx",
];

pub(crate) const EVENT_STORE_FOOD_AVAILABILITY_TABLE_NAMES: &[&str] = &[
    "radroots_event_store_addressable_feed_integrity_v1",
    "radroots_event_store_food_availability_cursor",
    "radroots_event_store_food_availability_image",
    "radroots_event_store_food_availability_projection",
    "radroots_event_store_food_availability_search_fts",
    "radroots_event_store_food_availability_search_fts_config",
    "radroots_event_store_food_availability_search_fts_content",
    "radroots_event_store_food_availability_search_fts_data",
    "radroots_event_store_food_availability_search_fts_docsize",
    "radroots_event_store_food_availability_search_fts_idx",
];

pub(crate) const EVENT_STORE_FOOD_AVAILABILITY_FTS5_TABLE_NAMES: &[&str] =
    &["radroots_event_store_food_availability_search_fts"];

pub(crate) const EVENT_STORE_NIP09_OBJECT_NAMES: &[&str] = &[
    "radroots_event_store_addressable_head_state",
    "radroots_event_store_addressable_head_transition",
    "radroots_event_store_addressable_canonical_state",
    "radroots_event_store_addressable_state_delete_guard",
    "radroots_event_store_addressable_state_identity_update_guard",
    "radroots_event_store_addressable_state_insert_guard",
    "radroots_event_store_addressable_state_old_update_guard",
    "radroots_event_store_addressable_transition_delete_guard",
    "radroots_event_store_addressable_transition_floor_guard",
    "radroots_event_store_addressable_transition_generation_idx",
    "radroots_event_store_addressable_transition_insert_guard",
    "radroots_event_store_addressable_transition_kind_idx",
    "radroots_event_store_addressable_transition_sequence_guard",
    "radroots_event_store_addressable_transition_update_guard",
    "radroots_event_store_event_envelopes_append_guard",
    "radroots_event_store_event_envelopes_delete_guard",
    "radroots_event_store_event_envelopes_derived_update_guard",
    "radroots_event_store_event_envelopes_insert_conflict_guard",
    "radroots_event_store_event_envelopes_kind_pubkey_idx",
    "radroots_event_store_event_envelopes_raw_update_guard",
    "radroots_event_store_event_envelopes_seq_event_id_idx",
    "radroots_event_store_event_head_delete_guard",
    "radroots_event_store_event_head_insert_guard",
    "radroots_event_store_event_head_update_guard",
    "radroots_event_store_event_coordinate",
    "radroots_event_store_event_coordinate_delete_guard",
    "radroots_event_store_event_coordinate_insert_guard",
    "radroots_event_store_event_coordinate_nip09_lookup_idx",
    "radroots_event_store_event_coordinate_raw_lookup_idx",
    "radroots_event_store_event_coordinate_update_guard",
    "radroots_event_store_event_tags_append_guard",
    "radroots_event_store_event_tags_delete_guard",
    "radroots_event_store_event_tags_derived_update_guard",
    "radroots_event_store_event_tags_insert_conflict_guard",
    "radroots_event_store_event_tags_raw_update_guard",
    "radroots_event_store_nip09_address_target",
    "radroots_event_store_nip09_address_target_delete_guard",
    "radroots_event_store_nip09_address_target_insert_guard",
    "radroots_event_store_nip09_address_target_lookup_idx",
    "radroots_event_store_nip09_address_target_update_guard",
    "radroots_event_store_nip09_event_target",
    "radroots_event_store_nip09_event_target_delete_guard",
    "radroots_event_store_nip09_event_target_insert_guard",
    "radroots_event_store_nip09_event_target_lookup_idx",
    "radroots_event_store_nip09_event_target_update_guard",
    "radroots_event_store_nip09_request",
    "radroots_event_store_nip09_request_author_idx",
    "radroots_event_store_nip09_request_delete_guard",
    "radroots_event_store_nip09_request_insert_guard",
    "radroots_event_store_nip09_request_update_guard",
    "radroots_event_store_projection_cursor_identity_insert",
    "radroots_event_store_projection_cursor_identity_update",
    "radroots_event_store_projection_cursor_delete_guard",
    "radroots_event_store_projection_cursor_insert_guard",
    "radroots_event_store_projection_cursor_source",
    "radroots_event_store_projection_cursor_source_delete_guard",
    "radroots_event_store_projection_cursor_source_insert_guard",
    "radroots_event_store_projection_cursor_source_update_guard",
    "radroots_event_store_projection_cursor_update_guard",
    "radroots_event_store_source_generation",
    "radroots_event_store_source_generation_append_guard",
    "radroots_event_store_source_generation_delete_guard",
    "radroots_event_store_source_generation_insert_conflict_guard",
    "radroots_event_store_source_generation_update_guard",
    "radroots_event_store_source_rebuild_commit_barrier",
    "radroots_event_store_source_rebuild_commit_barrier_delete_guard",
    "radroots_event_store_source_rebuild_commit_barrier_insert_guard",
    "radroots_event_store_source_rebuild_commit_barrier_update_guard",
    "radroots_event_store_source_rebuild_marker",
    "radroots_event_store_source_rebuild_marker_delete_guard",
    "radroots_event_store_source_rebuild_marker_insert_guard",
    "radroots_event_store_source_rebuild_marker_update_guard",
    "radroots_event_store_source_state",
    "radroots_event_store_source_state_active_generation_guard",
    "radroots_event_store_source_state_authority_update_guard",
    "radroots_event_store_source_state_delete_guard",
    "radroots_event_store_source_state_insert_conflict_guard",
    "radroots_event_store_write_lock",
    "radroots_event_store_write_lock_delete_guard",
    "radroots_event_store_write_lock_insert_guard",
    "radroots_event_store_write_lock_update_guard",
];

pub(crate) const EVENT_STORE_NIP09_TABLE_NAMES: &[&str] = &[
    "radroots_event_store_addressable_head_state",
    "radroots_event_store_addressable_head_transition",
    "radroots_event_store_event_coordinate",
    "radroots_event_store_nip09_address_target",
    "radroots_event_store_nip09_event_target",
    "radroots_event_store_nip09_request",
    "radroots_event_store_projection_cursor_source",
    "radroots_event_store_source_generation",
    "radroots_event_store_source_rebuild_commit_barrier",
    "radroots_event_store_source_rebuild_marker",
    "radroots_event_store_source_state",
    "radroots_event_store_write_lock",
];

#[derive(Clone, Copy)]
pub(crate) struct EventStoreMigration {
    pub(crate) version: u32,
    pub(crate) name: &'static str,
    pub(crate) up_sql: &'static str,
    pub(crate) down_sql: &'static str,
    pub(crate) up_len: usize,
    pub(crate) down_len: usize,
    pub(crate) up_sha256: &'static str,
    pub(crate) down_sha256: &'static str,
    pub(crate) schema_sha256: &'static str,
    pub(crate) owned_object_names: &'static [&'static str],
    pub(crate) replaced_object_names: &'static [&'static str],
    pub(crate) owned_table_names: &'static [&'static str],
    pub(crate) fts5_table_names: &'static [&'static str],
    pub(crate) hook: EventStoreMigrationHook,
    pub(crate) hook_manifest_sha256: Option<&'static str>,
    pub(crate) event_contract_registry_version: Option<u32>,
}

pub(crate) const EVENT_STORE_MIGRATIONS: &[EventStoreMigration] = &[
    EventStoreMigration {
        version: 1,
        name: "event_store",
        up_sql: include_str!("../migrations/0001_event_store.up.sql"),
        down_sql: include_str!("../migrations/0001_event_store.down.sql"),
        up_len: 10_712,
        down_len: 522,
        up_sha256: "4c03906a1cffd418a48d40907aa9a1ca51bb41766cff7250c4dfc7c2fd6eddde",
        down_sha256: "fa84d587f657f601947eaeb9cd239c962a48f6fcdce723588476e8d22f3c1f53",
        schema_sha256: "5b1f92779640f1a2dbd75e37a96996bda6c8be58883190f69eb3eced22a48f03",
        owned_object_names: EVENT_STORE_BASELINE_OBJECT_NAMES,
        replaced_object_names: &[],
        owned_table_names: EVENT_STORE_BASELINE_TABLE_NAMES,
        fts5_table_names: EVENT_STORE_BASELINE_FTS5_TABLE_NAMES,
        hook: EventStoreMigrationHook::None,
        hook_manifest_sha256: None,
        event_contract_registry_version: None,
    },
    EventStoreMigration {
        version: 2,
        name: "nip09",
        up_sql: include_str!("../migrations/0002_nip09.up.sql"),
        down_sql: include_str!("../migrations/0002_nip09.down.sql"),
        up_len: nip09_manifest::NIP09_RECONCILIATION_MIGRATION_UP_BYTE_LENGTH,
        down_len: nip09_manifest::NIP09_RECONCILIATION_MIGRATION_DOWN_BYTE_LENGTH,
        up_sha256: nip09_manifest::NIP09_RECONCILIATION_MIGRATION_UP_SHA256,
        down_sha256: nip09_manifest::NIP09_RECONCILIATION_MIGRATION_DOWN_SHA256,
        schema_sha256: nip09_manifest::NIP09_RECONCILIATION_SCHEMA_SHA256,
        owned_object_names: EVENT_STORE_NIP09_OBJECT_NAMES,
        replaced_object_names: &[],
        owned_table_names: EVENT_STORE_NIP09_TABLE_NAMES,
        fts5_table_names: &[],
        hook: EventStoreMigrationHook::Nip09ReconciliationV1,
        hook_manifest_sha256: Some(nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SHA256),
        event_contract_registry_version: Some(
            nip09_manifest::NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION,
        ),
    },
    EventStoreMigration {
        version: 3,
        name: "food_availability_projection",
        up_sql: include_str!("../migrations/0003_food_availability_projection.up.sql"),
        down_sql: include_str!("../migrations/0003_food_availability_projection.down.sql"),
        up_len: food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_UP_BYTE_LENGTH,
        down_len: food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_DOWN_BYTE_LENGTH,
        up_sha256: food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_UP_SHA256,
        down_sha256: food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_DOWN_SHA256,
        schema_sha256: food_manifest::FOOD_AVAILABILITY_PROJECTION_SCHEMA_SHA256,
        owned_object_names: EVENT_STORE_FOOD_AVAILABILITY_OBJECT_NAMES,
        replaced_object_names: &[],
        owned_table_names: EVENT_STORE_FOOD_AVAILABILITY_TABLE_NAMES,
        fts5_table_names: EVENT_STORE_FOOD_AVAILABILITY_FTS5_TABLE_NAMES,
        hook: EventStoreMigrationHook::FoodAvailabilityProjectionV1,
        hook_manifest_sha256: Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256),
        event_contract_registry_version: Some(
            food_manifest::FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION,
        ),
    },
    EventStoreMigration {
        version: 4,
        name: "source_maintenance",
        up_sql: include_str!("../migrations/0004_source_maintenance.up.sql"),
        down_sql: include_str!("../migrations/0004_source_maintenance.down.sql"),
        up_len: source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_UP_BYTE_LENGTH,
        down_len: source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_DOWN_BYTE_LENGTH,
        up_sha256: source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_UP_SHA256,
        down_sha256: source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_DOWN_SHA256,
        schema_sha256: source_maintenance_manifest::SOURCE_MAINTENANCE_SCHEMA_SHA256,
        owned_object_names: EVENT_STORE_SOURCE_MAINTENANCE_OBJECT_NAMES,
        replaced_object_names: EVENT_STORE_SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES,
        owned_table_names: EVENT_STORE_SOURCE_MAINTENANCE_TABLE_NAMES,
        fts5_table_names: &[],
        hook: EventStoreMigrationHook::SourceMaintenanceV1,
        hook_manifest_sha256: Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256),
        event_contract_registry_version: Some(
            source_maintenance_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,
        ),
    },
];

pub(crate) fn migration_for_version(
    registry: &[EventStoreMigration],
    version: u32,
) -> Option<&EventStoreMigration> {
    registry
        .iter()
        .find(|migration| migration.version == version)
}

pub(crate) fn is_event_store_owned_table_name(
    registry: &[EventStoreMigration],
    name: &str,
) -> bool {
    sqlite_identifier_starts_with(name, EVENT_STORE_RESERVED_PREFIX)
        || registry
            .iter()
            .flat_map(|migration| migration.owned_table_names)
            .any(|owned| name.eq_ignore_ascii_case(owned))
}

pub(crate) fn is_event_store_governed_schema_name(
    registry: &[EventStoreMigration],
    name: &str,
) -> bool {
    name.eq_ignore_ascii_case(EVENT_STORE_LEDGER_NAME)
        || is_event_store_owned_table_name(registry, name)
        || registry
            .iter()
            .flat_map(|migration| migration.owned_object_names)
            .any(|owned| name.eq_ignore_ascii_case(owned))
}

pub(crate) fn sqlite_identifier_starts_with(name: &str, prefix: &str) -> bool {
    name.get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

pub(crate) fn validate_embedded_migration_registry() -> Result<(), RadrootsEventStoreError> {
    validate_migration_registry(
        EVENT_STORE_MIGRATIONS,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
    )
}

pub(crate) fn validate_migration_registry(
    registry: &[EventStoreMigration],
    minimum: u32,
    current: u32,
) -> Result<(), RadrootsEventStoreError> {
    if EVENT_STORE_LEDGER_CREATE_DDL.strip_prefix("CREATE TABLE main.")
        != EVENT_STORE_LEDGER_DDL.strip_prefix("CREATE TABLE ")
    {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: "main-qualified ledger creation DDL does not match canonical catalog DDL"
                .to_owned(),
        });
    }
    if registry
        .iter()
        .any(|migration| migration.hook == EventStoreMigrationHook::Nip09ReconciliationV1)
    {
        validate_generated_nip09_manifest_descriptor()?;
    }
    if registry
        .iter()
        .any(|migration| migration.hook == EventStoreMigrationHook::FoodAvailabilityProjectionV1)
    {
        validate_generated_food_availability_projection_manifest_descriptor()?;
    }
    if registry
        .iter()
        .any(|migration| migration.hook == EventStoreMigrationHook::SourceMaintenanceV1)
    {
        validate_generated_source_maintenance_manifest_descriptor()?;
    }
    if minimum == 0 || current < minimum || registry.is_empty() {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: format!(
                "migration version range {minimum}..={current} requires a non-empty positive registry"
            ),
        });
    }

    let mut expected_version = minimum;
    let mut owned_object_names = BTreeSet::new();
    let mut owned_table_names = BTreeSet::new();
    let mut migration_hook_ids = BTreeSet::new();
    for (index, migration) in registry.iter().enumerate() {
        let canonical_hook_migration = match migration.hook {
            EventStoreMigrationHook::None => None,
            EventStoreMigrationHook::Nip09ReconciliationV1 => Some((
                nip09_manifest::NIP09_RECONCILIATION_MIGRATION_VERSION,
                nip09_manifest::NIP09_RECONCILIATION_MIGRATION_NAME,
            )),
            EventStoreMigrationHook::FoodAvailabilityProjectionV1 => Some((
                food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_VERSION,
                food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_NAME,
            )),
            EventStoreMigrationHook::SourceMaintenanceV1 => Some((
                source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_VERSION,
                source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_NAME,
            )),
        };
        if let Some((canonical_version, canonical_name)) = canonical_hook_migration {
            if !migration_hook_ids.insert(migration.hook.id()) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration hook `{}` is declared more than once",
                        migration.hook.id()
                    ),
                });
            }
            if migration.version != canonical_version || migration.name != canonical_name {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration hook `{}` is bound to canonical migration {canonical_version} `{canonical_name}`, not migration {} `{}`",
                        migration.hook.id(),
                        migration.version,
                        migration.name
                    ),
                });
            }
        }
        if migration.version != expected_version {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!(
                    "expected migration version {expected_version}, found {}",
                    migration.version
                ),
            });
        }
        if migration.name.is_empty() {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!("migration version {} has an empty name", migration.version),
            });
        }
        if registry[..index]
            .iter()
            .any(|prior| prior.name == migration.name)
        {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!("migration name `{}` is duplicated", migration.name),
            });
        }
        if migration.owned_object_names.is_empty() {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!(
                    "migration version {} declares no owned schema objects",
                    migration.version
                ),
            });
        }
        for object_name in migration.owned_object_names {
            validate_owned_schema_name(migration.version, "object", object_name)?;
            if index > 0 && !object_name.starts_with(EVENT_STORE_RESERVED_PREFIX) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} object `{object_name}` is outside the reserved `{EVENT_STORE_RESERVED_PREFIX}` namespace",
                        migration.version
                    ),
                });
            }
            if !owned_object_names.insert(*object_name) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "owned schema object `{object_name}` is declared more than once"
                    ),
                });
            }
        }
        if index == 0 && !migration.replaced_object_names.is_empty() {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: "the baseline migration cannot replace predecessor schema objects"
                    .to_owned(),
            });
        }
        if !migration.replaced_object_names.is_empty()
            && (migration.hook == EventStoreMigrationHook::None
                || migration.hook_manifest_sha256.is_none()
                || migration.event_contract_registry_version.is_none())
        {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!(
                    "migration version {} replaces predecessor schema objects without an authenticated successor hook",
                    migration.version
                ),
            });
        }
        let mut migration_replacement_names = BTreeSet::new();
        for object_name in migration.replaced_object_names {
            validate_owned_schema_name(migration.version, "replacement object", object_name)?;
            if !object_name.starts_with(EVENT_STORE_RESERVED_PREFIX) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} replacement object `{object_name}` is outside the reserved `{EVENT_STORE_RESERVED_PREFIX}` namespace",
                        migration.version
                    ),
                });
            }
            if !migration_replacement_names.insert(*object_name) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} replacement object `{object_name}` is declared more than once",
                        migration.version
                    ),
                });
            }
            if migration.owned_object_names.contains(object_name)
                || migration.owned_table_names.contains(object_name)
            {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} replacement object `{object_name}` is also newly owned by that migration",
                        migration.version
                    ),
                });
            }
            let prior_owners = registry[..index]
                .iter()
                .filter(|prior| prior.owned_object_names.contains(object_name))
                .collect::<Vec<_>>();
            if prior_owners.len() != 1 {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} replacement object `{object_name}` must be owned by exactly one prior migration; found {} owners",
                        migration.version,
                        prior_owners.len()
                    ),
                });
            }
            let prior_owner = prior_owners[0];
            if prior_owner.owned_table_names.contains(object_name)
                || prior_owner.fts5_table_names.contains(object_name)
            {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} replacement object `{object_name}` is a predecessor table; only non-table schema objects may be replaced",
                        migration.version
                    ),
                });
            }
        }
        for table_name in migration.owned_table_names {
            validate_owned_schema_name(migration.version, "table", table_name)?;
            if !migration.owned_object_names.contains(table_name) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} owned table `{table_name}` is not also an owned object",
                        migration.version
                    ),
                });
            }
            if !owned_table_names.insert(*table_name) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!("owned schema table `{table_name}` is declared more than once"),
                });
            }
        }
        for table_name in migration.fts5_table_names {
            if !migration.owned_table_names.contains(table_name) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} FTS5 table `{table_name}` is not also an owned table",
                        migration.version
                    ),
                });
            }
        }
        validate_embedded_migration_input(
            migration.version,
            "up",
            migration.up_sql,
            migration.up_len,
            migration.up_sha256,
        )?;
        validate_embedded_migration_input(
            migration.version,
            "down",
            migration.down_sql,
            migration.down_len,
            migration.down_sha256,
        )?;
        validate_sha256_literal(migration.version, "schema", migration.schema_sha256)?;
        if let Some(manifest_sha256) = migration.hook_manifest_sha256 {
            validate_sha256_literal(migration.version, "hook manifest", manifest_sha256)?;
        }
        if migration.hook.id().is_empty()
            || migration.hook.manifest_sha256() != migration.hook_manifest_sha256
        {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!(
                    "migration version {} has invalid `{}` hook manifest identity",
                    migration.version,
                    migration.hook.id()
                ),
            });
        }
        match (
            migration.hook,
            migration.hook_manifest_sha256,
            migration.event_contract_registry_version,
        ) {
            (EventStoreMigrationHook::None, None, None)
            | (
                EventStoreMigrationHook::Nip09ReconciliationV1,
                Some(nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SHA256),
                Some(nip09_manifest::NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION),
            )
            | (
                EventStoreMigrationHook::FoodAvailabilityProjectionV1,
                Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256),
                Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION),
            )
            | (
                EventStoreMigrationHook::SourceMaintenanceV1,
                Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256),
                Some(
                    source_maintenance_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,
                ),
            ) => {}
            (hook, manifest, registry_version) => {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} hook `{}` declares unsupported manifest {manifest:?} or historical event contract registry version {registry_version:?}",
                        migration.version,
                        hook.id(),
                    ),
                });
            }
        }
        expected_version = expected_version.checked_add(1).ok_or_else(|| {
            RadrootsEventStoreError::MigrationRegistryDefect {
                reason: "migration version overflow".to_owned(),
            }
        })?;
    }
    if expected_version - 1 != current {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: format!(
                "registry ends at version {}, declared current version is {}",
                expected_version - 1,
                current
            ),
        });
    }
    Ok(())
}

enum GeneratedManifestMetadataAxis<'a> {
    JsonU64 {
        pointer: &'static str,
        expected: u64,
    },
    JsonString {
        pointer: &'static str,
        expected: &'a str,
    },
    JsonU64Array {
        pointer: &'static str,
        expected: &'a [u64],
    },
    JsonStringArray {
        pointer: &'static str,
        expected: &'a [&'a str],
    },
    U64 {
        actual: u64,
        expected: u64,
    },
    PositiveI64 {
        actual: i64,
    },
    String {
        actual: &'a str,
        expected: &'a str,
    },
    U32Array {
        actual: &'a [u32],
        expected: &'a [u32],
    },
    StringArray {
        actual: &'a [&'a str],
        expected: &'a [&'a str],
    },
}

impl GeneratedManifestMetadataAxis<'_> {
    fn matches(&self, manifest: &serde_json::Value) -> bool {
        match self {
            Self::JsonU64 { pointer, expected } => {
                manifest.pointer(pointer).and_then(|value| value.as_u64()) == Some(*expected)
            }
            Self::JsonString { pointer, expected } => {
                manifest.pointer(pointer).and_then(|value| value.as_str()) == Some(*expected)
            }
            Self::JsonU64Array { pointer, expected } => {
                let Some(values) = manifest.pointer(pointer).and_then(|value| value.as_array())
                else {
                    return false;
                };
                if values.len() != expected.len() {
                    return false;
                }
                for (value, expected) in values.iter().zip(*expected) {
                    if value.as_u64() != Some(*expected) {
                        return false;
                    }
                }
                true
            }
            Self::JsonStringArray { pointer, expected } => {
                let Some(values) = manifest.pointer(pointer).and_then(|value| value.as_array())
                else {
                    return false;
                };
                if values.len() != expected.len() {
                    return false;
                }
                for (value, expected) in values.iter().zip(*expected) {
                    if value.as_str() != Some(*expected) {
                        return false;
                    }
                }
                true
            }
            Self::U64 { actual, expected } => actual == expected,
            Self::PositiveI64 { actual } => *actual > 0,
            Self::String { actual, expected } => actual == expected,
            Self::U32Array { actual, expected } => actual == expected,
            Self::StringArray { actual, expected } => actual == expected,
        }
    }
}

fn validate_generated_manifest_metadata(
    manifest: &serde_json::Value,
    axes: &[GeneratedManifestMetadataAxis<'_>],
    reason: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    for axis in axes {
        if !axis.matches(manifest) {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: reason.to_owned(),
            });
        }
    }
    Ok(())
}

fn parse_generated_manifest(
    bytes: &[u8],
    name: &'static str,
) -> Result<serde_json::Value, RadrootsEventStoreError> {
    match serde_json::from_slice(bytes) {
        Ok(manifest) => Ok(manifest),
        Err(error) => Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: format!("generated {name} manifest JSON is invalid: {error}"),
        }),
    }
}

struct GeneratedManifestEnvelope<'a> {
    name: &'static str,
    bytes: &'a [u8],
    expected_byte_length: usize,
    migration_version: u32,
    expected_sha256: &'a str,
    byte_length_reason: &'static str,
    digest_reason: &'static str,
}

fn validate_generated_manifest_envelope(
    envelope: GeneratedManifestEnvelope<'_>,
) -> Result<serde_json::Value, RadrootsEventStoreError> {
    if envelope.bytes.len() != envelope.expected_byte_length {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: envelope.byte_length_reason.to_owned(),
        });
    }
    validate_sha256_literal(
        envelope.migration_version,
        "hook manifest",
        envelope.expected_sha256,
    )?;
    if sha256_hex(envelope.bytes) != envelope.expected_sha256 {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: envelope.digest_reason.to_owned(),
        });
    }
    parse_generated_manifest(envelope.bytes, envelope.name)
}

fn generated_manifest_u128_to_u64(
    value: u128,
    reason: &'static str,
) -> Result<u64, RadrootsEventStoreError> {
    match u64::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: reason.to_owned(),
        }),
    }
}

fn generated_manifest_i64_to_u64(
    value: i64,
    reason: &'static str,
) -> Result<u64, RadrootsEventStoreError> {
    match u64::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: reason.to_owned(),
        }),
    }
}

fn validate_generated_nip09_manifest_descriptor() -> Result<(), RadrootsEventStoreError> {
    let bytes = nip09_manifest::NIP09_RECONCILIATION_MANIFEST_JSON.as_bytes();
    let manifest = validate_generated_manifest_envelope(GeneratedManifestEnvelope {
        name: "NIP-09",
        bytes,
        expected_byte_length: nip09_manifest::NIP09_RECONCILIATION_MANIFEST_BYTE_LENGTH,
        migration_version: nip09_manifest::NIP09_RECONCILIATION_MIGRATION_VERSION,
        expected_sha256: nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SHA256,
        byte_length_reason: "generated NIP-09 manifest byte length is inconsistent",
        digest_reason: "generated NIP-09 manifest digest is inconsistent",
    })?;
    let up_byte_length = generated_manifest_u128_to_u64(
        nip09_manifest::NIP09_RECONCILIATION_MIGRATION_UP_BYTE_LENGTH as u128,
        "generated NIP-09 migration up byte length is out of range",
    )?;
    let down_byte_length = generated_manifest_u128_to_u64(
        nip09_manifest::NIP09_RECONCILIATION_MIGRATION_DOWN_BYTE_LENGTH as u128,
        "generated NIP-09 migration down byte length is out of range",
    )?;
    let reconciliation_version = generated_manifest_i64_to_u64(
        nip09_manifest::NIP09_RECONCILIATION_VERSION,
        "generated NIP-09 reconciliation version is out of range",
    )?;
    let addressable_feed_version = generated_manifest_i64_to_u64(
        nip09_manifest::NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION,
        "generated NIP-09 addressable feed version is out of range",
    )?;
    validate_generated_manifest_metadata(
        &manifest,
        &[
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SCHEMA_VERSION),
                expected: 1,
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(nip09_manifest::NIP09_RECONCILIATION_MIGRATION_VERSION),
                expected: 2,
            },
            GeneratedManifestMetadataAxis::PositiveI64 {
                actual: nip09_manifest::NIP09_RECONCILIATION_VERSION,
            },
            GeneratedManifestMetadataAxis::PositiveI64 {
                actual: nip09_manifest::NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/schema_version",
                expected: u64::from(nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SCHEMA_VERSION),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/migration/version",
                expected: u64::from(nip09_manifest::NIP09_RECONCILIATION_MIGRATION_VERSION),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/migration/up_byte_length",
                expected: up_byte_length,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/migration/down_byte_length",
                expected: down_byte_length,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/profile/reconciliation_version",
                expected: reconciliation_version,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/profile/addressable_feed_version",
                expected: addressable_feed_version,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/profile/event_contract_registry_version",
                expected: u64::from(
                    nip09_manifest::NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION,
                ),
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/hook_id",
                expected: nip09_manifest::NIP09_RECONCILIATION_HOOK_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/name",
                expected: nip09_manifest::NIP09_RECONCILIATION_MIGRATION_NAME,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/up_sha256",
                expected: nip09_manifest::NIP09_RECONCILIATION_MIGRATION_UP_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/down_sha256",
                expected: nip09_manifest::NIP09_RECONCILIATION_MIGRATION_DOWN_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/schema_sha256",
                expected: nip09_manifest::NIP09_RECONCILIATION_SCHEMA_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/result_vector/sha256",
                expected: nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/result_vector/executor_id",
                expected: nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/result_vector/executor_sha256",
                expected: nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_SHA256,
            },
        ],
        "generated NIP-09 manifest metadata is inconsistent",
    )?;
    for (field, digest) in [
        (
            "migration up",
            nip09_manifest::NIP09_RECONCILIATION_MIGRATION_UP_SHA256,
        ),
        (
            "migration down",
            nip09_manifest::NIP09_RECONCILIATION_MIGRATION_DOWN_SHA256,
        ),
        ("schema", nip09_manifest::NIP09_RECONCILIATION_SCHEMA_SHA256),
        (
            "result vector",
            nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_SHA256,
        ),
        (
            "result-vector executor",
            nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_SHA256,
        ),
    ] {
        validate_sha256_literal(
            nip09_manifest::NIP09_RECONCILIATION_MIGRATION_VERSION,
            field,
            digest,
        )?;
    }
    Ok(())
}

fn validate_generated_food_availability_projection_manifest_descriptor()
-> Result<(), RadrootsEventStoreError> {
    let bytes = food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_JSON.as_bytes();
    let manifest = validate_generated_manifest_envelope(GeneratedManifestEnvelope {
        name: "FoodAvailability projection",
        bytes,
        expected_byte_length: food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_BYTE_LENGTH,
        migration_version: food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_VERSION,
        expected_sha256: food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256,
        byte_length_reason: "generated FoodAvailability projection manifest byte length is inconsistent",
        digest_reason: "generated FoodAvailability projection manifest digest is inconsistent",
    })?;
    let up_byte_length = generated_manifest_u128_to_u64(
        food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_UP_BYTE_LENGTH as u128,
        "generated FoodAvailability migration up byte length is out of range",
    )?;
    let down_byte_length = generated_manifest_u128_to_u64(
        food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_DOWN_BYTE_LENGTH as u128,
        "generated FoodAvailability migration down byte length is out of range",
    )?;
    validate_generated_manifest_metadata(
        &manifest,
        &[
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(
                    food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SCHEMA_VERSION,
                ),
                expected: 1,
            },
            GeneratedManifestMetadataAxis::String {
                actual: food_manifest::FOOD_AVAILABILITY_PROJECTION_CONTRACT_ID,
                expected: "radroots_event_store.food_availability_projection_v1",
            },
            GeneratedManifestMetadataAxis::String {
                actual: food_manifest::FOOD_AVAILABILITY_PROJECTION_HOOK_ID,
                expected: "food_availability_projection_v1",
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_VERSION),
                expected: 3,
            },
            GeneratedManifestMetadataAxis::String {
                actual: food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_NAME,
                expected: "food_availability_projection",
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(food_manifest::FOOD_AVAILABILITY_PROJECTION_VERSION),
                expected: 1,
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(food_manifest::FOOD_AVAILABILITY_PROJECTION_FEED_VERSION),
                expected: 1,
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(
                    food_manifest::FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION,
                ),
                expected: 7,
            },
            GeneratedManifestMetadataAxis::U32Array {
                actual: food_manifest::FOOD_AVAILABILITY_PROJECTION_SCOPE_KINDS,
                expected: &[30_402],
            },
            GeneratedManifestMetadataAxis::String {
                actual: food_manifest::FOOD_AVAILABILITY_PROJECTION_SCOPE_FINGERPRINT_SHA256,
                expected: "8b63c5ddc48a2cc7db69295238b96d5f814dba50427c80b4d0079f061e6d3de0",
            },
            GeneratedManifestMetadataAxis::String {
                actual: food_manifest::FOOD_AVAILABILITY_PROJECTION_PREDECESSOR_MANIFEST_SHA256,
                expected: nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SHA256,
            },
            GeneratedManifestMetadataAxis::String {
                actual: food_manifest::FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_EXECUTOR_ID,
                expected: "radroots_event_store.food_availability_projection_v1.result_vector_executor.v1",
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/schema_version",
                expected: u64::from(
                    food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SCHEMA_VERSION,
                ),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/migration/version",
                expected: u64::from(food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_VERSION),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/migration/up/byte_length",
                expected: up_byte_length,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/migration/down/byte_length",
                expected: down_byte_length,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/profile/projection_version",
                expected: u64::from(food_manifest::FOOD_AVAILABILITY_PROJECTION_VERSION),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/profile/addressable_feed_version",
                expected: u64::from(food_manifest::FOOD_AVAILABILITY_PROJECTION_FEED_VERSION),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/profile/event_contract_registry_version",
                expected: u64::from(
                    food_manifest::FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION,
                ),
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/contract_id",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_CONTRACT_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/hook_id",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_HOOK_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/predecessor/manifest/sha256",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_PREDECESSOR_MANIFEST_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/name",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_NAME,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/up/sha256",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_UP_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/down/sha256",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_DOWN_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/schema_sha256",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_SCHEMA_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/profile/scope_fingerprint_sha256",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_SCOPE_FINGERPRINT_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonU64Array {
                pointer: "/profile/scope_kinds",
                expected: &[30_402],
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/result_vector/sha256",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/result_vector/executor_id",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_EXECUTOR_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/result_vector/executor_sha256",
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_EXECUTOR_SHA256,
            },
        ],
        "generated FoodAvailability projection manifest metadata is inconsistent",
    )?;
    for (field, digest) in [
        (
            "migration up",
            food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_UP_SHA256,
        ),
        (
            "migration down",
            food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_DOWN_SHA256,
        ),
        (
            "schema",
            food_manifest::FOOD_AVAILABILITY_PROJECTION_SCHEMA_SHA256,
        ),
        (
            "scope fingerprint",
            food_manifest::FOOD_AVAILABILITY_PROJECTION_SCOPE_FINGERPRINT_SHA256,
        ),
        (
            "predecessor manifest",
            food_manifest::FOOD_AVAILABILITY_PROJECTION_PREDECESSOR_MANIFEST_SHA256,
        ),
        (
            "result vector",
            food_manifest::FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_SHA256,
        ),
        (
            "result-vector executor",
            food_manifest::FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_EXECUTOR_SHA256,
        ),
    ] {
        validate_sha256_literal(
            food_manifest::FOOD_AVAILABILITY_PROJECTION_MIGRATION_VERSION,
            field,
            digest,
        )?;
    }
    Ok(())
}

fn validate_generated_source_maintenance_manifest_descriptor() -> Result<(), RadrootsEventStoreError>
{
    use source_maintenance_manifest as source_manifest;

    let bytes = source_manifest::SOURCE_MAINTENANCE_MANIFEST_JSON.as_bytes();
    let manifest = validate_generated_manifest_envelope(GeneratedManifestEnvelope {
        name: "source-maintenance",
        bytes,
        expected_byte_length: source_manifest::SOURCE_MAINTENANCE_MANIFEST_BYTE_LENGTH,
        migration_version: source_manifest::SOURCE_MAINTENANCE_MIGRATION_VERSION,
        expected_sha256: source_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256,
        byte_length_reason: "generated source-maintenance manifest byte length is inconsistent",
        digest_reason: "generated source-maintenance manifest digest is inconsistent",
    })?;
    validate_generated_manifest_metadata(
        &manifest,
        &[
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(source_manifest::SOURCE_MAINTENANCE_MANIFEST_SCHEMA_VERSION),
                expected: 1,
            },
            GeneratedManifestMetadataAxis::String {
                actual: source_manifest::SOURCE_MAINTENANCE_CONTRACT_ID,
                expected: "radroots_event_store.source_maintenance_v1",
            },
            GeneratedManifestMetadataAxis::String {
                actual: source_manifest::SOURCE_MAINTENANCE_HOOK_ID,
                expected: "source_maintenance_v1",
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(source_manifest::SOURCE_MAINTENANCE_MIGRATION_VERSION),
                expected: 4,
            },
            GeneratedManifestMetadataAxis::String {
                actual: source_manifest::SOURCE_MAINTENANCE_MIGRATION_NAME,
                expected: "source_maintenance",
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(source_manifest::SOURCE_MAINTENANCE_CAPACITY_VERSION),
                expected: 1,
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(
                    source_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,
                ),
                expected: u64::from(
                    food_manifest::FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION,
                ),
            },
            GeneratedManifestMetadataAxis::String {
                actual: source_manifest::SOURCE_MAINTENANCE_PREDECESSOR_HOOK_ID,
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_HOOK_ID,
            },
            GeneratedManifestMetadataAxis::String {
                actual: source_manifest::SOURCE_MAINTENANCE_PREDECESSOR_MANIFEST_SHA256,
                expected: food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256,
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: source_manifest::SOURCE_MAINTENANCE_RAW_EVENT_COUNT_LIMIT,
                expected: crate::RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1,
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: source_manifest::SOURCE_MAINTENANCE_RAW_TAG_COUNT_LIMIT,
                expected: crate::RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1,
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: source_manifest::SOURCE_MAINTENANCE_RAW_EVENT_TEXT_BYTES_LIMIT,
                expected: crate::RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1,
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: source_manifest::SOURCE_MAINTENANCE_RAW_TAG_TEXT_BYTES_LIMIT,
                expected: crate::RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1,
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: u64::from(
                    source_manifest::SOURCE_MAINTENANCE_RETAINED_SOURCE_GENERATION_LIMIT,
                ),
                expected: u64::from(
                    crate::RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1,
                ),
            },
            GeneratedManifestMetadataAxis::StringArray {
                actual: source_manifest::SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES,
                expected: EVENT_STORE_SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES,
            },
            GeneratedManifestMetadataAxis::StringArray {
                actual: source_manifest::SOURCE_MAINTENANCE_RAW_EVENT_COLUMNS,
                expected: &[
                    "event_id",
                    "pubkey",
                    "tags_json",
                    "content",
                    "sig",
                    "raw_json",
                ],
            },
            GeneratedManifestMetadataAxis::StringArray {
                actual: source_manifest::SOURCE_MAINTENANCE_RAW_TAG_COLUMNS,
                expected: &["event_id", "tag_name", "tag_value", "tag_json"],
            },
            GeneratedManifestMetadataAxis::StringArray {
                actual: source_manifest::SOURCE_MAINTENANCE_NULLABLE_RAW_TAG_COLUMNS,
                expected: &["tag_value"],
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/schema_version",
                expected: u64::from(source_manifest::SOURCE_MAINTENANCE_MANIFEST_SCHEMA_VERSION),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/migration/version",
                expected: u64::from(source_manifest::SOURCE_MAINTENANCE_MIGRATION_VERSION),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/migration/up/byte_length",
                expected: source_manifest::SOURCE_MAINTENANCE_MIGRATION_UP_BYTE_LENGTH as u64,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/migration/down/byte_length",
                expected: source_manifest::SOURCE_MAINTENANCE_MIGRATION_DOWN_BYTE_LENGTH as u64,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/source_maintenance/version",
                expected: u64::from(source_manifest::SOURCE_MAINTENANCE_CAPACITY_VERSION),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/source_maintenance/event_contract_registry_version",
                expected: u64::from(
                    source_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,
                ),
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/source_maintenance/limits/raw_events",
                expected: source_manifest::SOURCE_MAINTENANCE_RAW_EVENT_COUNT_LIMIT,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/source_maintenance/limits/raw_tags",
                expected: source_manifest::SOURCE_MAINTENANCE_RAW_TAG_COUNT_LIMIT,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/source_maintenance/limits/raw_event_text_bytes",
                expected: source_manifest::SOURCE_MAINTENANCE_RAW_EVENT_TEXT_BYTES_LIMIT,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/source_maintenance/limits/raw_tag_text_bytes",
                expected: source_manifest::SOURCE_MAINTENANCE_RAW_TAG_TEXT_BYTES_LIMIT,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/source_maintenance/limits/retained_source_generations",
                expected: u64::from(
                    source_manifest::SOURCE_MAINTENANCE_RETAINED_SOURCE_GENERATION_LIMIT,
                ),
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/contract_id",
                expected: source_manifest::SOURCE_MAINTENANCE_CONTRACT_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/hook_id",
                expected: source_manifest::SOURCE_MAINTENANCE_HOOK_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/predecessor/hook_id",
                expected: source_manifest::SOURCE_MAINTENANCE_PREDECESSOR_HOOK_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/predecessor/manifest/sha256",
                expected: source_manifest::SOURCE_MAINTENANCE_PREDECESSOR_MANIFEST_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/name",
                expected: source_manifest::SOURCE_MAINTENANCE_MIGRATION_NAME,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/up/sha256",
                expected: source_manifest::SOURCE_MAINTENANCE_MIGRATION_UP_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/down/sha256",
                expected: source_manifest::SOURCE_MAINTENANCE_MIGRATION_DOWN_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/migration/schema_sha256",
                expected: source_manifest::SOURCE_MAINTENANCE_SCHEMA_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/source_maintenance/capacity_authority_id",
                expected: source_manifest::SOURCE_MAINTENANCE_CAPACITY_AUTHORITY_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/source_maintenance/accounting/algorithm",
                expected: source_manifest::SOURCE_MAINTENANCE_ACCOUNTING_ALGORITHM,
            },
            GeneratedManifestMetadataAxis::JsonStringArray {
                pointer: "/migration/catalog/replaced_objects",
                expected: source_manifest::SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES,
            },
            GeneratedManifestMetadataAxis::JsonStringArray {
                pointer: "/source_maintenance/accounting/raw_event_columns",
                expected: source_manifest::SOURCE_MAINTENANCE_RAW_EVENT_COLUMNS,
            },
            GeneratedManifestMetadataAxis::JsonStringArray {
                pointer: "/source_maintenance/accounting/raw_tag_columns",
                expected: source_manifest::SOURCE_MAINTENANCE_RAW_TAG_COLUMNS,
            },
            GeneratedManifestMetadataAxis::JsonStringArray {
                pointer: "/source_maintenance/accounting/nullable_raw_tag_columns",
                expected: source_manifest::SOURCE_MAINTENANCE_NULLABLE_RAW_TAG_COLUMNS,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/result_vector/sha256",
                expected: source_manifest::SOURCE_MAINTENANCE_RESULT_VECTOR_SHA256,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/result_vector/executor_id",
                expected: source_manifest::SOURCE_MAINTENANCE_RESULT_VECTOR_EXECUTOR_ID,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/result_vector/executor_sha256",
                expected: source_manifest::SOURCE_MAINTENANCE_RESULT_VECTOR_EXECUTOR_SHA256,
            },
        ],
        "generated source-maintenance manifest metadata is inconsistent",
    )?;
    for (field, digest) in [
        (
            "migration up",
            source_manifest::SOURCE_MAINTENANCE_MIGRATION_UP_SHA256,
        ),
        (
            "migration down",
            source_manifest::SOURCE_MAINTENANCE_MIGRATION_DOWN_SHA256,
        ),
        ("schema", source_manifest::SOURCE_MAINTENANCE_SCHEMA_SHA256),
        (
            "predecessor manifest",
            source_manifest::SOURCE_MAINTENANCE_PREDECESSOR_MANIFEST_SHA256,
        ),
        (
            "result vector",
            source_manifest::SOURCE_MAINTENANCE_RESULT_VECTOR_SHA256,
        ),
        (
            "result-vector executor",
            source_manifest::SOURCE_MAINTENANCE_RESULT_VECTOR_EXECUTOR_SHA256,
        ),
    ] {
        validate_sha256_literal(
            source_manifest::SOURCE_MAINTENANCE_MIGRATION_VERSION,
            field,
            digest,
        )?;
    }
    Ok(())
}

fn validate_owned_schema_name(
    version: u32,
    object_kind: &'static str,
    name: &str,
) -> Result<(), RadrootsEventStoreError> {
    if name.is_empty()
        || name == EVENT_STORE_LEDGER_NAME
        || name.to_ascii_lowercase().starts_with("sqlite_")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: format!(
                "migration version {version} has invalid owned {object_kind} name `{name}`"
            ),
        });
    }
    Ok(())
}

fn validate_embedded_migration_input(
    version: u32,
    direction: &'static str,
    sql: &str,
    expected_len: usize,
    expected_sha256: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    if sql.len() != expected_len {
        return Err(RadrootsEventStoreError::EmbeddedMigrationLengthMismatch {
            version,
            direction,
            expected: expected_len,
            actual: sql.len(),
        });
    }
    validate_sha256_literal(version, direction, expected_sha256)?;
    let actual = sha256_hex(sql.as_bytes());
    if actual != expected_sha256 {
        return Err(RadrootsEventStoreError::EmbeddedMigrationChecksumMismatch {
            version,
            direction,
            expected: expected_sha256,
            actual,
        });
    }
    Ok(())
}

fn validate_sha256_literal(
    version: u32,
    field: &'static str,
    value: &str,
) -> Result<(), RadrootsEventStoreError> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: format!("migration version {version} has an invalid {field} SHA-256 literal"),
        });
    }
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod migration_framework {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[derive(Debug, PartialEq, Eq)]
    struct DiscoveredMigration {
        version: u32,
        name: String,
        up: PathBuf,
        down: PathBuf,
    }

    const FROZEN_V1_UP_LEN: usize = 10_712;
    const FROZEN_V1_DOWN_LEN: usize = 522;
    const FROZEN_V1_UP_SHA256: &str =
        "4c03906a1cffd418a48d40907aa9a1ca51bb41766cff7250c4dfc7c2fd6eddde";
    const FROZEN_V1_DOWN_SHA256: &str =
        "fa84d587f657f601947eaeb9cd239c962a48f6fcdce723588476e8d22f3c1f53";
    const FROZEN_V1_SCHEMA_SHA256: &str =
        "5b1f92779640f1a2dbd75e37a96996bda6c8be58883190f69eb3eced22a48f03";
    const FROZEN_V1_OBJECT_COUNT: usize = 46;

    #[test]
    fn generated_manifest_metadata_axis_inventory_is_closed() {
        const REASON: &str = "generated manifest metadata is inconsistent";
        let manifest = serde_json::json!({
            "number": 7,
            "string": "value",
            "numbers": [1, 2],
            "strings": ["a", "b"],
            "wrong_type": false,
        });
        validate_generated_manifest_metadata(
            &manifest,
            &[
                GeneratedManifestMetadataAxis::JsonU64 {
                    pointer: "/number",
                    expected: 7,
                },
                GeneratedManifestMetadataAxis::JsonString {
                    pointer: "/string",
                    expected: "value",
                },
                GeneratedManifestMetadataAxis::JsonU64Array {
                    pointer: "/numbers",
                    expected: &[1, 2],
                },
                GeneratedManifestMetadataAxis::JsonStringArray {
                    pointer: "/strings",
                    expected: &["a", "b"],
                },
                GeneratedManifestMetadataAxis::U64 {
                    actual: 1,
                    expected: 1,
                },
                GeneratedManifestMetadataAxis::PositiveI64 { actual: 1 },
                GeneratedManifestMetadataAxis::String {
                    actual: "value",
                    expected: "value",
                },
                GeneratedManifestMetadataAxis::U32Array {
                    actual: &[1, 2],
                    expected: &[1, 2],
                },
                GeneratedManifestMetadataAxis::StringArray {
                    actual: &["a", "b"],
                    expected: &["a", "b"],
                },
            ],
            REASON,
        )
        .expect("matching metadata inventory");

        for axis in [
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/missing",
                expected: 7,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/wrong_type",
                expected: 7,
            },
            GeneratedManifestMetadataAxis::JsonU64 {
                pointer: "/number",
                expected: 8,
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/missing",
                expected: "value",
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/wrong_type",
                expected: "value",
            },
            GeneratedManifestMetadataAxis::JsonString {
                pointer: "/string",
                expected: "other",
            },
            GeneratedManifestMetadataAxis::JsonU64Array {
                pointer: "/missing",
                expected: &[1, 2],
            },
            GeneratedManifestMetadataAxis::JsonU64Array {
                pointer: "/numbers",
                expected: &[1],
            },
            GeneratedManifestMetadataAxis::JsonU64Array {
                pointer: "/numbers",
                expected: &[1, 3],
            },
            GeneratedManifestMetadataAxis::JsonStringArray {
                pointer: "/missing",
                expected: &["a", "b"],
            },
            GeneratedManifestMetadataAxis::JsonStringArray {
                pointer: "/strings",
                expected: &["a"],
            },
            GeneratedManifestMetadataAxis::JsonStringArray {
                pointer: "/strings",
                expected: &["a", "c"],
            },
            GeneratedManifestMetadataAxis::U64 {
                actual: 1,
                expected: 2,
            },
            GeneratedManifestMetadataAxis::PositiveI64 { actual: 0 },
            GeneratedManifestMetadataAxis::String {
                actual: "value",
                expected: "other",
            },
            GeneratedManifestMetadataAxis::U32Array {
                actual: &[1, 2],
                expected: &[1, 3],
            },
            GeneratedManifestMetadataAxis::StringArray {
                actual: &["a", "b"],
                expected: &["a", "c"],
            },
        ] {
            assert!(matches!(
                validate_generated_manifest_metadata(
                    &manifest,
                    std::slice::from_ref(&axis),
                    REASON,
                ),
                Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                    if reason == REASON
            ));
        }
    }

    #[test]
    fn generated_manifest_decoding_and_ranges_are_typed() {
        assert_eq!(
            parse_generated_manifest(br#"{"schema_version":1}"#, "fixture")
                .expect("valid generated manifest")["schema_version"],
            1,
        );
        assert!(matches!(
            parse_generated_manifest(b"{", "fixture"),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason.starts_with("generated fixture manifest JSON is invalid:")
        ));

        let valid_bytes = br#"{"schema_version":1}"#;
        let valid_sha256 = sha256_hex(valid_bytes);
        validate_generated_manifest_envelope(GeneratedManifestEnvelope {
            name: "fixture",
            bytes: valid_bytes,
            expected_byte_length: valid_bytes.len(),
            migration_version: 1,
            expected_sha256: valid_sha256.as_str(),
            byte_length_reason: "fixture length",
            digest_reason: "fixture digest",
        })
        .expect("valid generated manifest envelope");
        assert!(matches!(
            validate_generated_manifest_envelope(GeneratedManifestEnvelope {
                name: "fixture",
                bytes: valid_bytes,
                expected_byte_length: valid_bytes.len() + 1,
                migration_version: 1,
                expected_sha256: valid_sha256.as_str(),
                byte_length_reason: "fixture length",
                digest_reason: "fixture digest",
            }),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason == "fixture length"
        ));
        let other_sha256 = sha256_hex(b"{}");
        assert!(matches!(
            validate_generated_manifest_envelope(GeneratedManifestEnvelope {
                name: "fixture",
                bytes: valid_bytes,
                expected_byte_length: valid_bytes.len(),
                migration_version: 1,
                expected_sha256: other_sha256.as_str(),
                byte_length_reason: "fixture length",
                digest_reason: "fixture digest",
            }),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason == "fixture digest"
        ));
        assert!(matches!(
            validate_generated_manifest_envelope(GeneratedManifestEnvelope {
                name: "fixture",
                bytes: valid_bytes,
                expected_byte_length: valid_bytes.len(),
                migration_version: 1,
                expected_sha256: "invalid",
                byte_length_reason: "fixture length",
                digest_reason: "fixture digest",
            }),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason.contains("invalid hook manifest SHA-256 literal")
        ));
        let invalid_json = b"{";
        let invalid_json_sha256 = sha256_hex(invalid_json);
        assert!(matches!(
            validate_generated_manifest_envelope(GeneratedManifestEnvelope {
                name: "fixture",
                bytes: invalid_json,
                expected_byte_length: invalid_json.len(),
                migration_version: 1,
                expected_sha256: invalid_json_sha256.as_str(),
                byte_length_reason: "fixture length",
                digest_reason: "fixture digest",
            }),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason.starts_with("generated fixture manifest JSON is invalid:")
        ));

        assert_eq!(
            generated_manifest_u128_to_u64(u128::from(u64::MAX), "fixture u128 range")
                .expect("maximum u64"),
            u64::MAX,
        );
        assert!(matches!(
            generated_manifest_u128_to_u64(
                u128::from(u64::MAX) + 1,
                "fixture u128 range",
            ),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason == "fixture u128 range"
        ));
        assert_eq!(
            generated_manifest_i64_to_u64(i64::MAX, "fixture i64 range").expect("maximum i64"),
            i64::MAX as u64,
        );
        assert!(matches!(
            generated_manifest_i64_to_u64(-1, "fixture i64 range"),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason == "fixture i64 range"
        ));
    }

    fn assert_registry_defect(
        registry: &[EventStoreMigration],
        minimum: u32,
        current: u32,
        expected: &str,
    ) {
        let error = validate_migration_registry(registry, minimum, current)
            .expect_err("migration registry defect");
        assert!(
            matches!(&error, RadrootsEventStoreError::MigrationRegistryDefect { reason } if reason.contains(expected)),
            "unexpected registry error: {error}"
        );
    }

    fn discover_migration_directory(directory: &Path) -> Result<Vec<DiscoveredMigration>, String> {
        let directory_metadata = fs::symlink_metadata(directory).map_err(|error| {
            format!(
                "cannot inspect migration directory {}: {error}",
                directory.display()
            )
        })?;
        if directory_metadata.file_type().is_symlink() {
            return Err(format!(
                "migration directory must not be a symlink: {}",
                directory.display()
            ));
        }
        if !directory_metadata.is_dir() {
            return Err(format!(
                "migration source is not a directory: {}",
                directory.display()
            ));
        }
        let canonical_directory = fs::canonicalize(directory).map_err(|error| {
            format!(
                "cannot canonicalize migration directory {}: {error}",
                directory.display()
            )
        })?;
        let mut paths = Vec::new();
        for entry in fs::read_dir(directory)
            .map_err(|error| format!("cannot read migration directory: {error}"))?
        {
            let entry = entry.map_err(|error| format!("cannot read migration entry: {error}"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                format!(
                    "cannot inspect migration source {}: {error}",
                    path.display()
                )
            })?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "migration source must not be a symlink: {}",
                    path.display()
                ));
            }
            if !metadata.is_file() {
                return Err(format!(
                    "migration source is not a regular file: {}",
                    path.display()
                ));
            }
            let canonical_path = fs::canonicalize(&path).map_err(|error| {
                format!(
                    "cannot canonicalize migration source {}: {error}",
                    path.display()
                )
            })?;
            if canonical_path.parent() != Some(canonical_directory.as_path()) {
                return Err(format!(
                    "migration source escapes its directory: {}",
                    path.display()
                ));
            }
            paths.push(path);
        }
        discover_migration_files(paths)
    }

    fn discover_migration_files(
        paths: impl IntoIterator<Item = PathBuf>,
    ) -> Result<Vec<DiscoveredMigration>, String> {
        let mut discovered = BTreeMap::<(u32, String), (Option<PathBuf>, Option<PathBuf>)>::new();
        for path in paths {
            let filename = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("migration path is not valid UTF-8: {}", path.display()))?;
            let (stem, direction) = if let Some(stem) = filename.strip_suffix(".up.sql") {
                (stem, "up")
            } else if let Some(stem) = filename.strip_suffix(".down.sql") {
                (stem, "down")
            } else {
                return Err(format!("unsupported migration filename `{filename}`"));
            };
            let (version, name) = stem
                .split_once('_')
                .ok_or_else(|| format!("migration filename `{filename}` has no name"))?;
            if version.len() != 4 || !version.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!(
                    "migration filename `{filename}` has an invalid version"
                ));
            }
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            {
                return Err(format!(
                    "migration filename `{filename}` has an invalid name"
                ));
            }
            let version = version
                .parse::<u32>()
                .map_err(|error| format!("invalid migration version: {error}"))?;
            let pair = discovered.entry((version, name.to_owned())).or_default();
            let slot = if direction == "up" {
                &mut pair.0
            } else {
                &mut pair.1
            };
            if slot.replace(path).is_some() {
                return Err(format!(
                    "duplicate {direction} migration for version {version}"
                ));
            }
        }

        let mut migrations = Vec::with_capacity(discovered.len());
        for (expected_version, ((version, name), (up, down))) in
            (RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN..).zip(discovered)
        {
            if version != expected_version {
                return Err(format!(
                    "migration version gap: expected {expected_version}, found {version}"
                ));
            }
            let up = up.ok_or_else(|| format!("migration version {version} is missing up SQL"))?;
            let down =
                down.ok_or_else(|| format!("migration version {version} is missing down SQL"))?;
            migrations.push(DiscoveredMigration {
                version,
                name,
                up,
                down,
            });
        }
        Ok(migrations)
    }

    #[test]
    fn embedded_registry_is_contiguous_and_byte_pinned() {
        validate_embedded_migration_registry().expect("valid registry");
        assert_eq!(EVENT_STORE_MIGRATIONS.len(), 4);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].version, 1);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].name, "event_store");
        assert_eq!(EVENT_STORE_MIGRATIONS[0].up_len, FROZEN_V1_UP_LEN);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].down_len, FROZEN_V1_DOWN_LEN);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].up_sha256, FROZEN_V1_UP_SHA256);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].down_sha256, FROZEN_V1_DOWN_SHA256);
        assert_eq!(
            EVENT_STORE_MIGRATIONS[0].schema_sha256,
            FROZEN_V1_SCHEMA_SHA256
        );
        assert_eq!(
            EVENT_STORE_MIGRATIONS[0].owned_object_names.len(),
            FROZEN_V1_OBJECT_COUNT
        );
        assert_eq!(EVENT_STORE_MIGRATIONS[1].version, 2);
        assert_eq!(EVENT_STORE_MIGRATIONS[1].name, "nip09");
        assert_eq!(
            EVENT_STORE_MIGRATIONS[1].hook,
            EventStoreMigrationHook::Nip09ReconciliationV1
        );
        assert_eq!(
            EVENT_STORE_MIGRATIONS[1].event_contract_registry_version,
            Some(nip09_manifest::NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION)
        );
        assert_eq!(EVENT_STORE_MIGRATIONS[2].version, 3);
        assert_eq!(
            EVENT_STORE_MIGRATIONS[2].name,
            "food_availability_projection"
        );
        assert_eq!(
            EVENT_STORE_MIGRATIONS[2].hook,
            EventStoreMigrationHook::FoodAvailabilityProjectionV1
        );
        assert_eq!(
            EVENT_STORE_MIGRATIONS[2].event_contract_registry_version,
            Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION)
        );
        assert_eq!(EVENT_STORE_MIGRATIONS[3].version, 4);
        assert_eq!(EVENT_STORE_MIGRATIONS[3].name, "source_maintenance");
        assert_eq!(
            EVENT_STORE_MIGRATIONS[3].hook,
            EventStoreMigrationHook::SourceMaintenanceV1
        );
        assert_eq!(
            EVENT_STORE_MIGRATIONS[3].event_contract_registry_version,
            Some(source_maintenance_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,)
        );
    }

    #[test]
    fn registry_validator_rejects_each_structural_mutation() {
        const ZERO_SHA256: &str =
            "0000000000000000000000000000000000000000000000000000000000000000";
        const RESERVED_OBJECT: &[&str] = &["radroots_event_store_fixture"];
        const OTHER_RESERVED_OBJECT: &[&str] = &["radroots_event_store_other_fixture"];
        const DUPLICATE_OBJECTS: &[&str] = &[
            "radroots_event_store_fixture",
            "radroots_event_store_fixture",
        ];
        const ORDINARY_REPLACEMENT: &[&str] = &["event_envelope_kind_created_idx"];

        let baseline = EVENT_STORE_MIGRATIONS[0];
        assert_registry_defect(&[baseline], 0, 1, "non-empty positive registry");
        assert_registry_defect(&[baseline], 2, 1, "non-empty positive registry");
        assert_registry_defect(&[], 1, 1, "non-empty positive registry");

        let mut mutated = baseline;
        mutated.version = 2;
        assert_registry_defect(&[mutated], 1, 1, "expected migration version 1");

        mutated = baseline;
        mutated.name = "";
        assert_registry_defect(&[mutated], 1, 1, "empty name");

        let mut second = baseline;
        second.version = 2;
        second.owned_object_names = RESERVED_OBJECT;
        second.owned_table_names = RESERVED_OBJECT;
        assert_registry_defect(
            &[baseline, second],
            1,
            2,
            "migration name `event_store` is duplicated",
        );

        mutated = baseline;
        mutated.owned_object_names = &[];
        mutated.owned_table_names = &[];
        mutated.fts5_table_names = &[];
        assert_registry_defect(&[mutated], 1, 1, "declares no owned schema objects");

        mutated = baseline;
        mutated.owned_object_names = DUPLICATE_OBJECTS;
        mutated.owned_table_names = &[];
        mutated.fts5_table_names = &[];
        assert_registry_defect(&[mutated], 1, 1, "owned schema object");

        mutated = baseline;
        mutated.owned_table_names = &["event_envelopes", "event_envelopes"];
        mutated.fts5_table_names = &[];
        assert_registry_defect(&[mutated], 1, 1, "owned schema table");

        second.name = "fixture";
        second.owned_object_names = &["ordinary_fixture"];
        second.owned_table_names = &[];
        assert_registry_defect(&[baseline, second], 1, 2, "outside the reserved");

        second.owned_object_names = RESERVED_OBJECT;
        second.owned_table_names = OTHER_RESERVED_OBJECT;
        assert_registry_defect(&[baseline, second], 1, 2, "not also an owned object");

        second.owned_object_names = &[
            "radroots_event_store_fixture",
            "radroots_event_store_fixture_fts",
        ];
        second.owned_table_names = RESERVED_OBJECT;
        second.fts5_table_names = &["radroots_event_store_fixture_fts"];
        assert_registry_defect(&[baseline, second], 1, 2, "not also an owned table");

        let mut replacement = EVENT_STORE_MIGRATIONS[3];
        replacement.replaced_object_names = ORDINARY_REPLACEMENT;
        assert_registry_defect(
            &[
                EVENT_STORE_MIGRATIONS[0],
                EVENT_STORE_MIGRATIONS[1],
                EVENT_STORE_MIGRATIONS[2],
                replacement,
            ],
            1,
            4,
            "replacement object `event_envelope_kind_created_idx` is outside",
        );

        mutated = baseline;
        mutated.up_len += 1;
        assert!(matches!(
            validate_migration_registry(&[mutated], 1, 1),
            Err(RadrootsEventStoreError::EmbeddedMigrationLengthMismatch { .. })
        ));

        mutated = baseline;
        mutated.up_sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        assert_registry_defect(&[mutated], 1, 1, "invalid up SHA-256 literal");

        mutated = baseline;
        mutated.up_sha256 = ZERO_SHA256;
        assert!(matches!(
            validate_migration_registry(&[mutated], 1, 1),
            Err(RadrootsEventStoreError::EmbeddedMigrationChecksumMismatch { .. })
        ));

        mutated = baseline;
        mutated.schema_sha256 = "short";
        assert_registry_defect(&[mutated], 1, 1, "invalid schema SHA-256 literal");

        mutated = baseline;
        mutated.hook_manifest_sha256 = Some(ZERO_SHA256);
        assert_registry_defect(&[mutated], 1, 1, "invalid `none` hook manifest identity");

        mutated = baseline;
        mutated.event_contract_registry_version = Some(1);
        assert_registry_defect(&[mutated], 1, 1, "declares unsupported manifest");

        assert_registry_defect(&[baseline], 1, 2, "declared current version is 2");

        for invalid_name in ["", EVENT_STORE_LEDGER_NAME, "sqlite_fixture", "Invalid"] {
            mutated = baseline;
            mutated.owned_object_names = match invalid_name {
                "" => &[""],
                value if value == EVENT_STORE_LEDGER_NAME => &[EVENT_STORE_LEDGER_NAME],
                "sqlite_fixture" => &["sqlite_fixture"],
                _ => &["Invalid"],
            };
            mutated.owned_table_names = &[];
            mutated.fts5_table_names = &[];
            assert_registry_defect(&[mutated], 1, 1, "invalid owned object name");
        }

        mutated = baseline;
        mutated.version = u32::MAX;
        assert_registry_defect(&[mutated], u32::MAX, u32::MAX, "migration version overflow");
    }

    #[test]
    fn governed_name_matching_uses_sqlite_ascii_identifier_semantics() {
        for name in [
            "event_envelopes",
            "EVENT_ENVELOPES",
            "EvEnT_EnVeLoPeS",
            "RADROOTS_EVENT_STORE_CALLER_PROBE",
            "RaDrOoTs_EvEnT_StOrE_caller_probe",
        ] {
            assert!(is_event_store_owned_table_name(
                EVENT_STORE_MIGRATIONS,
                name
            ));
            assert!(is_event_store_governed_schema_name(
                EVENT_STORE_MIGRATIONS,
                name
            ));
        }
        assert!(is_event_store_governed_schema_name(
            EVENT_STORE_MIGRATIONS,
            "RADROOTS_EVENT_STORE_SCHEMA_MIGRATIONS"
        ));
        assert!(!is_event_store_governed_schema_name(
            EVENT_STORE_MIGRATIONS,
            "event_envelopes_caller"
        ));
        assert!(!is_event_store_governed_schema_name(
            EVENT_STORE_MIGRATIONS,
            "évent_envelopes"
        ));
    }

    #[test]
    fn migration_directory_exactly_matches_embedded_registry() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let discovered =
            discover_migration_directory(&directory).expect("bounded migration discovery");

        assert_eq!(discovered.len(), EVENT_STORE_MIGRATIONS.len());
        for (disk, embedded) in discovered.iter().zip(EVENT_STORE_MIGRATIONS) {
            assert_eq!(disk.version, embedded.version);
            assert_eq!(disk.name, embedded.name);
            let up = fs::read(&disk.up).expect("up migration bytes");
            let down = fs::read(&disk.down).expect("down migration bytes");
            assert_eq!(up.len(), embedded.up_len);
            assert_eq!(down.len(), embedded.down_len);
            assert_eq!(sha256_hex(&up), embedded.up_sha256);
            assert_eq!(sha256_hex(&down), embedded.down_sha256);
        }

        let frozen_v1 = discovered
            .iter()
            .find(|migration| migration.version == 1)
            .expect("frozen v1 migration");
        let frozen_v1_up = fs::read(&frozen_v1.up).expect("frozen v1 up bytes");
        let frozen_v1_down = fs::read(&frozen_v1.down).expect("frozen v1 down bytes");
        assert_eq!(frozen_v1_up.len(), FROZEN_V1_UP_LEN);
        assert_eq!(frozen_v1_down.len(), FROZEN_V1_DOWN_LEN);
        assert_eq!(sha256_hex(&frozen_v1_up), FROZEN_V1_UP_SHA256);
        assert_eq!(sha256_hex(&frozen_v1_down), FROZEN_V1_DOWN_SHA256);
    }

    #[test]
    fn migration_discovery_rejects_missing_pairs_gaps_and_invalid_names() {
        let missing_pair =
            discover_migration_files([PathBuf::from("migrations/0001_event_store.up.sql")])
                .expect_err("missing pair");
        assert!(missing_pair.contains("missing down SQL"));

        let gap = discover_migration_files([
            PathBuf::from("migrations/0001_event_store.up.sql"),
            PathBuf::from("migrations/0001_event_store.down.sql"),
            PathBuf::from("migrations/0003_future.up.sql"),
            PathBuf::from("migrations/0003_future.down.sql"),
        ])
        .expect_err("gap");
        assert!(gap.contains("expected 2, found 3"));

        let invalid = discover_migration_files([PathBuf::from("migrations/1_event-store.up.sql")])
            .expect_err("invalid filename");
        assert!(invalid.contains("invalid version"));
    }

    #[test]
    fn migration_directory_rejects_extra_and_nonregular_sources() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let directory = tempdir.path().join("migrations");
        fs::create_dir(&directory).expect("migration directory");
        fs::write(directory.join("0001_event_store.up.sql"), "SELECT 1;").expect("up source");
        fs::write(directory.join("0001_event_store.down.sql"), "SELECT 1;").expect("down source");
        fs::write(directory.join("README"), "unexpected").expect("extra source");

        let extra = discover_migration_directory(&directory).expect_err("extra source");
        assert!(extra.contains("unsupported migration filename"));

        fs::remove_file(directory.join("README")).expect("remove extra source");
        fs::create_dir(directory.join("0002_future.up.sql")).expect("directory source");
        let nonregular = discover_migration_directory(&directory).expect_err("nonregular source");
        assert!(nonregular.contains("not a regular file"));
    }

    #[cfg(unix)]
    #[test]
    fn migration_directory_rejects_symlinked_roots_and_files() {
        use std::os::unix::fs::symlink;

        let tempdir = tempfile::tempdir().expect("tempdir");
        let directory = tempdir.path().join("migrations");
        fs::create_dir(&directory).expect("migration directory");
        let outside = tempdir.path().join("outside.sql");
        fs::write(&outside, "SELECT 1;").expect("outside source");
        symlink(&outside, directory.join("0001_event_store.up.sql")).expect("source symlink");

        let source_error =
            discover_migration_directory(&directory).expect_err("source symlink rejected");
        assert!(source_error.contains("must not be a symlink"));

        let linked_directory = tempdir.path().join("linked-migrations");
        symlink(&directory, &linked_directory).expect("directory symlink");
        let directory_error = discover_migration_directory(&linked_directory)
            .expect_err("directory symlink rejected");
        assert!(directory_error.contains("directory must not be a symlink"));
    }
}
