use super::*;
use crate::{RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_event::{
    contract::RadrootsActorRole,
    farm::RadrootsFarmRef,
    ids::{RadrootsDTag, RadrootsInventoryBinId},
    listing::{RadrootsListingBin, RadrootsListingProduct},
    resource_area::RadrootsResourceAreaRef,
};

use crate::fixture_signer::FixtureSigner;
use crate::serializer_failure::assert_struct_serialize_error_paths;

const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const LISTING_A_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const LISTING_B_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const LISTING_C_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAw";
const RELAY_A: &str = "wss://relay-a.radroots.test";
const RELAY_B: &str = "wss://relay-b.radroots.test";

fn actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER, [RadrootsActorRole::Seller]).expect("actor")
}

fn listing(d_tag: &str, title: &str) -> RadrootsListing {
    listing_for_seller(SELLER, d_tag, title)
}

fn listing_for_seller(seller: &str, d_tag: &str, title: &str) -> RadrootsListing {
    RadrootsListing {
        d_tag: RadrootsDTag::parse(d_tag).expect("d tag"),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: seller.to_owned(),
            d_tag: FARM_D_TAG.to_owned(),
        },
        product: RadrootsListingProduct {
            key: "lettuce".to_owned(),
            title: title.to_owned(),
            category: "greens".to_owned(),
            summary: Some("Fresh greens".to_owned()),
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
                RadrootsCoreDecimal::from(12u32),
                RadrootsCoreUnit::Each,
            ),
            price_per_canonical_unit: RadrootsCoreQuantityPrice {
                amount: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(4u32),
                    RadrootsCoreCurrency::USD,
                ),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1u32),
                    RadrootsCoreUnit::Each,
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

#[test]
fn listing_runtime_request_builders_and_serializers_cover_success_paths() {
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_321);
    let prepare =
        ListingPreparePublishRequest::new(actor(), listing(LISTING_A_D_TAG, "Serialized Greens"))
            .with_created_at(created_at);
    assert_struct_serialize_error_paths(&prepare, 3);
    let prepare_json = serde_json::to_value(&prepare).expect("prepare json");
    assert_eq!(prepare_json["actor"]["pubkey"], SELLER);
    assert_eq!(prepare_json["created_at"], 1_700_000_321);

    let enqueue = ListingEnqueuePublishRequest::from_document(
        actor(),
        RadrootsListingEditDocumentV1::new(listing(LISTING_B_D_TAG, "Queued Greens")),
        TargetPolicy::default_profile(),
    )
    .try_with_nostr_targets([RELAY_A, RELAY_B], NostrRelayUrlPolicy::Public)
    .expect("relay targets")
    .with_idempotency_key(
        SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-00000000022f").expect("key"),
    )
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&enqueue, 5);
    let enqueue_json = serde_json::to_value(&enqueue).expect("enqueue json");
    assert_eq!(enqueue_json["target_policy"]["kind"], "explicit");
    assert_eq!(enqueue_json["created_at"], 1_700_000_321);
    assert!(
        !enqueue_json
            .to_string()
            .contains("01890f0e-6c00-7000-8000-00000000022f")
    );

    let try_key = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_C_D_TAG, "Try Key Greens"),
        TargetPolicy::default_profile(),
    )
    .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000230")
    .expect("try key");
    assert_eq!(
        serde_json::to_value(&try_key).expect("try key json")["idempotency_key"]["len"],
        "01890f0e-6c00-7000-8000-000000000230".len()
    );
}

#[test]
fn listing_request_builders_reject_invalid_options_and_timestamp_bounds() {
    let invalid_key = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_A_D_TAG, "Invalid Key Greens"),
        TargetPolicy::default_profile(),
    )
    .try_with_idempotency_key("");
    assert!(invalid_key.is_err());

    let timestamp_error = listing_publish_plan(
        &actor(),
        RadrootsListingEditDocumentV1::new(listing(LISTING_B_D_TAG, "Future Greens")),
        RadrootsSdkTimestamp::from_unix_seconds(u64::MAX),
    )
    .expect_err("timestamp error");
    assert!(matches!(
        timestamp_error,
        RadrootsSdkError::TimestampOutOfRange { .. }
    ));

    let mut invalid_resource_area_listing =
        listing(LISTING_C_D_TAG, "Invalid Resource Area Greens");
    invalid_resource_area_listing.resource_area = Some(RadrootsResourceAreaRef {
        pubkey: SELLER.to_owned(),
        d_tag: "bad d tag".to_owned(),
    });
    let mutation_error = listing_publish_plan(
        &actor(),
        RadrootsListingEditDocumentV1::new(invalid_resource_area_listing),
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
    )
    .expect_err("mutation error");
    assert!(matches!(
        mutation_error,
        RadrootsSdkError::ListingMutation { message } if message.contains("failed to encode")
    ));
}

#[tokio::test]
async fn listing_client_prepare_resolves_default_and_explicit_created_at() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_400))
        .build()
        .await
        .expect("sdk");
    let default_plan = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor(),
            listing(LISTING_A_D_TAG, "Default Clock Greens"),
        ))
        .expect("default plan");
    assert_eq!(
        default_plan.created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_400)
    );

    let explicit_plan = sdk
        .listings()
        .prepare_publish(
            ListingPreparePublishRequest::new(
                actor(),
                listing(LISTING_B_D_TAG, "Explicit Clock Greens"),
            )
            .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_401)),
        )
        .expect("explicit plan");
    assert_eq!(
        explicit_plan.created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_401)
    );
}

#[tokio::test]
async fn listing_client_prepare_reports_clock_errors() {
    let sdk = crate::RadrootsClient::builder()
        .clock(crate::RadrootsSdkClock::BeforeUnixEpoch)
        .build()
        .await
        .expect("sdk");
    let error = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor(),
            listing(LISTING_A_D_TAG, "Clock Error Greens"),
        ))
        .expect_err("clock error");
    assert!(matches!(error, RadrootsSdkError::ClockBeforeUnixEpoch));
}

#[tokio::test]
async fn listing_enqueue_publish_reports_prepare_errors_before_signing() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .build()
        .await
        .expect("sdk");
    let error = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(
            ListingEnqueuePublishRequest::new(
                actor(),
                listing(LISTING_A_D_TAG, "Future Enqueue Greens"),
                TargetPolicy::try_nostr_relays([RELAY_A], NostrRelayUrlPolicy::Public)
                    .expect("transport targets"),
            )
            .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX)),
            &FixtureSigner::new(SELLER),
        )
        .await
        .expect_err("prepare error");
    assert!(matches!(
        error,
        RadrootsSdkError::TimestampOutOfRange { .. }
    ));
}

#[tokio::test]
async fn listing_client_enqueue_methods_cover_source_attached_workflow_paths() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .build()
        .await
        .expect("sdk");
    let signer = FixtureSigner::new(SELLER);
    let actor = actor();
    let receipt = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(
            ListingEnqueuePublishRequest::new(
                actor.clone(),
                listing(LISTING_A_D_TAG, "Enqueued Greens"),
                TargetPolicy::try_nostr_relays([RELAY_A], NostrRelayUrlPolicy::Public)
                    .expect("transport targets"),
            )
            .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000231")
            .expect("idempotency"),
            &signer,
        )
        .await
        .expect("enqueue listing");
    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);

    let plan = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::from_document(
            actor.clone(),
            RadrootsListingEditDocumentV1::new(listing(LISTING_B_D_TAG, "Prepared Greens")),
        ))
        .expect("prepared listing");
    let prepared = sdk
        .listings()
        .enqueue_prepared_publish_with_explicit_signer(
            &actor,
            plan,
            TargetPolicy::try_nostr_relays([RELAY_B], NostrRelayUrlPolicy::Public)
                .expect("prepared transport targets"),
            Some(
                SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-000000000232")
                    .expect("prepared idempotency"),
            ),
            &signer,
        )
        .await
        .expect("enqueue prepared listing");
    assert_eq!(prepared.signed_event_id, prepared.expected_event_id);
    assert_eq!(prepared.local_event_seq, 2);
}

#[tokio::test]
async fn listing_configured_local_signer_enqueues_publish_without_explicit_signer() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::from_event_signer(FixtureSigner::new(SELLER))
                .expect("signer"),
        ))
        .build()
        .await
        .expect("sdk");
    let actor = RadrootsActorContext::test(SELLER, [RadrootsActorRole::Seller]).expect("actor");

    let receipt = sdk
        .listings()
        .enqueue_publish(
            ListingEnqueuePublishRequest::new(
                actor,
                listing_for_seller(SELLER, LISTING_C_D_TAG, "Configured Greens"),
                TargetPolicy::try_nostr_relays([RELAY_A], NostrRelayUrlPolicy::Public)
                    .expect("transport targets"),
            )
            .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000232")
            .expect("idempotency"),
        )
        .await
        .expect("enqueue listing");

    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
}

#[tokio::test]
async fn listing_configured_enqueue_reports_missing_signer_after_prepare() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .build()
        .await
        .expect("sdk");
    let actor = actor();
    assert!(matches!(
        sdk.listings()
            .enqueue_publish(
                ListingEnqueuePublishRequest::new(
                    actor.clone(),
                    listing(LISTING_A_D_TAG, "Configured Prepare Error Greens"),
                    TargetPolicy::try_nostr_relays([RELAY_A], NostrRelayUrlPolicy::Public)
                        .expect("transport targets"),
                )
                .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX)),
            )
            .await,
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    let plan = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor.clone(),
            listing(LISTING_A_D_TAG, "Missing Signer Greens"),
        ))
        .expect("plan");

    assert!(matches!(
        sdk.listings()
            .enqueue_prepared_publish(
                &actor,
                plan,
                TargetPolicy::try_nostr_relays([RELAY_A], NostrRelayUrlPolicy::Public)
                    .expect("transport targets"),
                None,
            )
            .await,
        Err(RadrootsSdkError::SignerUnavailable { .. })
    ));
}
