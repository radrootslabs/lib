use std::{
    fs,
    path::{Path, PathBuf},
};

struct ForbiddenSdkConcept {
    pattern: &'static str,
    reason: &'static str,
}

const FORBIDDEN_SDK_SOURCE_CONCEPTS: &[ForbiddenSdkConcept] = &[
    ForbiddenSdkConcept {
        pattern: "radroots_studio_app",
        reason: "SDK source must not depend on the app crate family",
    },
    ForbiddenSdkConcept {
        pattern: "radroots_cli",
        reason: "SDK source must not depend on the CLI crate family",
    },
    ForbiddenSdkConcept {
        pattern: "RadrootsApp",
        reason: "SDK source must not import app product concepts",
    },
    ForbiddenSdkConcept {
        pattern: "RadrootsCli",
        reason: "SDK source must not import CLI product concepts",
    },
    ForbiddenSdkConcept {
        pattern: "AppSdk",
        reason: "app SDK adapters belong outside the SDK crate",
    },
    ForbiddenSdkConcept {
        pattern: "DesktopApp",
        reason: "desktop app runtime concepts belong outside the SDK crate",
    },
    ForbiddenSdkConcept {
        pattern: "domains/radroots/studio_apps",
        reason: "SDK source must remain standalone and path-agnostic",
    },
];

const REQUIRED_TRADE_RUNTIME_EXPORTS: &[&str] = &[
    "TRADE_CANCELLATION_OPERATION_KIND",
    "TRADE_DECISION_OPERATION_KIND",
    "TRADE_REVISION_DECISION_OPERATION_KIND",
    "TRADE_REVISION_PROPOSAL_OPERATION_KIND",
    "TRADE_STATUS_DEFAULT_LIMIT",
    "TRADE_STATUS_MAX_LIMIT",
    "TRADE_SUBMIT_OPERATION_KIND",
    "TradeAcceptRequest",
    "TradeCancelRequest",
    "TradeCancellationEnqueueRequest",
    "TradeCancellationPlan",
    "TradeCancellationPrepareRequest",
    "TradeCancellationReceipt",
    "TradeDecisionEnqueueRequest",
    "TradeDecisionPlan",
    "TradeDecisionPrepareRequest",
    "TradeDecisionReceipt",
    "TradeDeclineRequest",
    "TradeEvidenceIngestReceipt",
    "TradeEvidenceIngestRequest",
    "TradeMutationOutcome",
    "TradeProposeRequest",
    "TradeRequestEvidenceIngestReceipt",
    "TradeRequestEvidenceIngestRequest",
    "TradeResyncReceipt",
    "TradeResyncRequest",
    "TradeRevisionDecisionEnqueueRequest",
    "TradeRevisionDecisionPlan",
    "TradeRevisionDecisionPrepareRequest",
    "TradeRevisionDecisionReceipt",
    "TradeRevisionDecisionRequest",
    "TradeRevisionProposalEnqueueRequest",
    "TradeRevisionProposalPlan",
    "TradeRevisionProposalPrepareRequest",
    "TradeRevisionProposalRequest",
    "TradeRevisionProposalReceipt",
    "TradeSellerInboxReceipt",
    "TradeSellerInboxRequest",
    "TradeStatusAmbiguityCandidate",
    "TradeStatusEligibility",
    "TradeStatusEvidenceSummary",
    "TradeStatusKind",
    "TradeStatusNextActionKind",
    "TradeStatusReceipt",
    "TradeStatusRequest",
    "TradeSubmitEnqueueRequest",
    "TradeSubmitPlan",
    "TradeSubmitPrepareRequest",
    "TradeSubmitReceipt",
    "TradeWorkflowEnqueueReceipt",
    "TradeWorkflowIdempotencyReceipt",
    "TradeWorkflowKind",
    "TradeWorkflowPlan",
    "TradeWorkflowRetryAdvice",
    "SdkTradeStatusIssue",
    "SdkTradeStatusIssueKind",
    "SdkTradeStatusSource",
];

const REQUIRED_DVM_RUNTIME_EXPORTS: &[&str] = &[
    "DVM_TRADE_TRANSITION_PROOF_REQUEST_CONTRACT_ID",
    "DVM_TRADE_TRANSITION_PROOF_REQUEST_OPERATION_KIND",
    "DvmProofMode",
    "DvmTradeTransitionProofEnqueueRequest",
    "DvmTradeTransitionProofPlan",
    "DvmTradeTransitionProofPrepareRequest",
    "DvmTradeTransitionProofReceipt",
    "DvmTradeTransitionProofRequestPayload",
    "DvmValidationReceiptIngestReceipt",
    "DvmValidationReceiptIngestRequest",
    "RadrootsTradeInventoryBinWitnessDto",
];

const REQUIRED_TRADE_POLICY_EXPORTS: &[&str] = &[
    "AckPolicy",
    "PublishMode",
    "RelayResolutionPolicy",
    "SdkTradeIdempotencyRecord",
    "PrivacyPreflightReceipt",
    "PrivacyPreflightStatus",
    "ProductSensitivityField",
    "SDK_TRADE_PROJECTION_CACHE_VERSION",
    "SdkTradeProjectionCache",
    "SdkTradeProjectionCacheKey",
    "SdkTradeProjectionCacheRecord",
];

const REQUIRED_TRADES_CLIENT_METHODS: &[&str] = &[
    "pub async fn ingest_evidence(",
    "pub async fn ingest_request_evidence(",
    "pub fn prepare_submit(",
    "pub async fn enqueue_submit(",
    "pub async fn enqueue_prepared_submit(",
    "pub fn prepare_decision(",
    "pub async fn enqueue_decision(",
    "pub async fn enqueue_prepared_decision(",
    "pub fn prepare_revision_proposal(",
    "pub async fn enqueue_revision_proposal(",
    "pub async fn enqueue_prepared_revision_proposal(",
    "pub fn prepare_revision_decision(",
    "pub async fn enqueue_revision_decision(",
    "pub async fn enqueue_prepared_revision_decision(",
    "pub fn prepare_cancellation(",
    "pub async fn enqueue_cancellation(",
    "pub async fn enqueue_prepared_cancellation(",
    "pub async fn status(",
];

const REQUIRED_DVM_CLIENT_METHODS: &[&str] = &[
    "pub fn prepare_trade_transition_proof_request(",
    "pub async fn enqueue_trade_transition_proof_request_with_explicit_signer(",
    "pub async fn ingest_validation_receipt(",
];

const REQUIRED_DVM_CLIENT_CONFIGURED_SIGNER_METHODS: &[&str] =
    &["pub async fn enqueue_trade_transition_proof_request("];

const REQUIRED_TRADES_CLIENT_ADVANCED_SIGNER_METHODS: &[&str] = &[
    "pub async fn enqueue_submit_with_explicit_signer(",
    "pub async fn enqueue_prepared_submit_with_explicit_signer(",
    "pub async fn enqueue_decision_with_explicit_signer(",
    "pub async fn enqueue_prepared_decision_with_explicit_signer(",
    "pub async fn enqueue_revision_proposal_with_explicit_signer(",
    "pub async fn enqueue_prepared_revision_proposal_with_explicit_signer(",
    "pub async fn enqueue_revision_decision_with_explicit_signer(",
    "pub async fn enqueue_prepared_revision_decision_with_explicit_signer(",
    "pub async fn enqueue_cancellation_with_explicit_signer(",
    "pub async fn enqueue_prepared_cancellation_with_explicit_signer(",
];

const REQUIRED_TRADE_BUYER_CLIENT_METHODS: &[&str] = &[
    "pub async fn propose_trade(",
    "pub async fn cancel_trade(",
    "pub async fn accept_revision(",
    "pub async fn decline_revision(",
];

const REQUIRED_TRADE_SELLER_CLIENT_METHODS: &[&str] = &[
    "pub async fn inbox(",
    "pub async fn accept_trade(",
    "pub async fn decline_trade(",
    "pub async fn propose_revision(",
];

const REQUIRED_TRADE_STATUS_CLIENT_METHODS: &[&str] = &["pub async fn status("];

const REQUIRED_TRADE_RESYNC_CLIENT_METHODS: &[&str] = &["pub async fn resync("];

const FORBIDDEN_ORDER_RUNTIME_PUBLIC_EXPORTS: &[&str] = &[
    "CheckoutClient",
    "EscrowClient",
    "InvoiceClient",
    concat!("Order", "FulfillmentStatusKind"),
    concat!("Order", "FulfillmentUpdateEnqueueRequest"),
    concat!("Order", "FulfillmentUpdatePlan"),
    concat!("Order", "FulfillmentUpdatePrepareRequest"),
    concat!("Order", "FulfillmentUpdateReceipt"),
    concat!("Order", "Payment", "HandoffKind"),
    concat!("Order", "PaymentRecordEnqueueRequest"),
    concat!("Order", "PaymentRecordPrepareRequest"),
    concat!("Order", "Payment", "Record", "Receipt"),
    concat!("Order", "Payment", "StateKind"),
    concat!("Order", "ReceiptRecordEnqueueRequest"),
    concat!("Order", "ReceiptRecordPlan"),
    concat!("Order", "ReceiptRecordPrepareRequest"),
    concat!("Order", "ReceiptRecord", "Receipt"),
    concat!("Order", "SettlementDecisionEnqueueRequest"),
    concat!("Order", "SettlementDecisionPrepareRequest"),
    concat!("Order", "SettlementDecisionReceipt"),
    concat!("Order", "Settlement", "StateKind"),
    concat!("Payment", "Client"),
    concat!("Payments", "Client"),
    "RefundClient",
    "WalletClient",
    "ORDER_FULFILLMENT_UPDATE_OPERATION_KIND",
    "ORDER_PAYMENT_RECORD_OPERATION_KIND",
    "ORDER_RECEIPT_RECORD_OPERATION_KIND",
    "ORDER_SETTLEMENT_DECISION_OPERATION_KIND",
];

const FORBIDDEN_LEGACY_ORDER_PRODUCT_EXPORTS: &[&str] = &[
    "ORDER_CANCELLATION_OPERATION_KIND",
    "ORDER_DECISION_OPERATION_KIND",
    "ORDER_REVISION_DECISION_OPERATION_KIND",
    "ORDER_REVISION_PROPOSAL_OPERATION_KIND",
    "ORDER_STATUS_DEFAULT_LIMIT",
    "ORDER_STATUS_MAX_LIMIT",
    "ORDER_SUBMIT_OPERATION_KIND",
    "OrderCancellationEnqueueRequest",
    "OrderCancellationPlan",
    "OrderCancellationPrepareRequest",
    "OrderCancellationReceipt",
    "OrderDecisionEnqueueRequest",
    "OrderDecisionPlan",
    "OrderDecisionPrepareRequest",
    "OrderDecisionReceipt",
    "OrderEvidenceIngestReceipt",
    "OrderEvidenceIngestRequest",
    "OrderRequestEvidenceIngestReceipt",
    "OrderRequestEvidenceIngestRequest",
    "OrderRevisionDecisionEnqueueRequest",
    "OrderRevisionDecisionPlan",
    "OrderRevisionDecisionPrepareRequest",
    "OrderRevisionDecisionReceipt",
    "OrderRevisionProposalEnqueueRequest",
    "OrderRevisionProposalPlan",
    "OrderRevisionProposalPrepareRequest",
    "OrderRevisionProposalReceipt",
    "OrderStatusEligibility",
    "OrderStatusEvidenceSummary",
    "OrderStatusReceipt",
    "OrderStatusRequest",
    "OrderSubmitEnqueueRequest",
    "OrderSubmitPlan",
    "OrderSubmitPrepareRequest",
    "OrderSubmitReceipt",
    "OrderWorkflowEnqueueReceipt",
    "OrderWorkflowIdempotencyReceipt",
    "OrderWorkflowKind",
    "OrderWorkflowPlan",
    "OrderWorkflowRetryAdvice",
    "SdkDvmInventoryBinWitness",
    "SdkOrderStatusIssue",
    "SdkOrderStatusIssueKind",
    "SdkOrderStatusSource",
    "TradeProtocolClient",
];

const FORBIDDEN_ORDER_RUNTIME_METHODS: &[&str] = &[
    "accept_settlement",
    "checkout",
    "enqueue_fulfillment",
    "enqueue_payment",
    "enqueue_receipt_record",
    "enqueue_settlement",
    "escrow",
    "fulfillment",
    "invoice",
    "payment_provider",
    "prepare_fulfillment",
    "prepare_payment",
    "prepare_receipt_record",
    "prepare_settlement",
    "receipt_record",
    "record_payment",
    "refund",
    "reject_settlement",
    "settle_payment",
    "wallet",
];

#[test]
fn sdk_sources_do_not_import_app_or_cli_concepts() {
    for path in rust_source_files(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path()) {
        let source = read_source(path.as_path());
        for concept in FORBIDDEN_SDK_SOURCE_CONCEPTS {
            assert!(
                !contains_forbidden_concept(&source, concept.pattern),
                "{} contains forbidden SDK source concept `{}`: {}",
                path.display(),
                concept.pattern,
                concept.reason
            );
        }
    }
}

#[test]
fn sdk_manifest_does_not_depend_on_app_or_cli_crates() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = read_source(manifest_path.as_path());

    for crate_name in ["radroots_studio_app", "radroots_cli"] {
        assert!(
            !manifest.contains(crate_name),
            "SDK manifest contains forbidden downstream crate dependency `{crate_name}`"
        );
    }
}

#[test]
fn farm_runtime_stays_on_product_runtime_boundary() {
    product_runtime_file_stays_on_boundary("src/farms_runtime.rs");
}

#[test]
fn order_runtime_stays_on_product_runtime_boundary() {
    product_runtime_file_stays_on_boundary("src/orders_runtime.rs");
}

#[test]
fn migrated_runtime_tests_stay_on_product_runtime_boundary() {
    for file in ["tests/farms_runtime.rs", "tests/orders_runtime.rs"] {
        product_runtime_file_stays_on_boundary(file);
    }
}

#[test]
fn agreement_order_runtime_excludes_post_agreement_surfaces() {
    let lib_source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );
    let order_source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/orders_runtime.rs")
            .as_path(),
    );

    for forbidden in FORBIDDEN_ORDER_RUNTIME_PUBLIC_EXPORTS {
        assert!(
            !lib_source.contains(forbidden),
            "src/lib.rs must not expose unsupported order runtime surface `{forbidden}`"
        );
    }

    for forbidden in FORBIDDEN_ORDER_RUNTIME_METHODS {
        assert!(
            !order_source.contains(forbidden),
            "src/orders_runtime.rs must not expose unsupported order runtime method or capability `{forbidden}`"
        );
    }
}

#[test]
fn order_runtime_public_exports_are_explicit() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );

    assert!(
        source.contains("mod orders_runtime;"),
        "src/lib.rs must keep orders_runtime as an internal implementation module"
    );
    assert!(
        source.contains("pub use crate::orders_runtime::{"),
        "src/lib.rs must explicitly re-export approved order runtime types"
    );
    assert!(
        !source.contains("pub mod orders_runtime;"),
        "src/lib.rs must not expose the orders_runtime module path"
    );
    assert!(
        !source.contains("pub use crate::orders_runtime::*;"),
        "src/lib.rs must not wildcard-export the order runtime"
    );

    for export in REQUIRED_TRADE_RUNTIME_EXPORTS {
        assert!(
            source.contains(export),
            "src/lib.rs must explicitly expose trade SDK runtime export `{export}`"
        );
    }

    for export in REQUIRED_TRADE_POLICY_EXPORTS {
        assert!(
            source.contains(export),
            "src/lib.rs must explicitly expose trade policy export `{export}`"
        );
    }

    for forbidden in FORBIDDEN_LEGACY_ORDER_PRODUCT_EXPORTS {
        assert!(
            !source.contains(forbidden),
            "src/lib.rs must not expose legacy order SDK product export `{forbidden}`"
        );
    }
}

#[test]
fn dvm_runtime_public_exports_are_explicit() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );

    assert!(
        source.contains("mod dvm_runtime;"),
        "src/lib.rs must keep dvm_runtime as an internal implementation module"
    );
    assert!(
        source.contains("pub use crate::dvm_runtime::{"),
        "src/lib.rs must explicitly re-export approved DVM runtime types"
    );
    assert!(
        !source.contains("pub mod dvm_runtime;"),
        "src/lib.rs must not expose the dvm_runtime module path"
    );
    assert!(
        !source.contains("pub use crate::dvm_runtime::*;"),
        "src/lib.rs must not wildcard-export the DVM runtime"
    );

    for export in REQUIRED_DVM_RUNTIME_EXPORTS {
        assert!(
            source.contains(export),
            "src/lib.rs must explicitly expose DVM SDK runtime export `{export}`"
        );
    }
}

#[test]
fn orders_client_surface_is_inventory_guarded() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/orders_runtime.rs")
            .as_path(),
    );

    assert!(
        source.contains("impl<'sdk> TradesClient<'sdk> {"),
        "src/orders_runtime.rs must own TradesClient runtime methods"
    );

    for method in REQUIRED_TRADES_CLIENT_METHODS {
        assert!(
            source.contains(method),
            "TradesClient must expose inventory-guarded method `{method}`"
        );
    }

    for method in REQUIRED_TRADES_CLIENT_ADVANCED_SIGNER_METHODS {
        assert!(
            source.contains(method),
            "TradesClient must expose explicit-signer advanced method `{method}`"
        );
    }
}

#[test]
fn trade_product_facade_methods_are_inventory_guarded() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/orders_runtime.rs")
            .as_path(),
    );

    for method in REQUIRED_TRADE_BUYER_CLIENT_METHODS {
        assert!(
            source.contains(method),
            "TradeBuyerClient must expose product workflow method `{method}`"
        );
    }

    for method in REQUIRED_TRADE_SELLER_CLIENT_METHODS {
        assert!(
            source.contains(method),
            "TradeSellerClient must expose product workflow method `{method}`"
        );
    }

    for method in REQUIRED_TRADE_STATUS_CLIENT_METHODS {
        assert!(
            source.contains(method),
            "TradeStatusClient must expose product workflow method `{method}`"
        );
    }

    for method in REQUIRED_TRADE_RESYNC_CLIENT_METHODS {
        assert!(
            source.contains(method),
            "TradeResyncClient must expose product workflow method `{method}`"
        );
    }
}

#[test]
fn dvm_client_surface_is_inventory_guarded() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/dvm_runtime.rs")
            .as_path(),
    );

    assert!(
        source.contains("impl<'sdk> DvmClient<'sdk> {"),
        "src/dvm_runtime.rs must own DvmClient runtime methods"
    );

    for method in REQUIRED_DVM_CLIENT_METHODS {
        assert!(
            source.contains(method),
            "DvmClient must expose inventory-guarded method `{method}`"
        );
    }

    for method in REQUIRED_DVM_CLIENT_CONFIGURED_SIGNER_METHODS {
        assert!(
            source.contains(method),
            "DvmClient must expose configured-signer method `{method}`"
        );
    }
}

#[test]
fn product_clients_remain_thin_sdk_handles() {
    let lib_source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );
    let clients_source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/product_clients.rs")
            .as_path(),
    );

    assert!(
        lib_source.contains("mod product_clients;"),
        "src/lib.rs must keep product_clients internal"
    );
    assert!(
        lib_source.contains("pub use crate::product_clients::{"),
        "src/lib.rs must explicitly export product client handles"
    );
    assert!(
        !lib_source.contains("pub mod product_clients;"),
        "src/lib.rs must not expose the product_clients module path"
    );

    for client in [
        "DvmClient",
        "FarmsClient",
        "ListingsClient",
        "MarketClient",
        "SyncClient",
        "TradeBuyerClient",
        "TradeResyncClient",
        "TradeSellerClient",
        "TradeStatusClient",
        "TradeValidationClient",
        "TradesClient",
    ] {
        assert!(
            lib_source.contains(client),
            "src/lib.rs must export product client handle `{client}`"
        );
        assert!(
            clients_source.contains(format!("pub struct {client}<'client>").as_str()),
            "product_clients.rs must define thin handle `{client}`"
        );
    }
}

#[test]
fn legacy_client_and_config_modules_are_removed() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative_path in ["src/client.rs", "src/config.rs"] {
        let path = manifest_dir.join(relative_path);
        assert!(
            !path.exists(),
            "{relative_path} must not exist after SDK runtime surface closure"
        );
    }
}

#[test]
fn legacy_trade_client_root_export_is_removed() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );

    assert!(
        !source.contains("TradeClient"),
        "src/lib.rs must not re-export the legacy TradeClient facade"
    );
}

#[test]
fn legacy_client_config_modules_are_not_public() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );

    for forbidden in [
        "pub mod client;",
        "pub mod config;",
        "pub use crate::client",
        "pub use crate::config",
        "RadrootsSdkClient",
        "RadrootsSdkConfig",
        "SdkTransportMode",
        "ProfileClient",
        "FarmClient",
        "ListingClient",
        "SdkPublishReceipt",
        "SdkTransportReceipt",
    ] {
        assert!(
            !source.contains(forbidden),
            "src/lib.rs must not expose legacy SDK client/config concept `{forbidden}`"
        );
    }
}

fn product_runtime_file_stays_on_boundary(relative_path: &str) {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(relative_path)
            .as_path(),
    );

    for forbidden in [
        "RadrootsSdkClient",
        "SdkTransportMode",
        "SdkPublishReceipt",
        "radrootsd",
        "publish_with_identity",
        "publish_parts_via_relay",
        "publish_listing_via_radrootsd",
        "publish_order_request_via_radrootsd",
        "publish_farm_via_radrootsd",
    ] {
        assert!(
            !source.contains(forbidden),
            "{relative_path} must not use legacy SDK client or transport concept `{forbidden}`"
        );
    }
}

fn read_source(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read source {}: {error}", path.display()))
}

fn contains_forbidden_concept(source: &str, pattern: &str) -> bool {
    source.match_indices(pattern).any(|(index, _)| {
        let before = source[..index].chars().next_back();
        let after = source[index + pattern.len()..].chars().next();
        before.is_none_or(|character| !is_rust_identifier_character(character))
            && after.is_none_or(|character| !is_rust_identifier_character(character))
    })
}

fn is_rust_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_source_files(root: &Path, paths: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root).unwrap_or_else(|error| {
        panic!(
            "failed to read source directory {}: {error}",
            root.display()
        )
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to inspect source directory {}: {error}",
                root.display()
            )
        });
        let path = entry.path();

        if path.is_dir() {
            collect_rust_source_files(path.as_path(), paths);
            continue;
        }

        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}
