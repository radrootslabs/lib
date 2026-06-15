#[cfg(feature = "runtime")]
use crate::{
    ListingsClient, RadrootsSdkError, RadrootsSdkEventReference, RadrootsSdkLocalMutationReceipt,
    RadrootsSdkRecoveryAction, RadrootsSdkTimestamp,
};
#[cfg(feature = "runtime")]
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner, sign_authorized_draft};
#[cfg(feature = "runtime")]
use radroots_event_store::RadrootsEventIngest;
#[cfg(feature = "runtime")]
use radroots_events::{
    RadrootsNostrEvent,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent},
    listing::RadrootsListing,
};
#[cfg(feature = "runtime")]
use radroots_outbox::RadrootsOutboxSignedOperationInput;
#[cfg(feature = "runtime")]
use radroots_trade::listing::{
    RadrootsCanonicalListingDraft, RadrootsListingDraftDocumentV1, RadrootsListingMutation,
    build_listing_mutation_draft, canonicalize_listing_draft,
};

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
pub struct ListingPublishRequest {
    pub listing: RadrootsListing,
    pub target_relays: Option<Vec<String>>,
    pub idempotency_key: Option<String>,
}

#[cfg(feature = "runtime")]
impl ListingPublishRequest {
    pub fn new(listing: RadrootsListing) -> Self {
        Self {
            listing,
            target_relays: None,
            idempotency_key: None,
        }
    }

    pub fn with_target_relays<I, S>(mut self, target_relays: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.target_relays = Some(target_relays.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedListingPublish {
    pub draft: RadrootsFrozenEventDraft,
    pub listing_address: String,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListingEnqueueReceipt {
    pub listing_address: String,
    pub local: RadrootsSdkLocalMutationReceipt,
}

#[cfg(feature = "runtime")]
impl<'sdk> ListingsClient<'sdk> {
    pub fn prepare_publish(
        &self,
        actor: &RadrootsActorContext,
        request: ListingPublishRequest,
    ) -> Result<PreparedListingPublish, RadrootsSdkError> {
        let created_at = self.sdk.now()?;
        let created_at_nostr = created_at.try_into_nostr_created_at()?;
        let canonical = canonical_listing_draft(actor, request.listing)?;
        let mutation = RadrootsListingMutation::publish(canonical);
        let listing_address = mutation.listing_addr()?.as_str().to_owned();
        let draft = build_listing_mutation_draft(&mutation, created_at_nostr)?;
        Ok(PreparedListingPublish {
            draft,
            listing_address,
            created_at,
        })
    }

    pub async fn enqueue_publish<S>(
        &self,
        actor: &RadrootsActorContext,
        signer: &S,
        request: ListingPublishRequest,
    ) -> Result<ListingEnqueueReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
        let target_relays = self.resolved_target_relays(&request);
        if target_relays.is_empty() {
            return Err(RadrootsSdkError::Outbox {
                message: "listing enqueue requires at least one target relay".to_owned(),
            });
        }
        let idempotency_key = request.idempotency_key.clone();
        let prepared = self.prepare_publish(actor, request)?;
        let signed_event = sign_authorized_draft(actor, signer, &prepared.draft)?;
        let observed_at_ms = i64::from(prepared.draft.created_at) * 1_000;
        let event = event_from_signed(&signed_event);
        let ingest = RadrootsEventIngest::new(event, observed_at_ms)
            .with_raw_json(signed_event.raw_json.clone());
        let ingest_receipt = self.sdk._event_store.ingest_event(ingest).await?;
        let outbox_input = signed_outbox_input(
            &prepared,
            signed_event.clone(),
            target_relays,
            idempotency_key,
            ingest_receipt.inserted,
            observed_at_ms,
        );
        let outbox_receipt = self
            .sdk
            ._outbox
            .enqueue_signed_operation(outbox_input)
            .await
            .map_err(|_| {
                RadrootsSdkError::partial_local_mutation(
                    true,
                    false,
                    RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey,
                )
            })?;
        Ok(ListingEnqueueReceipt {
            listing_address: prepared.listing_address,
            local: RadrootsSdkLocalMutationReceipt {
                event: RadrootsSdkEventReference {
                    event_id: signed_event.id,
                    pubkey: signed_event.pubkey,
                    kind: signed_event.kind,
                    created_at: signed_event.created_at,
                },
                stored: true,
                queued: true,
                outbox_event_id: Some(outbox_receipt.outbox_event_id),
                idempotency_key_digest_prefix: Some(
                    outbox_receipt.idempotency_digest.chars().take(12).collect(),
                ),
            },
        })
    }

    fn resolved_target_relays(&self, request: &ListingPublishRequest) -> Vec<String> {
        request
            .target_relays
            .clone()
            .unwrap_or_else(|| self.sdk.relay_urls().to_vec())
    }
}

#[cfg(feature = "runtime")]
fn canonical_listing_draft(
    actor: &RadrootsActorContext,
    listing: RadrootsListing,
) -> Result<RadrootsCanonicalListingDraft, RadrootsSdkError> {
    let document = RadrootsListingDraftDocumentV1::new(listing);
    canonicalize_listing_draft(actor, document).map_err(Into::into)
}

#[cfg(feature = "runtime")]
fn signed_outbox_input(
    prepared: &PreparedListingPublish,
    signed_event: RadrootsSignedNostrEvent,
    target_relays: Vec<String>,
    idempotency_key: Option<String>,
    event_store_inserted: bool,
    observed_at_ms: i64,
) -> RadrootsOutboxSignedOperationInput {
    let input = RadrootsOutboxSignedOperationInput::new(
        "listing.publish.v1",
        prepared.draft.clone(),
        signed_event,
        target_relays,
        event_store_inserted,
        observed_at_ms,
        observed_at_ms,
    );
    match idempotency_key {
        Some(idempotency_key) => input.with_idempotency_key(idempotency_key),
        None => input,
    }
}

#[cfg(feature = "runtime")]
fn event_from_signed(signed_event: &RadrootsSignedNostrEvent) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: signed_event.id.clone(),
        author: signed_event.pubkey.clone(),
        created_at: signed_event.created_at,
        kind: signed_event.kind,
        tags: signed_event.tags.clone(),
        content: signed_event.content.clone(),
        sig: signed_event.sig.clone(),
    }
}
