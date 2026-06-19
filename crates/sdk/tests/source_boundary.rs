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

const REQUIRED_ORDER_RUNTIME_EXPORTS: &[&str] = &[
    "ORDER_CANCELLATION_OPERATION_KIND",
    "ORDER_DECISION_OPERATION_KIND",
    "ORDER_FULFILLMENT_UPDATE_OPERATION_KIND",
    "ORDER_RECEIPT_RECORD_OPERATION_KIND",
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
    "OrderFulfillmentStatusKind",
    "OrderFulfillmentUpdateEnqueueRequest",
    "OrderFulfillmentUpdatePlan",
    "OrderFulfillmentUpdatePrepareRequest",
    "OrderFulfillmentUpdateReceipt",
    "OrderPaymentHandoffKind",
    "OrderPaymentStateKind",
    "OrderReceiptRecordEnqueueRequest",
    "OrderReceiptRecordPlan",
    "OrderReceiptRecordPrepareRequest",
    "OrderReceiptRecordReceipt",
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
    "OrderSettlementStateKind",
    "OrderStatusEligibility",
    "OrderStatusEvidenceSummary",
    "OrderStatusKind",
    "OrderStatusNextActionKind",
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
    "SdkOrderStatusIssue",
    "SdkOrderStatusIssueKind",
    "SdkOrderStatusSource",
];

const REQUIRED_ORDERS_CLIENT_METHODS: &[&str] = &[
    "pub async fn ingest_evidence(",
    "pub async fn ingest_request_evidence(",
    "pub fn prepare_submit(",
    "pub async fn enqueue_submit<",
    "pub async fn enqueue_prepared_submit<",
    "pub fn prepare_decision(",
    "pub async fn enqueue_decision<",
    "pub async fn enqueue_prepared_decision<",
    "pub fn prepare_revision_proposal(",
    "pub async fn enqueue_revision_proposal<",
    "pub async fn enqueue_prepared_revision_proposal<",
    "pub fn prepare_revision_decision(",
    "pub async fn enqueue_revision_decision<",
    "pub async fn enqueue_prepared_revision_decision<",
    "pub fn prepare_cancellation(",
    "pub async fn enqueue_cancellation<",
    "pub async fn enqueue_prepared_cancellation<",
    "pub fn prepare_fulfillment_update(",
    "pub async fn enqueue_fulfillment_update<",
    "pub async fn enqueue_prepared_fulfillment_update<",
    "pub fn prepare_receipt_record(",
    "pub async fn enqueue_receipt_record<",
    "pub async fn enqueue_prepared_receipt_record<",
    "pub async fn status(",
];

const FORBIDDEN_PAYMENT_WRITE_PUBLIC_EXPORTS: &[&str] = &[
    "CheckoutClient",
    "EscrowClient",
    "InvoiceClient",
    "OrderPaymentRecordEnqueueRequest",
    "OrderPaymentRecordPrepareRequest",
    "OrderPaymentRecordReceipt",
    "OrderSettlementDecisionEnqueueRequest",
    "OrderSettlementDecisionPrepareRequest",
    "OrderSettlementDecisionReceipt",
    "PaymentClient",
    "PaymentsClient",
    "RefundClient",
    "WalletClient",
    "ORDER_PAYMENT_RECORD_OPERATION_KIND",
    "ORDER_SETTLEMENT_DECISION_OPERATION_KIND",
];

const FORBIDDEN_PAYMENT_WRITE_ORDER_METHODS: &[&str] = &[
    "accept_settlement",
    "checkout",
    "enqueue_payment",
    "enqueue_settlement",
    "escrow",
    "invoice",
    "payment_provider",
    "prepare_payment",
    "prepare_settlement",
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
                !source.contains(concept.pattern),
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
fn payment_deferral_keeps_sdk_public_runtime_surface_passive_only() {
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

    for passive_export in [
        "OrderPaymentHandoffKind",
        "OrderPaymentStateKind",
        "OrderSettlementStateKind",
    ] {
        assert!(
            lib_source.contains(passive_export),
            "src/lib.rs must keep passive order payment status export `{passive_export}`"
        );
    }

    for forbidden in FORBIDDEN_PAYMENT_WRITE_PUBLIC_EXPORTS {
        assert!(
            !lib_source.contains(forbidden),
            "src/lib.rs must not expose deferred payment write surface `{forbidden}`"
        );
    }

    for forbidden in FORBIDDEN_PAYMENT_WRITE_ORDER_METHODS {
        assert!(
            !order_source.contains(forbidden),
            "src/orders_runtime.rs must not add deferred payment write method or capability `{forbidden}`"
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

    for export in REQUIRED_ORDER_RUNTIME_EXPORTS {
        assert!(
            source.contains(export),
            "src/lib.rs must explicitly expose order SDK runtime export `{export}`"
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
        source.contains("impl<'sdk> OrdersClient<'sdk> {"),
        "src/orders_runtime.rs must own OrdersClient runtime methods"
    );

    for method in REQUIRED_ORDERS_CLIENT_METHODS {
        assert!(
            source.contains(method),
            "OrdersClient must expose inventory-guarded method `{method}`"
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
        lib_source.contains("pub use crate::product_clients::{FarmsClient, ListingsClient, OrdersClient, SyncClient};"),
        "src/lib.rs must explicitly export product client handles"
    );
    assert!(
        !lib_source.contains("pub mod product_clients;"),
        "src/lib.rs must not expose the product_clients module path"
    );

    for client in [
        "FarmsClient",
        "ListingsClient",
        "OrdersClient",
        "SyncClient",
    ] {
        assert!(
            clients_source.contains(format!("pub struct {client}<'sdk>").as_str()),
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
