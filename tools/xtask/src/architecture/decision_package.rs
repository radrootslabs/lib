//! Prospective shared SDK authority. Validation does not claim implementation.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::Deserialize;

const CONTRACT: &str = "contracts/architecture/decisions/shared_sdk_refactor.v1.json";
const MAX_BYTES: usize = 65_536;

mod currency_registry;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: u16,
    contract_id: String,
    state: String,
    publication_authorized: bool,
    toolchain: Toolchain,
    ownership: Ownership,
    roles: Roles,
    deferred_application_migrations: Vec<String>,
    retained_capability_families: Vec<String>,
    independence: Independence,
    local_outbox_authority: LocalOutboxAuthority,
    native_seller_stock_authority: NativeSellerStockAuthority,
    provenance: Provenance,
    targets: Targets,
    portability: Portability,
    packages: Vec<Package>,
    values: Values,
    limits: BTreeMap<String, u64>,
    stock_accounting: StockAccounting,
    execution: Execution,
    protocol: Protocol,
    cutover: Cutover,
    qualification: Qualification,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockAccounting {
    value_owner: String,
    scope: String,
    obligation_equation: String,
    ticks_max: u64,
    values_create_store_effects: bool,
    deficit_counts_as_physical_stock: bool,
    receipt_consumes_again: bool,
    released_deficit_adds_available: bool,
    loss_deletes_obligation: bool,
    native_atomic_allocation_and_identity_required: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Toolchain {
    edition: String,
    stable: String,
    msrv: String,
    historical_receipts_immutable: bool,
    dedicated_nightly_and_sp1_separate: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ownership {
    shared_repository: String,
    ordinary_facade: String,
    native_bindings: String,
    browser_artifact: String,
    sdk_repository_role: String,
    app_owns_shared_semantics: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Roles {
    cli: Vec<String>,
    tera: Vec<String>,
    browser: Vec<String>,
    generic_ffi: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Independence {
    hosted_account_required: bool,
    farm_hierarchy_required: bool,
    rhi_required: bool,
    payment_implementation_in_scope: bool,
    seller_owner_executor_optional_to_single_device: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalOutboxAuthority {
    scopes: Vec<String>,
    seller_enrollment_explicit: bool,
    scope_retained_in_authenticated_binding_and_preparation: bool,
    legacy_preparation_v2_scope: String,
    scope_separates_route_genesis_and_credential_commitments: bool,
    coverage: String,
    stock_authority: bool,
    organization_or_delegation_authority: bool,
    existing_order_or_history_recovery: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockCapabilityContract {
    id: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockEnvelopeRetention {
    journal_family: u32,
    payload_version: u32,
    payload_prefix_bytes: u32,
    supported_catalogs: Vec<u32>,
    retained_unsupported_catalogs: Vec<u32>,
    raw_max_bytes: u32,
    worker_charge_bytes: u32,
    capture_port: String,
    query_port: String,
    recovery_port: String,
    original_preparation_plan_claim_seal_installation_required: bool,
    temporary_key_derivation_and_exact_decryption_required: bool,
    temporary_key_retained: bool,
    original_party_as_outer_author_allowed: bool,
    body_derived_projection_and_complete_ancestry_required: bool,
    original_credential_capture_requires_live_clock_or_grant: bool,
    query_requires_current_recovery_access: bool,
    observation_nullable: bool,
    existing_original_bytes_may_be_replaced: bool,
    capture_grants_dispatch: bool,
    capture_changes_physical_stock: bool,
    provider_io_inside_sql: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockRewrapping {
    journal_family: u16,
    admission_journal_family: u16,
    admission_payload_bytes: u16,
    admission_window_seconds_max: u16,
    admission_port: String,
    admission_recovery_port: String,
    admission_query_port: String,
    explicit_request_id_required: bool,
    newly_confirmed_admission_before_wrapping: bool,
    existing_or_uncertain_admission_grants_invocation: bool,
    unknown_admissions_count_toward_limit: bool,
    descriptor_includes_uncaptured_admissions: bool,
    admission_repair_grants_history_or_invocation: bool,
    ordinary_owner: String,
    ordinary_facade_methods: Vec<String>,
    ordinary_resume_generates_key_or_business_signature: bool,
    ordinary_status_is_atomic_absence_proof: bool,

    payload_version: u16,
    payload_prefix_bytes: u32,
    supported_catalogs: Vec<u16>,
    retained_unsupported_catalogs: Vec<u16>,
    raw_max_bytes: u32,
    worker_charge_bytes: u32,
    additional_envelopes_per_recipient_max: u16,
    descriptor_page_max: u16,
    capture_port: String,
    recovery_port: String,
    query_port: String,
    page_port: String,
    original_f10_and_same_s_t2_required: bool,
    default_selection_is_original: bool,
    new_envelope_selection_explicit: bool,
    first_policy_source_bound: bool,
    transport_counters_cumulative: bool,
    outcomes_scoped_to_exact_envelope: bool,
    natural_index_transaction_witness_required: bool,
    existing_bytes_may_be_replaced: bool,
    original_capture_requires_live_clock_or_grant: bool,
    capture_or_query_grants_dispatch: bool,
    physical_stock_effect: bool,
    temporary_key_retained: bool,
    provider_io_inside_sql: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeSellerStockAuthority {
    rewrapping: StockRewrapping,
    delivery: StockDelivery,
    envelope_retention: StockEnvelopeRetention,
    qualified_local_recovery: QualifiedLocalRecovery,
    source_payload_schemas: Vec<String>,
    source_profile_dispositions: Vec<String>,
    source_owner_administration_required: bool,
    inventory_scope_grants_source_access: bool,
    source_checkpoint_independent: bool,
    source_profile_implies_actionability: bool,
    causal_history_current_administrator_required: bool,
    causal_history_complete_local_scope_required: bool,
    causal_history_includes_actual_local_signed_records: bool,
    causal_history_inventory_minimum_required: bool,
    causal_history_logical_record_deduplication_required: bool,
    causal_history_implies_installation: bool,
    causal_history_uninterpreted_policy: String,
    causal_history_transaction_witnesses_required: bool,
    causal_history_implies_business_permission: bool,
    source_carrier_max_bytes: u32,
    source_chunk_max_bytes: u32,
    mapping_payload_schema: String,
    mapping_owner_administration_required: bool,
    mapping_changes_physical_stock: bool,
    mapping_changes_pool_version: bool,
    mapping_create_expected_version: u16,
    capability_contract: StockCapabilityContract,
    scope: String,
    database: String,
    lock: String,
    schema_version: u16,
    retained_enrollment_versions: Vec<u16>,
    retained_inventory_versions: Vec<u16>,
    authority_payload_schema: String,
    public_source_payload_schemas: Vec<String>,
    public_source_owner_administration_required: bool,
    inventory_scope_grants_public_source_access: bool,
    public_source_checkpoint_independent: bool,
    public_source_input_requires_signature_evidence: bool,
    public_source_read_reverifies_signature: bool,
    public_source_global_completeness_claimed: bool,
    public_source_profile_implies_actionability: bool,
    public_source_head_policy: String,
    public_source_duplicate_encodings: String,
    public_source_raw_max_bytes: u32,
    public_source_part_max_bytes: u32,
    public_source_part_max_count: u16,
    public_source_worker_charge_bytes: u32,
    inventory_operation_payload_schema: String,
    operation_part_payload_schema: String,
    operation_body_max_bytes: u32,
    operation_part_max_bytes: u32,
    operation_part_max_count: u16,
    operation_identity_requires_protected_ancestry: bool,
    acceptance_preparation: AcceptancePreparation,
    signing_plan_binding: SigningPlanBinding,
    signing_issuance: SigningIssuance,
    initial_agreement_installation: InitialAgreementInstallation,
    preparation_abort: PreparationAbort,
    business_consent_payload_version: u16,
    business_consent_action: String,
    business_consent_modes: Vec<String>,
    business_consent_admin_issuance_required: bool,
    business_consent_binds_exact_command_and_subject_grant: bool,
    business_consent_issuance_has_order_or_stock_effects: bool,
    business_consent_worker_charge_bytes: u32,
    inventory_movement_payload_schema: String,
    grant_payload_schema: String,
    grant_change_payload_schema: String,
    namespace_excludes_caller_grant: bool,
    actor_operation_unique_across_grants: bool,
    grant_checkpoint_minimum_required: bool,
    grant_checkpoint_policy: String,
    grant_set_max: u16,
    grant_pool_scope_max: u16,
    grant_action_max: u16,
    grant_scope_actions: Vec<String>,
    grant_write_requires_receipt_and_recovery_read: bool,
    inventory_scope_grants_administration: bool,
    grant_admin_prepared_transaction_required: bool,
    retained_grants_automatically_evicted: bool,
    old_self_revoke_api_supported: bool,
    pool_tick_scale_explicit: bool,
    pool_tick_scale_encoding: String,
    inventory_checkpoint_minimum_required: bool,
    empty_pool_has_physical_movement: bool,
    enrollment_prepared_before_durable_creation: bool,
    prepared_enrollment_single_use: bool,
    same_connection_owner_as_outbox: bool,
    protected_authority_and_grants_separate: bool,
    automatic_outbox_migration: bool,
    historical_obligations_recovered: bool,
    business_effects_qualified: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockDelivery {
    supported_catalogs: Vec<u16>,
    retained_unsupported_catalogs: Vec<u16>,
    attempt_journal_family: u16,
    observation_journal_family: u16,
    attempt_payload_version: u16,
    observation_payload_version: u16,
    attempt_body_max_bytes: u32,
    observation_body_max_bytes: u32,
    target_max: u16,
    attempts_per_target: u16,
    attempts_per_operation_max: u16,
    observations_per_operation_max: u16,
    observation_page_max: u16,
    summary_target_max: u16,
    worker_charge_bytes: u32,
    storage_ports: Vec<String>,
    original_stock_evidence_installation_outer_required: bool,
    original_captured_route_and_current_write_policy_required: bool,
    fresh_current_scoped_consent_grant_clock_required: bool,
    installed_original_recovery_delivery_supported: bool,
    fresh_claim_requires_confirmed_commit: bool,
    existing_attempt_grants_dispatch: bool,
    unknown_commit_grants_dispatch: bool,
    query_grants_dispatch: bool,
    first_complete_targets_and_criterion_immutable: bool,
    ordinary_command_replays_existing_attempt: bool,
    ordinary_fresh_claim_binds_observed_attempt_count: bool,
    ordinary_repeat_uses_selected_envelope_attempt_count: bool,
    ordinary_obtained_material_owner: String,
    ordinary_peer_delivery_precedes_self_signing: bool,
    installed_original_delivery_requires_signer: bool,
    explicit_resume_claims_bounded_new_attempt: bool,
    retries_use_original_outer_bytes: bool,
    observation_nullable: bool,
    original_credential_capture_requires_live_clock_or_grant: bool,
    same_owner_obtained_material_reconciliation_required: bool,
    first_observation_per_class_immutable: bool,
    wrong_event_request_target_ack_rejected: bool,
    protected_refs_indexes_complete_ancestry_required: bool,
    final_transaction_witnesses_required: bool,
    unbounded_attempt_or_observation_collections: bool,
    physical_stock_effect: bool,
    transport_inside_sql: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptancePreparation {
    shared_storage_port: String,
    shared_model_owner: String,
    legacy_native_aliases: bool,
    unsupported_hosts_refuse: bool,
    model_constructs_runtime: bool,
    planner_owner: String,
    payload_version: u32,
    order_payload_schema: String,
    movement_payload_schema: String,
    worker_charge_bytes: u32,
    max_allocations: u32,
    max_pool_observations: u32,
    max_first_issue_window_seconds: u32,
    original_replay_before_fresh_planning: bool,
    complete_allocation_required: bool,
    atomic_holds_and_order_fence_required: bool,
    current_issued_consent_required: bool,
    final_source_counts_and_witnesses_required: bool,
    signer_invoked: bool,
    agreement_installed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QualifiedLocalRecovery {
    scoped_method: String,
    scoped_identity_and_capability_rechecked: bool,
    scoped_callback_binding_enforced: bool,
    scoped_close_waits_for_active_call: bool,
    scoped_method_grants_store_authority: bool,
    provider_method: String,
    adapter: String,
    native_authorizer: String,
    captured_replay: String,
    native_catalog: u16,
    worker_charge_bytes: u32,
    existing_protected_claim_required: bool,
    current_execute_consent_required: bool,
    original_window_required: bool,
    call_budget_extends_original_window: bool,
    recovery_refusal_proves_original_unissued: bool,
    creates_claim_or_stock_effect: bool,
    provider_work_inside_sql: bool,
    unsupported_legacy_fallback: bool,
    after_expiry_retrieval_available: bool,
    legacy_and_first_dispatch_unchanged: bool,
    current_replay_admission_required: bool,
    replay_admission_journal_family: u16,
    replay_admission_payload_version: u16,
    replay_admission_payload_bytes: u16,
    replay_admission_unknown_returns_permit: bool,
    replay_admission_reauthorizes_current: bool,
    replay_admission_clear_on_refusal: bool,
    replay_admission_stock_effect: bool,
    retained_untracked_catalogs: Vec<u16>,
    replay_admission_report_is_release_authority: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparationAbort {
    shared_storage_port: String,
    shared_model_owner: String,
    native_workspace_type_aliases: bool,
    unsupported_hosts_return: String,
    payload_version: u32,
    journal_family: u32,
    worker_charge_bytes: u32,
    retained_payload_versions: Vec<u32>,
    retained_v1_requires_absent_claims: bool,
    claimed_cleanup_catalogs: Vec<u32>,
    untracked_claimed_catalogs: Vec<u32>,
    per_recipient_proof_count: u32,
    original_unissued_required_for_every_claim: bool,
    replay_admission_blocks_release: bool,
    obtained_evidence_blocks_release: bool,
    contradictory_evidence_retained: bool,
    contradiction_requires_recovery: bool,
    caller_reports_are_release_authority: bool,
    caller_nonissuance_assertion_is_proof: bool,
    current_exact_recovery_consent_required: bool,
    atomic_exact_original_release_required: bool,
    original_preparations_retained: bool,
    current_order_projection_authenticated: bool,
    later_initial_requires_original_abort: bool,
    fresh_signing_after_abort: bool,
    unknown_commit_fences_owner: bool,
    signer_or_transport_invoked: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialAgreementInstallation {
    payload_version: u32,
    journal_family: u32,
    worker_charge_bytes: u32,
    required_peer_evidence: bool,
    missing_self_copy_blocks_installation: bool,
    original_order_fence_immutable: bool,
    exact_original_hold_quantities_required: bool,
    current_exact_recovery_consent_required: bool,
    fresh_time_not_before_original_bound_plan: bool,
    atomic_pool_agreement_and_work_required: bool,
    recipient_work_count: u32,
    unknown_commit_fences_owner: bool,
    signer_or_transport_invoked: bool,
    relay_delivery_claimed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SigningIssuance {
    shared_reconciliation_port: String,
    reconciliation_retains_writer_through_replacement: bool,
    reconciliation_authenticates_before_connection_close: bool,
    reconciliation_preflight_initializes_or_migrates: bool,
    reconciliation_clears_fence_only_after_confirmed_capture: bool,
    shared_capture_port: String,
    shared_capture_model_owner: String,
    shared_capture_descriptors_are_authority: bool,
    shared_capture_reauthenticates_original: bool,
    native_capture_report_aliases: bool,
    coherent_operation_snapshot_required: bool,
    final_evidence_selectors_and_page_rows_required: bool,
    recovery_view_has_dispatch_or_installation_rights: bool,
    fresh_current_backing_projection_required: bool,
    evidence_page_max: u32,
    claim_family: u32,
    evidence_family: u32,
    claim_worker_charge_bytes: u32,
    capture_worker_charge_bytes: u32,
    capture_raw_max_bytes: u32,
    first_dispatch_requires_confirmed_commit: bool,
    existing_claim_returns_dispatch_permit: bool,
    protected_identity_presence_and_absence_required: bool,
    capture_requires_original_claim_and_credential: bool,
    capture_requires_current_consent: bool,
    capture_requires_observation_clock: bool,
    capture_preserves_first_optional_time: bool,
    capture_retains_distinct_valid_encodings: bool,
    agreement_installed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SigningPlanBinding {
    ordinary_acceptance_owner: String,
    ordinary_acceptance_facade: String,
    ordinary_peer_installed_before_self_work: bool,
    ordinary_accepted_task_survives_observer_loss: bool,
    ordinary_obtained_owned_before_descriptor_conversion: bool,
    generic_obtained_owned_before_descriptor_conversion: bool,
    retained_drain_preserves_original_error_and_operation: bool,
    rejected_signer_error: String,
    effectful_command_result: String,
    common_error_max_bytes: u16,
    execution_failure_max_bytes: u16,
    rejected_signer_metadata_owner: String,
    rejected_signer_report_is_nonissuance: bool,
    generic_peer_installed_before_rejected_self_return: bool,
    unknown_installation_blocks_delivery: bool,
    native_first_dispatch_owns_issuer_invocation: bool,
    shared_first_dispatch_port: String,
    shared_unissued_owner: String,
    shared_unissued_retained_before_capture: bool,
    shared_unissued_drained_before_storage_close: bool,
    shared_unissued_failure_blocks_fresh_commands: bool,
    public_native_intent_extraction: bool,
    coherent_snapshot_preserves_original_unissued: bool,
    exact_issuer_port: String,
    native_failure_boundary: String,
    native_failure_preserves_signer_rejection: bool,
    native_failure_fingerprint_bytes: u8,
    native_failure_coverage: String,
    native_failure_max_bytes: u16,
    browser_failure_boundary: String,
    browser_failure_preserves_signer_rejection: bool,
    browser_failure_max_bytes: u16,
    original_unissued_capture_owner: String,
    original_unissued_payload_version: u32,
    original_unissued_journal_family: u32,
    original_unissued_capture_requires_owned_observation: bool,
    original_unissued_capture_borrows_observation: bool,
    original_unissued_capture_rechecks_original_claim: bool,
    original_unissued_capture_requires_fresh_permission: bool,
    original_unissued_capture_requires_fresh_clock: bool,
    original_unissued_capture_releases_stock: bool,
    original_unissued_reconciliation_same_owner: bool,
    original_unissued_late_evidence_retained: bool,
    original_unissued_worker_charge_bytes: u32,
    original_unissued_live_observation_owner: String,
    original_unissued_observation_is_durable: bool,
    original_unissued_observation_releases_stock: bool,
    recovery_refusal_creates_original_unissued_observation: bool,
    ordinary_existing_claim_grants_first_dispatch: bool,
    ordinary_replay_requires_original_policy_and_current_permit: bool,
    ordinary_acceptance_implies_delivery: bool,
    crypto_verifier_owner: String,
    scoped_crypto_verification: String,
    predecessor_nostr_verifier_alias: bool,
    crypto_verification_grants_dispatch_authority: bool,
    payload_version: u16,
    journal_family: u16,
    worker_charge_bytes: u32,
    complete_recipient_count: u16,
    crypto_and_route_verification_required: bool,
    original_t0_rumor_required: bool,
    exact_intent_digests_retained: bool,
    protected_identity_presence_and_absence_required: bool,
    current_issued_consent_required: bool,
    final_permission_and_clock_witnesses_required: bool,
    signer_invoked: bool,
    agreement_installed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    app_behavior_implementation: String,
    retain_source_provenance: bool,
    retain_notices: bool,
    map_meaningful_removed_tests_before_removal: bool,
    relabel_app_source_license: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Targets {
    native: Vec<String>,
    apple_artifacts: Vec<String>,
    browsers: Vec<String>,
    browser_contexts: Vec<String>,
    generic_binding_languages: Vec<String>,
    node_ssr_import_required: bool,
    full_node_client_in_scope: bool,
    actual_beta_iphone_required: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Portability {
    native_futures: String,
    browser_futures: String,
    unsafe_send_bridge_allowed: bool,
    portable_core_has_native_hosts: bool,
    memory_store_firm_stock_allowed: bool,
    nostr_forces_media: bool,
    global_no_std_promise: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    key: String,
    package: String,
    directory: String,
    dependencies: Vec<String>,
    dependency_mode: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Values {
    quantity_coefficient_max: u64,
    quantity_scale_max: u8,
    count_quantities_integral: bool,
    money_minor_max: u64,
    currency_exponent_max: u8,
    currency_registry: String,
    rounding: String,
    binding_numbers: String,
    stock_ticks_max: u64,
    stock_scale_max: u8,
    stock_units: Vec<String>,
    stock_physical_categories: Vec<String>,
    generation_max: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Execution {
    ordinary_inventory_owner: OrdinaryInventoryOwner,
    phases: Vec<String>,
    selected_executor_required_for_stock: bool,
    fresh_same_key_cache_can_allocate: bool,
    shared_route_local_fallback_allowed: bool,
    effects_inside_stock_transaction: bool,
    unknown_is_nonspendable: bool,
    expiry_proves_nonissuance: bool,
    obtained_evidence_append_only: bool,
    observation_time_nullable: bool,
    positive_amendment_hold_before_issuance: bool,
    negative_amendment_release_at_installation: bool,
    handover_consumes_once: bool,
    buyer_receipt_moves_stock: bool,
    private_causal_heads_use_timestamp_lww: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OrdinaryInventoryOwner {
    capture_drain_uses_explicit_reconciliation: bool,
    owned_capture_method: String,
    retained_capture_survives_observer_loss: bool,
    failed_capture_blocks_fresh_work: bool,
    failed_capture_blocks_disposal: bool,
    capture_preserves_actual_error_code: bool,
    pending_capture_retry: String,
    capture_reports_must_match_original: bool,
    capture_scope_metadata_renews_issuance: bool,
    owned_acceptance_method: String,
    acceptance_is_preparation_only: bool,
    lost_accepted_admission_is_unknown: bool,
    execution: String,
    facade: String,
    selected_workspace_views_share_owner: bool,
    exact_owned_attachment_required: bool,
    bounded_task_host_required: bool,
    observer_loss_cancels_accepted_work: bool,
    close_drains_before_workspace_disposal: bool,
    stock_signer_or_transport_constructed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Protocol {
    listing_kind: u16,
    chat_rumor_kind: u16,
    seal_kind: u16,
    gift_wrap_kind: u16,
    commerce_kind: Option<u16>,
    capability_kind: Option<u16>,
    assignment_contract: String,
    structured_authoring: String,
    independent_vectors_required: bool,
    duplicate_json_fields_rejected: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cutover {
    new_persistent_namespace: bool,
    protected_export_and_quarantine_required: bool,
    restore_starts_nonpublishing: bool,
    unknowns_and_obligations_protected: bool,
    automatic_legacy_reissue_allowed: bool,
    meaningful_state_reset_requires_operator_approval: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    implementation_claimed: bool,
    software_matrix_executed: bool,
    device_matrix_executed: bool,
    final_gate_acceptance_claimed: bool,
}

pub(super) fn validate(workspace_root: &Path) -> Result<(), String> {
    let raw = crate::safe_artifact_io::read_regular(
        workspace_root,
        Path::new(CONTRACT),
        MAX_BYTES as u64,
    )
    .map_err(|_| "shared SDK contract: bounded regular source admission failed".to_owned())?;
    validate_bytes(&raw)?;
    crate::shared_protocol::validate_contract(workspace_root)?;
    currency_registry::validate(workspace_root)
}

fn require(condition: bool, rule: &'static str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(format!("shared SDK contract: {rule}"))
    }
}

fn exact_set(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == expected.iter().copied().collect::<BTreeSet<_>>()
}

fn validate_bytes(raw: &[u8]) -> Result<(), String> {
    require(raw.len() <= MAX_BYTES, "source byte limit exceeded")?;
    let p: Policy = serde_json::from_slice(raw)
        .map_err(|_| "shared SDK contract: invalid closed JSON shape".to_owned())?;
    let accounting = &p.stock_accounting;
    require(
        accounting.value_owner == "radroots_domain.stock"
            && accounting.scope == "one_fixed_pool_unit"
            && accounting.obligation_equation == "remaining = backed + deficit"
            && accounting.ticks_max == 1_000_000_000_000_000_000
            && !accounting.values_create_store_effects
            && !accounting.deficit_counts_as_physical_stock
            && !accounting.receipt_consumes_again
            && !accounting.released_deficit_adds_available
            && !accounting.loss_deletes_obligation
            && accounting.native_atomic_allocation_and_identity_required,
        "stock accounting value or native effect authority drift",
    )?;
    require(
        p.schema_version == 1
            && p.contract_id == "radroots.shared_sdk.refactor.v1"
            && p.state == "approved_target_implementation_pending"
            && !p.publication_authorized,
        "prospective identity or publication authority drift",
    )?;
    validate_ownership(&p)?;
    let local = &p.local_outbox_authority;
    require(
        exact_set(&local.scopes, &["new_buyer_outbox", "new_seller_outbox"])
            && local.seller_enrollment_explicit
            && local.scope_retained_in_authenticated_binding_and_preparation
            && local.legacy_preparation_v2_scope == "new_buyer_outbox"
            && local.scope_separates_route_genesis_and_credential_commitments
            && local.coverage == "partial"
            && !local.stock_authority
            && !local.organization_or_delegation_authority
            && !local.existing_order_or_history_recovery,
        "local outbox authority drift",
    )?;
    let stock = &p.native_seller_stock_authority;
    let envelope = &stock.envelope_retention;
    require(
        envelope.journal_family == 10
            && envelope.payload_version == 1
            && envelope.payload_prefix_bytes == 213
            && envelope.supported_catalogs == [16, 17, 18]
            && envelope.retained_unsupported_catalogs
                == [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
            && envelope.raw_max_bytes == 65536
            && envelope.worker_charge_bytes == 1048576
            && envelope.capture_port == "InventoryStore.capture_stock_envelope"
            && envelope.query_port == "InventoryStore.stock_envelope"
            && envelope.recovery_port == "InventoryStore.reconcile_stock_envelope"
            && envelope.original_preparation_plan_claim_seal_installation_required
            && envelope.temporary_key_derivation_and_exact_decryption_required
            && !envelope.temporary_key_retained
            && !envelope.original_party_as_outer_author_allowed
            && envelope.body_derived_projection_and_complete_ancestry_required
            && !envelope.original_credential_capture_requires_live_clock_or_grant
            && envelope.query_requires_current_recovery_access
            && envelope.observation_nullable
            && !envelope.existing_original_bytes_may_be_replaced
            && !envelope.capture_grants_dispatch
            && !envelope.capture_changes_physical_stock
            && !envelope.provider_io_inside_sql,
        "stock envelope retention drift",
    )?;
    let rewrap = &stock.rewrapping;
    require(
        rewrap.journal_family == 14
            && rewrap.admission_journal_family == 13
            && rewrap.admission_payload_bytes == 248
            && rewrap.admission_window_seconds_max == 30
            && rewrap.admission_port == "InventoryStore.prepare_stock_rewrap"
            && rewrap.admission_recovery_port == "InventoryStore.reconcile_stock_rewrap_admission"
            && rewrap.admission_query_port == "InventoryStore.stock_rewrap_attempt"
            && rewrap.explicit_request_id_required
            && rewrap.newly_confirmed_admission_before_wrapping
            && !rewrap.existing_or_uncertain_admission_grants_invocation
            && rewrap.unknown_admissions_count_toward_limit
            && rewrap.descriptor_includes_uncaptured_admissions
            && !rewrap.admission_repair_grants_history_or_invocation
            && rewrap.ordinary_owner == "ClientSession.stock_captures"
            && rewrap.ordinary_facade_methods
                == [
                    "Orders.rewrap_acceptance",
                    "Orders.resume_acceptance_rewrap",
                    "Orders.acceptance_rewrap_status",
                ]
            && !rewrap.ordinary_resume_generates_key_or_business_signature
            && !rewrap.ordinary_status_is_atomic_absence_proof
            && rewrap.payload_version == 1
            && rewrap.payload_prefix_bytes == 117
            && rewrap.supported_catalogs == [18]
            && rewrap.retained_unsupported_catalogs
                == [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]
            && rewrap.raw_max_bytes == 65536
            && rewrap.worker_charge_bytes == 1048576
            && rewrap.additional_envelopes_per_recipient_max == 192
            && rewrap.descriptor_page_max == 16
            && rewrap.capture_port == "InventoryStore.capture_stock_rewrap"
            && rewrap.recovery_port == "InventoryStore.reconcile_stock_rewrap"
            && rewrap.query_port == "InventoryStore.stock_envelope"
            && rewrap.page_port == "InventoryStore.stock_rewraps"
            && rewrap.original_f10_and_same_s_t2_required
            && rewrap.default_selection_is_original
            && rewrap.new_envelope_selection_explicit
            && rewrap.first_policy_source_bound
            && rewrap.transport_counters_cumulative
            && rewrap.outcomes_scoped_to_exact_envelope
            && rewrap.natural_index_transaction_witness_required
            && !rewrap.existing_bytes_may_be_replaced
            && !rewrap.original_capture_requires_live_clock_or_grant
            && !rewrap.capture_or_query_grants_dispatch
            && !rewrap.physical_stock_effect
            && !rewrap.temporary_key_retained
            && !rewrap.provider_io_inside_sql,
        "stock rewrapping authority or retention drift",
    )?;
    let delivery = &stock.delivery;
    require(
        delivery.supported_catalogs == [17, 18]
            && delivery.retained_unsupported_catalogs
                == [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
            && delivery.attempt_journal_family == 11
            && delivery.observation_journal_family == 12
            && delivery.attempt_payload_version == 1
            && delivery.observation_payload_version == 1
            && delivery.attempt_body_max_bytes == 65536
            && delivery.observation_body_max_bytes == 16384
            && delivery.target_max == 16
            && delivery.attempts_per_target == 12
            && delivery.attempts_per_operation_max == 384
            && delivery.observations_per_operation_max == 2304
            && delivery.observation_page_max == 16
            && delivery.summary_target_max == 16
            && delivery.worker_charge_bytes == 1048576
            && delivery.storage_ports
                == [
                    "InventoryStore.prepare_stock_delivery_attempt",
                    "InventoryStore.stock_delivery_attempt",
                    "InventoryStore.stock_delivery_summary",
                    "InventoryStore.record_stock_delivery_observation",
                    "InventoryStore.reconcile_stock_delivery_observation",
                    "InventoryStore.stock_delivery_observations",
                ]
            && delivery.original_stock_evidence_installation_outer_required
            && delivery.original_captured_route_and_current_write_policy_required
            && delivery.fresh_current_scoped_consent_grant_clock_required
            && delivery.installed_original_recovery_delivery_supported
            && delivery.fresh_claim_requires_confirmed_commit
            && !delivery.existing_attempt_grants_dispatch
            && !delivery.unknown_commit_grants_dispatch
            && !delivery.query_grants_dispatch
            && delivery.first_complete_targets_and_criterion_immutable
            && !delivery.ordinary_command_replays_existing_attempt
            && delivery.ordinary_fresh_claim_binds_observed_attempt_count
            && delivery.ordinary_repeat_uses_selected_envelope_attempt_count
            && delivery.ordinary_obtained_material_owner == "ClientSession.stock_captures"
            && delivery.ordinary_peer_delivery_precedes_self_signing
            && !delivery.installed_original_delivery_requires_signer
            && delivery.explicit_resume_claims_bounded_new_attempt
            && delivery.retries_use_original_outer_bytes
            && delivery.observation_nullable
            && !delivery.original_credential_capture_requires_live_clock_or_grant
            && delivery.same_owner_obtained_material_reconciliation_required
            && delivery.first_observation_per_class_immutable
            && delivery.wrong_event_request_target_ack_rejected
            && delivery.protected_refs_indexes_complete_ancestry_required
            && delivery.final_transaction_witnesses_required
            && !delivery.unbounded_attempt_or_observation_collections
            && !delivery.physical_stock_effect
            && !delivery.transport_inside_sql,
        "stock delivery ownership or bounds drift",
    )?;
    require(
        stock.capability_contract.id == "radroots.seller_stock.native_inventory"
            && stock.capability_contract.version == "1"
            && stock.scope == "seller_stock_owner"
            && stock.database == "stock_execution_v1.sqlite"
            && stock.lock == ".stock_execution_v1.lock"
            && stock.schema_version == 18
            && stock.retained_enrollment_versions == [1]
            && stock.retained_inventory_versions
                == [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]
            && stock.authority_payload_schema == "radroots.seller_stock.authority.v18"
            && stock.public_source_payload_schemas
                == [
                    "radroots.seller_stock.public_source_event.v1",
                    "radroots.seller_stock.public_source_head.v1",
                    "radroots.seller_stock.public_source_part.v1",
                ]
            && stock.public_source_owner_administration_required
            && !stock.inventory_scope_grants_public_source_access
            && stock.public_source_checkpoint_independent
            && stock.public_source_input_requires_signature_evidence
            && stock.public_source_read_reverifies_signature
            && !stock.public_source_global_completeness_claimed
            && !stock.public_source_profile_implies_actionability
            && stock.public_source_head_policy == "nip01_timestamp_then_lowest_id_before_profile"
            && stock.public_source_duplicate_encodings == "retain_original_authenticated_event_id"
            && stock.public_source_raw_max_bytes == 65536
            && stock.public_source_part_max_bytes == 16384
            && stock.public_source_part_max_count == 4
            && stock.public_source_worker_charge_bytes == 262144
            && stock.inventory_operation_payload_schema == "radroots.seller_stock.operation.v5"
            && stock.operation_part_payload_schema == "radroots.seller_stock.operation_part.v1"
            && stock.operation_body_max_bytes == 262144
            && stock.operation_part_max_bytes == 16384
            && stock.operation_part_max_count == 16
            && stock.operation_identity_requires_protected_ancestry
            && stock.acceptance_preparation.planner_owner == "radroots_domain"
            && stock.acceptance_preparation.shared_storage_port
                == "radroots_storage.inventory.InventoryStore.prepare_stock_acceptance"
            && stock.acceptance_preparation.shared_model_owner == "radroots_storage.inventory"
            && !stock.acceptance_preparation.legacy_native_aliases
            && stock.acceptance_preparation.unsupported_hosts_refuse
            && !stock.acceptance_preparation.model_constructs_runtime
            && stock.acceptance_preparation.payload_version == 1
            && stock.acceptance_preparation.order_payload_schema
                == "radroots.seller_stock.order.v1"
            && stock.acceptance_preparation.movement_payload_schema
                == "radroots.seller_stock.acceptance_movement.v1"
            && stock.acceptance_preparation.worker_charge_bytes == 524288
            && stock.acceptance_preparation.max_allocations == 64
            && stock.acceptance_preparation.max_pool_observations == 96
            && stock.acceptance_preparation.max_first_issue_window_seconds == 120
            && stock
                .acceptance_preparation
                .original_replay_before_fresh_planning
            && stock.acceptance_preparation.complete_allocation_required
            && stock
                .acceptance_preparation
                .atomic_holds_and_order_fence_required
            && stock.acceptance_preparation.current_issued_consent_required
            && stock
                .acceptance_preparation
                .final_source_counts_and_witnesses_required
            && !stock.acceptance_preparation.signer_invoked
            && !stock.acceptance_preparation.agreement_installed
            && stock.signing_plan_binding.ordinary_acceptance_owner
                == "radroots_execution.CommandRuntime"
            && stock.signing_plan_binding.ordinary_acceptance_facade == "radroots.Orders.accept"
            && stock
                .signing_plan_binding
                .ordinary_peer_installed_before_self_work
            && stock
                .signing_plan_binding
                .ordinary_accepted_task_survives_observer_loss
            && stock
                .signing_plan_binding
                .ordinary_obtained_owned_before_descriptor_conversion
            && stock
                .signing_plan_binding
                .generic_obtained_owned_before_descriptor_conversion
            && stock
                .signing_plan_binding
                .retained_drain_preserves_original_error_and_operation
            && stock.signing_plan_binding.rejected_signer_error == "SignerTampering"
            && stock.signing_plan_binding.effectful_command_result == "ExecutionResult"
            && stock.signing_plan_binding.common_error_max_bytes == 64
            && stock.signing_plan_binding.execution_failure_max_bytes == 128
            && stock.signing_plan_binding.rejected_signer_metadata_owner
                == "radroots_signing.issuance.RejectedArtifact"
            && !stock
                .signing_plan_binding
                .rejected_signer_report_is_nonissuance
            && stock
                .signing_plan_binding
                .generic_peer_installed_before_rejected_self_return
            && stock
                .signing_plan_binding
                .unknown_installation_blocks_delivery
            && stock
                .signing_plan_binding
                .native_first_dispatch_owns_issuer_invocation
            && stock.signing_plan_binding.shared_first_dispatch_port
                == "radroots_execution.session.commerce.AcceptanceDispatch.dispatch"
            && stock.signing_plan_binding.shared_unissued_owner == "ClientSession.stock_captures"
            && stock
                .signing_plan_binding
                .shared_unissued_retained_before_capture
            && stock
                .signing_plan_binding
                .shared_unissued_drained_before_storage_close
            && stock
                .signing_plan_binding
                .shared_unissued_failure_blocks_fresh_commands
            && !stock.signing_plan_binding.public_native_intent_extraction
            && stock
                .signing_plan_binding
                .coherent_snapshot_preserves_original_unissued
            && stock.signing_plan_binding.exact_issuer_port
                == "radroots_signing.issuance.ExactIssuer"
            && stock.signing_plan_binding.native_failure_boundary == "radroots_ffi.Error"
            && stock
                .signing_plan_binding
                .native_failure_preserves_signer_rejection
            && stock.signing_plan_binding.native_failure_fingerprint_bytes == 32
            && stock.signing_plan_binding.native_failure_coverage == "canonical_decimal_strings"
            && stock.signing_plan_binding.native_failure_max_bytes == 128
            && stock.signing_plan_binding.browser_failure_boundary
                == "radroots_browser.bindings.Failure"
            && stock
                .signing_plan_binding
                .browser_failure_preserves_signer_rejection
            && stock.signing_plan_binding.browser_failure_max_bytes == 128
            && stock
                .signing_plan_binding
                .original_unissued_live_observation_owner
                == "radroots_storage_sqlite.workspace.StockUnissuedInvocation"
            && !stock
                .signing_plan_binding
                .original_unissued_observation_is_durable
            && !stock
                .signing_plan_binding
                .original_unissued_observation_releases_stock
            && !stock
                .signing_plan_binding
                .recovery_refusal_creates_original_unissued_observation
            && stock.signing_plan_binding.original_unissued_capture_owner
                == "NativeWorkspace.capture_stock_original_unissued"
            && stock.signing_plan_binding.original_unissued_payload_version == 1
            && stock.signing_plan_binding.original_unissued_journal_family == 8
            && stock
                .signing_plan_binding
                .original_unissued_capture_requires_owned_observation
            && stock
                .signing_plan_binding
                .original_unissued_capture_borrows_observation
            && stock
                .signing_plan_binding
                .original_unissued_capture_rechecks_original_claim
            && !stock
                .signing_plan_binding
                .original_unissued_capture_requires_fresh_permission
            && !stock
                .signing_plan_binding
                .original_unissued_capture_requires_fresh_clock
            && !stock
                .signing_plan_binding
                .original_unissued_capture_releases_stock
            && stock
                .signing_plan_binding
                .original_unissued_reconciliation_same_owner
            && stock
                .signing_plan_binding
                .original_unissued_late_evidence_retained
            && stock
                .signing_plan_binding
                .original_unissued_worker_charge_bytes
                == 262144
            && !stock
                .signing_plan_binding
                .ordinary_existing_claim_grants_first_dispatch
            && stock
                .signing_plan_binding
                .ordinary_replay_requires_original_policy_and_current_permit
            && !stock
                .signing_plan_binding
                .ordinary_acceptance_implies_delivery
            && stock.signing_plan_binding.crypto_verifier_owner == "radroots_signing.commerce"
            && stock.signing_plan_binding.scoped_crypto_verification
                == "SignerSession.verify_commerce_plan"
            && !stock.signing_plan_binding.predecessor_nostr_verifier_alias
            && !stock
                .signing_plan_binding
                .crypto_verification_grants_dispatch_authority
            && stock.signing_plan_binding.payload_version == 1
            && stock.signing_plan_binding.journal_family == 3
            && stock.signing_plan_binding.worker_charge_bytes == 1048576
            && stock.signing_plan_binding.complete_recipient_count == 2
            && stock
                .signing_plan_binding
                .crypto_and_route_verification_required
            && stock.signing_plan_binding.original_t0_rumor_required
            && stock.signing_plan_binding.exact_intent_digests_retained
            && stock
                .signing_plan_binding
                .protected_identity_presence_and_absence_required
            && stock.signing_plan_binding.current_issued_consent_required
            && stock
                .signing_plan_binding
                .final_permission_and_clock_witnesses_required
            && !stock.signing_plan_binding.signer_invoked
            && !stock.signing_plan_binding.agreement_installed
            && stock
                .signing_issuance
                .fresh_current_backing_projection_required
            && stock.signing_issuance.coherent_operation_snapshot_required
            && stock.signing_issuance.shared_capture_port
                == "InventoryStore.capture_stock_evidence"
            && stock.signing_issuance.shared_reconciliation_port
                == "InventoryStore.reconcile_stock_evidence"
            && stock
                .signing_issuance
                .reconciliation_retains_writer_through_replacement
            && stock
                .signing_issuance
                .reconciliation_authenticates_before_connection_close
            && !stock
                .signing_issuance
                .reconciliation_preflight_initializes_or_migrates
            && stock
                .signing_issuance
                .reconciliation_clears_fence_only_after_confirmed_capture
            && stock.signing_issuance.shared_capture_model_owner == "radroots_storage.inventory"
            && !stock
                .signing_issuance
                .shared_capture_descriptors_are_authority
            && stock
                .signing_issuance
                .shared_capture_reauthenticates_original
            && !stock.signing_issuance.native_capture_report_aliases
            && stock
                .signing_issuance
                .final_evidence_selectors_and_page_rows_required
            && !stock
                .signing_issuance
                .recovery_view_has_dispatch_or_installation_rights
            && stock.signing_issuance.evidence_page_max == 32
            && stock.signing_issuance.claim_family == 4
            && stock.signing_issuance.evidence_family == 5
            && stock.signing_issuance.claim_worker_charge_bytes == 1048576
            && stock.signing_issuance.capture_worker_charge_bytes == 262144
            && stock.signing_issuance.capture_raw_max_bytes == 65536
            && stock
                .signing_issuance
                .first_dispatch_requires_confirmed_commit
            && !stock
                .signing_issuance
                .existing_claim_returns_dispatch_permit
            && stock
                .signing_issuance
                .protected_identity_presence_and_absence_required
            && stock
                .signing_issuance
                .capture_requires_original_claim_and_credential
            && !stock.signing_issuance.capture_requires_current_consent
            && !stock.signing_issuance.capture_requires_observation_clock
            && stock.signing_issuance.capture_preserves_first_optional_time
            && stock
                .signing_issuance
                .capture_retains_distinct_valid_encodings
            && !stock.signing_issuance.agreement_installed
            && stock.qualified_local_recovery.provider_method
                == "radroots_signing.provider.ParticipantProvider.recover_exact"
            && stock.qualified_local_recovery.scoped_method == "SignerSession.recover_exact"
            && stock
                .qualified_local_recovery
                .scoped_identity_and_capability_rechecked
            && stock
                .qualified_local_recovery
                .scoped_callback_binding_enforced
            && stock
                .qualified_local_recovery
                .scoped_close_waits_for_active_call
            && !stock
                .qualified_local_recovery
                .scoped_method_grants_store_authority
            && stock.qualified_local_recovery.adapter == "radroots_nostr.signing.LocalSigner"
            && stock.qualified_local_recovery.native_authorizer
                == "NativeWorkspace.authorize_stock_exact_replay"
            && stock.qualified_local_recovery.captured_replay == "local_replay_safe"
            && stock.qualified_local_recovery.native_catalog == 18
            && stock.qualified_local_recovery.worker_charge_bytes == 1048576
            && stock
                .qualified_local_recovery
                .existing_protected_claim_required
            && stock
                .qualified_local_recovery
                .current_execute_consent_required
            && stock.qualified_local_recovery.original_window_required
            && !stock
                .qualified_local_recovery
                .call_budget_extends_original_window
            && !stock
                .qualified_local_recovery
                .recovery_refusal_proves_original_unissued
            && !stock.qualified_local_recovery.creates_claim_or_stock_effect
            && !stock.qualified_local_recovery.provider_work_inside_sql
            && !stock.qualified_local_recovery.unsupported_legacy_fallback
            && !stock
                .qualified_local_recovery
                .after_expiry_retrieval_available
            && stock
                .qualified_local_recovery
                .legacy_and_first_dispatch_unchanged
            && stock
                .qualified_local_recovery
                .current_replay_admission_required
            && stock
                .qualified_local_recovery
                .replay_admission_journal_family
                == 9
            && stock
                .qualified_local_recovery
                .replay_admission_payload_version
                == 1
            && stock
                .qualified_local_recovery
                .replay_admission_payload_bytes
                == 184
            && !stock
                .qualified_local_recovery
                .replay_admission_unknown_returns_permit
            && stock
                .qualified_local_recovery
                .replay_admission_reauthorizes_current
            && !stock
                .qualified_local_recovery
                .replay_admission_clear_on_refusal
            && !stock.qualified_local_recovery.replay_admission_stock_effect
            && stock.qualified_local_recovery.retained_untracked_catalogs == [13, 14]
            && !stock
                .qualified_local_recovery
                .replay_admission_report_is_release_authority
            && stock.preparation_abort.payload_version == 2
            && stock.preparation_abort.journal_family == 7
            && stock.preparation_abort.worker_charge_bytes == 1048576
            && stock.preparation_abort.shared_storage_port
                == "radroots_storage.inventory.InventoryStore"
            && stock.preparation_abort.shared_model_owner == "radroots_storage.inventory"
            && !stock.preparation_abort.native_workspace_type_aliases
            && stock.preparation_abort.unsupported_hosts_return == "capability_unavailable"
            && stock.preparation_abort.retained_payload_versions == [1]
            && stock.preparation_abort.retained_v1_requires_absent_claims
            && stock.preparation_abort.claimed_cleanup_catalogs == [15, 16, 17, 18]
            && stock.preparation_abort.untracked_claimed_catalogs == [13, 14]
            && stock.preparation_abort.per_recipient_proof_count == 2
            && stock
                .preparation_abort
                .original_unissued_required_for_every_claim
            && stock.preparation_abort.replay_admission_blocks_release
            && stock.preparation_abort.obtained_evidence_blocks_release
            && stock.preparation_abort.contradictory_evidence_retained
            && stock.preparation_abort.contradiction_requires_recovery
            && !stock.preparation_abort.caller_reports_are_release_authority
            && !stock
                .preparation_abort
                .caller_nonissuance_assertion_is_proof
            && stock
                .preparation_abort
                .current_exact_recovery_consent_required
            && stock
                .preparation_abort
                .atomic_exact_original_release_required
            && stock.preparation_abort.original_preparations_retained
            && stock
                .preparation_abort
                .current_order_projection_authenticated
            && stock
                .preparation_abort
                .later_initial_requires_original_abort
            && !stock.preparation_abort.fresh_signing_after_abort
            && stock.preparation_abort.unknown_commit_fences_owner
            && !stock.preparation_abort.signer_or_transport_invoked
            && stock.initial_agreement_installation.payload_version == 1
            && stock.initial_agreement_installation.journal_family == 6
            && stock.initial_agreement_installation.worker_charge_bytes == 1048576
            && stock.initial_agreement_installation.required_peer_evidence
            && !stock
                .initial_agreement_installation
                .missing_self_copy_blocks_installation
            && stock
                .initial_agreement_installation
                .original_order_fence_immutable
            && stock
                .initial_agreement_installation
                .exact_original_hold_quantities_required
            && stock
                .initial_agreement_installation
                .current_exact_recovery_consent_required
            && stock
                .initial_agreement_installation
                .fresh_time_not_before_original_bound_plan
            && stock
                .initial_agreement_installation
                .atomic_pool_agreement_and_work_required
            && stock.initial_agreement_installation.recipient_work_count == 2
            && stock
                .initial_agreement_installation
                .unknown_commit_fences_owner
            && !stock
                .initial_agreement_installation
                .signer_or_transport_invoked
            && !stock.initial_agreement_installation.relay_delivery_claimed
            && stock.business_consent_payload_version == 1
            && stock.business_consent_action == "orders.accept"
            && stock.business_consent_modes == ["execute_and_recover", "recover_only"]
            && stock.business_consent_admin_issuance_required
            && stock.business_consent_binds_exact_command_and_subject_grant
            && !stock.business_consent_issuance_has_order_or_stock_effects
            && stock.business_consent_worker_charge_bytes == 524288
            && stock.mapping_payload_schema == "radroots.seller_stock.mapping.v1"
            && stock.source_payload_schemas
                == [
                    "radroots.seller_stock.source_record.v1",
                    "radroots.seller_stock.source_evidence.v1",
                    "radroots.seller_stock.source_part.v1",
                ]
            && stock.source_profile_dispositions == ["known", "unsupported", "rejected"]
            && stock.source_owner_administration_required
            && !stock.inventory_scope_grants_source_access
            && stock.source_checkpoint_independent
            && !stock.source_profile_implies_actionability
            && stock.causal_history_current_administrator_required
            && stock.causal_history_complete_local_scope_required
            && stock.causal_history_includes_actual_local_signed_records
            && stock.causal_history_inventory_minimum_required
            && stock.causal_history_logical_record_deduplication_required
            && !stock.causal_history_implies_installation
            && stock.causal_history_uninterpreted_policy
                == "conservative_authenticated_participant_pair_barrier"
            && stock.causal_history_transaction_witnesses_required
            && !stock.causal_history_implies_business_permission
            && stock.source_carrier_max_bytes == 65536
            && stock.source_chunk_max_bytes == 16384
            && stock.mapping_owner_administration_required
            && !stock.mapping_changes_physical_stock
            && !stock.mapping_changes_pool_version
            && stock.mapping_create_expected_version == 0
            && stock.inventory_movement_payload_schema == "radroots.seller_stock.movement.v2"
            && stock.grant_payload_schema == "radroots.seller_stock.grant.v2"
            && stock.grant_change_payload_schema == "radroots.seller_stock.grant_change.v1"
            && stock.namespace_excludes_caller_grant
            && stock.actor_operation_unique_across_grants
            && stock.grant_checkpoint_minimum_required
            && stock.grant_checkpoint_policy
                == "exact_current_for_fresh_work_original_prior_for_exact_admin_recovery"
            && stock.grant_set_max == 64
            && stock.grant_pool_scope_max == 64
            && stock.grant_action_max == 6
            && stock.grant_scope_actions
                == [
                    "read_pools",
                    "read_operations",
                    "create_pool",
                    "add_stock",
                    "remove_stock",
                    "spoil_stock",
                ]
            && stock.grant_write_requires_receipt_and_recovery_read
            && !stock.inventory_scope_grants_administration
            && stock.grant_admin_prepared_transaction_required
            && !stock.retained_grants_automatically_evicted
            && !stock.old_self_revoke_api_supported
            && stock.pool_tick_scale_explicit
            && stock.pool_tick_scale_encoding == "canonical_integer_string"
            && stock.inventory_checkpoint_minimum_required
            && !stock.empty_pool_has_physical_movement
            && stock.enrollment_prepared_before_durable_creation
            && stock.prepared_enrollment_single_use
            && stock.same_connection_owner_as_outbox
            && stock.protected_authority_and_grants_separate
            && !stock.automatic_outbox_migration
            && !stock.historical_obligations_recovered
            && !stock.business_effects_qualified,
        "native seller stock enrollment boundary drift",
    )?;
    validate_targets(&p.targets, &p.portability)?;
    validate_packages(&p.packages)?;
    validate_values(&p.values, &p.limits)?;
    validate_execution(&p.execution, &p.protocol, &p.cutover)?;
    require(
        !p.qualification.implementation_claimed
            && !p.qualification.software_matrix_executed
            && !p.qualification.device_matrix_executed
            && !p.qualification.final_gate_acceptance_claimed,
        "target metadata cannot claim qualification",
    )
}

fn validate_ownership(p: &Policy) -> Result<(), String> {
    require(
        p.toolchain.edition == "2024"
            && p.toolchain.stable == super::STABLE_TOOLCHAIN
            && p.toolchain.msrv == super::STABLE_TOOLCHAIN
            && p.toolchain.historical_receipts_immutable
            && p.toolchain.dedicated_nightly_and_sp1_separate,
        "exact compiler target drift",
    )?;
    let o = &p.ownership;
    require(
        o.shared_repository == "https://github.com/radrootslabs/lib"
            && o.ordinary_facade == "radroots"
            && o.native_bindings == "radroots_ffi"
            && o.browser_artifact == "radroots_browser"
            && o.sdk_repository_role == "generated_artifact_consumer"
            && !o.app_owns_shared_semantics,
        "shared semantic ownership drift",
    )?;
    require(
        exact_set(&p.roles.cli, &["buyer", "seller"])
            && exact_set(&p.roles.browser, &["buyer", "seller"])
            && exact_set(&p.roles.generic_ffi, &["buyer", "seller"])
            && exact_set(&p.roles.tera, &["seller"]),
        "approved actor scope drift",
    )?;
    require(
        exact_set(
            &p.deferred_application_migrations,
            &["harvestcircle", "applesauce", "kotlin_application"],
        ) && exact_set(
            &p.retained_capability_families,
            &[
                "farm",
                "social",
                "calendar",
                "media",
                "places",
                "knowledge",
                "communication",
                "optional_attestation",
            ],
        ),
        "retained or deferred capability scope drift",
    )?;
    let i = &p.independence;
    require(
        !i.hosted_account_required
            && !i.farm_hierarchy_required
            && !i.rhi_required
            && !i.payment_implementation_in_scope
            && i.seller_owner_executor_optional_to_single_device,
        "independent participation scope drift",
    )?;
    let s = &p.provenance;
    require(
        s.app_behavior_implementation == "independent_reimplementation"
            && s.retain_source_provenance
            && s.retain_notices
            && s.map_meaningful_removed_tests_before_removal
            && !s.relabel_app_source_license,
        "implementation provenance or retained-test duty drift",
    )
}

fn validate_targets(t: &Targets, p: &Portability) -> Result<(), String> {
    require(
        exact_set(
            &t.native,
            &["aarch64-apple-darwin", "x86_64-unknown-linux-gnu"],
        ) && exact_set(
            &t.apple_artifacts,
            &["aarch64-apple-ios", "aarch64-apple-ios-sim"],
        ) && exact_set(&t.browsers, &["chromium", "firefox", "webkit"])
            && exact_set(&t.browser_contexts, &["main_thread", "dedicated_worker"])
            && exact_set(&t.generic_binding_languages, &["swift", "kotlin"])
            && t.node_ssr_import_required
            && !t.full_node_client_in_scope
            && t.actual_beta_iphone_required,
        "supported target matrix drift",
    )?;
    require(
        p.native_futures == "send"
            && p.browser_futures == "local"
            && !p.unsafe_send_bridge_allowed
            && !p.portable_core_has_native_hosts
            && !p.memory_store_firm_stock_allowed
            && !p.nostr_forces_media
            && !p.global_no_std_promise,
        "target-local portability or host capability drift",
    )
}

fn validate_packages(packages: &[Package]) -> Result<(), String> {
    let expected = [
        "identity",
        "domain",
        "protocol",
        "signing",
        "storage",
        "transport",
        "execution",
        "storage_sqlite",
        "storage_indexeddb",
        "transport_nostr",
        "transport_nostr_native",
        "transport_nostr_browser",
        "secrets",
        "nostr_connect",
        "blossom",
        "geonames",
        "nostrdb",
        "sql_core",
        "replica_schema",
        "replica_store",
        "replica_sync",
        "runtime_paths",
        "runtime_distribution",
        "runtime_manager",
        "service_host",
        "service_sqlite",
        "mesh_agent_proto",
        "mesh_agent_client",
        "mesh",
        "transport_reticulum",
        "simplex_agent_proto",
        "simplex_smp_crypto",
        "simplex_smp_proto",
        "simplex_smp_transport",
        "simplex_chat_proto",
        "simplex_app_store",
        "radroots",
        "radroots_browser",
        "radroots_ffi",
        "test_fixtures",
        "xtask",
    ];
    let nodes = packages
        .iter()
        .map(|p| (p.key.as_str(), p))
        .collect::<BTreeMap<_, _>>();
    require(
        packages.len() == expected.len()
            && nodes.len() == expected.len()
            && nodes.keys().copied().collect::<BTreeSet<_>>() == expected.into_iter().collect(),
        "complete target package set drift",
    )?;
    for p in packages {
        let (name, directory) = match p.key.as_str() {
            "xtask" => ("xtask".to_owned(), "tools/xtask".to_owned()),
            "radroots" | "radroots_browser" | "radroots_ffi" => {
                (p.key.clone(), format!("crates/{}", p.key))
            }
            _ => (format!("radroots_{}", p.key), format!("crates/{}", p.key)),
        };
        let mode = if p.key == "radroots" {
            "target_specific_optional_concrete_hosts"
        } else {
            "selected_target_required"
        };
        require(
            p.package == name && p.directory == directory && p.dependency_mode == mode,
            "package identity, path or host dependency policy drift",
        )?;
        let deps = p
            .dependencies
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        require(
            deps.len() == p.dependencies.len() && deps.iter().all(|d| nodes.contains_key(d)),
            "duplicate or unknown package dependency",
        )?;
    }
    for (key, deps) in [
        ("identity", &[][..]),
        ("domain", &["identity"][..]),
        ("protocol", &["domain", "identity"][..]),
        ("signing", &["domain", "identity", "protocol"][..]),
        ("storage", &["domain", "protocol"][..]),
        ("transport", &["protocol", "identity"][..]),
        (
            "execution",
            &["domain", "protocol", "signing", "storage", "transport"][..],
        ),
        (
            "radroots_browser",
            &["execution", "storage_indexeddb", "transport_nostr_browser"][..],
        ),
        ("radroots_ffi", &["radroots"][..]),
    ] {
        require(
            exact_set(&nodes[key].dependencies, deps),
            "portable layer or concrete facade boundary drift",
        )?;
    }
    require(
        nodes["radroots"]
            .dependencies
            .iter()
            .any(|d| d == "execution"),
        "ordinary facade must execute shared commands",
    )?;
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    for key in nodes.keys() {
        visit(key, &nodes, &mut active, &mut done)?;
    }
    Ok(())
}

fn visit<'a>(
    key: &'a str,
    nodes: &BTreeMap<&'a str, &'a Package>,
    active: &mut BTreeSet<&'a str>,
    done: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    if done.contains(key) {
        return Ok(());
    }
    require(active.insert(key), "target dependency cycle")?;
    for dep in &nodes[key].dependencies {
        visit(dep, nodes, active, done)?;
    }
    active.remove(key);
    done.insert(key);
    Ok(())
}

fn validate_values(v: &Values, limits: &BTreeMap<String, u64>) -> Result<(), String> {
    require(
        v.quantity_coefficient_max == 999_999_999_999
            && v.quantity_scale_max == 6
            && v.count_quantities_integral
            && v.money_minor_max == 999_999_999_999_999
            && v.currency_exponent_max == 4
            && v.currency_registry == "pinned_supported_registry_with_verified_redistribution"
            && v.rounding == "half_even_once_per_net_line_then_checked_sum"
            && v.binding_numbers == "exact_decimal_or_integer_strings"
            && v.stock_ticks_max == 1_000_000_000_000_000_000
            && v.stock_scale_max == 6
            && v.stock_units == ["g", "kg", "mL", "L", "each"]
            && v.stock_physical_categories
                == [
                    "available",
                    "preparatory_hold",
                    "committed_hold",
                    "physical_quarantine",
                    "consumed",
                    "waste",
                    "removed",
                ]
            && v.generation_max == i64::MAX as u64,
        "exact economics, registry or binding value contract drift",
    )?;
    let expected = BTreeMap::from([
        ("public_event_bytes", 65_536),
        ("private_record_bytes", 16_384),
        ("complete_rumor_bytes", 24_576),
        ("chat_bytes", 4_096),
        ("json_depth", 16),
        ("single_profile_tag_bytes", 32_768),
        ("order_lines", 32),
        ("listing_options", 16),
        ("parent_records", 8),
        ("causal_records_per_scope", 1_000),
        ("causal_source_identities_per_collection", 8_000),
        ("read_page_events", 1_000),
        ("unknown_quarantine_events_per_scope", 1_024),
        ("live_queue_events", 256),
        ("live_queue_bytes", 4_194_304),
        ("default_relays", 4),
        ("maximum_session_relays", 16),
        ("concurrent_verifications", 4),
        ("delivery_attempts_per_target", 12),
        ("first_issuance_seconds", 120),
    ]);
    require(
        limits
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect::<BTreeMap<_, _>>()
            == expected,
        "initial resource contract requires an explicit amendment",
    )
}

fn validate_execution(e: &Execution, p: &Protocol, c: &Cutover) -> Result<(), String> {
    require(
        e.ordinary_inventory_owner.execution == "radroots_execution.ClientSession"
            && e.ordinary_inventory_owner.owned_capture_method == "capture_stock_evidence"
            && e.ordinary_inventory_owner
                .capture_drain_uses_explicit_reconciliation
            && e.ordinary_inventory_owner
                .retained_capture_survives_observer_loss
            && e.ordinary_inventory_owner.failed_capture_blocks_fresh_work
            && e.ordinary_inventory_owner.failed_capture_blocks_disposal
            && e.ordinary_inventory_owner
                .capture_preserves_actual_error_code
            && e.ordinary_inventory_owner.pending_capture_retry == "manual_reconciliation"
            && e.ordinary_inventory_owner
                .capture_reports_must_match_original
            && !e
                .ordinary_inventory_owner
                .capture_scope_metadata_renews_issuance
            && e.ordinary_inventory_owner.owned_acceptance_method == "prepare_stock_acceptance"
            && e.ordinary_inventory_owner.acceptance_is_preparation_only
            && e.ordinary_inventory_owner
                .lost_accepted_admission_is_unknown
            && e.ordinary_inventory_owner.facade == "radroots.Client"
            && e.ordinary_inventory_owner
                .selected_workspace_views_share_owner
            && e.ordinary_inventory_owner.exact_owned_attachment_required
            && e.ordinary_inventory_owner.bounded_task_host_required
            && !e
                .ordinary_inventory_owner
                .observer_loss_cancels_accepted_work
            && e.ordinary_inventory_owner
                .close_drains_before_workspace_disposal
            && !e
                .ordinary_inventory_owner
                .stock_signer_or_transport_constructed
            && e.phases == ["T0", "T1", "S", "T2", "D"]
            && e.selected_executor_required_for_stock
            && !e.fresh_same_key_cache_can_allocate
            && !e.shared_route_local_fallback_allowed
            && !e.effects_inside_stock_transaction
            && e.unknown_is_nonspendable
            && !e.expiry_proves_nonissuance
            && e.obtained_evidence_append_only
            && e.observation_time_nullable
            && e.positive_amendment_hold_before_issuance
            && e.negative_amendment_release_at_installation
            && e.handover_consumes_once
            && !e.buyer_receipt_moves_stock
            && !e.private_causal_heads_use_timestamp_lww,
        "durable issuance, stock or authority contract drift",
    )?;
    require(
        (
            p.listing_kind,
            p.chat_rumor_kind,
            p.seal_kind,
            p.gift_wrap_kind,
        ) == (30_402, 14, 13, 1_059)
            && p.commerce_kind == Some(radroots_protocol::profile::COMMERCE_KIND)
            && p.capability_kind == Some(radroots_protocol::profile::CAPABILITIES_KIND)
            && p.assignment_contract == "radroots.shared_sdk.protocol.v1"
            && p.structured_authoring == "blocked_until_wire_and_crypto_qualification"
            && p.independent_vectors_required
            && p.duplicate_json_fields_rejected,
        "protocol assignment or authenticated admission prerequisites drift",
    )?;
    require(
        c.new_persistent_namespace
            && c.protected_export_and_quarantine_required
            && c.restore_starts_nonpublishing
            && c.unknowns_and_obligations_protected
            && !c.automatic_legacy_reissue_allowed
            && c.meaningful_state_reset_requires_operator_approval,
        "protected cutover or reset authority drift",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const SOURCE: &str =
        include_str!("../../../../contracts/architecture/decisions/shared_sdk_refactor.v1.json");

    fn changed(pointer: &str, replacement: Value) -> Vec<u8> {
        let mut value: Value = serde_json::from_str(SOURCE).unwrap();
        *value.pointer_mut(pointer).expect("fixture path") = replacement;
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn approved_scope_is_valid_without_claiming_current_implementation() {
        validate_bytes(SOURCE.as_bytes()).unwrap();
    }

    #[test]
    fn physical_accounting_cannot_mint_stock_erase_deficits_or_claim_native_effects() {
        for (field, value) in [
            ("value_owner", json!("application")),
            ("scope", json!("mixed_pool_units")),
            ("obligation_equation", json!("remaining = backed")),
            ("ticks_max", json!(1_000_000_000_000_000_001u64)),
            ("values_create_store_effects", json!(true)),
            ("deficit_counts_as_physical_stock", json!(true)),
            ("receipt_consumes_again", json!(true)),
            ("released_deficit_adds_available", json!(true)),
            ("loss_deletes_obligation", json!(true)),
            (
                "native_atomic_allocation_and_identity_required",
                json!(false),
            ),
        ] {
            assert!(
                validate_bytes(&changed(&format!("/stock_accounting/{field}"), value)).is_err()
            );
        }
    }

    #[test]
    fn shared_consumers_cannot_silently_lose_buyer_or_seller_roles() {
        for host in ["cli", "browser", "generic_ffi"] {
            assert!(
                validate_bytes(&changed(&format!("/roles/{host}"), json!(["seller"]))).is_err()
            );
        }
        assert!(validate_bytes(&changed("/roles/tera", json!(["buyer", "seller"]))).is_err());
        assert!(validate_bytes(&changed("/roles/cli", json!(["buyer", "buyer"]))).is_err());
    }

    #[test]
    fn active_compiler_targets_do_not_relabel_historical_or_dedicated_toolchains() {
        for field in ["stable", "msrv"] {
            assert!(
                validate_bytes(&changed(&format!("/toolchain/{field}"), json!("1.97.1"))).is_err()
            );
        }
        assert!(
            validate_bytes(&changed(
                "/toolchain/historical_receipts_immutable",
                json!(false)
            ))
            .is_err()
        );
        assert!(
            validate_bytes(&changed(
                "/toolchain/dedicated_nightly_and_sp1_separate",
                json!(false)
            ))
            .is_err()
        );
    }

    #[test]
    fn browser_local_ports_and_truthful_memory_capabilities_are_required() {
        assert!(validate_bytes(&changed("/portability/browser_futures", json!("send"))).is_err());
        for field in [
            "unsafe_send_bridge_allowed",
            "memory_store_firm_stock_allowed",
            "portable_core_has_native_hosts",
            "nostr_forces_media",
        ] {
            assert!(
                validate_bytes(&changed(&format!("/portability/{field}"), json!(true))).is_err()
            );
        }
        assert!(
            validate_bytes(&changed(
                "/targets/browser_contexts",
                json!(["main_thread"])
            ))
            .is_err()
        );
        assert!(
            validate_bytes(&changed(
                "/targets/actual_beta_iphone_required",
                json!(false)
            ))
            .is_err()
        );
    }

    #[test]
    fn dependency_graph_rejects_undefined_edges_cycles_private_paths_and_layer_inversion() {
        for deps in [
            json!(["undeclared_private_engine"]),
            json!(["runtime_paths"]),
            json!(["identity", "identity"]),
        ] {
            assert!(validate_bytes(&changed("/packages/0/dependencies", deps)).is_err());
        }
        let mut value: Value = serde_json::from_str(SOURCE).unwrap();
        let index = value["packages"]
            .as_array()
            .unwrap()
            .iter()
            .position(|p| p["key"] == "runtime_paths")
            .unwrap();
        value["packages"][index]["dependencies"] = json!(["runtime_paths"]);
        assert!(
            validate_bytes(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .contains("cycle")
        );
        assert!(
            validate_bytes(&changed(
                "/packages/0/directory",
                json!("../enterprise/identity")
            ))
            .is_err()
        );
    }

    #[test]
    fn retained_capabilities_and_independent_implementation_cannot_be_removed() {
        assert!(
            validate_bytes(&changed("/retained_capability_families", json!(["farm"]))).is_err()
        );
        assert!(
            validate_bytes(&changed(
                "/provenance/relabel_app_source_license",
                json!(true)
            ))
            .is_err()
        );
        assert!(
            validate_bytes(&changed(
                "/provenance/map_meaningful_removed_tests_before_removal",
                json!(false)
            ))
            .is_err()
        );
        assert!(
            validate_bytes(&changed(
                "/ownership/app_owns_shared_semantics",
                json!(true)
            ))
            .is_err()
        );
    }

    #[test]
    fn local_outboxes_cannot_upgrade_legacy_buyer_or_claim_stock_and_restored_history() {
        for (field, bad) in [
            ("scopes", json!(["new_buyer_outbox", "seller_stock"])),
            (
                "scopes",
                json!(["new_buyer_outbox", "new_buyer_outbox", "new_seller_outbox"]),
            ),
            ("seller_enrollment_explicit", json!(false)),
            (
                "scope_retained_in_authenticated_binding_and_preparation",
                json!(false),
            ),
            ("legacy_preparation_v2_scope", json!("new_seller_outbox")),
            (
                "scope_separates_route_genesis_and_credential_commitments",
                json!(false),
            ),
            ("coverage", json!("satisfied")),
            ("stock_authority", json!(true)),
            ("organization_or_delegation_authority", json!(true)),
            ("existing_order_or_history_recovery", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(&format!("/local_outbox_authority/{field}"), bad)).is_err(),
                "{field}"
            );
        }
        let mut unknown: Value = serde_json::from_str(SOURCE).unwrap();
        unknown["local_outbox_authority"]["unknown"] = json!(true);
        assert!(validate_bytes(&serde_json::to_vec(&unknown).unwrap()).is_err());
    }
    #[test]
    fn signing_unknown_and_receipt_cannot_become_spend_authority() {
        for (field, bad) in [
            ("fresh_same_key_cache_can_allocate", true),
            ("shared_route_local_fallback_allowed", true),
            ("expiry_proves_nonissuance", true),
            ("unknown_is_nonspendable", false),
            ("obtained_evidence_append_only", false),
            ("observation_time_nullable", false),
            ("buyer_receipt_moves_stock", true),
            ("private_causal_heads_use_timestamp_lww", true),
        ] {
            assert!(validate_bytes(&changed(&format!("/execution/{field}"), json!(bad))).is_err());
        }
    }

    #[test]
    fn stock_claims_require_confirmed_dispatch_and_clock_independent_original_capture() {
        for (field, bad) in [
            ("coherent_operation_snapshot_required", json!(false)),
            (
                "final_evidence_selectors_and_page_rows_required",
                json!(false),
            ),
            (
                "recovery_view_has_dispatch_or_installation_rights",
                json!(true),
            ),
            ("fresh_current_backing_projection_required", json!(false)),
            ("evidence_page_max", json!(33)),
            ("claim_family", json!(3)),
            ("evidence_family", json!(4)),
            ("claim_worker_charge_bytes", json!(16384)),
            ("capture_worker_charge_bytes", json!(16384)),
            ("capture_raw_max_bytes", json!(65537)),
            ("shared_capture_port", json!("arbitrary_transaction")),
            ("shared_reconciliation_port", json!("blind_capture_retry")),
            (
                "reconciliation_retains_writer_through_replacement",
                json!(false),
            ),
            (
                "reconciliation_authenticates_before_connection_close",
                json!(false),
            ),
            (
                "reconciliation_preflight_initializes_or_migrates",
                json!(true),
            ),
            (
                "reconciliation_clears_fence_only_after_confirmed_capture",
                json!(false),
            ),
            (
                "shared_capture_model_owner",
                json!("radroots_storage_sqlite"),
            ),
            ("shared_capture_descriptors_are_authority", json!(true)),
            ("shared_capture_reauthenticates_original", json!(false)),
            ("native_capture_report_aliases", json!(true)),
            ("first_dispatch_requires_confirmed_commit", json!(false)),
            ("existing_claim_returns_dispatch_permit", json!(true)),
            (
                "protected_identity_presence_and_absence_required",
                json!(false),
            ),
            (
                "capture_requires_original_claim_and_credential",
                json!(false),
            ),
            ("capture_requires_current_consent", json!(true)),
            ("capture_requires_observation_clock", json!(true)),
            ("capture_preserves_first_optional_time", json!(false)),
            ("capture_retains_distinct_valid_encodings", json!(false)),
            ("agreement_installed", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/signing_issuance/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn initial_installation_requires_peer_evidence_exact_conservation_and_atomic_recipient_work() {
        for (field, bad) in [
            ("payload_version", json!(2)),
            ("journal_family", json!(7)),
            ("worker_charge_bytes", json!(1048577)),
            ("required_peer_evidence", json!(false)),
            ("missing_self_copy_blocks_installation", json!(true)),
            ("original_order_fence_immutable", json!(false)),
            ("exact_original_hold_quantities_required", json!(false)),
            ("current_exact_recovery_consent_required", json!(false)),
            ("fresh_time_not_before_original_bound_plan", json!(false)),
            ("atomic_pool_agreement_and_work_required", json!(false)),
            ("recipient_work_count", json!(3)),
            ("unknown_commit_fences_owner", json!(false)),
            ("signer_or_transport_invoked", json!(true)),
            ("relay_delivery_claimed", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!(
                        "/native_seller_stock_authority/initial_agreement_installation/{field}"
                    ),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn abort_requires_protected_absence_exact_original_release_and_retained_preparations() {
        for (field, bad) in [
            ("shared_storage_port", json!("native_only")),
            ("shared_model_owner", json!("radroots_storage_sqlite")),
            ("native_workspace_type_aliases", json!(true)),
            ("unsupported_hosts_return", json!("aborted")),
            ("payload_version", json!(3)),
            ("journal_family", json!(6)),
            ("worker_charge_bytes", json!(1048577)),
            ("retained_payload_versions", json!([])),
            ("retained_v1_requires_absent_claims", json!(false)),
            ("claimed_cleanup_catalogs", json!([14, 15])),
            ("untracked_claimed_catalogs", json!([13])),
            ("per_recipient_proof_count", json!(1)),
            ("original_unissued_required_for_every_claim", json!(false)),
            ("replay_admission_blocks_release", json!(false)),
            ("obtained_evidence_blocks_release", json!(false)),
            ("contradictory_evidence_retained", json!(false)),
            ("contradiction_requires_recovery", json!(false)),
            ("caller_reports_are_release_authority", json!(true)),
            ("caller_nonissuance_assertion_is_proof", json!(true)),
            ("current_exact_recovery_consent_required", json!(false)),
            ("atomic_exact_original_release_required", json!(false)),
            ("original_preparations_retained", json!(false)),
            ("current_order_projection_authenticated", json!(false)),
            ("later_initial_requires_original_abort", json!(false)),
            ("fresh_signing_after_abort", json!(true)),
            ("unknown_commit_fences_owner", json!(false)),
            ("signer_or_transport_invoked", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/preparation_abort/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn ordinary_inventory_keeps_selected_owner_and_accepted_task_lifetime() {
        for (field, bad) in [
            ("owned_acceptance_method", json!("inline_fallback")),
            ("owned_capture_method", json!("drop_after_refusal")),
            ("capture_drain_uses_explicit_reconciliation", json!(false)),
            ("retained_capture_survives_observer_loss", json!(false)),
            ("failed_capture_blocks_fresh_work", json!(false)),
            ("failed_capture_blocks_disposal", json!(false)),
            ("capture_preserves_actual_error_code", json!(false)),
            ("pending_capture_retry", json!("new_signing_attempt")),
            ("capture_reports_must_match_original", json!(false)),
            ("capture_scope_metadata_renews_issuance", json!(true)),
            ("acceptance_is_preparation_only", json!(false)),
            ("lost_accepted_admission_is_unknown", json!(false)),
            ("execution", json!("application")),
            ("facade", json!("native_backend")),
            ("selected_workspace_views_share_owner", json!(false)),
            ("exact_owned_attachment_required", json!(false)),
            ("bounded_task_host_required", json!(false)),
            ("observer_loss_cancels_accepted_work", json!(true)),
            ("close_drains_before_workspace_disposal", json!(false)),
            ("stock_signer_or_transport_constructed", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/execution/ordinary_inventory_owner/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn qualified_local_recovery_keeps_original_policy_window_and_current_rights() {
        for (field, bad) in [
            ("scoped_method", json!("legacy_replay")),
            ("scoped_identity_and_capability_rechecked", json!(false)),
            ("scoped_callback_binding_enforced", json!(false)),
            ("scoped_close_waits_for_active_call", json!(false)),
            ("scoped_method_grants_store_authority", json!(true)),
            ("provider_method", json!("sign_exact")),
            ("adapter", json!("legacy_signer")),
            ("native_authorizer", json!("caller_assertion")),
            ("captured_replay", json!("non_replayable")),
            ("native_catalog", json!(12)),
            ("worker_charge_bytes", json!(1048577)),
            ("existing_protected_claim_required", json!(false)),
            ("current_execute_consent_required", json!(false)),
            ("original_window_required", json!(false)),
            ("call_budget_extends_original_window", json!(true)),
            ("recovery_refusal_proves_original_unissued", json!(true)),
            ("creates_claim_or_stock_effect", json!(true)),
            ("provider_work_inside_sql", json!(true)),
            ("unsupported_legacy_fallback", json!(true)),
            ("after_expiry_retrieval_available", json!(true)),
            ("legacy_and_first_dispatch_unchanged", json!(false)),
            ("current_replay_admission_required", json!(false)),
            ("replay_admission_journal_family", json!(8)),
            ("replay_admission_payload_version", json!(2)),
            ("replay_admission_payload_bytes", json!(185)),
            ("replay_admission_unknown_returns_permit", json!(true)),
            ("replay_admission_reauthorizes_current", json!(false)),
            ("replay_admission_clear_on_refusal", json!(true)),
            ("replay_admission_stock_effect", json!(true)),
            ("retained_untracked_catalogs", json!([15])),
            ("replay_admission_report_is_release_authority", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/qualified_local_recovery/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn public_sources_require_original_signatures_bounded_parts_and_profile_independent_heads() {
        for (field, bad) in [
            ("schema_version", json!(7)),
            ("retained_inventory_versions", json!([2, 3, 4, 5, 6])),
            ("public_source_payload_schemas", json!([])),
            ("public_source_owner_administration_required", json!(false)),
            ("inventory_scope_grants_public_source_access", json!(true)),
            ("public_source_checkpoint_independent", json!(false)),
            (
                "public_source_input_requires_signature_evidence",
                json!(false),
            ),
            ("public_source_read_reverifies_signature", json!(false)),
            ("public_source_global_completeness_claimed", json!(true)),
            ("public_source_profile_implies_actionability", json!(true)),
            ("public_source_head_policy", json!("latest_supported")),
            (
                "public_source_duplicate_encodings",
                json!("replace_original"),
            ),
            ("public_source_raw_max_bytes", json!(65537)),
            ("public_source_part_max_bytes", json!(16385)),
            ("public_source_part_max_count", json!(5)),
            ("public_source_worker_charge_bytes", json!(16384)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn acceptance_uses_shared_models_without_native_aliases_or_runtime_fallback() {
        for (field, bad) in [
            ("shared_storage_port", json!("native_only")),
            (
                "shared_model_owner",
                json!("radroots_storage_sqlite.workspace"),
            ),
            ("legacy_native_aliases", json!(true)),
            ("unsupported_hosts_refuse", json!(false)),
            ("model_constructs_runtime", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/acceptance_preparation/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn signing_plans_require_complete_original_qualified_intents_before_any_signing() {
        for (field, bad) in [
            ("effectful_command_result", json!("unbounded_error")),
            (
                "generic_obtained_owned_before_descriptor_conversion",
                json!(false),
            ),
            (
                "retained_drain_preserves_original_error_and_operation",
                json!(false),
            ),
            ("common_error_max_bytes", json!(128)),
            ("execution_failure_max_bytes", json!(256)),
            ("rejected_signer_error", json!("success")),
            ("rejected_signer_metadata_owner", json!("raw_payload")),
            ("rejected_signer_report_is_nonissuance", json!(true)),
            (
                "generic_peer_installed_before_rejected_self_return",
                json!(false),
            ),
            ("unknown_installation_blocks_delivery", json!(false)),
            ("native_first_dispatch_owns_issuer_invocation", json!(false)),
            ("shared_first_dispatch_port", json!("into_intent")),
            ("shared_unissued_owner", json!("caller_waiter")),
            ("shared_unissued_retained_before_capture", json!(false)),
            ("shared_unissued_drained_before_storage_close", json!(false)),
            (
                "shared_unissued_failure_blocks_fresh_commands",
                json!(false),
            ),
            ("public_native_intent_extraction", json!(true)),
            (
                "coherent_snapshot_preserves_original_unissued",
                json!(false),
            ),
            ("exact_issuer_port", json!("legacy_signer_fallback")),
            ("native_failure_boundary", json!("discarded_primitive")),
            ("native_failure_preserves_signer_rejection", json!(false)),
            ("native_failure_fingerprint_bytes", json!(64)),
            ("native_failure_coverage", json!("floating_point")),
            ("native_failure_max_bytes", json!(256)),
            ("browser_failure_boundary", json!("discarded_primitive")),
            ("browser_failure_preserves_signer_rejection", json!(false)),
            ("browser_failure_max_bytes", json!(256)),
            (
                "original_unissued_live_observation_owner",
                json!("caller_assertion"),
            ),
            (
                "original_unissued_capture_owner",
                json!("caller_certificate"),
            ),
            ("original_unissued_payload_version", json!(2)),
            ("original_unissued_journal_family", json!(5)),
            (
                "original_unissued_capture_requires_owned_observation",
                json!(false),
            ),
            (
                "original_unissued_capture_borrows_observation",
                json!(false),
            ),
            (
                "original_unissued_capture_rechecks_original_claim",
                json!(false),
            ),
            (
                "original_unissued_capture_requires_fresh_permission",
                json!(true),
            ),
            (
                "original_unissued_capture_requires_fresh_clock",
                json!(true),
            ),
            ("original_unissued_capture_releases_stock", json!(true)),
            ("original_unissued_reconciliation_same_owner", json!(false)),
            ("original_unissued_late_evidence_retained", json!(false)),
            ("original_unissued_worker_charge_bytes", json!(1)),
            ("original_unissued_observation_is_durable", json!(true)),
            ("original_unissued_observation_releases_stock", json!(true)),
            (
                "recovery_refusal_creates_original_unissued_observation",
                json!(true),
            ),
            (
                "ordinary_obtained_owned_before_descriptor_conversion",
                json!(false),
            ),
            ("ordinary_acceptance_owner", json!("application")),
            ("ordinary_acceptance_facade", json!("radroots_sdk")),
            ("ordinary_peer_installed_before_self_work", json!(false)),
            (
                "ordinary_accepted_task_survives_observer_loss",
                json!(false),
            ),
            ("ordinary_existing_claim_grants_first_dispatch", json!(true)),
            (
                "ordinary_replay_requires_original_policy_and_current_permit",
                json!(false),
            ),
            ("ordinary_acceptance_implies_delivery", json!(true)),
            ("crypto_verifier_owner", json!("radroots_nostr.commerce")),
            (
                "scoped_crypto_verification",
                json!("ParticipantProvider.sign_exact"),
            ),
            ("predecessor_nostr_verifier_alias", json!(true)),
            ("crypto_verification_grants_dispatch_authority", json!(true)),
            ("payload_version", json!(2)),
            ("journal_family", json!(2)),
            ("worker_charge_bytes", json!(524288)),
            ("complete_recipient_count", json!(1)),
            ("crypto_and_route_verification_required", json!(false)),
            ("original_t0_rumor_required", json!(false)),
            ("exact_intent_digests_retained", json!(false)),
            (
                "protected_identity_presence_and_absence_required",
                json!(false),
            ),
            ("current_issued_consent_required", json!(false)),
            (
                "final_permission_and_clock_witnesses_required",
                json!(false),
            ),
            ("signer_invoked", json!(true)),
            ("agreement_installed", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/signing_plan_binding/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn acceptance_requires_complete_atomic_preparation_without_signing_or_installation() {
        for (field, bad) in [
            ("payload_version", json!(2)),
            ("planner_owner", json!("radroots_storage_sqlite")),
            ("order_payload_schema", json!("unsupported")),
            ("movement_payload_schema", json!("unsupported")),
            ("worker_charge_bytes", json!(524289)),
            ("max_allocations", json!(65)),
            ("max_pool_observations", json!(97)),
            ("max_first_issue_window_seconds", json!(121)),
            ("original_replay_before_fresh_planning", json!(false)),
            ("complete_allocation_required", json!(false)),
            ("atomic_holds_and_order_fence_required", json!(false)),
            ("current_issued_consent_required", json!(false)),
            ("final_source_counts_and_witnesses_required", json!(false)),
            ("signer_invoked", json!(true)),
            ("agreement_installed", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/acceptance_preparation/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn consent_ledger_requires_bounded_parts_explicit_scopes_and_protected_identity() {
        for (field, bad) in [
            ("schema_version", json!(6)),
            ("retained_inventory_versions", json!([2, 3, 4, 5])),
            (
                "operation_part_payload_schema",
                json!("radroots.seller_stock.operation_part.v2"),
            ),
            ("operation_body_max_bytes", json!(262145)),
            ("operation_part_max_bytes", json!(16385)),
            ("operation_part_max_count", json!(17)),
            (
                "operation_identity_requires_protected_ancestry",
                json!(false),
            ),
            ("business_consent_payload_version", json!(2)),
            ("business_consent_action", json!("stock.add")),
            ("business_consent_modes", json!(["execute_and_recover"])),
            ("business_consent_admin_issuance_required", json!(false)),
            (
                "business_consent_binds_exact_command_and_subject_grant",
                json!(false),
            ),
            (
                "business_consent_issuance_has_order_or_stock_effects",
                json!(true),
            ),
            ("business_consent_worker_charge_bytes", json!(16384)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn causal_history_contract_requires_current_access_full_scope_and_non_actionability() {
        for (field, bad) in [
            (
                "causal_history_current_administrator_required",
                json!(false),
            ),
            ("causal_history_complete_local_scope_required", json!(false)),
            (
                "causal_history_includes_actual_local_signed_records",
                json!(false),
            ),
            ("causal_history_inventory_minimum_required", json!(false)),
            (
                "causal_history_logical_record_deduplication_required",
                json!(false),
            ),
            ("causal_history_implies_installation", json!(true)),
            (
                "causal_history_uninterpreted_policy",
                json!("ignore_unknown_scope"),
            ),
            (
                "causal_history_transaction_witnesses_required",
                json!(false),
            ),
            ("causal_history_implies_business_permission", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn source_retention_contract_requires_provenance_bounds_and_separate_authority() {
        for (field, bad) in [
            ("schema_version", json!(5)),
            ("retained_inventory_versions", json!([2, 3, 4])),
            ("source_payload_schemas", json!([])),
            ("source_profile_dispositions", json!(["known"])),
            ("source_owner_administration_required", json!(false)),
            ("inventory_scope_grants_source_access", json!(true)),
            ("source_checkpoint_independent", json!(false)),
            ("source_profile_implies_actionability", json!(true)),
            ("source_carrier_max_bytes", json!(65537)),
            ("source_chunk_max_bytes", json!(16385)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn native_mapping_contract_preserves_catalogs_owner_access_and_physical_non_effects() {
        for (field, bad) in [
            ("schema_version", json!(4)),
            ("retained_inventory_versions", json!([2, 3])),
            (
                "mapping_payload_schema",
                json!("radroots.seller_stock.mapping.v2"),
            ),
            (
                "inventory_operation_payload_schema",
                json!("radroots.seller_stock.operation.v3"),
            ),
            (
                "authority_payload_schema",
                json!("radroots.seller_stock.authority.v4"),
            ),
            ("mapping_owner_administration_required", json!(false)),
            ("mapping_changes_physical_stock", json!(true)),
            ("mapping_changes_pool_version", json!(true)),
            ("mapping_create_expected_version", json!(1)),
            ("inventory_scope_grants_administration", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn assigned_protocol_kinds_and_target_metadata_cannot_authorize_publication() {
        assert!(validate_bytes(&changed("/protocol/commerce_kind", json!(8891))).is_err());
        assert!(validate_bytes(&changed("/protocol/commerce_kind", json!(null))).is_err());
        assert!(validate_bytes(&changed("/protocol/capability_kind", json!(38990))).is_err());
        assert!(validate_bytes(&changed("/protocol/capability_kind", json!(null))).is_err());
        assert!(validate_bytes(&changed("/protocol/assignment_contract", json!("other"))).is_err());
        assert!(
            validate_bytes(&changed(
                "/protocol/structured_authoring",
                json!("qualified")
            ))
            .is_err()
        );
        assert!(validate_bytes(&changed("/publication_authorized", json!(true))).is_err());
        assert!(
            validate_bytes(&changed(
                "/qualification/implementation_claimed",
                json!(true)
            ))
            .is_err()
        );
        assert!(
            validate_bytes(&changed(
                "/cutover/automatic_legacy_reissue_allowed",
                json!(true)
            ))
            .is_err()
        );
    }

    #[test]
    fn stock_rewrapping_cannot_replace_originals_reset_policy_or_invent_dispatch() {
        for (field, bad) in [
            ("journal_family", json!(13)),
            ("admission_journal_family", json!(14)),
            ("admission_payload_bytes", json!(247)),
            ("admission_window_seconds_max", json!(31)),
            ("explicit_request_id_required", json!(false)),
            ("newly_confirmed_admission_before_wrapping", json!(false)),
            (
                "existing_or_uncertain_admission_grants_invocation",
                json!(true),
            ),
            ("unknown_admissions_count_toward_limit", json!(false)),
            ("descriptor_includes_uncaptured_admissions", json!(false)),
            ("admission_repair_grants_history_or_invocation", json!(true)),
            ("ordinary_owner", json!("application")),
            ("ordinary_facade_methods", json!([])),
            (
                "ordinary_resume_generates_key_or_business_signature",
                json!(true),
            ),
            ("ordinary_status_is_atomic_absence_proof", json!(true)),
            ("supported_catalogs", json!([17, 18])),
            ("payload_prefix_bytes", json!(118)),
            ("additional_envelopes_per_recipient_max", json!(193)),
            ("descriptor_page_max", json!(17)),
            ("default_selection_is_original", json!(false)),
            ("new_envelope_selection_explicit", json!(false)),
            ("first_policy_source_bound", json!(false)),
            ("transport_counters_cumulative", json!(false)),
            ("outcomes_scoped_to_exact_envelope", json!(false)),
            ("natural_index_transaction_witness_required", json!(false)),
            ("existing_bytes_may_be_replaced", json!(true)),
            ("capture_or_query_grants_dispatch", json!(true)),
            ("physical_stock_effect", json!(true)),
            ("temporary_key_retained", json!(true)),
            ("provider_io_inside_sql", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/rewrapping/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn stock_envelope_retention_cannot_become_dispatch_permission_or_a_cache_authority() {
        for (field, bad) in [
            ("journal_family", json!(9)),
            ("payload_prefix_bytes", json!(212)),
            ("supported_catalogs", json!([15, 16])),
            ("raw_max_bytes", json!(65537)),
            ("worker_charge_bytes", json!(65536)),
            (
                "original_preparation_plan_claim_seal_installation_required",
                json!(false),
            ),
            (
                "temporary_key_derivation_and_exact_decryption_required",
                json!(false),
            ),
            ("temporary_key_retained", json!(true)),
            ("original_party_as_outer_author_allowed", json!(true)),
            (
                "body_derived_projection_and_complete_ancestry_required",
                json!(false),
            ),
            (
                "original_credential_capture_requires_live_clock_or_grant",
                json!(true),
            ),
            ("query_requires_current_recovery_access", json!(false)),
            ("observation_nullable", json!(false)),
            ("existing_original_bytes_may_be_replaced", json!(true)),
            ("capture_grants_dispatch", json!(true)),
            ("capture_changes_physical_stock", json!(true)),
            ("provider_io_inside_sql", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/envelope_retention/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn stock_delivery_requires_fresh_confirmed_claims_and_bounded_original_outcomes() {
        for (field, bad) in [
            ("supported_catalogs", json!([16, 17])),
            ("attempt_journal_family", json!(10)),
            ("observation_journal_family", json!(11)),
            ("attempts_per_target", json!(13)),
            ("observation_page_max", json!(1000)),
            ("worker_charge_bytes", json!(65536)),
            (
                "original_stock_evidence_installation_outer_required",
                json!(false),
            ),
            (
                "original_captured_route_and_current_write_policy_required",
                json!(false),
            ),
            (
                "fresh_current_scoped_consent_grant_clock_required",
                json!(false),
            ),
            ("fresh_claim_requires_confirmed_commit", json!(false)),
            ("existing_attempt_grants_dispatch", json!(true)),
            ("unknown_commit_grants_dispatch", json!(true)),
            ("query_grants_dispatch", json!(true)),
            (
                "first_complete_targets_and_criterion_immutable",
                json!(false),
            ),
            ("ordinary_command_replays_existing_attempt", json!(true)),
            (
                "ordinary_fresh_claim_binds_observed_attempt_count",
                json!(false),
            ),
            ("ordinary_obtained_material_owner", json!("application")),
            (
                "ordinary_repeat_uses_selected_envelope_attempt_count",
                json!(false),
            ),
            ("ordinary_peer_delivery_precedes_self_signing", json!(false)),
            ("installed_original_delivery_requires_signer", json!(true)),
            ("retries_use_original_outer_bytes", json!(false)),
            ("observation_nullable", json!(false)),
            (
                "original_credential_capture_requires_live_clock_or_grant",
                json!(true),
            ),
            (
                "same_owner_obtained_material_reconciliation_required",
                json!(false),
            ),
            ("first_observation_per_class_immutable", json!(false)),
            ("wrong_event_request_target_ack_rejected", json!(false)),
            ("final_transaction_witnesses_required", json!(false)),
            ("unbounded_attempt_or_observation_collections", json!(true)),
            ("physical_stock_effect", json!(true)),
            ("transport_inside_sql", json!(true)),
        ] {
            assert!(
                validate_bytes(&changed(
                    &format!("/native_seller_stock_authority/delivery/{field}"),
                    bad
                ))
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn initial_limits_and_exact_binding_economics_require_explicit_amendment() {
        for (pointer, bad) in [
            ("/limits/public_event_bytes", json!(65537)),
            ("/limits/live_queue_bytes", json!(0)),
            ("/limits/order_lines", json!(33)),
            ("/limits/causal_records_per_scope", json!(1001)),
            ("/limits/causal_source_identities_per_collection", json!(0)),
            ("/values/count_quantities_integral", json!(false)),
            ("/values/currency_exponent_max", json!(5)),
            ("/values/rounding", json!("away_from_zero")),
        ] {
            assert!(validate_bytes(&changed(pointer, bad)).is_err());
        }
    }

    #[test]
    fn malformed_oversized_duplicate_or_unknown_input_has_bounded_fixed_errors() {
        assert!(
            validate_bytes(&vec![b' '; MAX_BYTES + 1])
                .unwrap_err()
                .contains("byte limit")
        );
        for raw in [
            b"{\"schema_version\":1,\"schema_version\":1}".as_slice(),
            b"{\"private_marker\":\"synthetic-private-input\"}",
            b"not json",
        ] {
            let error = validate_bytes(raw).unwrap_err();
            assert_eq!(error, "shared SDK contract: invalid closed JSON shape");
            assert!(!error.contains("synthetic-private-input"));
        }
    }
}
