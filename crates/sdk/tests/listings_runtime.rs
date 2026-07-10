#![cfg(feature = "runtime")]

use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_event_store::RadrootsEventStore;
use radroots_events::{
    contract::RadrootsActorRole,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent, RadrootsSignedNostrEventParts},
    farm::RadrootsFarmRef,
    ids::{RadrootsDTag, RadrootsInventoryBinId},
    kinds::KIND_LISTING,
    listing::{RadrootsListing, RadrootsListingBin, RadrootsListingProduct},
};
use radroots_outbox::{
    RadrootsOutbox, RadrootsOutboxDeliveryPlanStatus, RadrootsOutboxDeliveryTargetStatus,
    RadrootsOutboxEventState,
};
use radroots_sdk::{
    HybridProfile, LISTING_PUBLISH_OPERATION_KIND, ListingEnqueuePublishRequest,
    ListingPreparePublishRequest, NostrProfile, NostrRelayUrlPolicy, PushOutboxEventState,
    PushOutboxRequest, PushOutboxTargetOutcomeKind, RadrootsClient, RadrootsSdkError,
    RadrootsSdkRecoveryAction, RadrootsSdkTimestamp, ReticulumPreviewProfile, SdkIdempotencyKey,
    SdkMutationState, TargetPolicy, TargetSet, TransportProfile,
};
use radroots_trade::listing::RadrootsListingDraftDocumentV1;
use radroots_transport_nostr::RadrootsMockRelayPublishAdapter;

#[path = "support/serializer_failure.rs"]
mod serializer_failure;

use serializer_failure::assert_struct_serialize_error_paths;

const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const LISTING_A_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const LISTING_B_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const LISTING_C_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAw";
const LISTING_D_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABA";
const LISTING_E_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABQ";
const LISTING_F_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABg";
const LISTING_G_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABw";
const LISTING_H_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAACA";
const LISTING_I_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAACQ";
const LISTING_J_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAACg";
const LISTING_K_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAACw";
const RELAY: &str = "wss://relay.example.com";
const RELAY_B: &str = "wss://relay-b.example.com";

#[derive(Clone)]
struct FixtureSigner {
    identity: RadrootsSignerIdentity,
}

impl FixtureSigner {
    fn new(pubkey: &str) -> Self {
        Self {
            identity: RadrootsSignerIdentity::new(pubkey).expect("identity"),
        }
    }
}

impl RadrootsEventSigner for FixtureSigner {
    fn pubkey(&self) -> &radroots_events::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
        if self.pubkey().as_str() != draft.expected_pubkey.as_str() {
            return Err(RadrootsSignerError::SigningFailed {
                message: "wrong fixture signer".to_owned(),
            });
        }
        let sig = "f".repeat(128);
        let raw_json = serde_json::json!({
            "id": draft.expected_event_id,
            "pubkey": self.pubkey().as_str(),
            "created_at": draft.created_at,
            "kind": draft.kind,
            "tags": draft.tags,
            "content": draft.content,
            "sig": sig,
        })
        .to_string();
        RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: draft.expected_event_id.clone(),
            pubkey: self.pubkey().as_str().to_owned(),
            created_at: draft.created_at,
            kind: draft.kind,
            tags: draft.tags.clone(),
            content: draft.content.clone(),
            sig,
            raw_json,
        })
        .map_err(|error| RadrootsSignerError::SigningFailed {
            message: error.to_string(),
        })
    }
}

fn actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER, [RadrootsActorRole::Seller]).expect("actor")
}

fn non_seller_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER, [RadrootsActorRole::Buyer]).expect("actor")
}

fn listing(d_tag: &str, title: &str) -> RadrootsListing {
    RadrootsListing {
        d_tag: RadrootsDTag::parse(d_tag).expect("d tag"),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: SELLER.to_owned(),
            d_tag: FARM_D_TAG.to_owned(),
        },
        product: RadrootsListingProduct {
            key: "coffee".to_owned(),
            title: title.to_owned(),
            category: "coffee".to_owned(),
            summary: Some("Single origin coffee".to_owned()),
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
        bins: vec![RadrootsListingBin {
            bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
            quantity: RadrootsCoreQuantity::new(
                RadrootsCoreDecimal::from(1000u32),
                RadrootsCoreUnit::MassG,
            ),
            price_per_canonical_unit: RadrootsCoreQuantityPrice {
                amount: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(20u32),
                    RadrootsCoreCurrency::USD,
                ),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1u32),
                    RadrootsCoreUnit::MassG,
                ),
            },
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        }],
        resource_area: None,
        plot: None,
        discounts: None,
        inventory_available: None,
        availability: None,
        delivery_method: None,
        location: None,
        images: None,
    }
}

async fn directory_sdk() -> (tempfile::TempDir, RadrootsClient) {
    directory_sdk_with_relays(&[RELAY]).await
}

async fn directory_sdk_with_relays(relays: &[&str]) -> (tempfile::TempDir, RadrootsClient) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let mut builder = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000));
    if !relays.is_empty() {
        builder = builder.transport_profile(TransportProfile::nostr(
            NostrProfile::new(relays.iter().copied(), NostrRelayUrlPolicy::Public)
                .expect("Nostr profile"),
        ));
    }
    let sdk = builder.build().await.expect("sdk");
    (tempdir, sdk)
}

async fn hybrid_directory_sdk() -> (tempfile::TempDir, RadrootsClient) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .transport_profile(TransportProfile::hybrid(HybridProfile::new(
            NostrProfile::new([RELAY], NostrRelayUrlPolicy::Public).expect("Nostr profile"),
            ReticulumPreviewProfile::preview_unavailable(),
        )))
        .build()
        .await
        .expect("sdk");
    (tempdir, sdk)
}

#[tokio::test]
async fn prepare_publish_is_side_effect_free() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = ListingPreparePublishRequest::new(actor(), listing(LISTING_A_D_TAG, "Coffee"));
    let prepared = sdk.listings().prepare_publish(request).expect("prepared");

    assert_eq!(prepared.frozen_draft.kind, KIND_LISTING);
    assert_eq!(prepared.created_at.unix_seconds(), 1_700_000_000);
    assert_eq!(
        prepared.expected_event_id,
        prepared.frozen_draft.expected_event_id
    );
    assert_eq!(
        prepared.public_listing_addr.as_str(),
        format!("{KIND_LISTING}:{SELLER}:{LISTING_A_D_TAG}")
    );

    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    assert!(
        event_store
            .get_event(prepared.expected_event_id.as_str())
            .await
            .expect("event lookup")
            .is_none()
    );
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    assert!(
        outbox
            .claim_next_ready_event("worker", "claim", 2_000, 1_700_000_000_000)
            .await
            .expect("claim")
            .is_none()
    );
}

#[tokio::test]
async fn prepare_publish_rejects_non_seller_actor() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request =
        ListingPreparePublishRequest::new(non_seller_actor(), listing(LISTING_B_D_TAG, "Coffee"));

    let error = sdk
        .listings()
        .prepare_publish(request)
        .expect_err("non seller");

    assert!(matches!(error, RadrootsSdkError::UnauthorizedActor { .. }));
}

#[tokio::test]
async fn enqueue_publish_stores_event_and_queues_signed_outbox_without_publish() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_B_D_TAG, "Coffee"),
        TargetPolicy::default_profile(),
    )
    .try_with_idempotency_key("idem-b")
    .expect("idempotency key");
    let prepared = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor(),
            listing(LISTING_B_D_TAG, "Coffee"),
        ))
        .expect("prepared");
    let receipt = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(request, &FixtureSigner::new(SELLER))
        .await
        .expect("enqueue");

    assert_eq!(receipt.expected_event_id, prepared.expected_event_id);
    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.public_listing_addr, prepared.public_listing_addr);
    assert_eq!(receipt.draft_listing_addr, prepared.draft_listing_addr);
    assert_eq!(receipt.local_event_seq, 1);
    assert_eq!(receipt.outbox_operation_id, 1);
    assert_eq!(receipt.outbox_event_id, 1);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
    assert!(receipt.idempotency_digest_prefix.is_some());

    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    assert!(
        event_store
            .get_event(receipt.signed_event_id.as_str())
            .await
            .expect("event lookup")
            .is_some()
    );

    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let outbox_event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("outbox event")
        .expect("outbox event");
    assert_eq!(outbox_event.state, RadrootsOutboxEventState::Signed);
    assert!(outbox_event.signed_event.is_some());
}

#[tokio::test]
async fn enqueue_publish_default_profile_rejects_empty_transport_targets() {
    let (_tempdir, sdk) = directory_sdk_with_relays(&[]).await;
    let request = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_A_D_TAG, "Coffee"),
        TargetPolicy::default_profile(),
    );

    let error = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(request, &FixtureSigner::new(SELLER))
        .await
        .expect_err("empty transport profile");

    assert!(matches!(
        error,
        RadrootsSdkError::EmptyTransportTargets { operation }
            if operation == "publish transport profile"
    ));
}

#[tokio::test]
async fn prepare_then_enqueue_prepared_uses_same_event_id() {
    let (_tempdir, sdk) = directory_sdk().await;
    let actor = actor();
    let prepared = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor.clone(),
            listing(LISTING_G_D_TAG, "Coffee"),
        ))
        .expect("prepared");
    let receipt = sdk
        .listings()
        .enqueue_prepared_publish_with_explicit_signer(
            &actor,
            prepared.clone(),
            TargetPolicy::default_profile(),
            None,
            &FixtureSigner::new(SELLER),
        )
        .await
        .expect("prepared enqueue");

    assert_eq!(receipt.expected_event_id, prepared.expected_event_id);
    assert_eq!(receipt.signed_event_id, prepared.expected_event_id);

    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    assert!(
        event_store
            .get_event(prepared.expected_event_id.as_str())
            .await
            .expect("event lookup")
            .is_some()
    );

    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let outbox_event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("outbox event")
        .expect("outbox event");
    assert_eq!(outbox_event.event_id, prepared.expected_event_id.as_str());
}

#[tokio::test]
async fn enqueue_receipt_debug_omits_signed_event_payload_material() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_A_D_TAG, "Coffee"),
        TargetPolicy::default_profile(),
    )
    .try_with_idempotency_key("debug-secret-idempotency")
    .expect("idempotency key");
    let receipt = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(request, &FixtureSigner::new(SELLER))
        .await
        .expect("enqueue");
    let debug = format!("{receipt:?}");

    assert!(debug.contains("ListingEnqueueReceipt"));
    assert!(debug.contains("StoredAndQueued"));
    assert!(!debug.contains("debug-secret-idempotency"));
    assert!(!debug.contains("raw_json"));
    assert!(!debug.contains("\"tags\""));
    assert!(!debug.contains("\"content\""));
    assert!(!debug.contains(&"f".repeat(128)));
}

#[test]
fn mutation_state_debug_uses_product_state_names() {
    assert_eq!(
        format!("{:?}", SdkMutationState::StoredAndQueued),
        "StoredAndQueued"
    );
    assert_eq!(
        format!("{:?}", SdkMutationState::AlreadyQueued),
        "AlreadyQueued"
    );
}

#[tokio::test]
async fn listing_runtime_dtos_serialize_deterministically() {
    let (_tempdir, sdk) = directory_sdk().await;
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_123);
    let prepare_request = ListingPreparePublishRequest::from_document(
        actor(),
        RadrootsListingDraftDocumentV1::new(listing(LISTING_A_D_TAG, "Serialized Coffee")),
    )
    .with_created_at(created_at);
    let prepare_json = serde_json::to_value(&prepare_request).expect("prepare request json");
    assert_struct_serialize_error_paths(&prepare_request, 3);

    assert_eq!(prepare_json["actor"]["pubkey"], SELLER);
    assert_eq!(
        prepare_json["actor"]["roles"],
        serde_json::json!(["seller"])
    );
    assert_eq!(prepare_json["actor"]["source"], "test");
    assert_eq!(prepare_json["created_at"], 1_700_000_123);
    assert_eq!(
        prepare_json["document"]["listing"]["product"]["title"],
        "Serialized Coffee"
    );

    let enqueue_request = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_B_D_TAG, "Queued Coffee"),
        TargetPolicy::default_profile(),
    )
    .try_with_nostr_targets([RELAY, RELAY_B], NostrRelayUrlPolicy::Public)
    .expect("relay targets")
    .with_idempotency_key(SdkIdempotencyKey::new("serialized-idempotency").expect("idempotency"))
    .with_created_at(created_at);
    let enqueue_json = serde_json::to_value(&enqueue_request).expect("enqueue request json");
    assert_struct_serialize_error_paths(&enqueue_request, 5);

    assert_eq!(enqueue_json["target_policy"]["kind"], "explicit");
    assert_eq!(
        enqueue_json["target_policy"]["targets"],
        serde_json::json!([
            {
                "kind": "nostr",
                "uri": RELAY,
                "scope": null,
                "label": null,
                "fingerprint": "a1997ec4596596af6ffc65e6a30ab7cffa53ea71f524c1c86d64018b96d130af"
            },
            {
                "kind": "nostr",
                "uri": RELAY_B,
                "scope": null,
                "label": null,
                "fingerprint": "5136077cfe7eddcbfaddc5d7bf1f42cdbb8191f3691b86ccc3a81047851cef05"
            }
        ])
    );
    assert_eq!(
        enqueue_json["target_policy"]["canonical_targets"],
        serde_json::json!([
            "5136077cfe7eddcbfaddc5d7bf1f42cdbb8191f3691b86ccc3a81047851cef05",
            "a1997ec4596596af6ffc65e6a30ab7cffa53ea71f524c1c86d64018b96d130af"
        ])
    );
    assert_eq!(
        enqueue_json["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 22 })
    );
    assert!(!enqueue_json.to_string().contains("serialized-idempotency"));

    let try_key_request = ListingEnqueuePublishRequest::from_document(
        actor(),
        RadrootsListingDraftDocumentV1::new(listing(LISTING_C_D_TAG, "Queued Coffee")),
        TargetPolicy::default_profile(),
    )
    .try_with_idempotency_key("listing-serialized-try-key")
    .expect("try idempotency key");
    assert_eq!(
        serde_json::to_value(&try_key_request).expect("try key request json")["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 26 })
    );

    let receipt = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(enqueue_request, &FixtureSigner::new(SELLER))
        .await
        .expect("enqueue");
    let receipt_json = serde_json::to_value(&receipt).expect("receipt json");

    assert_eq!(receipt_json["state"], "stored_and_queued");
    assert_eq!(receipt_json["local_event_seq"], 1);
    assert!(receipt_json["idempotency_digest_prefix"].is_string());
}

#[tokio::test]
async fn enqueue_publish_convenience_matches_prepare_plus_enqueue_prepared() {
    let (_prepared_tempdir, prepared_sdk) = directory_sdk().await;
    let prepared_actor = actor();
    let prepared_plan = prepared_sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            prepared_actor.clone(),
            listing(LISTING_H_D_TAG, "Coffee"),
        ))
        .expect("prepared plan");
    let prepared_receipt = prepared_sdk
        .listings()
        .enqueue_prepared_publish_with_explicit_signer(
            &prepared_actor,
            prepared_plan,
            TargetPolicy::default_profile(),
            None,
            &FixtureSigner::new(SELLER),
        )
        .await
        .expect("prepared enqueue");

    let (_convenience_tempdir, convenience_sdk) = directory_sdk().await;
    let convenience_request = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_H_D_TAG, "Coffee"),
        TargetPolicy::default_profile(),
    );
    let convenience_receipt = convenience_sdk
        .listings()
        .enqueue_publish_with_explicit_signer(convenience_request, &FixtureSigner::new(SELLER))
        .await
        .expect("convenience enqueue");

    assert_eq!(convenience_receipt, prepared_receipt);
}

#[tokio::test]
async fn enqueue_prepared_publish_returns_structured_actor_errors() {
    let (_tempdir, sdk) = directory_sdk().await;
    let prepared = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor(),
            listing(LISTING_I_D_TAG, "Coffee"),
        ))
        .expect("prepared");
    let error = sdk
        .listings()
        .enqueue_prepared_publish_with_explicit_signer(
            &non_seller_actor(),
            prepared,
            TargetPolicy::default_profile(),
            None,
            &FixtureSigner::new(SELLER),
        )
        .await
        .expect_err("actor error");

    assert!(matches!(error, RadrootsSdkError::UnauthorizedActor { .. }));
}

#[tokio::test]
async fn enqueue_prepared_publish_returns_sanitized_signer_errors() {
    let (_tempdir, sdk) = directory_sdk().await;
    let actor = actor();
    let prepared = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor.clone(),
            listing(LISTING_J_D_TAG, "Coffee"),
        ))
        .expect("prepared");
    let error = sdk
        .listings()
        .enqueue_prepared_publish_with_explicit_signer(
            &actor,
            prepared,
            TargetPolicy::default_profile(),
            None,
            &FixtureSigner::new(OTHER),
        )
        .await
        .expect_err("signer error");
    let message = error.to_string();

    assert!(matches!(
        error,
        RadrootsSdkError::SignerPubkeyMismatch { .. }
    ));
    assert!(!message.contains("raw"));
    assert!(!message.contains("ffff"));
}

#[tokio::test]
async fn explicit_historical_created_at_does_not_backdate_observed_at_ms() {
    let (_tempdir, sdk) = directory_sdk().await;
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_600_000_000);
    let observed_at_ms = 1_700_000_000_000;
    let request = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_K_D_TAG, "Coffee"),
        TargetPolicy::default_profile(),
    )
    .with_created_at(created_at);

    let receipt = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(request, &FixtureSigner::new(SELLER))
        .await
        .expect("enqueue");

    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    let stored_event = event_store
        .get_event(receipt.signed_event_id.as_str())
        .await
        .expect("event lookup")
        .expect("stored event");
    assert_eq!(stored_event.created_at, 1_600_000_000);
    assert_eq!(stored_event.inserted_at_ms, observed_at_ms);
    assert_eq!(stored_event.updated_at_ms, observed_at_ms);

    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let outbox_event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("outbox event")
        .expect("outbox event");
    assert_eq!(outbox_event.draft.created_at, 1_600_000_000);
    assert_eq!(
        outbox_event.event_store_ingested_at_ms,
        Some(observed_at_ms)
    );
    assert_eq!(outbox_event.created_at_ms, observed_at_ms);
    assert_eq!(outbox_event.updated_at_ms, observed_at_ms);
}

#[tokio::test]
async fn enqueue_publish_returns_sanitized_signer_errors() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_C_D_TAG, "Coffee"),
        TargetPolicy::default_profile(),
    );
    let error = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(request, &FixtureSigner::new(OTHER))
        .await
        .expect_err("signer error");
    let message = error.to_string();

    assert!(matches!(
        error,
        RadrootsSdkError::SignerPubkeyMismatch { .. }
    ));
    assert!(!message.contains("raw"));
    assert!(!message.contains("ffff"));
}

#[tokio::test]
async fn enqueue_publish_reports_preflight_idempotency_conflict_without_mutation() {
    let (_tempdir, sdk) = directory_sdk().await;
    let first = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_D_D_TAG, "Coffee"),
        TargetPolicy::default_profile(),
    )
    .try_with_idempotency_key("idem-d")
    .expect("idempotency key");
    sdk.listings()
        .enqueue_publish_with_explicit_signer(first, &FixtureSigner::new(SELLER))
        .await
        .expect("first enqueue");
    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    assert_eq!(
        event_store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        1
    );
    assert_eq!(
        outbox
            .status_summary(0)
            .await
            .expect("outbox status")
            .total_events,
        1
    );

    let second = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_E_D_TAG, "Changed"),
        TargetPolicy::default_profile(),
    )
    .try_with_idempotency_key("idem-d")
    .expect("idempotency key");
    let error = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(second, &FixtureSigner::new(SELLER))
        .await
        .expect_err("conflict");

    assert!(matches!(
        error,
        RadrootsSdkError::IdempotencyConflict { ref operation_kind, .. }
            if operation_kind == LISTING_PUBLISH_OPERATION_KIND
    ));
    assert_eq!(
        error.recovery_actions(),
        vec![RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey]
    );
    assert!(!error.to_string().contains("idem-d"));
    assert_eq!(
        event_store
            .status_summary()
            .await
            .expect("event store status after conflict")
            .total_events,
        1
    );
    assert_eq!(
        outbox
            .status_summary(0)
            .await
            .expect("outbox status after conflict")
            .total_events,
        1
    );
}

#[tokio::test]
async fn enqueue_publish_derives_order_independent_idempotency_key() {
    let (_tempdir, sdk) = directory_sdk().await;
    let first = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_F_D_TAG, "Coffee"),
        TargetPolicy::default_profile(),
    )
    .try_with_nostr_targets([RELAY_B, RELAY], NostrRelayUrlPolicy::Public)
    .expect("first transport targets");
    let second = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_F_D_TAG, "Coffee"),
        TargetPolicy::explicit(
            TargetSet::nostr_relays([RELAY, RELAY_B], NostrRelayUrlPolicy::Public)
                .expect("second transport targets"),
        ),
    );

    let first_receipt = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(first, &FixtureSigner::new(SELLER))
        .await
        .expect("first enqueue");
    let second_receipt = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(second, &FixtureSigner::new(SELLER))
        .await
        .expect("second enqueue");

    assert_eq!(
        first_receipt.outbox_event_id,
        second_receipt.outbox_event_id
    );
    assert_eq!(
        first_receipt.idempotency_digest_prefix,
        second_receipt.idempotency_digest_prefix
    );
    assert_eq!(second_receipt.state, SdkMutationState::AlreadyQueued);

    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let relay_urls = outbox
        .delivery_targets(first_receipt.outbox_event_id)
        .await
        .expect("delivery targets")
        .into_iter()
        .map(|target| target.endpoint_uri.to_string())
        .collect::<Vec<_>>();
    assert_eq!(relay_urls, vec![RELAY_B.to_owned(), RELAY.to_owned()]);
}

#[tokio::test]
async fn listing_hybrid_profile_publishes_after_nostr_success_and_retains_reticulum_preview() {
    let (_tempdir, sdk) = hybrid_directory_sdk().await;
    let enqueue_receipt = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(
            ListingEnqueuePublishRequest::new(
                actor(),
                listing(LISTING_G_D_TAG, "Hybrid Coffee"),
                TargetPolicy::default_profile(),
            ),
            &FixtureSigner::new(SELLER),
        )
        .await
        .expect("enqueue");
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let push_receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect("push");

    assert_eq!(push_receipt.attempted_events, 1);
    assert_eq!(push_receipt.published_events, 1);
    assert_eq!(push_receipt.retryable_events, 0);
    assert_eq!(push_receipt.terminal_events, 0);
    let event = &push_receipt.events[0];
    assert_eq!(event.outbox_event_id, enqueue_receipt.outbox_event_id);
    assert_eq!(event.final_state, PushOutboxEventState::Published);
    assert_eq!(event.quorum, 1);
    assert!(event.quorum_met);
    assert_eq!(event.targets.len(), 1);
    assert_eq!(event.targets[0].endpoint_uri, RELAY);
    assert_eq!(
        event.targets[0].outcome_kind,
        PushOutboxTargetOutcomeKind::Accepted
    );
    assert_eq!(adapter.captured_raw_events().len(), 1);

    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    let stored = outbox
        .get_event(enqueue_receipt.outbox_event_id)
        .await
        .expect("stored")
        .expect("stored");
    assert_eq!(stored.state, RadrootsOutboxEventState::Published);
    let plans = outbox
        .delivery_plans(enqueue_receipt.outbox_event_id)
        .await
        .expect("delivery plans");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].required_success_count, 1);
    assert_eq!(plans[0].status, RadrootsOutboxDeliveryPlanStatus::Complete);
    let targets = outbox
        .delivery_targets(enqueue_receipt.outbox_event_id)
        .await
        .expect("delivery targets");
    assert_eq!(targets.len(), 2);
    assert!(targets.iter().any(|target| {
        target.endpoint_uri.to_string() == RELAY
            && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
    }));
    assert!(targets.iter().any(|target| {
        target.endpoint_uri.to_string() == "reticulum:preview-unavailable"
            && target.status == RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
            && target.attempt_count == 0
    }));
}
