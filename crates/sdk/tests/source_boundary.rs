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

const FORBIDDEN_SDK_README_CONCEPTS: &[ForbiddenSdkConcept] = &[
    ForbiddenSdkConcept {
        pattern: "RadrootsSdk::builder()",
        reason: "SDK docs must describe RadrootsClient as the product runtime entrypoint",
    },
    ForbiddenSdkConcept {
        pattern: "sdk.orders()",
        reason: "SDK docs must describe the current trade product clients",
    },
    ForbiddenSdkConcept {
        pattern: "OrderStatusRequest",
        reason: "SDK docs must describe TradeStatusRequest as the status request DTO",
    },
    ForbiddenSdkConcept {
        pattern: "protocol::",
        reason: "SDK docs must not advertise a public protocol workflow bypass",
    },
    ForbiddenSdkConcept {
        pattern: "sdk.trade_buyer()",
        reason: "SDK docs must use grouped trade product handles",
    },
    ForbiddenSdkConcept {
        pattern: "sdk.trade_seller()",
        reason: "SDK docs must use grouped trade product handles",
    },
    ForbiddenSdkConcept {
        pattern: "sdk.trade_status()",
        reason: "SDK docs must use sdk.trades().status(...) as the only product status entrypoint",
    },
    ForbiddenSdkConcept {
        pattern: "sdk.trade_resync()",
        reason: "SDK docs must use grouped trade product handles",
    },
    ForbiddenSdkConcept {
        pattern: "sdk.trade_validation()",
        reason: "SDK docs must use sdk.dvm() for validation receipt ingestion",
    },
];

const REQUIRED_SDK_README_CONCEPTS: &[&str] = &[
    "RadrootsClient::builder()",
    "sdk.trades()",
    "TradeStatusRequest",
    "sdk.trades().buyer()",
    "sdk.trades().seller()",
    "sdk.trades().status(...)",
    "sdk.trades().resync()",
    "sdk.dvm()",
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
    "TradeCancellationPlan",
    "TradeCancellationReceipt",
    "TradeDecisionPlan",
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
    "TradeRevisionDecisionPlan",
    "TradeRevisionDecisionReceipt",
    "TradeRevisionDecisionRequest",
    "TradeRevisionProposalPlan",
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
    "TradeSubmitPlan",
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

const REQUIRED_IDENTITY_MODEL_EXPORTS: &[&str] = &[
    "DEFAULT_IDENTITY_PATH",
    "IdentityError",
    "RADROOTS_USERNAME_MAX_LEN",
    "RADROOTS_USERNAME_MIN_LEN",
    "RADROOTS_USERNAME_REGEX",
    "RadrootsIdentity",
    "RadrootsIdentityFile",
    "RadrootsIdentityId",
    "RadrootsIdentityProfile",
    "RadrootsIdentityPublic",
    "RadrootsIdentitySecretKeyFormat",
    "radroots_username_is_valid",
    "radroots_username_normalize",
];

const REQUIRED_IDENTITY_STORAGE_EXPORTS: &[&str] = &[
    "RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT",
    "RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX",
    "RadrootsEncryptedIdentityFile",
    "encrypted_identity_wrapping_key_path",
    "load_encrypted_identity",
    "load_encrypted_identity_with_key_slot",
    "load_identity_profile",
    "rotate_encrypted_identity",
    "rotate_encrypted_identity_with_key_slot",
    "store_encrypted_identity",
    "store_encrypted_identity_with_key_slot",
    "store_identity_profile",
];

const REQUIRED_TRADE_POLICY_EXPORTS: &[&str] = &[
    "AckPolicy",
    "PublishMode",
    "RelayResolutionPolicy",
    "SdkTradeIdempotencyRecord",
    "PrivacyPreflightConfirmation",
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
    "pub async fn status(",
];

const REQUIRED_DVM_CLIENT_METHODS: &[&str] = &[
    "pub fn prepare_trade_transition_proof_request(",
    "pub async fn enqueue_trade_transition_proof_request_with_explicit_signer(",
    "pub async fn ingest_validation_receipt(",
];

const REQUIRED_DVM_CLIENT_CONFIGURED_SIGNER_METHODS: &[&str] =
    &["pub async fn enqueue_trade_transition_proof_request("];

const FORBIDDEN_TRADES_CLIENT_PUBLIC_METHODS: &[&str] = &[
    "pub fn prepare_submit(",
    "pub async fn enqueue_submit(",
    "pub async fn enqueue_prepared_submit(",
    "pub async fn enqueue_submit_with_explicit_signer(",
    "pub async fn enqueue_prepared_submit_with_explicit_signer(",
    "pub fn prepare_decision(",
    "pub async fn enqueue_decision(",
    "pub async fn enqueue_prepared_decision(",
    "pub async fn enqueue_decision_with_explicit_signer(",
    "pub async fn enqueue_prepared_decision_with_explicit_signer(",
    "pub fn prepare_revision_proposal(",
    "pub async fn enqueue_revision_proposal(",
    "pub async fn enqueue_prepared_revision_proposal(",
    "pub async fn enqueue_revision_proposal_with_explicit_signer(",
    "pub async fn enqueue_prepared_revision_proposal_with_explicit_signer(",
    "pub fn prepare_revision_decision(",
    "pub async fn enqueue_revision_decision(",
    "pub async fn enqueue_prepared_revision_decision(",
    "pub async fn enqueue_revision_decision_with_explicit_signer(",
    "pub async fn enqueue_prepared_revision_decision_with_explicit_signer(",
    "pub fn prepare_cancellation(",
    "pub async fn enqueue_cancellation(",
    "pub async fn enqueue_prepared_cancellation(",
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

const REQUIRED_TRADE_RESYNC_CLIENT_METHODS: &[&str] = &["pub async fn resync("];

const FORBIDDEN_PRODUCT_CLIENT_HANDLES: &[&str] = &[
    "TradeStatusClient",
    "TradeValidationClient",
    "pub struct TradeStatusClient",
    "pub struct TradeValidationClient",
];

const FORBIDDEN_PRODUCT_CLIENT_METHODS: &[&str] = &[
    "pub fn status_client(",
    "pub fn validation(",
    "pub fn root(&self) -> &'client RadrootsClient",
];

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

const FORBIDDEN_REMOVED_ORDER_PRODUCT_EXPORTS: &[&str] = &[
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

const FORBIDDEN_TRADE_LOW_LEVEL_REQUEST_EXPORTS: &[&str] = &[
    "TradeCancellationEnqueueRequest",
    "TradeCancellationPrepareRequest",
    "TradeDecisionEnqueueRequest",
    "TradeDecisionPrepareRequest",
    "TradeRevisionDecisionEnqueueRequest",
    "TradeRevisionDecisionPrepareRequest",
    "TradeRevisionProposalEnqueueRequest",
    "TradeRevisionProposalPrepareRequest",
    "TradeSubmitEnqueueRequest",
    "TradeSubmitPrepareRequest",
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
fn sdk_readme_documents_current_public_product_surface() {
    let readme_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README");
    let readme = read_source(readme_path.as_path());

    for concept in FORBIDDEN_SDK_README_CONCEPTS {
        assert!(
            !readme.contains(concept.pattern),
            "README contains forbidden SDK public API concept `{}`: {}",
            concept.pattern,
            concept.reason
        );
    }

    for concept in REQUIRED_SDK_README_CONCEPTS {
        assert!(
            readme.contains(concept),
            "README must document current SDK public API concept `{concept}`"
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
fn default_status_noise_test_uses_production_ingest_not_perf_sql_ballast() {
    let orders_runtime_tests = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/orders_runtime.rs")
            .as_path(),
    );
    let default_status_noise_test = function_source(
        orders_runtime_tests.as_str(),
        "order_status_query_uses_indexed_order_id_under_background_event_noise",
    );
    let manual_perf_test = function_source(
        orders_runtime_tests.as_str(),
        "manual_local_status_perf_gate_measures_100k_events",
    );

    assert!(
        default_status_noise_test.contains("ingest_status_noise_events("),
        "default status noise test must use production-equivalent event-store ingest"
    );
    for forbidden in [
        "insert_perf_non_trade_events(",
        "insert_perf_trade_background_events(",
        "sqlx::query(",
    ] {
        assert!(
            !default_status_noise_test.contains(forbidden),
            "default status noise test must not use manual SQL ballast helper `{forbidden}`"
        );
    }
    for required in [
        "insert_perf_non_trade_events(",
        "insert_perf_trade_background_events(",
    ] {
        assert!(
            manual_perf_test.contains(required),
            "manual performance gate must retain explicit SQL ballast helper `{required}`"
        );
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

    for forbidden in FORBIDDEN_REMOVED_ORDER_PRODUCT_EXPORTS {
        assert!(
            !source.contains(forbidden),
            "src/lib.rs must not expose removed order SDK product export `{forbidden}`"
        );
    }

    for forbidden in FORBIDDEN_TRADE_LOW_LEVEL_REQUEST_EXPORTS {
        assert!(
            !source.contains(forbidden),
            "src/lib.rs must not expose low-level trade mutation request export `{forbidden}`"
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
fn identity_public_surface_is_an_explicit_feature_module() {
    let lib_source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );
    let identity_source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/identity.rs")
            .as_path(),
    );

    assert!(
        lib_source
            .lines()
            .any(|line| line.trim() == "pub mod identity;"),
        "src/lib.rs must expose identity as an explicit feature-gated public module"
    );
    assert!(
        lib_source
            .lines()
            .all(|line| line.trim() != "mod identity;"),
        "src/lib.rs must not hide identity exports in a private module"
    );
    assert!(
        !lib_source.contains("pub use crate::identity::{"),
        "src/lib.rs must not flatten identity exports into root aliases"
    );

    for export in REQUIRED_IDENTITY_MODEL_EXPORTS {
        assert!(
            identity_source.contains(export),
            "src/identity.rs must expose identity model export `{export}`"
        );
    }

    for export in REQUIRED_IDENTITY_STORAGE_EXPORTS {
        assert!(
            identity_source.contains(export),
            "src/identity.rs must expose identity storage export `{export}`"
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

    for method in FORBIDDEN_TRADES_CLIENT_PUBLIC_METHODS {
        assert!(
            !source.contains(method),
            "TradesClient must not expose low-level trade mutation method `{method}`"
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

    for forbidden in FORBIDDEN_PRODUCT_CLIENT_HANDLES {
        assert!(
            !lib_source.contains(forbidden),
            "src/lib.rs must not export removed product client handle `{forbidden}`"
        );
        assert!(
            !clients_source.contains(forbidden),
            "product_clients.rs must not define removed product client handle `{forbidden}`"
        );
    }

    for forbidden in FORBIDDEN_PRODUCT_CLIENT_METHODS {
        assert!(
            !clients_source.contains(forbidden),
            "product_clients.rs must not expose removed product client method `{forbidden}`"
        );
    }
}

#[test]
fn removed_client_and_config_modules_are_absent() {
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
fn removed_trade_client_root_export_is_absent() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/lib.rs")
            .as_path(),
    );

    assert!(
        !source.contains("TradeClient"),
        "src/lib.rs must not re-export the removed TradeClient facade"
    );
}

#[test]
fn removed_client_config_modules_are_not_public() {
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
            "src/lib.rs must not expose removed SDK client/config concept `{forbidden}`"
        );
    }
}

#[test]
fn sdk_public_api_does_not_export_protocol_workflow_bypass() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = read_source(manifest_dir.join("src/lib.rs").as_path());

    for forbidden in ["pub mod protocol;", "pub use crate::protocol"] {
        assert!(
            !source.contains(forbidden),
            "src/lib.rs must not expose protocol workflow bypass `{forbidden}`"
        );
    }

    assert!(
        !manifest_dir.join("src/protocol").exists(),
        "src/protocol must not remain as a public protocol re-export surface"
    );
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
            "{relative_path} must not use removed SDK client or transport concept `{forbidden}`"
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

fn function_source<'source>(source: &'source str, function_name: &str) -> &'source str {
    let signature = format!("async fn {function_name}");
    let start = source
        .find(signature.as_str())
        .unwrap_or_else(|| panic!("failed to find test function `{function_name}`"));
    let source_after_start = &source[start..];
    let end = source_after_start
        .find("\n#[tokio::test]")
        .unwrap_or(source_after_start.len());
    &source_after_start[..end]
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
