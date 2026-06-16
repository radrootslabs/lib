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
use radroots_outbox::{RadrootsOutbox, RadrootsOutboxEventState};
use radroots_sdk::{
    ListingEnqueuePublishRequest, ListingPreparePublishRequest, RadrootsSdk, RadrootsSdkError,
    RadrootsSdkPartialLocalMutationFailure, RadrootsSdkRecoveryAction, RadrootsSdkTimestamp,
    SdkMutationState, SdkRelayTargetPolicy, SdkRelayTargetSet, SdkRelayUrlPolicy,
};

const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const LISTING_A_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const LISTING_B_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const LISTING_C_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAw";
const LISTING_D_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABA";
const LISTING_E_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABQ";
const LISTING_F_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABg";
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

async fn directory_sdk() -> (tempfile::TempDir, RadrootsSdk) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsSdk::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .relay_url(RELAY)
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
        SdkRelayTargetPolicy::UseConfiguredRelays,
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
        .enqueue_publish(request, &FixtureSigner::new(SELLER))
        .await
        .expect("enqueue");

    assert_eq!(receipt.expected_event_id, prepared.expected_event_id);
    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.public_listing_addr, prepared.public_listing_addr);
    assert_eq!(receipt.draft_listing_addr, prepared.draft_listing_addr);
    assert_eq!(receipt.local_event_seq, 1);
    assert_eq!(receipt.outbox_operation_id, 1);
    assert_eq!(receipt.outbox_event_id, 1);
    assert_eq!(receipt.state, SdkMutationState::Inserted);
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
async fn enqueue_publish_returns_sanitized_signer_errors() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_C_D_TAG, "Coffee"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    );
    let error = sdk
        .listings()
        .enqueue_publish(request, &FixtureSigner::new(OTHER))
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
async fn enqueue_publish_reports_partial_local_mutation_after_outbox_conflict() {
    let (_tempdir, sdk) = directory_sdk().await;
    let first = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_D_D_TAG, "Coffee"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_idempotency_key("idem-d")
    .expect("idempotency key");
    sdk.listings()
        .enqueue_publish(first, &FixtureSigner::new(SELLER))
        .await
        .expect("first enqueue");

    let second = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_E_D_TAG, "Changed"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_idempotency_key("idem-d")
    .expect("idempotency key");
    let error = sdk
        .listings()
        .enqueue_publish(second, &FixtureSigner::new(SELLER))
        .await
        .expect_err("partial");

    assert!(matches!(
        error,
        RadrootsSdkError::PartialLocalMutation(ref partial)
            if partial.stored
                && !partial.queued
                && partial.event_id.is_some()
                && partial.operation_kind == "listing.publish.v1"
                && partial.idempotency_digest_prefix.is_some()
                && partial.failure == RadrootsSdkPartialLocalMutationFailure::OutboxIdempotencyConflict
                && partial.recovery == RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey
    ));
    assert!(!error.to_string().contains("idem-d"));
}

#[tokio::test]
async fn enqueue_publish_derives_order_independent_idempotency_key() {
    let (_tempdir, sdk) = directory_sdk().await;
    let first = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_F_D_TAG, "Coffee"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY_B, RELAY, RELAY], SdkRelayUrlPolicy::Public)
    .expect("first target relays");
    let second = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_F_D_TAG, "Coffee"),
        SdkRelayTargetPolicy::explicit(
            SdkRelayTargetSet::new([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
                .expect("second target relays"),
        ),
    );

    let first_receipt = sdk
        .listings()
        .enqueue_publish(first, &FixtureSigner::new(SELLER))
        .await
        .expect("first enqueue");
    let second_receipt = sdk
        .listings()
        .enqueue_publish(second, &FixtureSigner::new(SELLER))
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
    assert_eq!(second_receipt.state, SdkMutationState::Existing);
}
