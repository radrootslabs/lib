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
    ForbiddenSdkConcept {
        pattern: "`relay-client` is retained",
        reason: "SDK docs must describe canonical feature behavior instead of retained surfaces",
    },
    ForbiddenSdkConcept {
        pattern: "direct relay publish callers",
        reason: "SDK docs must describe adapter behavior rather than caller compatibility posture",
    },
    ForbiddenSdkConcept {
        pattern: "compatibility",
        reason: "SDK docs must not frame target-state runtime surfaces as compatibility policy",
    },
    ForbiddenSdkConcept {
        pattern: "legacy",
        reason: "SDK docs must not frame target-state runtime surfaces as legacy policy",
    },
    ForbiddenSdkConcept {
        pattern: "preserve serde",
        reason: "SDK docs must describe serialized-field stability without serde-compat wording",
    },
];

const FORBIDDEN_SDK_ROOT_TRADE_ALIAS_NAMES: &[&str] = &[
    "trade_buyer",
    "trade_seller",
    "trade_status",
    "trade_resync",
    "trade_validation",
];

const FORBIDDEN_DAEMON_PUBLISH_PROXY_IDENTIFIERS: &[&str] = &[
    "\"radrootsd_proxy\"",
    "radrootsd.publish_proxy.v1",
    "radroots_publish_proxy_protocol",
    "publish_proxy_protocol",
    "publish.relays.resolve",
    "\"publish.event\"",
    "PublishRelayPolicy",
    "PublishDeliveryPolicy",
    "PublishEventRequest",
    "PublishEventResponse",
    "PublishJobView",
    "PublishRelayOutcome",
    "PublishRelaySource",
];

const REQUIRED_SDK_README_CONCEPTS: &[&str] = &[
    "RadrootsClient::builder()",
    "sdk.trades()",
    "TradeStatusRequest",
    "sdk.trades().buyer()",
    "sdk.trades().seller()",
    "sdk.trades().status(...)",
    "sdk.trades().resync()",
    "sdk.trades().validation_receipts()",
    "sdk.dvm()",
];

const REQUIRED_TRADE_RUNTIME_EXPORTS: &[&str] = &[
    "TRADE_STATUS_DEFAULT_LIMIT",
    "TRADE_STATUS_MAX_LIMIT",
    "TradeEvidenceBranchReceipt",
    "TradeEvidenceIngestReceipt",
    "TradeEvidenceIngestRequest",
    "TradeEvidenceQueryBranch",
    "TradeEvidenceQueryBranchKind",
    "TradeEvidenceQueryPlan",
    "TradeEvidenceNostrRelayFilter",
    "TradeEvidenceNostrRelayTagFilter",
    "TradeRequestEvidenceIngestReceipt",
    "TradeRequestEvidenceIngestRequest",
    "TradeResyncEventImportReceipt",
    "TradeResyncEvidenceReceipt",
    "TradeResyncNostrRelayOutcomeKind",
    "TradeResyncNostrRelayOutcomeReceipt",
    "TradeResyncNostrRelayTransportOutcomeKind",
    "TradeResyncReceipt",
    "TradeResyncRequest",
    "TradeSellerInboxReceipt",
    "TradeSellerInboxRequest",
    "TradeStatusAmbiguityCandidate",
    "TradeStatusEligibility",
    "TradeStatusEvidenceSummary",
    "TradeStatusKind",
    "TradeStatusNextActionKind",
    "TradeStatusReceipt",
    "TradeStatusRequest",
    "TradeValidationReceiptEvent",
    "TradeValidationReceiptInspectReceipt",
    "TradeValidationReceiptInspectRequest",
    "TradeValidationReceiptInvalidCandidate",
    "TradeValidationReceiptListReceipt",
    "TradeValidationReceiptListRequest",
    "TradeValidationReceiptNostrEvidenceReceipt",
    "TradeValidationReceiptNostrRelayOutcomeKind",
    "TradeValidationReceiptNostrRelayOutcomeReceipt",
    "TradeValidationReceiptNostrRelayTransportOutcomeKind",
    "TradeValidationReceiptTags",
    "TradeValidationReceiptVerifyRequest",
    "TradeValidationTrustDecision",
    "TradeValidationReceiptWorkerEvidence",
    "TradeValidationReceiptWorkerEvidenceSelection",
    "SdkTradeStatusIssue",
    "SdkTradeStatusIssueKind",
    "SdkTradeStatusSource",
];

const REQUIRED_TRADE_SIGNER_EXPORTS: &[&str] = &[
    "TRADE_CANCELLATION_OPERATION_KIND",
    "TRADE_DECISION_OPERATION_KIND",
    "TRADE_REVISION_DECISION_OPERATION_KIND",
    "TRADE_REVISION_PROPOSAL_OPERATION_KIND",
    "TRADE_SUBMIT_OPERATION_KIND",
    "TradeAcceptRequest",
    "TradeCancelRequest",
    "TradeCancellationPlan",
    "TradeCancellationReceipt",
    "TradeDecisionPlan",
    "TradeDecisionReceipt",
    "TradeDeclineRequest",
    "TradeEvidenceMode",
    "TradeMutationOutcome",
    "TradeProposeRequest",
    "TradeRevisionDecisionPlan",
    "TradeRevisionDecisionReceipt",
    "TradeRevisionDecisionRequest",
    "TradeRevisionProposalPlan",
    "TradeRevisionProposalRequest",
    "TradeRevisionProposalReceipt",
    "TradeSubmitPlan",
    "TradeSubmitReceipt",
    "TradeWorkflowEnqueueReceipt",
    "TradeWorkflowIdempotencyReceipt",
    "TradeWorkflowKind",
    "TradeWorkflowPlan",
    "TradeWorkflowRetryAdvice",
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
    "SatisfactionPolicy",
    "PublishMode",
    "TargetPolicy",
    "SdkTradeIdempotencyRecord",
    "PrivacyPreflightConfirmation",
    "PrivacyPreflightReceipt",
    "PrivacyPreflightStatus",
    "ProductSensitivityField",
    "RadrootsTradeValidationTrustPolicy",
    "RadrootsTradeValidationTrustState",
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

const REQUIRED_TRADE_VALIDATION_RECEIPTS_CLIENT_METHODS: &[&str] = &[
    "pub async fn list(",
    "pub async fn inspect(",
    "pub async fn verify(",
];

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
fn sdk_sources_reject_production_dead_code_suppressions() {
    let offenders = rust_source_files(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path())
        .into_iter()
        .filter_map(|path| {
            let source = read_source(path.as_path());
            source
                .contains("allow(dead_code)")
                .then(|| path.display().to_string())
        })
        .collect::<Vec<_>>();

    assert!(
        offenders.is_empty(),
        "SDK sources contain production dead-code suppressions: {offenders:?}"
    );
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
fn sdk_transport_nostr_features_do_not_retain_relay_named_aliases() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = read_source(manifest_dir.join("Cargo.toml").as_path());

    for required in [
        "transport-nostr-client = [",
        "transport-nostr-runtime = [",
        "\"transport-nostr-runtime\",",
        "\"transport-nostr-client\",",
    ] {
        assert!(
            manifest.contains(required),
            "Cargo.toml must retain canonical SDK transport feature witness `{required}`"
        );
    }
    for forbidden in ["relay-client", "relay-runtime"] {
        assert!(
            !manifest.contains(forbidden),
            "Cargo.toml must not retain removed SDK transport feature `{forbidden}`"
        );
    }

    let lib_source = read_source(manifest_dir.join("src/lib.rs").as_path());
    assert!(
        lib_source.contains("feature = \"transport-nostr-client\""),
        "src/lib.rs must gate adapters on transport-nostr-client"
    );
    assert!(
        !lib_source.contains("relay-client") && !lib_source.contains("relay-runtime"),
        "src/lib.rs must not retain removed transport feature names"
    );

    let adapters_mod_source = read_source(manifest_dir.join("src/adapters/mod.rs").as_path());
    assert!(
        adapters_mod_source.contains("pub mod nostr;"),
        "src/adapters/mod.rs must expose the Nostr adapter module by transport kind"
    );
    assert!(
        !adapters_mod_source.contains("pub mod relay;"),
        "src/adapters/mod.rs must not retain a relay-named public adapter module"
    );
    assert!(
        !manifest_dir.join("src/adapters/relay.rs").exists(),
        "src/adapters/relay.rs must not remain as a relay-named public adapter module"
    );

    for relative_path in [
        "src/sync_runtime.rs",
        "src/orders_runtime.rs",
        "examples/runtime_local.rs",
        "README",
    ] {
        let source = read_source(manifest_dir.join(relative_path).as_path());
        for forbidden in ["relay-client", "relay-runtime"] {
            assert!(
                !source.contains(forbidden),
                "{relative_path} must not retain removed SDK transport feature `{forbidden}`"
            );
        }
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
fn sdk_sources_do_not_reintroduce_root_trade_alias_surface() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    for path in sdk_root_alias_guard_files(manifest_dir) {
        let source = read_source(path.as_path());
        let relative_path = relative_manifest_path(manifest_dir, path.as_path());
        let findings = root_trade_alias_findings(relative_path.as_str(), source.as_str());
        assert!(
            findings.is_empty(),
            "SDK root trade aliases are forbidden:\n{}",
            findings.join("\n")
        );
    }
}

#[test]
fn radroots_client_does_not_expose_root_trade_alias_methods() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/runtime.rs")
            .as_path(),
    );
    let client_impl = impl_block(source.as_str(), "impl RadrootsClient {");
    let findings = root_trade_alias_findings("src/runtime.rs RadrootsClient impl", client_impl);

    assert!(
        findings.is_empty(),
        "RadrootsClient must expose grouped product handles instead of root trade aliases:\n{}",
        findings.join("\n")
    );
}

#[test]
fn root_trade_alias_scanner_preserves_internal_status_identifiers() {
    let allowed_source = r#"
        let code = "trade_status_limit_invalid";
        let error = RadrootsSdkError::trade_status_limit_invalid(0, 1, 100);
        async fn dvm_validation_receipt_ingest_commits_pending_trade_status() {}
    "#;
    assert!(
        root_trade_alias_findings("allowed_fixture.rs", allowed_source).is_empty(),
        "internal status identifiers must remain allowed when they are not root alias calls or definitions"
    );

    let forbidden_source = r#"
        impl RadrootsClient {
            pub async fn trade_status(&self) {}
        }
        async fn workflow(sdk: RadrootsClient) {
            sdk.trade_resync (request).await;
            RadrootsClient::trade_buyer(&sdk);
        }
    "#;
    let findings = root_trade_alias_findings("forbidden_fixture.rs", forbidden_source);

    for alias in ["trade_status", "trade_resync", "trade_buyer"] {
        assert!(
            findings.iter().any(|finding| finding.contains(alias)),
            "root trade alias scanner must reject `{alias}`"
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
fn private_protocol_helper_modules_are_runtime_gated_by_lib() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_source = read_source(manifest_dir.join("src/lib.rs").as_path());

    for module in ["farm", "order"] {
        let module_gate = format!("#[cfg(feature = \"runtime\")]\nmod {module};");
        assert!(
            lib_source.contains(module_gate.as_str()),
            "src/lib.rs must keep private protocol helper module `{module}` behind runtime"
        );
    }

    for relative_path in ["src/farm.rs", "src/order.rs"] {
        let source = read_source(manifest_dir.join(relative_path).as_path());
        assert!(
            !source.contains("feature = \"runtime\""),
            "{relative_path} must rely on the lib.rs module-level runtime gate"
        );
        assert!(
            !source.contains("feature = \"serde_json\""),
            "{relative_path} must rely on the runtime feature's serde_json contract instead of duplicating it"
        );
    }

    let order_source = read_source(manifest_dir.join("src/order.rs").as_path());
    for helper in [
        "build_order_request_draft",
        "build_order_decision_draft",
        "build_order_revision_proposal_draft",
        "build_order_revision_decision_draft",
        "build_order_cancellation_draft",
    ] {
        let helper_gate =
            format!("#[cfg(any(feature = \"signer-adapters\", test))]\npub fn {helper}(");
        assert!(
            order_source.contains(helper_gate.as_str()),
            "src/order.rs must keep `{helper}` available only for signer adapters and unit tests"
        );
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

    let runtime_exports = export_block(
        source.as_str(),
        "#[cfg(feature = \"runtime\")]\npub use crate::orders_runtime::{",
    );
    let signer_exports = export_block(
        source.as_str(),
        "#[cfg(all(feature = \"runtime\", feature = \"signer-adapters\"))]\npub use crate::orders_runtime::{",
    );

    for export in REQUIRED_TRADE_RUNTIME_EXPORTS {
        assert!(
            runtime_exports.contains(export),
            "src/lib.rs must explicitly expose trade SDK runtime export `{export}`"
        );
    }

    for export in REQUIRED_TRADE_SIGNER_EXPORTS {
        assert!(
            signer_exports.contains(export),
            "src/lib.rs must expose trade SDK signer export `{export}` behind runtime plus signer-adapters"
        );
        assert!(
            !runtime_exports.contains(export),
            "src/lib.rs runtime-only export block must not expose signer workflow export `{export}`"
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
fn trade_resync_and_validation_receipts_use_nostr_scoped_public_evidence_names() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let orders_source = read_source(manifest_dir.join("src/orders_runtime.rs").as_path());
    let lib_source = read_source(manifest_dir.join("src/lib.rs").as_path());

    for required in [
        "pub nostr_relay_urls: Vec<String>,",
        "pub nostr_evidence: TradeValidationReceiptNostrEvidenceReceipt,",
        "pub nostr_relay_outcomes: Vec<TradeResyncNostrRelayOutcomeReceipt>,",
        "pub nostr_relay_outcomes: Vec<TradeValidationReceiptNostrRelayOutcomeReceipt>,",
        "pub nostr_relay_url: String,",
        "TradeResyncNostrRelayOutcomeReceipt",
        "TradeResyncNostrRelayOutcomeKind",
        "TradeResyncNostrRelayTransportOutcomeKind",
        "TradeValidationReceiptNostrEvidenceReceipt",
        "TradeValidationReceiptNostrRelayOutcomeReceipt",
        "TradeValidationReceiptNostrRelayOutcomeKind",
        "TradeValidationReceiptNostrRelayTransportOutcomeKind",
    ] {
        assert!(
            orders_source.contains(required) || lib_source.contains(required),
            "SDK trade evidence source must retain Nostr-scoped public witness `{required}`"
        );
    }

    for forbidden in [
        "TradeResyncRelayOutcomeReceipt",
        "TradeResyncRelayOutcomeKind",
        "TradeResyncRelayTransportOutcomeKind",
        "TradeValidationReceiptRelayEvidenceReceipt",
        "TradeValidationReceiptRelayOutcomeReceipt",
        "TradeValidationReceiptRelayOutcomeKind",
        "TradeValidationReceiptRelayTransportOutcomeKind",
        "pub relay_targets:",
        "pub relay_evidence:",
        "pub relay_url:",
        "pub relays:",
        "relay_transport",
    ] {
        assert!(
            !orders_source.contains(forbidden) && !lib_source.contains(forbidden),
            "SDK trade evidence public source must not retain generic relay-shaped name `{forbidden}`"
        );
    }
}

#[test]
fn order_runtime_rejects_retired_status_source_names() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/orders_runtime.rs")
            .as_path(),
    );

    for retired in ["LocalEventStore", "local_event_store"] {
        assert!(
            !source.contains(retired),
            "src/orders_runtime.rs must not expose retired trade status source `{retired}`"
        );
    }
}

#[test]
fn order_runtime_mutation_requests_require_evidence_mode() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/orders_runtime.rs")
            .as_path(),
    );

    assert!(
        source.contains("pub enum TradeEvidenceMode"),
        "src/orders_runtime.rs must define explicit trade mutation evidence modes"
    );
    for variant in [
        "LocalOnly",
        "ResyncBeforeMutation",
        "RequireExplicitEvidence",
    ] {
        assert!(
            source.contains(variant),
            "TradeEvidenceMode must retain `{variant}`"
        );
    }
    for request in [
        "TradeAcceptRequest",
        "TradeDeclineRequest",
        "TradeCancelRequest",
        "TradeRevisionProposalRequest",
        "TradeRevisionDecisionRequest",
    ] {
        let struct_block = struct_block(source.as_str(), request);
        assert!(
            struct_block.contains("pub evidence_mode: TradeEvidenceMode"),
            "{request} must carry explicit evidence_mode"
        );
    }
    for constructor in [
        "impl TradeAcceptRequest",
        "impl TradeDeclineRequest",
        "impl TradeCancelRequest",
        "impl TradeRevisionProposalRequest",
        "impl TradeRevisionDecisionRequest",
    ] {
        let impl_source = impl_block(source.as_str(), constructor);
        assert!(
            impl_source.contains("evidence_mode: TradeEvidenceMode"),
            "{constructor}::new must require explicit evidence mode"
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

    for method in REQUIRED_TRADE_VALIDATION_RECEIPTS_CLIENT_METHODS {
        assert!(
            source.contains(method),
            "TradeValidationReceiptsClient must expose product workflow method `{method}`"
        );
    }
}

#[test]
fn trade_product_facade_feature_gates_are_explicit() {
    let orders_source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/orders_runtime.rs")
            .as_path(),
    );
    let product_clients_source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/product_clients.rs")
            .as_path(),
    );

    assert!(
        product_clients_source.contains(
            "#[cfg(feature = \"signer-adapters\")]\n    pub fn buyer(&self) -> TradeBuyerClient<'client> {"
        ),
        "TradesClient::buyer must be gated by signer-adapters inside the runtime TradesClient impl"
    );
    assert!(
        product_clients_source.contains(
            "pub fn validation_receipts(&self) -> TradeValidationReceiptsClient<'client>"
        ),
        "TradesClient must expose validation receipts through the grouped trade product facade"
    );
    assert!(
        product_clients_source.contains(
            "#[cfg(all(feature = \"runtime\", feature = \"signer-adapters\"))]\n#[derive(Clone, Copy)]\npub struct TradeBuyerClient<'client>"
        ),
        "TradeBuyerClient must be gated by runtime plus signer-adapters"
    );
    assert!(
        product_clients_source.contains(
            "#[cfg(feature = \"runtime\")]\n#[derive(Clone, Copy)]\npub struct TradeSellerClient<'client>"
        ),
        "TradeSellerClient must remain runtime-visible for seller inbox reads"
    );
    assert!(
        orders_source.contains(
            "#[cfg(all(feature = \"runtime\", feature = \"signer-adapters\"))]\nimpl<'sdk> TradeBuyerClient<'sdk> {"
        ),
        "TradeBuyerClient product mutation impl must require runtime plus signer-adapters"
    );
    assert!(
        orders_source
            .contains("#[cfg(feature = \"runtime\")]\nimpl<'sdk> TradeSellerClient<'sdk> {"),
        "TradeSellerClient impl must remain runtime-visible for seller inbox"
    );

    let seller_impl = impl_block(
        orders_source.as_str(),
        "impl<'sdk> TradeSellerClient<'sdk> {",
    );
    assert!(
        seller_impl.contains("pub async fn inbox("),
        "TradeSellerClient must expose seller inbox in the runtime impl"
    );
    for method in ["accept_trade", "decline_trade", "propose_revision"] {
        let gated_method =
            format!("#[cfg(feature = \"signer-adapters\")]\n    pub async fn {method}(");
        assert!(
            seller_impl.contains(gated_method.as_str()),
            "TradeSellerClient::{method} must be gated by signer-adapters"
        );
    }
}

#[test]
fn trade_propose_request_stays_product_shaped() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/orders_runtime.rs")
            .as_path(),
    );
    let request = struct_block(source.as_str(), "TradeProposeRequest");

    assert!(
        !request.contains("RadrootsOrderRequest"),
        "TradeProposeRequest must not expose protocol-shaped order request input"
    );
    for required_field in [
        "pub order_id: RadrootsOrderId",
        "pub listing_addr: RadrootsListingAddress",
        "pub seller_pubkey: RadrootsPublicKey",
        "pub items: Vec<RadrootsOrderItem>",
        "pub economics: RadrootsOrderEconomics",
        "pub public_note: Option<String>",
    ] {
        assert!(
            request.contains(required_field),
            "TradeProposeRequest must expose product field `{required_field}`"
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
        "TradeResyncClient",
        "TradeSellerClient",
        "TradeValidationReceiptsClient",
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

    let runtime_product_exports = export_block(
        lib_source.as_str(),
        "#[cfg(feature = \"runtime\")]\npub use crate::product_clients::{",
    );
    assert!(
        !runtime_product_exports.contains("TradeBuyerClient"),
        "src/lib.rs runtime-only product client export block must not expose TradeBuyerClient"
    );
    assert!(
        lib_source.contains(
            "#[cfg(all(feature = \"runtime\", feature = \"signer-adapters\"))]\npub use crate::product_clients::TradeBuyerClient;"
        ),
        "src/lib.rs must export TradeBuyerClient only behind runtime plus signer-adapters"
    );
    assert!(
        clients_source.contains("pub struct TradeBuyerClient<'client>"),
        "product_clients.rs must define thin signer-gated handle `TradeBuyerClient`"
    );

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

#[test]
fn sdk_proxy_surfaces_reject_removed_daemon_publish_proxy_identifiers() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative_path in [
        "Cargo.toml",
        "src/lib.rs",
        "src/adapters/radrootsd.rs",
        "src/sync_runtime.rs",
        "src/transport.rs",
    ] {
        let source = read_source(manifest_dir.join(relative_path).as_path());
        for forbidden in FORBIDDEN_DAEMON_PUBLISH_PROXY_IDENTIFIERS {
            assert!(
                !contains_forbidden_concept(source.as_str(), forbidden),
                "{relative_path} must not reintroduce removed daemon publish proxy identifier `{forbidden}`"
            );
        }
    }

    let transport_source = read_source(manifest_dir.join("src/transport.rs").as_path());
    assert!(
        transport_source.contains("RadrootsTransportKind::Proxy"),
        "src/transport.rs must model SDK proxy targets with RadrootsTransportKind::Proxy"
    );

    let sync_runtime_source = read_source(manifest_dir.join("src/sync_runtime.rs").as_path());
    assert!(
        sync_runtime_source.contains("target.transport_kind == RadrootsTransportKind::Proxy"),
        "src/sync_runtime.rs must identify proxy delegate targets with RadrootsTransportKind::Proxy"
    );
    assert!(
        sync_runtime_source
            .contains("radrootsd proxy outbox publish explicit targets are Nostr-only"),
        "src/sync_runtime.rs must reject non-Nostr proxy outbox explicit targets before behavior is lost"
    );
    assert!(
        sync_runtime_source.contains("active_delivery_plan_id(claimed"),
        "src/sync_runtime.rs must derive proxy publish behavior from the claimed active delivery plan"
    );
    assert!(
        sync_runtime_source.contains("mixed proxy delegate targets"),
        "src/sync_runtime.rs must fail closed if proxy delegate targets are mixed in a claimed publish set"
    );
    for required in [
        "let mut completed_target_ids = std::collections::BTreeSet::new();",
        "let mut matched_outcomes = Vec::new();",
        "matched multiple ready delivery targets",
        "matched delivery target",
        "more than once",
    ] {
        assert!(
            sync_runtime_source.contains(required),
            "src/sync_runtime.rs must retain proxy completion uniqueness witness `{required}`"
        );
    }
    assert!(
        !sync_runtime_source.contains("TransportPublishPreviewBehavior::RejectDeliveryAttempts"),
        "src/sync_runtime.rs must not rewrite Reticulum proxy outbox targets to reject attempts"
    );
    let sync_runtime_unit_source = read_source(
        manifest_dir
            .join("tests/unit/sync_runtime_tests.rs")
            .as_path(),
    );
    for required in [
        "claimed_uningested_proxy_event",
        "assert_no_transport_publish_request",
        "assert!(!stored_before.event_store_ingested)",
        "assert!(!stored.event_store_ingested)",
        "with_timeout(Duration::from_millis(50))",
        "proxy_delivery_policy_rejects_delivered_satisfaction_before_daemon_publish",
        "proxy_outbox_target_conversion_preserves_nostr_scope_and_label",
        "proxy_completion_matches_duplicate_endpoint_targets_by_scope",
        "push_proxy_event_receipt_preserves_daemon_target_metadata",
        "proxy_completion_rejects_duplicate_daemon_outcome_before_local_mutation",
    ] {
        assert!(
            sync_runtime_unit_source.contains(required),
            "tests/unit/sync_runtime_tests.rs must retain proxy local-validation ordering proof `{required}`"
        );
    }

    let adapter_source = read_source(manifest_dir.join("src/adapters/radrootsd.rs").as_path());
    assert!(
        !adapter_source
            .contains("impl RadrootsRelayPublishAdapter for RadrootsdProxyPublishAdapter"),
        "src/adapters/radrootsd.rs must not implement relay publish traits for RadrootsdProxyPublishAdapter"
    );
    assert!(
        !adapter_source.contains("proxy_relay_receipt_from_response"),
        "src/adapters/radrootsd.rs must not convert typed transport publish jobs into relay receipts"
    );
    assert!(
        !adapter_source.contains("TransportPublishPreviewBehavior::RejectDeliveryAttempts"),
        "src/adapters/radrootsd.rs must not rewrite Reticulum relay targets to reject attempts"
    );

    for required in [
        "TransportPublishOutcomeKind::DeferredUntilImplemented",
        "mark_delivery_target_deferred_until_implemented",
        "TransportPublishOutcomeKind::PreviewUnavailable",
        "mark_delivery_target_preview_unavailable",
        "PushOutboxEventState::DeferredUntilImplemented",
        "PushOutboxEventState::PreviewUnavailable",
        "PushOutboxTargetOutcomeKind::DeferredUntilImplemented",
        "PushOutboxTargetOutcomeKind::PreviewUnavailable",
        "reject_delivered_proxy_satisfaction",
        "RadrootsTransportSatisfactionClass::Delivered",
        "target.target_scope.as_ref()",
        "outcome.target_scope.as_deref()",
        "target_scope: outcome.target_scope",
        "target_label: outcome.target_label",
    ] {
        assert!(
            sync_runtime_source.contains(required),
            "src/sync_runtime.rs must preserve proxy preview/deferred outcome witness `{required}`"
        );
    }

    let receipt_source = source_between(
        sync_runtime_source.as_str(),
        "fn push_proxy_event_receipt",
        "fn push_proxy_target_receipt",
    );
    assert!(
        receipt_source.contains("push_receipt_event_id("),
        "push_proxy_event_receipt must convert daemon event ids through the typed receipt helper"
    );
    for forbidden in [".expect(", ".unwrap(", "panic!("] {
        assert!(
            !receipt_source.contains(forbidden),
            "push_proxy_event_receipt must not use production panic path `{forbidden}`"
        );
    }

    let proxy_target_receipt_source = source_between(
        sync_runtime_source.as_str(),
        "fn push_proxy_target_receipt",
        "fn push_proxy_target_outcome_kind",
    );
    for forbidden in ["target_scope: None", "target_label: None"] {
        assert!(
            !proxy_target_receipt_source.contains(forbidden),
            "push_proxy_target_receipt must not hard-code daemon metadata field `{forbidden}`"
        );
    }
}

#[test]
fn sdk_transport_policy_sources_reject_configured_profile_and_proxy_relay_bridge() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative_path in [
        "src",
        "tests",
        "examples",
        "packages/events-bindings/src",
        "packages/events-codec-js/src",
    ] {
        let root = manifest_dir.join(relative_path);
        if !root.exists() {
            continue;
        }
        for path in rust_source_files(root.as_path()) {
            if path.file_name().and_then(|file_name| file_name.to_str())
                == Some("source_boundary.rs")
            {
                continue;
            }
            let source = read_source(path.as_path());
            for forbidden in [
                "UseConfiguredProfile",
                "use_configured_profile",
                "configured_profile()",
                "UseTransportProfile",
                "use_transport_profile",
                "impl RadrootsRelayPublishAdapter for RadrootsdProxyPublishAdapter",
                "proxy_relay_receipt_from_response",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "{} must not reintroduce removed transport policy or proxy relay bridge surface `{forbidden}`",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn sdk_sync_status_sources_reject_retired_relay_shaped_generic_fields() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sync_runtime = read_source(manifest_dir.join("src/sync_runtime.rs").as_path());
    let status_summary = source_between(
        sync_runtime.as_str(),
        "pub struct SyncTransportStatusSummary",
        "\n}\n\n#[cfg(feature = \"runtime\")]\nimpl SyncTransportStatusSummary",
    );

    for required in [
        "pub configured_transport_target_count: usize,",
        "pub configured_transport_targets: Vec<SyncTransportTargetSummary>,",
        "pub transport_statuses: Vec<SyncTransportStatusSummary>,",
        ".configured_transport_targets()?",
        ".transport_statuses()",
        "pub target_scope: Option<String>,",
        "pub target_label: Option<String>,",
        "target.scope.as_ref()",
        "target.label.as_ref()",
        "pub transport: String,",
        "pub configured: bool,",
        "pub implementation: String,",
        "pub usable_for_delivery: bool,",
        "pub message: String,",
    ] {
        assert!(
            sync_runtime.contains(required),
            "SDK sync status must retain transport-neutral field witness `{required}`"
        );
    }

    for required in [
        "pub transport: String,",
        "pub configured: bool,",
        "pub implementation: String,",
        "pub usable_for_delivery: bool,",
        "pub message: String,",
    ] {
        assert!(
            status_summary.contains(required),
            "SDK sync status summary must retain canonical status field witness `{required}`"
        );
    }

    for forbidden in [
        concat!("configured_nostr", "_relay", "_count"),
        concat!("configured_nostr", "_relays"),
        concat!("target", "_relays"),
        concat!("connected", "_relays"),
        concat!("acknowledged", "_relays"),
        concat!("failed", "_relays"),
        concat!("relay", "_count"),
    ] {
        assert!(
            !sync_runtime.contains(forbidden),
            "SDK sync status source must not expose retired relay-shaped generic field `{forbidden}`"
        );
    }

    for forbidden in [
        concat!("pub transport", "_kind: String,"),
        concat!("pub implementation", "_state: String,"),
        "pub readiness: String,",
        concat!("pub publish", "_usable: bool,"),
        concat!("pub fetch", "_usable: bool,"),
        concat!("pub redacted", "_message: Option<String>,"),
    ] {
        assert!(
            !status_summary.contains(forbidden),
            "SDK sync status summary must not expose retired status field `{forbidden}`"
        );
    }
}

#[test]
fn sdk_transport_sources_expose_full_satisfaction_policy_without_legacy_aliases() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let transport_source = read_source(manifest_dir.join("src/transport.rs").as_path());
    let workflow_runtime = read_source(manifest_dir.join("src/workflow_runtime.rs").as_path());

    for required in [
        "pub enum SatisfactionPolicy",
        "AnyAccepted",
        "AllAccepted",
        "QuorumAccepted",
        "AnyDelivered",
        "AllDelivered",
        "QuorumDelivered",
        "RequiredAcceptedTargets",
        "RequiredDeliveredTargets",
        "pub fn quorum_accepted(",
        "pub fn quorum_delivered(",
        "pub fn required_accepted_targets",
        "pub fn required_delivered_targets",
        "RadrootsTransportSatisfactionPolicy::required_targets",
        "RadrootsTransportSatisfactionClass::Accepted",
        "RadrootsTransportSatisfactionClass::Delivered",
    ] {
        assert!(
            transport_source.contains(required),
            "src/transport.rs must retain full SDK satisfaction policy witness `{required}`"
        );
    }

    for forbidden in [
        "AtLeastOneTarget",
        "AllTargets",
        "pub fn at_least",
        concat!("at_least", "_one_target"),
        concat!("all", "_targets"),
    ] {
        assert!(
            !transport_source.contains(forbidden) && !workflow_runtime.contains(forbidden),
            "SDK transport/workflow sources must not retain legacy satisfaction alias `{forbidden}`"
        );
    }
}

#[test]
fn sdk_workflow_runtime_records_local_import_observations() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow_runtime = read_source(manifest_dir.join("src/workflow_runtime.rs").as_path());

    for required in [
        "SDK_LOCAL_EVENT_ENDPOINT_URI",
        "RadrootsTransportKind::Local",
        "RadrootsTransportObservationType::LocalImport",
        ".with_observation(local_import_observation)",
    ] {
        assert!(
            workflow_runtime.contains(required),
            "SDK workflow runtime must retain local import observation witness `{required}`"
        );
    }
}

#[test]
fn sdk_transport_sources_keep_reticulum_preview_push_boundary() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative_path in [
        "src/transport.rs",
        "src/sync_runtime.rs",
        "src/error.rs",
        "src/adapters/radrootsd.rs",
    ] {
        let source = read_source(manifest_dir.join(relative_path).as_path());
        for line in removed_reticulum_preview_endpoint_lines(source.as_str()) {
            panic!(
                "{relative_path}:{line} contains removed Reticulum preview endpoint `reticulum:preview`"
            );
        }
    }

    let transport_source = read_source(manifest_dir.join("src/transport.rs").as_path());
    assert!(
        transport_source.contains("RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI"),
        "src/transport.rs must consume the shared Reticulum preview endpoint constant"
    );
    assert!(
        !transport_source.contains("const RETICULUM_PREVIEW_ENDPOINT_URI"),
        "src/transport.rs must not duplicate the Reticulum preview endpoint constant"
    );
    assert!(
        !transport_source.contains("reticulum:preview-unavailable"),
        "src/transport.rs must not duplicate the Reticulum preview endpoint literal"
    );

    let sync_runtime = read_source(manifest_dir.join("src/sync_runtime.rs").as_path());
    for required in [
        "TransportProfile::ReticulumPreview { .. }",
        "reticulum_preview_push_receipt",
        "reticulum_preview_event_receipt",
        "push_reported_event",
        "RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE",
        "ReticulumPreviewTryNowRequest",
        "try_reticulum_preview_now",
        "\"sync.try_reticulum_preview_now\"",
        "RadrootsSdkError::ReticulumPreviewTransportUnavailable",
        "pub target_scope: Option<String>,",
        "pub target_label: Option<String>,",
        "pub transport_outcome_kind: Option<PushOutboxTransportOutcomeKind>,",
        "PushOutboxTransportOutcomeKind::TransportUnavailable",
    ] {
        assert!(
            sync_runtime.contains(required),
            "src/sync_runtime.rs must retain Reticulum preview push boundary `{required}`"
        );
    }
    assert!(
        !sync_runtime.contains("push_outbox_has_no_reticulum_preview_work"),
        "src/sync_runtime.rs must not revive the Reticulum preview ready-work error probe"
    );
    assert!(
        !sync_runtime.contains("RadrootsSdkError::reticulum_preview_transport_unavailable(\n"),
        "src/sync_runtime.rs must not return Reticulum preview unavailable errors from push_outbox"
    );

    let error_source = read_source(manifest_dir.join("src/error.rs").as_path());
    for required in [
        "reticulum_preview_transport_unavailable",
        "reticulum_preview_transport_deferred",
        "Reticulum preview endpoint",
    ] {
        assert!(
            error_source.contains(required),
            "src/error.rs must retain Reticulum preview error witness `{required}`"
        );
    }
}

#[test]
fn sdk_feature_matrix_keeps_reticulum_preview_runtime_owned_without_alias() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_source = read_source(manifest_dir.join("Cargo.toml").as_path());
    let features_source = source_between(manifest_source.as_str(), "[features]", "[dependencies]");
    let runtime_source = source_between(features_source, "runtime = [", "local-signer = [");
    let nostr_runtime_source = source_between(
        features_source,
        "transport-nostr-runtime = [",
        "local-runtime = [",
    );

    for required in [
        "\"dep:radroots_transport_reticulum\"",
        "\"dep:radroots_transport\"",
        "\"dep:radroots_outbox\"",
    ] {
        assert!(
            runtime_source.contains(required),
            "SDK runtime feature must retain Reticulum preview matrix witness `{required}`"
        );
    }
    for required in [
        "\"runtime\"",
        "\"dep:radroots_nostr\"",
        "\"radroots_nostr/client\"",
        "\"radroots_transport_nostr/client\"",
    ] {
        assert!(
            nostr_runtime_source.contains(required),
            "SDK Nostr runtime feature must retain real delivery matrix witness `{required}`"
        );
    }

    for forbidden in [
        "transport-reticulum-preview",
        "radroots_transport_reticulum/client",
        "reticulum-runtime",
        "dep:rns",
        "dep:rnsd",
        "dep:reticulum",
        "dep:pyo3",
        "package = \"reticulum\"",
        "package = \"pyo3\"",
    ] {
        assert!(
            !manifest_source.contains(forbidden),
            "SDK feature matrix must not introduce Reticulum preview alias or real runtime dependency `{forbidden}`"
        );
    }

    let xtask_check_source = read_source(
        manifest_dir
            .join("../../tools/xtask/src/check.rs")
            .as_path(),
    );
    for required in [
        "check_sdk_workspace_reticulum_dependency_boundaries",
        "check_manifest_reticulum_dependency_boundaries",
        "check_cargo_lock_reticulum_package_names",
        "check_cargo_metadata_reticulum_package_names",
        "package = \"reticulum\"",
        "package = \"pyo3\"",
        "dto_bindgen_backend_python",
    ] {
        assert!(
            xtask_check_source.contains(required),
            "SDK xtask Reticulum release gate must retain package-graph bypass witness `{required}`"
        );
    }

    let sync_runtime_source = read_source(manifest_dir.join("src/sync_runtime.rs").as_path());
    for required in [
        "#[cfg(not(feature = \"radrootsd-proxy\"))]",
        "TransportProfile::Proxy { .. } => Err(RadrootsSdkError::ProductSyncUnsupported",
        "required_feature: \"radrootsd-proxy\"",
    ] {
        assert!(
            sync_runtime_source.contains(required),
            "SDK runtime-only push_outbox must retain proxy feature gate witness `{required}`"
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
            "{relative_path} must not use removed SDK client or transport concept `{forbidden}`"
        );
    }
}

fn sdk_root_alias_guard_files(manifest_dir: &Path) -> Vec<PathBuf> {
    let mut paths = vec![manifest_dir.join("README")];

    for relative_root in ["src", "examples", "tests"] {
        let root = manifest_dir.join(relative_root);
        if root.exists() {
            paths.extend(rust_source_files(root.as_path()));
        }
    }

    paths.retain(|path| {
        path.file_name().and_then(|file_name| file_name.to_str()) != Some("source_boundary.rs")
    });
    paths.sort();
    paths
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

fn removed_reticulum_preview_endpoint_lines(source: &str) -> Vec<usize> {
    source
        .match_indices("reticulum:preview")
        .filter_map(|(index, _)| {
            let after = source[index + "reticulum:preview".len()..].chars().next();
            (after != Some('-')).then(|| line_number(source, index))
        })
        .collect()
}

fn root_trade_alias_findings(relative_path: &str, source: &str) -> Vec<String> {
    let mut findings = Vec::new();

    for alias in FORBIDDEN_SDK_ROOT_TRADE_ALIAS_NAMES {
        for (index, _) in source.match_indices(alias) {
            let before = source[..index].chars().next_back();
            let after_index = index + alias.len();
            let after = source[after_index..].chars().next();

            if before.is_some_and(is_rust_identifier_character)
                || after.is_some_and(is_rust_identifier_character)
            {
                continue;
            }

            if source[after_index..]
                .chars()
                .find(|character| !character.is_whitespace())
                != Some('(')
            {
                continue;
            }

            let prefix = source[..index].trim_end();
            let reason = if prefix.ends_with('.') {
                "root client method call"
            } else if prefix.ends_with("::") {
                "root client associated function call"
            } else if prefix_ends_with_keyword(prefix, "fn") {
                "function or method definition"
            } else {
                continue;
            };

            findings.push(format!(
                "{relative_path}:{} uses `{alias}` as a {reason}",
                line_number(source, index)
            ));
        }
    }

    findings
}

fn prefix_ends_with_keyword(source: &str, keyword: &str) -> bool {
    source.ends_with(keyword)
        && source[..source.len() - keyword.len()]
            .chars()
            .next_back()
            .is_none_or(|character| !is_rust_identifier_character(character))
}

fn line_number(source: &str, index: usize) -> usize {
    source[..index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn relative_manifest_path(manifest_dir: &Path, path: &Path) -> String {
    path.strip_prefix(manifest_dir)
        .unwrap_or(path)
        .display()
        .to_string()
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

fn export_block<'source>(source: &'source str, marker: &str) -> &'source str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("failed to find export block marker `{marker}`"));
    let source_after_start = &source[start..];
    let end = source_after_start
        .find("\n};")
        .unwrap_or_else(|| panic!("failed to find end of export block `{marker}`"));
    &source_after_start[..end]
}

fn impl_block<'source>(source: &'source str, marker: &str) -> &'source str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("failed to find impl block marker `{marker}`"));
    let source_after_start = &source[start..];
    let end = source_after_start
        .find("\n}\n\n#[cfg(")
        .unwrap_or(source_after_start.len());
    &source_after_start[..end]
}

fn struct_block<'source>(source: &'source str, marker: &str) -> &'source str {
    let signature = format!("pub struct {marker}");
    let start = source
        .find(signature.as_str())
        .unwrap_or_else(|| panic!("failed to find struct block marker `{marker}`"));
    let source_after_start = &source[start..];
    let end = source_after_start
        .find("\n}\n\n#[cfg(")
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
