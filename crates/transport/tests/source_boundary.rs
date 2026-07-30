use std::{
    fs,
    path::{Path, PathBuf},
};

struct ForbiddenConcept {
    pattern: &'static str,
    reason: &'static str,
}

const TRANSPORT_HARDENING_CRATE_SOURCE_ROOTS: &[&str] = &[
    "transport/src",
    "transport_reticulum/src",
    "transport_publish_protocol/src",
    "transport_nostr/src",
    "outbox/src",
];

const GENERIC_TRANSPORT_STATUS_SOURCE_ROOTS: &[&str] = &[
    "event_store/src",
    "mesh_agent_proto/src",
    "outbox/src",
    "transport/src",
    "transport_publish_protocol/src",
    "transport_reticulum/src",
];

const CORE_STATUS_CONTRACT_SOURCE_ROOTS: &[&str] = &["transport/src", "transport_reticulum/src"];

const CORE_TRANSPORT_CONTRACT_SOURCE_ROOTS: &[&str] = &["transport/src"];

const DELIVERY_PAYLOAD_CONTRACT_SOURCE_ROOTS: &[&str] =
    &["transport/src", "runtime/src", "transport_reticulum/src"];

const FOUNDATION_HARDENING_DOC_ROOTS: &[&str] = &["contracts", "docs"];

const FORBIDDEN_TRANSPORT_CONCEPTS: &[ForbiddenConcept] = &[
    ForbiddenConcept {
        pattern: concat!("\"radrootsd", "_", "pro", "xy\""),
        reason: "radrootsd execution must not be modeled as transport identity",
    },
    ForbiddenConcept {
        pattern: concat!("radrootsd.publish", "_", "pro", "xy.v1"),
        reason: "transport publish protocol v1 radrootsd execution identifiers are removed",
    },
    ForbiddenConcept {
        pattern: "publish.relays.resolve",
        reason: "relay-resolution RPC is replaced by transport publish target policy",
    },
    ForbiddenConcept {
        pattern: "\"publish.event\"",
        reason: "publish.event is replaced by transport.publish.event",
    },
    ForbiddenConcept {
        pattern: "transport_kinds",
        reason: "capabilities must expose per-transport readiness instead of kind-only lists",
    },
    ForbiddenConcept {
        pattern: "allowed_relay_policy",
        reason: "relay policy is Nostr-specific and must not be a generic transport API",
    },
    ForbiddenConcept {
        pattern: "relay_policy",
        reason: "relay policy is Nostr-specific and must not be a generic transport API",
    },
    ForbiddenConcept {
        pattern: "PublishRelayPolicy",
        reason: "old relay-shaped publish policy names must not return",
    },
    ForbiddenConcept {
        pattern: "PublishRelayOutcome",
        reason: "old relay-shaped publish outcome names must not return",
    },
    ForbiddenConcept {
        pattern: "PublishRelaySource",
        reason: "old relay-shaped publish source names must not return",
    },
    ForbiddenConcept {
        pattern: concat!("Nostr", "Fetch"),
        reason: "generic transport observations must use transport-neutral fetch naming",
    },
    ForbiddenConcept {
        pattern: concat!("Nostr", "Subscription"),
        reason: "generic transport observations must use transport-neutral subscription naming",
    },
    ForbiddenConcept {
        pattern: concat!("Nostr", "PublishAck"),
        reason: "generic transport observations must use transport-neutral publish ack naming",
    },
    ForbiddenConcept {
        pattern: concat!("nostr", "_fetch"),
        reason: "generic transport observation storage strings must be transport-neutral",
    },
    ForbiddenConcept {
        pattern: concat!("nostr", "_subscription"),
        reason: "generic transport observation storage strings must be transport-neutral",
    },
    ForbiddenConcept {
        pattern: concat!("nostr", "_publish_ack"),
        reason: "generic transport observation storage strings must be transport-neutral",
    },
];

const FORBIDDEN_CORE_STATUS_CONCEPTS: &[ForbiddenConcept] = &[
    ForbiddenConcept {
        pattern: "implementation_state",
        reason: "public transport status must use implementation",
    },
    ForbiddenConcept {
        pattern: "readiness",
        reason: "public transport status must use configured, usable_for_delivery, and message",
    },
    ForbiddenConcept {
        pattern: "publish_usable",
        reason: "public transport status must use usable_for_delivery",
    },
    ForbiddenConcept {
        pattern: "fetch_usable",
        reason: "public transport status must use usable_for_delivery",
    },
    ForbiddenConcept {
        pattern: "redacted_message",
        reason: "public transport status must use message",
    },
    ForbiddenConcept {
        pattern: "RadrootsTransportReadinessState",
        reason: "readiness state is no longer a public transport status contract",
    },
    ForbiddenConcept {
        pattern: "Misconfigured",
        reason: "configuration is modeled by configured and message",
    },
    ForbiddenConcept {
        pattern: "Disabled",
        reason: "disabled state is modeled by configured, usable_for_delivery, and message",
    },
];

const FORBIDDEN_GENERIC_TRANSPORT_STATUS_CONCEPTS: &[ForbiddenConcept] = &[
    ForbiddenConcept {
        pattern: concat!("configured_nostr", "_relay", "_count"),
        reason: "generic status surfaces must expose configured transport target counts",
    },
    ForbiddenConcept {
        pattern: concat!("configured_nostr", "_relays"),
        reason: "generic status surfaces must expose configured transport targets",
    },
    ForbiddenConcept {
        pattern: concat!("target", "_relays"),
        reason: "generic transport target surfaces must use endpoint terminology",
    },
    ForbiddenConcept {
        pattern: concat!("connected", "_relays"),
        reason: "generic transport attempt surfaces must use endpoint terminology",
    },
    ForbiddenConcept {
        pattern: concat!("acknowledged", "_relays"),
        reason: "generic transport acknowledgement surfaces must use endpoint terminology",
    },
    ForbiddenConcept {
        pattern: concat!("failed", "_relays"),
        reason: "generic transport failure surfaces must use target terminology",
    },
    ForbiddenConcept {
        pattern: concat!("relay", "_count"),
        reason: "generic transport status counts must use transport target terminology",
    },
];

const FORBIDDEN_CORE_TRANSPORT_CONCEPTS: &[ForbiddenConcept] = &[
    ForbiddenConcept {
        pattern: concat!("Radroots", "Relay"),
        reason: "core transport contracts must not expose Nostr relay-shaped APIs",
    },
    ForbiddenConcept {
        pattern: concat!("Relay", "Transport"),
        reason: "core transport contracts must use transport-neutral names",
    },
    ForbiddenConcept {
        pattern: concat!("relay", "_transport"),
        reason: "core transport contracts must use transport-neutral names",
    },
];

const FORBIDDEN_DELIVERY_PAYLOAD_CONCEPTS: &[ForbiddenConcept] = &[
    ForbiddenConcept {
        pattern: "payload_digest",
        reason: "delivery requests must carry RadrootsTransportPayload instead of digest-only fields",
    },
    ForbiddenConcept {
        pattern: "DigestOnly",
        reason: "runtime dispatch must not retain a digest-only payload path",
    },
    ForbiddenConcept {
        pattern: "RadrootsTransportPayload::signed_event_json(",
        reason: "signed-event payload construction must name unchecked validation explicitly",
    },
    ForbiddenConcept {
        pattern: "RadrootsTransportPayload::signed_event_json_with_digest(",
        reason: "signed-event digest validation must name unchecked validation explicitly",
    },
];

const FORBIDDEN_FOUNDATION_HARDENING_RETIRED_CONCEPTS: &[ForbiddenConcept] = &[
    ForbiddenConcept {
        pattern: "SignedNostrEvent",
        reason: "generic signed-event surfaces must use product-neutral signed-event names",
    },
    ForbiddenConcept {
        pattern: "RadrootsEventIndexIndexCheckpoint",
        reason: "event-index checkpoint names must not duplicate the index noun",
    },
    ForbiddenConcept {
        pattern: "RadrootsEventsIndexed",
        reason: "event-indexed APIs must use the singular event-index crate family",
    },
    ForbiddenConcept {
        pattern: "RADROOTS_EVENTS_VERSION",
        reason: "event contract version constants must use the current singular event namespace",
    },
    ForbiddenConcept {
        pattern: "radroots_events",
        reason: "crate and manifest surfaces must use the current singular event crate names",
    },
    ForbiddenConcept {
        pattern: "radroots_events_codec",
        reason: "event codec crate surfaces must use the current singular event-codec name",
    },
    ForbiddenConcept {
        pattern: "radroots_events_indexed",
        reason: "event index crate surfaces must use the current singular event-index name",
    },
    ForbiddenConcept {
        pattern: "radroots_local_events",
        reason: "local event storage must not reintroduce retired local-events crate names",
    },
    ForbiddenConcept {
        pattern: "radroots_local_store",
        reason: "runtime storage must not reintroduce retired local-store crate names",
    },
    ForbiddenConcept {
        pattern: "radroots_types",
        reason: "shared type surfaces must use current crate ownership instead of retired types crates",
    },
    ForbiddenConcept {
        pattern: "radroots_types_bindings",
        reason: "generated bindings must not reintroduce retired types-binding crate names",
    },
    ForbiddenConcept {
        pattern: "radroots_nostr_ndb",
        reason: "Nostr database ownership must not reintroduce retired ndb crate names",
    },
    ForbiddenConcept {
        pattern: "radroots_replica_db",
        reason: "replica database surfaces must use current replica-store ownership",
    },
    ForbiddenConcept {
        pattern: "radroots_replica_db_schema",
        reason: "replica schema surfaces must use current replica-schema ownership",
    },
    ForbiddenConcept {
        pattern: "radroots_sp1_guest_trade",
        reason: "trade SP1 crate surfaces must use the current trade_sp1 crate names",
    },
    ForbiddenConcept {
        pattern: "radroots_sp1_host_trade",
        reason: "trade SP1 crate surfaces must use the current trade_sp1 crate names",
    },
];

const FORBIDDEN_FOUNDATION_HARDENING_DOC_CONCEPTS: &[ForbiddenConcept] = &[
    ForbiddenConcept {
        pattern: "Nostr event timestamp",
        reason: "generic docs must describe event-envelope timestamps without protocol leakage",
    },
    ForbiddenConcept {
        pattern: "Forwarded satisfies Delivered",
        reason: "forwarded evidence must not be documented as strict delivery",
    },
    ForbiddenConcept {
        pattern: "StoredByGateway satisfies Delivered",
        reason: "gateway storage evidence must not be documented as strict delivery",
    },
    ForbiddenConcept {
        pattern: "Seen satisfies Delivered",
        reason: "seen evidence must not be documented as strict delivery",
    },
];

#[test]
fn transport_hardening_sources_reject_removed_protocol_identifiers() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");
    let mut findings = Vec::new();

    for relative_root in TRANSPORT_HARDENING_CRATE_SOURCE_ROOTS {
        for path in rust_source_files(crates_root.join(relative_root).as_path()) {
            let source_raw = read_source(path.as_path());
            let source = production_source(source_raw.as_str());
            let relative_path = relative_path(crates_root, path.as_path());

            for concept in FORBIDDEN_TRANSPORT_CONCEPTS {
                if contains_forbidden_concept(source, concept.pattern) {
                    findings.push(format!(
                        "{} contains removed transport concept `{}`: {}",
                        relative_path, concept.pattern, concept.reason
                    ));
                }
            }

            for line in removed_reticulum_stage_endpoint_lines(source) {
                findings.push(format!(
                    "{relative_path}:{line} contains removed Reticulum staging endpoint"
                ));
            }
        }
    }

    assert!(
        findings.is_empty(),
        "transport hardening source-boundary violations:\n{}",
        findings.join("\n")
    );
}

#[test]
fn core_status_contract_sources_reject_retired_public_status_fields() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");
    let mut findings = Vec::new();

    for relative_root in CORE_STATUS_CONTRACT_SOURCE_ROOTS {
        for path in rust_source_files(crates_root.join(relative_root).as_path()) {
            let source_raw = read_source(path.as_path());
            let source = production_source(source_raw.as_str());
            let relative_path = relative_path(crates_root, path.as_path());

            for concept in FORBIDDEN_CORE_STATUS_CONCEPTS {
                if contains_forbidden_concept(source, concept.pattern) {
                    findings.push(format!(
                        "{} contains retired core transport status concept `{}`: {}",
                        relative_path, concept.pattern, concept.reason
                    ));
                }
            }
        }
    }

    assert!(
        findings.is_empty(),
        "core transport status source-boundary violations:\n{}",
        findings.join("\n")
    );
}

#[test]
fn generic_transport_status_sources_reject_retired_relay_shaped_names() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");
    let mut findings = Vec::new();

    for relative_root in GENERIC_TRANSPORT_STATUS_SOURCE_ROOTS {
        for path in rust_source_files(crates_root.join(relative_root).as_path()) {
            let source_raw = read_source(path.as_path());
            let source = production_source(source_raw.as_str());
            let relative_path = relative_path(crates_root, path.as_path());

            for concept in FORBIDDEN_GENERIC_TRANSPORT_STATUS_CONCEPTS {
                if contains_forbidden_concept(source, concept.pattern) {
                    findings.push(format!(
                        "{} contains retired generic transport status concept `{}`: {}",
                        relative_path, concept.pattern, concept.reason
                    ));
                }
            }
        }
    }

    assert!(
        findings.is_empty(),
        "generic transport status source-boundary violations:\n{}",
        findings.join("\n")
    );
}

#[test]
fn core_transport_sources_reject_relay_shaped_public_contracts() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");
    let mut findings = Vec::new();

    for relative_root in CORE_TRANSPORT_CONTRACT_SOURCE_ROOTS {
        for path in rust_source_files(crates_root.join(relative_root).as_path()) {
            let source_raw = read_source(path.as_path());
            let source = production_source(source_raw.as_str());
            let relative_path = relative_path(crates_root, path.as_path());

            for concept in FORBIDDEN_CORE_TRANSPORT_CONCEPTS {
                if contains_forbidden_concept(source, concept.pattern) {
                    findings.push(format!(
                        "{} contains relay-shaped core transport concept `{}`: {}",
                        relative_path, concept.pattern, concept.reason
                    ));
                }
            }
        }
    }

    assert!(
        findings.is_empty(),
        "core transport public contract source-boundary violations:\n{}",
        findings.join("\n")
    );
}

#[test]
fn delivery_request_sources_require_payload_objects() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");
    let mut findings = Vec::new();

    for relative_root in DELIVERY_PAYLOAD_CONTRACT_SOURCE_ROOTS {
        for path in rust_source_files(crates_root.join(relative_root).as_path()) {
            let source_raw = read_source(path.as_path());
            let source = production_source(source_raw.as_str());
            let relative_path = relative_path(crates_root, path.as_path());

            for concept in FORBIDDEN_DELIVERY_PAYLOAD_CONCEPTS {
                if contains_forbidden_concept(source, concept.pattern) {
                    findings.push(format!(
                        "{} contains digest-only delivery concept `{}`: {}",
                        relative_path, concept.pattern, concept.reason
                    ));
                }
            }
        }
    }

    assert!(
        findings.is_empty(),
        "delivery payload source-boundary violations:\n{}",
        findings.join("\n")
    );
}

#[test]
fn foundation_hardening_repo_sources_reject_retired_names_and_ambiguous_docs() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");
    let repo_root = crates_root.parent().expect("repo root");
    let mut findings = Vec::new();

    for path in foundation_hardening_guard_files(repo_root) {
        let source = read_source(path.as_path());
        let relative_path = relative_path(repo_root, path.as_path());

        for concept in FORBIDDEN_FOUNDATION_HARDENING_RETIRED_CONCEPTS {
            if contains_forbidden_concept(source.as_str(), concept.pattern) {
                findings.push(format!(
                    "{} contains retired Foundation Hardening concept `{}`: {}",
                    relative_path, concept.pattern, concept.reason
                ));
            }
        }

        if is_doc_surface(path.as_path()) {
            for concept in FORBIDDEN_FOUNDATION_HARDENING_DOC_CONCEPTS {
                if source.contains(concept.pattern) {
                    findings.push(format!(
                        "{} contains ambiguous Foundation Hardening wording `{}`: {}",
                        relative_path, concept.pattern, concept.reason
                    ));
                }
            }
        }
    }

    assert!(
        findings.is_empty(),
        "Foundation Hardening V1 source-boundary violations:\n{}",
        findings.join("\n")
    );
}

#[test]
fn runtime_transport_registry_uses_core_transport_contract() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");
    let runtime_source_raw = read_source(crates_root.join("runtime/src/transport.rs").as_path());
    let runtime_source = production_source(runtime_source_raw.as_str());
    let reticulum_source_raw =
        read_source(crates_root.join("transport_reticulum/src/lib.rs").as_path());
    let reticulum_source = production_source(reticulum_source_raw.as_str());

    assert!(
        runtime_source.contains("Arc<dyn RadrootsTransport>"),
        "runtime registry must store the core RadrootsTransport trait object"
    );
    assert!(
        runtime_source.contains("T: RadrootsTransport + 'static"),
        "runtime registry registration must accept the core RadrootsTransport trait"
    );
    assert!(
        runtime_source.contains("transport.transport_kind()"),
        "runtime registry must key transports through the core trait transport_kind"
    );
    let removed_reticulum_runtime_transport =
        ["RadrootsRuntimeReticulum", "Pre", "viewTransport"].concat();
    for forbidden in [
        "pub trait RadrootsRuntimeTransportAdapter".to_owned(),
        "dyn RadrootsRuntimeTransportAdapter".to_owned(),
        removed_reticulum_runtime_transport,
    ] {
        assert!(
            !runtime_source.contains(forbidden.as_str()),
            "runtime transport source must not retain split adapter contract `{forbidden}`"
        );
    }
    assert!(
        reticulum_source.contains("impl RadrootsTransport for RadrootsReticulumTransport"),
        "Reticulum transport must implement the core transport contract"
    );
}

#[test]
fn transport_publish_capabilities_keep_canonical_status_fields() {
    let source_raw = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("transport crate parent")
            .join("transport_publish_protocol/src/lib.rs")
            .as_path(),
    );
    let source = production_source(source_raw.as_str());

    for required in [
        "pub transport: String,",
        "pub configured: bool,",
        "pub implementation: TransportPublishImplementation,",
        "pub maturity: TransportPublishCapabilityMaturity,",
        "pub availability: TransportPublishCapabilityAvailability,",
        "pub usable_for_delivery: bool,",
        "pub capabilities: TransportPublishOperationCapabilities,",
        "pub struct TransportPublishOperationCapabilities",
        "pub deliver: bool,",
        "pub fetch: bool,",
        "pub discovery: bool,",
        "pub gateway_forwarding: bool,",
        "pub receipt_observation: bool,",
        "TransportPublishImplementation::Real",
        "TransportPublishCapabilityMaturity::Preview",
        "TransportPublishCapabilityAvailability::Unavailable",
        "configured: true",
        "usable_for_delivery: true",
        "usable_for_delivery: false",
        "capabilities: TransportPublishOperationCapabilities",
        "deliver: true",
        "fetch: false",
        "discovery: false",
        "gateway_forwarding: false",
        "receipt_observation: false",
    ] {
        assert!(
            source.contains(required),
            "transport publish capabilities must retain canonical status field `{required}`"
        );
    }

    for forbidden in [
        "pub implementation_state: TransportPublishImplementationState,",
        "TransportPublishImplementationState",
    ] {
        assert!(
            !source.contains(forbidden),
            "transport publish capabilities must not retain retired status field `{forbidden}`"
        );
    }
}

#[test]
fn transport_target_identity_sources_reject_silent_dedupe() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");

    let transport_source = read_source(crates_root.join("transport/src/target.rs").as_path());
    for required in [
        "let mut fingerprints = BTreeSet::new();",
        "RadrootsTransportError::DuplicateTargetFingerprint",
    ] {
        assert!(
            transport_source.contains(required),
            "transport target set source must retain duplicate rejection witness `{required}`"
        );
    }
    let target_struct = source_between(
        transport_source.as_str(),
        "pub struct RadrootsTransportTarget {",
        "impl RadrootsTransportTarget {",
    );
    for forbidden in [
        "pub kind:",
        "pub uri:",
        "pub scope:",
        "pub label:",
        "pub fingerprint:",
    ] {
        assert!(
            !target_struct.contains(forbidden),
            "transport target identity field must remain sealed: `{forbidden}`"
        );
    }
    for required in [
        "impl<'de> serde::Deserialize<'de> for RadrootsTransportTarget",
        "impl<'de> serde::Deserialize<'de> for RadrootsTransportTargetSet",
    ] {
        assert!(
            transport_source.contains(required),
            "transport target source must retain checked deserialization witness `{required}`"
        );
    }

    let reticulum_source = read_source(crates_root.join("transport/src/reticulum.rs").as_path());
    let destination_struct = source_between(
        reticulum_source.as_str(),
        "pub struct ReticulumDestinationV1 {",
        "impl ReticulumDestinationV1 {",
    );
    for forbidden in ["pub uri:", "pub routing:", "pub label:", "pub fingerprint:"] {
        assert!(
            !destination_struct.contains(forbidden),
            "Reticulum destination identity field must remain sealed: `{forbidden}`"
        );
    }
    assert!(
        reticulum_source.contains("impl<'de> serde::Deserialize<'de> for ReticulumDestinationV1"),
        "Reticulum destination source must retain checked deserialization"
    );

    let relay_source = read_source(crates_root.join("transport_nostr/src/relay.rs").as_path());
    for required in [
        "RadrootsTransportTarget::nostr_relay(original)",
        "RadrootsRelayTransportError::DuplicateRelayUrl",
    ] {
        assert!(
            relay_source.contains(required),
            "Nostr relay target source must retain canonical identity witness `{required}`"
        );
    }
    for forbidden in [
        "impl<'de> Deserialize<'de> for RelayUrl",
        "impl<'de> Deserialize<'de> for RadrootsRelayTargetSet",
    ] {
        assert!(
            !relay_source.contains(forbidden),
            "policy-free Nostr relay identity must not regain deserialization: `{forbidden}`"
        );
    }

    let protocol_source = read_source(
        crates_root
            .join("transport_publish_protocol/src/lib.rs")
            .as_path(),
    );
    for required in [
        "validate_explicit_target_uniqueness(targets)?;",
        "TransportPublishProtocolError::DuplicateTarget { index }",
        "duplicate_targets.validate(2)",
    ] {
        assert!(
            protocol_source.contains(required),
            "transport publish protocol must retain explicit-target duplicate rejection witness `{required}`"
        );
    }

    let outbox_source = read_source(crates_root.join("outbox/src/store.rs").as_path());
    for required in [
        "validate_unique_targets(&targets)?;",
        "RadrootsTransportError::DuplicateTargetFingerprint",
        "enqueue_rejects_duplicate_delivery_targets_before_persistence",
    ] {
        assert!(
            outbox_source.contains(required),
            "outbox source must retain duplicate target rejection witness `{required}`"
        );
    }
    assert!(
        !outbox_source.contains("ordered_unique_targets"),
        "outbox source must not reintroduce silent ordered target dedupe before delivery-plan preparation"
    );
}

#[test]
fn required_target_semantics_stay_fingerprint_exact() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");

    let protocol_source = read_source(
        crates_root
            .join("transport_publish_protocol/src/lib.rs")
            .as_path(),
    );
    for required in [
        "Self::RequiredTargets { targets } => targets.len()",
        "pub fn validate_target_membership",
        "TransportPublishProtocolError::RequiredTargetNotInTargetSet { index }",
        "let required_outcomes = required_policy_outcomes(targets, &job.targets)?;",
        "required_outcomes.iter().any(|outcome|",
        "fingerprint == *required",
    ] {
        assert!(
            protocol_source.contains(required),
            "transport publish protocol must retain exact required-target witness `{required}`"
        );
    }

    let nostr_publish_source =
        read_source(crates_root.join("transport_nostr/src/publish.rs").as_path());
    for required in [
        "RadrootsTransportSatisfactionPolicy::RequiredTargets { class, targets } =>",
        "let mut satisfied_required_targets = BTreeSet::new();",
        "targets.contains(target.fingerprint())",
        "counts_as_satisfied(*class)",
        "targets\n                .iter()\n                .all(|target| satisfied_required_targets.contains(target))",
    ] {
        assert!(
            nostr_publish_source.contains(required),
            "direct Nostr publish must retain exact required-target witness `{required}`"
        );
    }

    let nostr_outbox_source =
        read_source(crates_root.join("transport_nostr/src/outbox.rs").as_path());
    let publishable_relays_source = source_between(
        nostr_outbox_source.as_str(),
        "let required_targets = match &plan.satisfaction_policy",
        "Ok(PublishableRelays {",
    );
    for required in [
        "RadrootsTransportSatisfactionPolicy::RequiredTargets { targets, .. } =>",
        ".is_none_or(|required| required.contains(&target.endpoint_fingerprint))",
        ".is_some_and(|required| required.contains(&target.endpoint_fingerprint))",
        "required_targets.is_none() || required_for_satisfaction",
        "required_targets.is_some()",
    ] {
        assert!(
            publishable_relays_source.contains(required),
            "direct Nostr outbox publish must retain exact required-target witness `{required}`"
        );
    }
}

#[test]
fn transport_identity_is_extensible_and_reticulum_contracts_remain_explicit() {
    let crates_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("transport crate parent");
    let transport_id = read_source(crates_root.join("transport/src/id.rs").as_path());
    for required in [
        "pub struct TransportId(",
        "pub const LOCAL:",
        "pub const NOSTR:",
        "pub const RETICULUM:",
        "pub const RADROOTSD:",
        "ProtocolTransportKind::parse",
    ] {
        assert!(
            transport_id.contains(required),
            "transport identity source must retain extensible identity witness `{required}`"
        );
    }
    assert!(!transport_id.contains("pub enum TransportId"));

    let protocol_identity =
        read_source(crates_root.join("protocol/src/capability/v1.rs").as_path());
    assert!(protocol_identity.contains("pub struct TransportKind"));
    assert!(!protocol_identity.contains("pub enum TransportKind"));
    assert!(protocol_identity.contains("MAX_TRANSPORT_KIND_BYTES"));

    let transport_message_source =
        read_source(crates_root.join("transport/src/message.rs").as_path());
    for required in [
        "RADROOTS_RETICULUM_ENDPOINT_URI",
        "reticulum:local",
        "RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE",
        "Reticulum transport is configured, ",
        "but this build does not implement Reticulum delivery.",
    ] {
        assert!(
            transport_message_source.contains(required),
            "transport message source must retain Reticulum unavailable message witness `{required}`"
        );
    }
    for forbidden in [
        "Reticulum prerelease transport is registered, ",
        "future compatibility",
        "compatibility mode",
        "fallback behavior",
        "hidden transport substitution",
    ] {
        assert!(
            !transport_message_source.contains(forbidden),
            "transport message source must not retain superseded Reticulum unavailable copy `{forbidden}`"
        );
    }

    let reticulum_source =
        read_source(crates_root.join("transport_reticulum/src/lib.rs").as_path());
    assert!(
        reticulum_source.contains("RADROOTS_RETICULUM_ENDPOINT_URI"),
        "Reticulum source must consume the shared endpoint URI constant"
    );
    assert!(
        !reticulum_source.contains(["reticulum:", "pre", "view-unavailable"].concat().as_str()),
        "Reticulum source must not duplicate the shared endpoint URI"
    );
    assert!(
        reticulum_source.contains("RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE"),
        "Reticulum source must consume the shared unavailable message constant"
    );
    assert!(
        !reticulum_source.contains("Reticulum transport is configured in preview mode"),
        "Reticulum source must not duplicate the shared unavailable message"
    );
    assert!(
        !reticulum_source.contains("future compatibility"),
        "Reticulum source must not duplicate compatibility copy"
    );

    let protocol_source_raw = read_source(
        crates_root
            .join("transport_publish_protocol/src/lib.rs")
            .as_path(),
    );
    let protocol_source = production_source(protocol_source_raw.as_str());
    assert!(
        protocol_source.contains("RADROOTS_RETICULUM_ENDPOINT_URI"),
        "transport publish protocol must consume the shared Reticulum endpoint URI constant"
    );
    assert!(
        !protocol_source.contains(["reticulum:", "pre", "view-unavailable"].concat().as_str()),
        "transport publish protocol must not duplicate the shared endpoint URI"
    );
    assert!(
        protocol_source.contains("RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE"),
        "transport publish capabilities must consume the shared Reticulum unavailable message"
    );
    assert!(
        !protocol_source.contains("Reticulum transport is configured in preview mode"),
        "transport publish protocol must not duplicate the shared unavailable message"
    );
    assert!(
        !protocol_source.contains("future compatibility"),
        "transport publish protocol must not duplicate compatibility copy"
    );
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn foundation_hardening_guard_files(repo_root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    for path in [
        repo_root.join("Cargo.toml"),
        repo_root.join("README"),
        repo_root.join("README.md"),
    ] {
        if path.exists() {
            paths.push(path);
        }
    }

    let crates_root = repo_root.join("crates");
    for entry in fs::read_dir(crates_root.as_path())
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", crates_root.display()))
    {
        let path = entry.expect("crate entry").path();
        if !path.is_dir() {
            continue;
        }

        let src = path.join("src");
        if src.exists() {
            paths.extend(rust_source_files(src.as_path()));
        }

        for file_name in ["Cargo.toml", "README", "README.md"] {
            let candidate = path.join(file_name);
            if candidate.exists() {
                paths.push(candidate);
            }
        }
    }

    for relative_root in FOUNDATION_HARDENING_DOC_ROOTS {
        let root = repo_root.join(relative_root);
        if root.exists() {
            collect_doc_surface_files(root.as_path(), &mut paths);
        }
    }

    paths.sort();
    paths
}

fn collect_doc_surface_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
    {
        let path = entry.expect("doc surface entry").path();
        if path.is_dir() {
            collect_doc_surface_files(path.as_path(), paths);
            continue;
        }

        if is_doc_surface(path.as_path()) {
            paths.push(path);
        }
    }
}

fn collect_rust_source_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
    {
        let entry = entry.expect("read source entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files(path.as_path(), paths);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}

fn read_source(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read source {}: {error}", path.display()))
}

fn production_source(source: &str) -> &str {
    source
        .find("\n#[cfg(test)]")
        .map_or(source, |index| &source[..index])
}

fn is_doc_surface(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|file_name| file_name.to_str()),
        Some("README") | Some("README.md")
    ) || matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("md")
    )
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("source path is under crate root")
        .to_string_lossy()
        .replace('\\', "/")
}

fn source_between<'source>(
    source: &'source str,
    start_marker: &str,
    end_marker: &str,
) -> &'source str {
    let start = source
        .find(start_marker)
        .unwrap_or_else(|| panic!("failed to find source marker `{start_marker}`"));
    let source_after_start = &source[start..];
    let end = source_after_start
        .find(end_marker)
        .unwrap_or_else(|| panic!("failed to find source marker `{end_marker}`"));
    &source_after_start[..end]
}

fn contains_forbidden_concept(source: &str, pattern: &str) -> bool {
    if !pattern.chars().all(is_rust_identifier_character) {
        return source.contains(pattern);
    }

    source.match_indices(pattern).any(|(index, _)| {
        let before = source[..index].chars().next_back();
        let after = source[index + pattern.len()..].chars().next();
        before.is_none_or(|character| !is_rust_identifier_character(character))
            && after.is_none_or(|character| !is_rust_identifier_character(character))
    })
}

fn removed_reticulum_stage_endpoint_lines(source: &str) -> Vec<usize> {
    let removed_endpoint_prefix = ["reticulum:", "pre", "view"].concat();
    source
        .match_indices(removed_endpoint_prefix.as_str())
        .filter_map(|(index, _)| {
            let after = source[index + removed_endpoint_prefix.len()..]
                .chars()
                .next();
            (after != Some('-')).then(|| line_number(source, index))
        })
        .collect()
}

fn is_rust_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn line_number(source: &str, index: usize) -> usize {
    source[..index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}
