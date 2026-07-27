#![cfg(all(feature = "storage", feature = "runtime-tokio"))]

use core::cell::Cell;

use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsLocalEventSigner,
    RadrootsPhase1PublicationSigner, RadrootsSignerError, sign_authorized_phase1_publication,
};
use radroots_blossom::{
    RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl, RadrootsBlossomByteVerifiedDescriptor,
    RadrootsBlossomMediaType, RadrootsBlossomPublicationReadinessEvidence, RadrootsBlossomSha256,
};
use radroots_event::{
    RadrootsAuthoredImage,
    calendar::{
        RadrootsAuthoredCalendarDateEvent, RadrootsAuthoredCalendarTimeEvent, RadrootsCalendarDate,
    },
    contract::event_contract,
    draft::{RadrootsEventDraft, RadrootsSignedEvent},
    food_availability::{
        RadrootsFoodAvailabilityDetails, RadrootsFoodAvailabilityDetailsParts,
        RadrootsFoodAvailabilityImage, RadrootsFoodAvailabilityStatus, RadrootsFoodContent,
        RadrootsFoodCurrency, RadrootsFoodIdentifier, RadrootsFoodImageDimensions,
        RadrootsFoodPrice, RadrootsFoodPublishedAt, RadrootsFoodQuantity, RadrootsFoodText,
        RadrootsFoodUnit,
    },
    ids::{RadrootsEventId, RadrootsPublicKey},
    post::{
        RadrootsAuthoredAsk, RadrootsAuthoredPhotoUpdate, RadrootsAuthoredPostImage,
        RadrootsAuthoredUpdate, RadrootsPostImageDimensions,
    },
    profile::{RadrootsAuthoredProfile, RadrootsNip05Identifier},
};
use radroots_event_codec::wire::publication::{
    RadrootsPhase1MediaReadyPublicationArtifact, RadrootsPhase1PublicationArtifact,
    RadrootsPhase1PublicationDraft, RadrootsPhase1PublicationMediaReference,
    allowlist::allow_phase1_publication_artifact, bind_phase1_publication_media_readiness,
};
use radroots_nostr::prelude::{RadrootsNostrKeys, RadrootsNostrSecretKey};
use radroots_outbox::{RadrootsOutbox, RadrootsPhase1PublicationTargetPolicy};
use radroots_transport::{RadrootsTransportDeliveryTargetStatus, RadrootsTransportPayload};
use radroots_transport_nostr::{
    RadrootsMockRelayPublishAdapter, RadrootsNostrTransport, RadrootsRelayOutcome,
    phase1_publication_delivery_request, publish_claimed_phase1_publication_target_with_transport,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const SECRET_KEY: &str = "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const PUBLIC_KEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const CREATED_AT: u64 = 1_784_347_200;
const RELAY: &str = "wss://relay.example";

struct CountingPhase1Signer {
    inner: RadrootsLocalEventSigner,
    invocations: Cell<usize>,
}

impl CountingPhase1Signer {
    fn fixture() -> Self {
        let secret = RadrootsNostrSecretKey::from_hex(SECRET_KEY).unwrap();
        Self {
            inner: RadrootsLocalEventSigner::new(RadrootsNostrKeys::new(secret)).unwrap(),
            invocations: Cell::new(0),
        }
    }

    fn invocations(&self) -> usize {
        self.invocations.get()
    }
}

impl RadrootsEventSigner for CountingPhase1Signer {
    fn pubkey(&self) -> &RadrootsPublicKey {
        self.inner.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsEventDraft,
    ) -> Result<RadrootsSignedEvent, RadrootsSignerError> {
        self.inner.sign_frozen_draft(draft)
    }
}

impl RadrootsPhase1PublicationSigner for CountingPhase1Signer {
    fn sign_phase1_publication_draft(
        &self,
        draft: &RadrootsPhase1PublicationDraft,
        expected_pubkey: &RadrootsPublicKey,
        expected_event_id: &RadrootsEventId,
    ) -> Result<RadrootsSignedEvent, RadrootsSignerError> {
        self.invocations
            .set(self.invocations.get().saturating_add(1));
        self.inner
            .sign_phase1_publication_draft(draft, expected_pubkey, expected_event_id)
    }
}

#[tokio::test]
async fn outbox_publication_all_seven_leaves_reuse_exact_bytes_and_dispatch_identity() {
    let outbox = RadrootsOutbox::open_memory().await.unwrap();
    let signer = CountingPhase1Signer::fixture();
    let policy = RadrootsPhase1PublicationTargetPolicy::new([RELAY], 1).unwrap();
    let ready_artifacts = all_ready_artifacts();
    assert_eq!(
        ready_artifacts
            .iter()
            .map(|ready| ready.artifact().semantic_variant().as_str())
            .collect::<Vec<_>>(),
        [
            "profile",
            "update",
            "photo_update",
            "ask",
            "event_date",
            "event_time",
            "food_availability",
        ]
    );

    for (index, ready) in ready_artifacts.iter().enumerate() {
        let base = 1_000_i64 + i64::try_from(index).unwrap() * 1_000;
        let enqueue = outbox
            .enqueue_phase1_publication(ready, &policy, base)
            .await
            .unwrap();
        let signing_claim = outbox
            .claim_phase1_publication_for_signing(
                enqueue.record().publication_id(),
                enqueue.record().revision(),
                base + 1,
                100,
            )
            .await
            .unwrap();
        let preflight = outbox
            .preflight_phase1_publication_signing(&signing_claim, base + 2)
            .await
            .unwrap();
        let contract = event_contract(ready.artifact().event_contract_id()).unwrap();
        let actor = RadrootsActorContext::test(PUBLIC_KEY, [contract.author_role]).unwrap();
        let verified =
            sign_authorized_phase1_publication(&actor, &signer, preflight.ready_artifact())
                .unwrap();
        assert_eq!(signer.invocations(), index + 1);
        let exact_raw = verified.signed_event().raw_json().to_owned();
        let signed = outbox
            .complete_phase1_publication_signing(&preflight, &verified, base + 3)
            .await
            .unwrap();
        assert_eq!(
            signed.signed_event().unwrap().signed_event().raw_json(),
            exact_raw
        );

        let target = &signed.targets()[0];
        let first_claim = outbox
            .claim_phase1_publication_target(
                signed.publication_id(),
                signed.revision(),
                target.target_id(),
                target.revision(),
                base + 4,
                100,
            )
            .await
            .unwrap();
        let first_request = phase1_publication_delivery_request(&first_claim, base + 5).unwrap();
        assert_eq!(
            first_request.request_id(),
            hex::encode(first_claim.dispatch_digest())
        );
        assert_payload_exact(first_request.payload(), &exact_raw);

        let first_adapter = RadrootsMockRelayPublishAdapter::new().with_outcome(
            RELAY,
            RadrootsRelayOutcome::connection_failed("relay unavailable")
                .expect("bounded relay outcome"),
        );
        let first_transport = RadrootsNostrTransport::new(first_adapter.clone());
        let first_receipt = publish_claimed_phase1_publication_target_with_transport(
            &first_transport,
            &first_claim,
            base + 5,
        )
        .await
        .unwrap();
        assert_eq!(
            first_receipt.target_receipts()[0].status(),
            RadrootsTransportDeliveryTargetStatus::FailedRetryable
        );
        assert_eq!(
            first_adapter.captured_raw_events().as_slice(),
            core::slice::from_ref(&exact_raw)
        );

        let retryable = outbox
            .fail_phase1_target_retryable(&first_claim, base + 6, base + 7, "relay unavailable")
            .await
            .unwrap();
        let retry_target = retryable
            .targets()
            .iter()
            .find(|candidate| candidate.target_id() == first_claim.target_id())
            .unwrap();
        let retry_claim = outbox
            .claim_phase1_publication_target(
                retryable.publication_id(),
                retryable.revision(),
                retry_target.target_id(),
                retry_target.revision(),
                base + 7,
                100,
            )
            .await
            .unwrap();
        let retry_request = phase1_publication_delivery_request(&retry_claim, base + 8).unwrap();
        assert_eq!(retry_request.request_id(), first_request.request_id());
        assert_eq!(retry_claim.dispatch_digest(), first_claim.dispatch_digest());
        assert_payload_exact(retry_request.payload(), &exact_raw);

        let retry_adapter = RadrootsMockRelayPublishAdapter::new();
        let retry_transport = RadrootsNostrTransport::new(retry_adapter.clone());
        let retry_receipt = publish_claimed_phase1_publication_target_with_transport(
            &retry_transport,
            &retry_claim,
            base + 8,
        )
        .await
        .unwrap();
        assert_eq!(
            retry_receipt.target_receipts()[0].status(),
            RadrootsTransportDeliveryTargetStatus::Accepted
        );
        assert_eq!(
            retry_adapter.captured_raw_events().as_slice(),
            core::slice::from_ref(&exact_raw)
        );
        outbox
            .complete_phase1_target_accepted_observed(&retry_claim, base + 9)
            .await
            .unwrap();
        assert_eq!(signer.invocations(), index + 1, "retry must not re-sign");
    }
}

fn assert_payload_exact(payload: &RadrootsTransportPayload, expected_raw: &str) {
    let (_, raw_json) = payload
        .signed_event_json_parts()
        .expect("Phase 1 Nostr dispatch must carry signed event JSON");
    assert_eq!(raw_json.as_bytes(), expected_raw.as_bytes());
}

fn all_ready_artifacts() -> Vec<RadrootsPhase1MediaReadyPublicationArtifact> {
    all_artifacts()
        .into_iter()
        .map(|artifact| {
            let dimensions = expected_dimensions(artifact.semantic_variant().as_str());
            let evidence = artifact
                .media_references()
                .iter()
                .zip(dimensions)
                .map(|(reference, dimensions)| evidence_for_reference(reference, dimensions))
                .collect::<Vec<_>>();
            bind_phase1_publication_media_readiness(
                allow_phase1_publication_artifact(artifact).unwrap(),
                evidence,
            )
            .unwrap()
        })
        .collect()
}

fn all_artifacts() -> Vec<RadrootsPhase1PublicationArtifact> {
    let picture = authored_image(b"profile-picture", "media.example", "png", "image/png");
    let banner = authored_image(b"profile-banner", "media.example", "webp", "image/webp");
    let profile = RadrootsAuthoredProfile::new("victoria-farm")
        .unwrap()
        .with_display_name("Victoria Farm")
        .with_about("Seasonal produce from the Saanich Peninsula")
        .with_picture(picture)
        .with_banner(banner)
        .with_nip05(RadrootsNip05Identifier::parse("farm@example.com").unwrap())
        .with_bot(false);

    let post_image = authored_post_image(b"ask-and-photo");
    let post_url = post_image.url().to_string();
    let photo = RadrootsAuthoredPhotoUpdate::new(
        format!("Strawberries at the farm stand {post_url}"),
        vec![post_image.clone()],
    )
    .unwrap();
    let ask = RadrootsAuthoredAsk::new(
        format!("When will strawberries be ready? {post_url}"),
        vec![post_image],
    )
    .unwrap();

    let event_image = authored_image(b"farm-event", "events.example", "jpeg", "image/jpeg");
    let date = RadrootsAuthoredCalendarDateEvent::new(
        "farmers-market-2026",
        "Moss Street Farmers Market",
        RadrootsCalendarDate::parse("2026-07-25").unwrap(),
    )
    .unwrap()
    .with_end(RadrootsCalendarDate::parse("2026-07-26").unwrap())
    .unwrap()
    .with_description("Saturday market in Victoria")
    .unwrap()
    .with_locations(vec!["Victoria, BC".to_owned()])
    .unwrap()
    .with_image(event_image.clone())
    .unwrap();
    let time = RadrootsAuthoredCalendarTimeEvent::new(
        "farm-tour-2026",
        "Saanich Farm Tour",
        1_785_003_600,
    )
    .unwrap()
    .with_end(1_785_007_200)
    .unwrap()
    .with_start_tzid("America/Vancouver")
    .unwrap()
    .with_description("A one-hour farm tour")
    .unwrap()
    .with_image(event_image)
    .unwrap();

    vec![
        RadrootsPhase1PublicationArtifact::from_profile(&profile, CREATED_AT, PUBLIC_KEY).unwrap(),
        RadrootsPhase1PublicationArtifact::from_update(
            &RadrootsAuthoredUpdate::new("Carrots harvested today").unwrap(),
            CREATED_AT,
            PUBLIC_KEY,
        )
        .unwrap(),
        RadrootsPhase1PublicationArtifact::from_photo_update(&photo, CREATED_AT, PUBLIC_KEY)
            .unwrap(),
        RadrootsPhase1PublicationArtifact::from_ask(&ask, CREATED_AT, PUBLIC_KEY).unwrap(),
        RadrootsPhase1PublicationArtifact::from_calendar_date_event(&date, CREATED_AT, PUBLIC_KEY)
            .unwrap(),
        RadrootsPhase1PublicationArtifact::from_calendar_time_event(&time, CREATED_AT, PUBLIC_KEY)
            .unwrap(),
        RadrootsPhase1PublicationArtifact::from_food_availability(
            &food_details(),
            CREATED_AT,
            PUBLIC_KEY,
        )
        .unwrap(),
    ]
}

fn authored_post_image(bytes: &[u8]) -> RadrootsAuthoredPostImage {
    let image = authored_image(bytes, "media.example", "webp", "image/webp");
    let hash = image.descriptor().sha256();
    let fallback = RadrootsBlossomBlobUrl::parse(&format!("https://backup.example/{hash}.webp"))
        .unwrap()
        .approve()
        .unwrap();
    RadrootsAuthoredPostImage::new(
        image,
        RadrootsPostImageDimensions::new(1_200, 900).unwrap(),
        "Fresh strawberries",
    )
    .unwrap()
    .try_with_fallback(fallback)
    .unwrap()
}

fn food_details() -> RadrootsFoodAvailabilityDetails {
    let image = RadrootsFoodAvailabilityImage::new(
        authored_image(b"nantes-carrots", "food.example", "png", "image/png"),
        RadrootsFoodImageDimensions::new(1_200, 800).unwrap(),
    );
    RadrootsFoodAvailabilityDetails::new(RadrootsFoodAvailabilityDetailsParts {
        content: RadrootsFoodContent::new("Fresh Nantes carrots available this week.").unwrap(),
        identifier: RadrootsFoodIdentifier::parse("nantes-carrots").unwrap(),
        title: RadrootsFoodText::new("Nantes Carrots").unwrap(),
        summary: RadrootsFoodText::new("Fresh bunches").unwrap(),
        published_at: RadrootsFoodPublishedAt::new(CREATED_AT - 60).unwrap(),
        location: RadrootsFoodText::new("Central Saanich, BC").unwrap(),
        price: RadrootsFoodPrice::new(
            "3",
            RadrootsFoodCurrency::parse("CAD").unwrap(),
            RadrootsFoodUnit::Pound,
        )
        .unwrap(),
        quantity: Some(RadrootsFoodQuantity::new("24", RadrootsFoodUnit::Pound).unwrap()),
        status: RadrootsFoodAvailabilityStatus::Active,
        images: vec![image],
    })
    .unwrap()
}

fn authored_image(
    bytes: &[u8],
    host: &str,
    extension: &str,
    media_type: &str,
) -> RadrootsAuthoredImage {
    RadrootsAuthoredImage::try_from(verified_descriptor(bytes, host, extension, media_type))
        .unwrap()
}

fn verified_descriptor(
    bytes: &[u8],
    host: &str,
    extension: &str,
    media_type: &str,
) -> RadrootsBlossomByteVerifiedDescriptor {
    let sha256 = RadrootsBlossomSha256::digest(bytes);
    let media_type = RadrootsBlossomMediaType::parse(media_type).unwrap();
    RadrootsBlossomBlobDescriptor::new(
        RadrootsBlossomBlobUrl::parse(&format!("https://{host}/{sha256}.{extension}")).unwrap(),
        sha256,
        u64::try_from(bytes.len()).unwrap(),
        media_type.clone(),
        CREATED_AT,
    )
    .unwrap()
    .approve_reference()
    .unwrap()
    .verify_bytes(bytes, &media_type)
    .unwrap()
}

fn expected_dimensions(variant: &str) -> Vec<(u32, u32)> {
    match variant {
        "profile" => vec![(640, 640), (1_600, 600)],
        "update" => Vec::new(),
        "photo_update" | "ask" => vec![(1_200, 900), (1_200, 900)],
        "event_date" | "event_time" => vec![(640, 480)],
        "food_availability" => vec![(1_200, 800)],
        _ => panic!("unknown Phase 1 publication variant {variant}"),
    }
}

#[derive(Serialize)]
struct EvidenceDimensionsWire {
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct EvidenceWire<'a> {
    schema_version: u32,
    policy_version: u16,
    url: &'a str,
    sha256: String,
    size: u64,
    media_type: &'a str,
    raster_format: &'a str,
    dimensions: EvidenceDimensionsWire,
    bud02_status: u16,
    bud01_head_status: u16,
    bud01_get_status: u16,
    uploaded: u64,
    evidence_digest: String,
}

fn evidence_for_reference(
    reference: &RadrootsPhase1PublicationMediaReference,
    dimensions: (u32, u32),
) -> RadrootsBlossomPublicationReadinessEvidence {
    let media_type = reference.media_type().as_str();
    let (raster_format, format_code) = match media_type {
        "image/jpeg" => ("jpeg", 1),
        "image/png" => ("png", 2),
        "image/webp" => ("still_webp", 3),
        _ => panic!("unsupported test MIME {media_type}"),
    };
    let uploaded = 1_800_000_001_u64;
    let wire = EvidenceWire {
        schema_version: 1,
        policy_version: 1,
        url: reference.url().as_str(),
        sha256: reference.sha256().to_hex(),
        size: reference.size(),
        media_type,
        raster_format,
        dimensions: EvidenceDimensionsWire {
            width: dimensions.0,
            height: dimensions.1,
        },
        bud02_status: 201,
        bud01_head_status: 200,
        bud01_get_status: 200,
        uploaded,
        evidence_digest: evidence_digest(reference, format_code, dimensions, uploaded),
    };
    RadrootsBlossomPublicationReadinessEvidence::from_canonical_json(
        &serde_json::to_vec(&wire).unwrap(),
    )
    .unwrap()
}

fn evidence_digest(
    reference: &RadrootsPhase1PublicationMediaReference,
    format_code: u8,
    dimensions: (u32, u32),
    uploaded: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"radroots.blossom.publication-readiness-evidence.v1\0");
    hasher.update(1_u16.to_be_bytes());
    update_length_prefixed(&mut hasher, reference.url().as_str().as_bytes());
    hasher.update(reference.sha256().as_bytes());
    hasher.update(reference.size().to_be_bytes());
    update_length_prefixed(&mut hasher, reference.media_type().as_str().as_bytes());
    hasher.update([format_code]);
    hasher.update(dimensions.0.to_be_bytes());
    hasher.update(dimensions.1.to_be_bytes());
    hasher.update(201_u16.to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(uploaded.to_be_bytes());
    hex::encode(hasher.finalize())
}

fn update_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap().to_be_bytes());
    hasher.update(bytes);
}
