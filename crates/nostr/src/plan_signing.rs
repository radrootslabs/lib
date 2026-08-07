//! Exact Nostr unsigned-event construction and completion for authored plans.

#![forbid(unsafe_code)]

use crate::{
    error::Error,
    types::{
        RadrootsNostrEvent, RadrootsNostrEventId, RadrootsNostrKind, RadrootsNostrPublicKey,
        RadrootsNostrTag, RadrootsNostrTimestamp,
    },
};
use radroots_event_codec::authoring::AuthoredEventPlan;
#[cfg(feature = "signing")]
use radroots_signing::SignRequest;

#[cfg(feature = "signing")]
use nostr::JsonUtil;
#[cfg(feature = "signing")]
use radroots_event::{SignedEvent, wire::Nip01EventWire};

pub(crate) fn unsigned_event_from_plan(
    plan: &AuthoredEventPlan,
) -> Result<nostr::UnsignedEvent, Error> {
    let kind = u16::try_from(plan.body().kind()).map_err(|_| Error::KindOutOfRange {
        kind: plan.body().kind(),
        max: u16::MAX,
    })?;
    let tags = plan
        .body()
        .tags()
        .iter()
        .cloned()
        .map(RadrootsNostrTag::parse)
        .collect::<Result<alloc::vec::Vec<_>, _>>()
        .map_err(|_| Error::TagConversion)?;
    let expected_event_id = RadrootsNostrEventId::from_slice(plan.expected_event_id().as_bytes())
        .map_err(|_| Error::EventConversion {
        field: "expected_event_id",
    })?;
    let expected_public_key = RadrootsNostrPublicKey::from_slice(plan.author().as_bytes())
        .map_err(|_| Error::EventConversion { field: "author" })?;

    let unsigned = nostr::UnsignedEvent {
        id: Some(expected_event_id),
        pubkey: expected_public_key,
        created_at: RadrootsNostrTimestamp::from_secs(plan.created_at()),
        kind: RadrootsNostrKind::Custom(kind),
        tags: nostr::Tags::from_list(tags),
        content: plan.body().content().into(),
    };
    validate_unsigned_event_matches_plan(&unsigned, plan)?;
    Ok(unsigned)
}

#[cfg(feature = "signing")]
fn unsigned_event_from_request(request: &SignRequest) -> Result<nostr::UnsignedEvent, Error> {
    let kind = u16::try_from(request.kind()).map_err(|_| Error::KindOutOfRange {
        kind: request.kind(),
        max: u16::MAX,
    })?;
    let tags = request
        .tags()
        .iter()
        .cloned()
        .map(RadrootsNostrTag::parse)
        .collect::<Result<alloc::vec::Vec<_>, _>>()
        .map_err(|_| Error::TagConversion)?;
    let expected_event_id =
        RadrootsNostrEventId::from_slice(request.expected_event_id().as_bytes()).map_err(|_| {
            Error::EventConversion {
                field: "expected_event_id",
            }
        })?;
    let expected_public_key =
        RadrootsNostrPublicKey::from_slice(request.expected_author().as_bytes())
            .map_err(|_| Error::EventConversion { field: "author" })?;
    Ok(nostr::UnsignedEvent {
        id: Some(expected_event_id),
        pubkey: expected_public_key,
        created_at: RadrootsNostrTimestamp::from_secs(request.created_at()),
        kind: RadrootsNostrKind::Custom(kind),
        tags: nostr::Tags::from_list(tags),
        content: request.content().into(),
    })
}

#[cfg(feature = "signing")]
pub(crate) fn sign_request(
    keys: &nostr::Keys,
    request: &SignRequest,
) -> Result<SignedEvent, Error> {
    let event = unsigned_event_from_request(request)?.sign_with_keys(keys)?;
    let raw_json = event.as_json();
    let wire = Nip01EventWire::parse_json(&raw_json)?;
    SignedEvent::from_wire_verified_id(wire, raw_json).map_err(Into::into)
}

pub(crate) fn validate_signed_event_matches_plan(
    event: &RadrootsNostrEvent,
    plan: &AuthoredEventPlan,
) -> Result<(), Error> {
    validate_exact_fields(
        event.pubkey,
        event.created_at,
        event.kind,
        &event.tags,
        &event.content,
        plan,
    )
}

fn validate_unsigned_event_matches_plan(
    event: &nostr::UnsignedEvent,
    plan: &AuthoredEventPlan,
) -> Result<(), Error> {
    if event.id.map(|id| id.to_bytes()) != Some(*plan.expected_event_id().as_bytes()) {
        return Err(Error::ExternalSigningPlanMismatch {
            field: "expected_event_id",
        });
    }
    validate_exact_fields(
        event.pubkey,
        event.created_at,
        event.kind,
        &event.tags,
        &event.content,
        plan,
    )
}

fn validate_exact_fields(
    author: RadrootsNostrPublicKey,
    created_at: RadrootsNostrTimestamp,
    kind: RadrootsNostrKind,
    tags: &nostr::Tags,
    content: &str,
    plan: &AuthoredEventPlan,
) -> Result<(), Error> {
    if author.to_bytes() != *plan.author().as_bytes() {
        return Err(Error::ExternalSigningPlanMismatch { field: "author" });
    }
    if created_at.as_secs() != plan.created_at() {
        return Err(Error::ExternalSigningPlanMismatch {
            field: "created_at",
        });
    }
    if u32::from(kind.as_u16()) != plan.body().kind() {
        return Err(Error::ExternalSigningPlanMismatch { field: "kind" });
    }
    if tags.len() != plan.body().tags().len()
        || tags
            .iter()
            .zip(plan.body().tags())
            .any(|(actual, expected)| actual.as_slice() != expected)
    {
        return Err(Error::ExternalSigningPlanMismatch { field: "tags" });
    }
    if content != plan.body().content() {
        return Err(Error::ExternalSigningPlanMismatch { field: "content" });
    }
    Ok(())
}
