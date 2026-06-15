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
    ListingPublishRequest, RadrootsSdk, RadrootsSdkError, RadrootsSdkRecoveryAction,
    RadrootsSdkTimestamp,
};

const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const LISTING_A_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const LISTING_B_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const LISTING_C_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAw";
const LISTING_D_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABA";
const LISTING_E_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABQ";
const RELAY: &str = "wss://relay.example.com";

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
    let request = ListingPublishRequest::new(listing(LISTING_A_D_TAG, "Coffee"));
    let prepared = sdk
        .listings()
        .prepare_publish(&actor(), request)
        .expect("prepared");

    assert_eq!(prepared.draft.kind, KIND_LISTING);
    assert_eq!(prepared.created_at.unix_seconds(), 1_700_000_000);

    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    assert!(
        event_store
            .get_event(prepared.draft.expected_event_id.as_str())
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
async fn enqueue_publish_stores_event_and_queues_signed_outbox_without_publish() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = ListingPublishRequest::new(listing(LISTING_B_D_TAG, "Coffee"))
        .with_idempotency_key("idem-b");
    let prepared = sdk
        .listings()
        .prepare_publish(&actor(), request.clone())
        .expect("prepared");
    let receipt = sdk
        .listings()
        .enqueue_publish(&actor(), &FixtureSigner::new(SELLER), request)
        .await
        .expect("enqueue");

    assert_eq!(
        receipt.local.event.event_id,
        prepared.draft.expected_event_id
    );
    assert_eq!(receipt.local.event.kind, KIND_LISTING);
    assert!(receipt.local.stored);
    assert!(receipt.local.queued);
    assert!(receipt.local.idempotency_key_digest_prefix.is_some());

    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    assert!(
        event_store
            .get_event(receipt.local.event.event_id.as_str())
            .await
            .expect("event lookup")
            .is_some()
    );

    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let outbox_event = outbox
        .get_event(receipt.local.outbox_event_id.expect("outbox event"))
        .await
        .expect("outbox event")
        .expect("outbox event");
    assert_eq!(outbox_event.state, RadrootsOutboxEventState::Signed);
    assert!(outbox_event.signed_event.is_some());
}

#[tokio::test]
async fn enqueue_publish_returns_sanitized_signer_errors() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = ListingPublishRequest::new(listing(LISTING_C_D_TAG, "Coffee"));
    let error = sdk
        .listings()
        .enqueue_publish(&actor(), &FixtureSigner::new(OTHER), request)
        .await
        .expect_err("signer error");
    let message = error.to_string();

    assert!(matches!(error, RadrootsSdkError::Authority { .. }));
    assert!(!message.contains("raw"));
    assert!(!message.contains("ffff"));
}

#[tokio::test]
async fn enqueue_publish_reports_partial_local_mutation_after_outbox_conflict() {
    let (_tempdir, sdk) = directory_sdk().await;
    let first = ListingPublishRequest::new(listing(LISTING_D_D_TAG, "Coffee"))
        .with_idempotency_key("idem-d");
    sdk.listings()
        .enqueue_publish(&actor(), &FixtureSigner::new(SELLER), first)
        .await
        .expect("first enqueue");

    let second = ListingPublishRequest::new(listing(LISTING_E_D_TAG, "Changed"))
        .with_idempotency_key("idem-d");
    let error = sdk
        .listings()
        .enqueue_publish(&actor(), &FixtureSigner::new(SELLER), second)
        .await
        .expect_err("partial");

    assert!(matches!(
        error,
        RadrootsSdkError::PartialLocalMutation(ref partial)
            if partial.stored
                && !partial.queued
                && partial.recovery == RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey
    ));
    assert!(!error.to_string().contains("idem-d"));
}
