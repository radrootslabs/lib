use std::collections::BTreeSet;

#[allow(unused_imports)]
use radroots_trade::{evidence as _, model as _, reducer as _, validation as _, workflow as _};

const MANIFEST: &str = include_str!("../Cargo.toml");
const DRAFT: &str = include_str!("../src/operational_listing/draft.rs");
const MODEL: &str = include_str!("../src/model.rs");
const OPERATIONS: &str = include_str!("../../../contracts/operations.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const REDUCER_IMPLEMENTATION: &str = include_str!("../src/reducer_impl.rs");
const CANONICAL_REDUCER_VECTORS: &str =
    include_str!("../../../contracts/conformance/vectors/trade/reduce_records.v1.json");
const PACKAGED_REDUCER_VECTORS: &str = include_str!("fixtures/reduce_records.v1.json");
const CANONICAL_WORKFLOW_VECTORS: &str =
    include_str!("../../../contracts/conformance/vectors/trade/prepare_workflow.v1.json");
const PACKAGED_WORKFLOW_VECTORS: &str = include_str!("fixtures/prepare_workflow.v1.json");
const WORKFLOW: &str = include_str!("../src/workflow.rs");
const PACKAGE_TIERS: &str = include_str!("../../../contracts/releases/package_tiers.toml");
const README: &str = include_str!("../README.md");
const EXAMPLE: &str = include_str!("../examples/reduce_trade.rs");
const PUBLIC_API: &str = include_str!("../../../docs/api/radroots_trade.txt");
const COMPATIBILITY: &str = include_str!("../COMPATIBILITY.md");

#[test]
fn manifest_has_final_identity_and_required_radroots_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_trade\""));
    assert!(MANIFEST.contains("version = \"0.1.0-alpha\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_trade\""));

    let dependencies = table_keys(MANIFEST, "[dependencies]");
    for dependency in ["radroots_core", "radroots_event", "radroots_identity"] {
        assert!(
            dependencies.contains(dependency),
            "missing required Radroots dependency {dependency}"
        );
    }
}

#[test]
fn crate_root_declares_every_approved_module() {
    let declared = root_declarations("pub mod ");
    for module in ["evidence", "model", "reducer", "validation", "workflow"] {
        assert!(
            declared.contains(module) || ROOT.contains(&format!("pub mod {module} {{")),
            "missing approved module {module}"
        );
    }
}

#[test]
fn expired_upward_development_dependencies_are_absent() {
    let dev_dependencies = table_keys(MANIFEST, "[dev-dependencies]");
    for dependency in ["radroots_nostr", "radroots_transport"] {
        assert!(
            !dev_dependencies.contains(dependency),
            "expired development dependency remains: {dependency}"
        );
        assert!(
            !PACKAGE_TIERS.contains(&format!(
                "owner = \"radroots_trade\"\ndependency = \"{dependency}\""
            )),
            "expired tier exception remains: {dependency}"
        );
    }
}

#[test]
fn protocol_trade_id_is_singular_and_business_order_id_is_distinct() {
    let trade_id = radroots_event::trade::TradeId::parse("11".repeat(16))
        .expect("canonical protocol trade id");
    let order_id = radroots_trade::model::OrderId::parse("order-1").expect("business order id");

    assert_eq!(trade_id.to_string(), "11".repeat(16));
    assert_eq!(order_id.as_str(), "order-1");
    assert!(!ROOT.contains("pub mod identity;"));
    assert!(MODEL.contains("pub struct OrderId(String);"));
    assert!(MODEL.contains("No conversion exists between them."));
    assert!(!MODEL.contains("From<OrderId> for TradeId"));
    assert!(!MODEL.contains("From<TradeId> for OrderId"));
}

#[test]
fn trade_canonicalization_accepts_validated_identity_without_authority_or_signing() {
    let dependencies = table_keys(MANIFEST, "[dependencies]");

    assert!(!dependencies.contains("radroots_authority"));
    assert!(!MANIFEST.contains("radroots_authority/std"));
    assert!(!DRAFT.contains("radroots_authority"));
    assert!(!DRAFT.contains("RadrootsActorContext"));
    assert!(!DRAFT.contains("ActorRoleUnsatisfied"));
    assert!(DRAFT.contains("seller_pubkey: PublicKey"));
    assert!(DRAFT.contains("performs no signing or authorization"));
}

#[test]
fn trade_feature_graph_has_no_persistence_or_sql_boundary() {
    let features = table_keys(MANIFEST, "[features]");
    let dependencies = table_keys(MANIFEST, "[dependencies]");
    let dev_dependencies = table_keys(MANIFEST, "[dev-dependencies]");

    assert!(!features.contains("event_store"));
    for forbidden in [
        "radroots_authority",
        "radroots_event_store",
        "radroots_outbox",
        "radroots_transport",
        "reqwest",
        "sqlx",
        "tokio",
    ] {
        assert!(
            !dependencies.contains(forbidden),
            "trade acquired forbidden production dependency {forbidden}"
        );
    }
    for forbidden in ["sqlx", "tokio"] {
        assert!(
            !dev_dependencies.contains(forbidden),
            "trade retained forbidden development dependency {forbidden}"
        );
    }
    assert!(!MANIFEST.contains("sqlite-bundled"));
    assert!(!MANIFEST.contains("runtime-tokio"));
}

#[test]
fn portable_root_exports_and_native_traits_are_compile_checked() {
    use radroots_trade::{
        Error, Projection, ReducerIssue, ReductionInput, ValidationError, WorkflowPlan,
    };

    fn assert_portable<T: Clone + core::fmt::Debug + Eq + Send + Sync>() {}
    fn assert_native_error<T: core::error::Error + Send + Sync>() {}

    assert_portable::<Projection>();
    assert_portable::<ReducerIssue>();
    assert_portable::<ReductionInput>();
    assert_portable::<WorkflowPlan>();
    assert_native_error::<Error>();
    assert_native_error::<ValidationError>();

    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert!(!ROOT.contains("pub trait "));
    for export in [
        "pub use model::RadrootsTradeProjectionV1 as Projection;",
        "RadrootsTradeReducerIssueV1 as ReducerIssue",
        "RadrootsTradeReductionInputV1 as ReductionInput",
        "pub use validation::ValidationError;",
        "pub use workflow::{Error, WorkflowPlan};",
    ] {
        assert!(
            ROOT.contains(export),
            "missing canonical root export {export}"
        );
    }
}

#[test]
fn packaged_trade_vectors_match_canonical_operation_contracts() {
    assert_eq!(PACKAGED_REDUCER_VECTORS, CANONICAL_REDUCER_VECTORS);
    assert_eq!(PACKAGED_WORKFLOW_VECTORS, CANONICAL_WORKFLOW_VECTORS);
    for required in [
        "id = \"trade.reduce_records\"",
        "crates/trade/src/reducer.rs",
        "radroots_trade::model::RadrootsTradeProjectionV1",
        "id = \"trade.prepare_workflow\"",
        "crates/trade/src/workflow.rs",
        "contracts/conformance/vectors/trade/prepare_workflow.v1.json",
    ] {
        assert!(
            OPERATIONS.contains(required),
            "operation contract is missing {required}"
        );
    }
    assert!(!OPERATIONS.contains("radroots_trade::workflow::RadrootsTradeProjectionV1"));
}

#[test]
fn trade_model_reducer_and_evidence_have_final_public_owners() {
    use radroots_trade::{Projection, ReducerIssue, ReductionInput};
    use radroots_trade::{evidence::RadrootsTradeEvidenceStateV1, reducer::reduce_trade_records};

    let trade_id = radroots_event::trade::TradeId::parse("22".repeat(16))
        .expect("canonical protocol trade id");
    let input = ReductionInput::new(trade_id)
        .with_evidence_state(RadrootsTradeEvidenceStateV1::Complete)
        .with_mutations(Vec::new())
        .with_private_terms(Vec::new())
        .with_attestations(Vec::new())
        .with_observed_at_unix_s(Some(42));

    assert_eq!(input.trade_id(), &trade_id);
    assert_eq!(
        input.evidence_state(),
        RadrootsTradeEvidenceStateV1::Complete
    );
    assert_eq!(input.observed_at_unix_s(), Some(42));
    assert!(input.mutations().is_empty());

    let projection: Projection = reduce_trade_records(input.clone());
    let _: &[ReducerIssue] = projection.issues();
    assert_eq!(projection.trade_id(), &trade_id);
    assert_eq!(
        projection.evidence_state(),
        RadrootsTradeEvidenceStateV1::Missing
    );

    let serialized = serde_json::to_value(input).expect("serialize reduction input");
    assert_eq!(serialized["trade_id"], trade_id.to_string());
    assert_eq!(serialized["observed_at_unix_s"], 42);
    assert_eq!(serialized["mutations"], serde_json::json!([]));

    for declaration in [
        "pub trade_id:",
        "pub mutations:",
        "pub projection_digest:",
        "pub candidate_id:",
        "pub claim_mutation_id:",
    ] {
        assert!(
            !REDUCER_IMPLEMENTATION.contains(declaration),
            "native trade contract field must remain private: {declaration}"
        );
    }
}

#[test]
fn workflow_plan_is_root_exported_and_side_effect_free() {
    let _: Option<radroots_trade::WorkflowPlan> = None;
    let _: Option<radroots_trade::Error> = None;

    for forbidden in [
        "std::fs",
        "std::net",
        "sqlx",
        "tokio",
        "reqwest",
        "event_store",
        "outbox",
        ".sign(",
        ".deliver(",
        ".execute(",
    ] {
        assert!(
            !WORKFLOW.contains(forbidden),
            "workflow plan acquired host side-effect authority: {forbidden}"
        );
    }
    for action in ["Sign", "Persist", "Deliver", "VerifyPrivateTerms"] {
        assert!(
            WORKFLOW.contains(action),
            "missing workflow action {action}"
        );
    }
}

#[test]
fn superseded_trade_surfaces_are_removed_or_explicitly_quarantined() {
    assert!(!ROOT.contains("pub mod identity;"));
    assert!(!ROOT.contains("pub mod prelude;"));
    assert!(!WORKFLOW.contains("Temporary migration reexports"));
    assert!(!WORKFLOW.contains("pub use crate::trade_contract_v1"));

    for declaration in [
        "#[doc(hidden)]\npub mod dto;",
        "#[doc(hidden)]\npub mod operational_listing;",
        "#[doc(hidden)]\npub mod validation_receipt;",
    ] {
        assert!(
            ROOT.contains(declaration),
            "temporary surface is not documentation-hidden: {declaration}"
        );
    }
    for required in [
        "Step 238",
        "Steps 261-262",
        "Step 313 is the exact final-removal checkpoint",
        "radroots_event_codec",
        "serde_json",
        "dto-bindgen",
    ] {
        assert!(
            COMPATIBILITY.contains(required),
            "compatibility contract is missing {required}"
        );
    }
}

#[test]
fn package_documentation_and_reviewed_api_baseline_are_complete() {
    for section in [
        "## Canonical surface",
        "## Deterministic reduction",
        "## Workflow planning",
        "## Features",
        "## Serialization and versioning",
        "## Security and trust boundaries",
        "## Side effects, cancellation, and commit points",
        "## Intended consumers",
        "## Package charter",
    ] {
        assert!(README.contains(section), "README is missing {section}");
    }
    assert!(ROOT.contains("#![doc = include_str!(\"../README.md\")]"));
    assert!(
        EXAMPLE.contains("use radroots_trade::{ReductionInput, reducer::reduce_trade_records};")
    );

    assert!(PUBLIC_API.starts_with("pub mod radroots_trade\n"));
    for item in [
        "pub mod radroots_trade::evidence",
        "pub mod radroots_trade::model",
        "pub mod radroots_trade::reducer",
        "pub mod radroots_trade::validation",
        "pub mod radroots_trade::workflow",
        "pub struct radroots_trade::Projection",
        "pub enum radroots_trade::ReducerIssue",
        "pub struct radroots_trade::ReductionInput",
        "pub struct radroots_trade::ValidationError",
        "pub struct radroots_trade::WorkflowPlan",
    ] {
        assert!(PUBLIC_API.contains(item), "API baseline is missing {item}");
    }
    for forbidden in [
        "radroots_authority::",
        "radroots_event_store::",
        "radroots_outbox::",
        "radroots_transport::",
        "reqwest::",
        "sqlx::",
        "tokio::",
    ] {
        assert!(
            !PUBLIC_API.contains(forbidden),
            "API baseline exposes forbidden host path {forbidden}"
        );
    }
}

fn table_keys<'a>(manifest: &'a str, heading: &str) -> BTreeSet<&'a str> {
    let Some((_, table)) = manifest.split_once(heading) else {
        return BTreeSet::new();
    };
    table
        .lines()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let line = line.trim();
            (line
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
                && !line.starts_with('#'))
            .then(|| line.split_once('=').map(|(key, _)| key.trim()))
            .flatten()
        })
        .collect()
}

fn root_declarations(prefix: &str) -> BTreeSet<&str> {
    ROOT.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|name| name.strip_suffix(';'))
        .collect()
}
