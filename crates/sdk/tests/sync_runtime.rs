#![cfg(feature = "runtime")]

use futures::future::BoxFuture;
use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::{
    contract::RadrootsActorRole,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent, RadrootsSignedNostrEventParts},
    farm::RadrootsFarmRef,
    ids::{RadrootsDTag, RadrootsInventoryBinId},
    listing::{RadrootsListing, RadrootsListingBin, RadrootsListingProduct},
};
use radroots_outbox::{RadrootsOutbox, RadrootsOutboxEventState, RadrootsOutboxOperationInput};
use radroots_relay_transport::{
    RadrootsMockRelayPublishAdapter, RadrootsRelayOutcome, RadrootsRelayPublishAdapter,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, RadrootsRelayTransportError,
};
use radroots_sdk::{
    ListingEnqueuePublishRequest, ListingPreparePublishRequest, PUSH_OUTBOX_DEFAULT_LIMIT,
    PUSH_OUTBOX_MAX_LIMIT, PushOutboxEventState, PushOutboxRelayOutcomeKind, PushOutboxRequest,
    RadrootsSdk, RadrootsSdkError, RadrootsSdkTimestamp, SdkRelayTargetPolicy, SdkRelayUrlPolicy,
};

const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const LISTING_A_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const LISTING_B_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const LISTING_C_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAw";
const RELAY_A: &str = "wss://relay-a.example.com";
const RELAY_B: &str = "wss://relay-b.example.com";
const RELAY_C: &str = "wss://relay-c.example.com";

#[derive(Clone)]
struct FixtureSigner {
    identity: RadrootsSignerIdentity,
}

struct TransportFailurePublishAdapter;

impl RadrootsRelayPublishAdapter for TransportFailurePublishAdapter {
    fn publish<'a>(
        &'a self,
        _request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async {
            Err(RadrootsRelayTransportError::Transport(
                "adapter boundary unavailable".to_owned(),
            ))
        })
    }
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

async fn directory_sdk(relays: &[&str]) -> (tempfile::TempDir, RadrootsSdk) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let mut builder = RadrootsSdk::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000));
    for relay in relays {
        builder = builder.relay_url(*relay);
    }
    let sdk = builder.build().await.expect("sdk");
    (tempdir, sdk)
}

async fn enqueue_listing(sdk: &RadrootsSdk, d_tag: &str, title: &str, relays: &[&str]) -> i64 {
    sdk.listings()
        .enqueue_publish(
            ListingEnqueuePublishRequest::new(
                actor(),
                listing(d_tag, title),
                SdkRelayTargetPolicy::UseConfiguredRelays,
            )
            .try_with_target_relays(relays, SdkRelayUrlPolicy::Public)
            .expect("relay targets"),
            &FixtureSigner::new(SELLER),
        )
        .await
        .expect("enqueue")
        .outbox_event_id
}

#[tokio::test]
async fn push_outbox_empty_queue_returns_zero_counts() {
    let (_tempdir, sdk) = directory_sdk(&[]).await;
    let adapter = RadrootsMockRelayPublishAdapter::new();
    let request = PushOutboxRequest::new();

    assert_eq!(request.limit, PUSH_OUTBOX_DEFAULT_LIMIT);

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, request)
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 0);
    assert!(receipt.events.is_empty());
    assert!(adapter.captured_raw_events().is_empty());
}

#[cfg(not(feature = "relay-runtime"))]
#[tokio::test]
async fn product_push_outbox_without_relay_runtime_returns_structured_error() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A]).await;

    let error = sdk
        .sync()
        .push_outbox(PushOutboxRequest::new())
        .await
        .expect_err("unsupported product push");

    assert!(matches!(
        error,
        RadrootsSdkError::ProductSyncUnsupported { .. }
    ));
}

#[cfg(feature = "relay-runtime")]
#[tokio::test]
async fn product_push_outbox_empty_queue_does_not_require_builder_relays() {
    let (_tempdir, sdk) = directory_sdk(&[]).await;

    let receipt = sdk
        .sync()
        .push_outbox(PushOutboxRequest::default())
        .await
        .expect("product push");

    assert_eq!(receipt.attempted_events, 0);
    assert!(receipt.events.is_empty());
}

#[tokio::test]
async fn push_outbox_rejects_invalid_limits_before_claiming() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let zero = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(0))
        .await
        .expect_err("zero limit");
    let too_large = sdk
        .sync()
        .push_outbox_with_adapter(
            &adapter,
            PushOutboxRequest::new().with_limit(PUSH_OUTBOX_MAX_LIMIT + 1),
        )
        .await
        .expect_err("too large");

    assert!(matches!(zero, RadrootsSdkError::InvalidRequest { .. }));
    assert!(matches!(too_large, RadrootsSdkError::InvalidRequest { .. }));
    assert!(adapter.captured_raw_events().is_empty());
}

#[tokio::test]
async fn push_outbox_with_adapter_uses_queued_targets_without_builder_relays() {
    let (_tempdir, sdk) = directory_sdk(&[]).await;
    let outbox_event_id = enqueue_listing(&sdk, LISTING_A_D_TAG, "Coffee", &[RELAY_A]).await;
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 1);
    assert_eq!(receipt.published_events, 1);
    assert_eq!(receipt.retryable_events, 0);
    assert_eq!(receipt.terminal_events, 0);
    assert_eq!(receipt.events.len(), 1);
    let event = &receipt.events[0];
    assert_eq!(event.outbox_event_id, outbox_event_id);
    assert_eq!(event.final_state, PushOutboxEventState::Published);
    assert_eq!(event.attempted_count, 1);
    assert_eq!(event.accepted_count, 1);
    assert_eq!(event.retryable_count, 0);
    assert_eq!(event.terminal_count, 0);
    assert_eq!(event.quorum, 1);
    assert!(event.quorum_met);
    assert_eq!(event.relays.len(), 1);
    assert_eq!(
        event.relays[0].outcome_kind,
        PushOutboxRelayOutcomeKind::Accepted
    );
    assert_eq!(adapter.captured_raw_events().len(), 1);

    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    let stored = outbox
        .get_event(outbox_event_id)
        .await
        .expect("stored")
        .expect("stored");
    assert_eq!(stored.state, RadrootsOutboxEventState::Published);
}

#[tokio::test]
async fn push_outbox_preserves_retryable_and_terminal_relay_outcomes() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A, RELAY_B, RELAY_C]).await;
    enqueue_listing(
        &sdk,
        LISTING_B_D_TAG,
        "Coffee",
        &[RELAY_A, RELAY_B, RELAY_C],
    )
    .await;
    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(
            RELAY_A,
            RadrootsRelayOutcome::duplicate_accepted("duplicate: already accepted"),
        )
        .with_outcome(
            RELAY_B,
            RadrootsRelayOutcome::classify("auth-required: login"),
        )
        .with_outcome(
            RELAY_C,
            RadrootsRelayOutcome::classify("restricted: denied"),
        );

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 1);
    assert_eq!(receipt.published_events, 0);
    assert_eq!(receipt.retryable_events, 1);
    assert_eq!(receipt.terminal_events, 0);
    let event = &receipt.events[0];
    assert_eq!(event.final_state, PushOutboxEventState::PublishRetryable);
    assert_eq!(event.accepted_count, 1);
    assert_eq!(event.retryable_count, 1);
    assert_eq!(event.terminal_count, 1);
    assert!(!event.quorum_met);

    let relay_a = event
        .relays
        .iter()
        .find(|relay| relay.relay_url == RELAY_A)
        .expect("relay a");
    let relay_b = event
        .relays
        .iter()
        .find(|relay| relay.relay_url == RELAY_B)
        .expect("relay b");
    let relay_c = event
        .relays
        .iter()
        .find(|relay| relay.relay_url == RELAY_C)
        .expect("relay c");

    assert_eq!(
        relay_a.outcome_kind,
        PushOutboxRelayOutcomeKind::DuplicateAccepted
    );
    assert_eq!(
        relay_b.outcome_kind,
        PushOutboxRelayOutcomeKind::AuthRequired
    );
    assert_eq!(relay_c.outcome_kind, PushOutboxRelayOutcomeKind::Restricted);
    assert_eq!(relay_b.message.as_deref(), Some("auth-required: login"));
}

#[tokio::test]
async fn push_outbox_continues_after_adapter_transport_failure_and_releases_claims() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A, RELAY_B]).await;
    let first_outbox_event_id =
        enqueue_listing(&sdk, LISTING_A_D_TAG, "Coffee One", &[RELAY_A]).await;
    let second_outbox_event_id =
        enqueue_listing(&sdk, LISTING_B_D_TAG, "Coffee Two", &[RELAY_B]).await;

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(
            &TransportFailurePublishAdapter,
            PushOutboxRequest::new().with_limit(2),
        )
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 2);
    assert_eq!(receipt.published_events, 0);
    assert_eq!(receipt.retryable_events, 2);
    assert_eq!(receipt.terminal_events, 0);
    assert_eq!(
        receipt
            .events
            .iter()
            .map(|event| event.outbox_event_id)
            .collect::<Vec<_>>(),
        vec![first_outbox_event_id, second_outbox_event_id]
    );
    assert!(
        receipt
            .events
            .iter()
            .all(|event| event.final_state == PushOutboxEventState::PublishRetryable)
    );
    assert!(
        receipt
            .events
            .iter()
            .flat_map(|event| event.relays.iter())
            .all(|relay| {
                relay.attempted
                    && relay.outcome_kind == PushOutboxRelayOutcomeKind::ConnectionFailed
                    && relay.message.as_deref() == Some("adapter boundary unavailable")
            })
    );

    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    for outbox_event_id in [first_outbox_event_id, second_outbox_event_id] {
        let stored = outbox
            .get_event(outbox_event_id)
            .await
            .expect("stored")
            .expect("stored");
        assert_eq!(stored.state, RadrootsOutboxEventState::PublishRetryable);
        assert!(stored.claim_token.is_none());
    }
}

#[tokio::test]
async fn push_outbox_does_not_claim_unsigned_outbox_work() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let prepared = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor(),
            listing(LISTING_C_D_TAG, "Unsigned"),
        ))
        .expect("prepared");
    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    let unsigned = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "listing.publish.v1",
            prepared.frozen_draft,
            vec![RELAY_A.to_owned()],
            1_700_000_000_000,
        ))
        .await
        .expect("unsigned enqueue");
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 0);
    assert!(adapter.captured_raw_events().is_empty());

    let stored = outbox
        .get_event(unsigned.outbox_event_id)
        .await
        .expect("unsigned event")
        .expect("unsigned event");
    assert_eq!(stored.state, RadrootsOutboxEventState::DraftQueued);
    assert!(stored.claim_token.is_none());
}
