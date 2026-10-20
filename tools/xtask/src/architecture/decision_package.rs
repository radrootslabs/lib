//! Prospective shared SDK authority. Validation does not claim implementation.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::Deserialize;

const CONTRACT: &str = "contracts/architecture/decisions/shared_sdk_refactor.v1.json";
const MAX_BYTES: usize = 65_536;

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
    provenance: Provenance,
    targets: Targets,
    portability: Portability,
    packages: Vec<Package>,
    values: Values,
    limits: BTreeMap<String, u64>,
    execution: Execution,
    protocol: Protocol,
    cutover: Cutover,
    qualification: Qualification,
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
    generation_max: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Execution {
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
struct Protocol {
    listing_kind: u16,
    chat_rumor_kind: u16,
    seal_kind: u16,
    gift_wrap_kind: u16,
    commerce_kind: Option<u16>,
    capability_kind: Option<u16>,
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
    validate_bytes(&raw)
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
        e.phases == ["T0", "T1", "S", "T2", "D"]
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
            && p.commerce_kind.is_none()
            && p.capability_kind.is_none()
            && p.structured_authoring == "blocked_until_qualified_kind_assignment"
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
    fn unassigned_kinds_and_target_metadata_cannot_authorize_publication() {
        assert!(validate_bytes(&changed("/protocol/commerce_kind", json!(8890))).is_err());
        assert!(validate_bytes(&changed("/protocol/capability_kind", json!(38990))).is_err());
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
    fn initial_limits_and_exact_binding_economics_require_explicit_amendment() {
        for (pointer, bad) in [
            ("/limits/public_event_bytes", json!(65537)),
            ("/limits/live_queue_bytes", json!(0)),
            ("/limits/order_lines", json!(33)),
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
