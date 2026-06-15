#[cfg(feature = "runtime")]
use crate::{
    ListingsClient, RadrootsSdkError, RadrootsSdkEventReference, RadrootsSdkLocalMutationReceipt,
    RadrootsSdkRecoveryAction, RadrootsSdkTimestamp, SdkIdempotencyKey, SdkRelayTargetPolicy,
    SdkRelayTargetSet, SdkRelayUrlPolicy,
};
#[cfg(feature = "runtime")]
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner, sign_authorized_draft};
#[cfg(feature = "runtime")]
use radroots_event_store::RadrootsEventIngest;
#[cfg(feature = "runtime")]
use radroots_events::{
    RadrootsNostrEvent,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent},
    ids::{RadrootsEventId, RadrootsListingAddress},
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
const LISTING_PUBLISH_OPERATION_KIND: &str = "listing.publish.v1";

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
pub struct ListingPreparePublishRequest {
    pub actor: RadrootsActorContext,
    pub document: RadrootsListingDraftDocumentV1,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl ListingPreparePublishRequest {
    pub fn new(actor: RadrootsActorContext, listing: RadrootsListing) -> Self {
        Self {
            actor,
            document: RadrootsListingDraftDocumentV1::new(listing),
            created_at: None,
        }
    }

    pub fn from_document(
        actor: RadrootsActorContext,
        document: RadrootsListingDraftDocumentV1,
    ) -> Self {
        Self {
            actor,
            document,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
pub struct ListingEnqueuePublishRequest {
    pub actor: RadrootsActorContext,
    pub document: RadrootsListingDraftDocumentV1,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl ListingEnqueuePublishRequest {
    pub fn new(
        actor: RadrootsActorContext,
        listing: RadrootsListing,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self::from_document(
            actor,
            RadrootsListingDraftDocumentV1::new(listing),
            target_relays,
        )
    }

    pub fn from_document(
        actor: RadrootsActorContext,
        document: RadrootsListingDraftDocumentV1,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            actor,
            document,
            target_relays,
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn try_with_target_relays<I, S>(
        mut self,
        target_relays: I,
        policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.target_relays = SdkRelayTargetPolicy::try_explicit(target_relays, policy)?;
        Ok(self)
    }

    pub fn with_idempotency_key(mut self, idempotency_key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn try_with_idempotency_key(
        mut self,
        idempotency_key: impl AsRef<str>,
    ) -> Result<Self, RadrootsSdkError> {
        self.idempotency_key = Some(SdkIdempotencyKey::new(idempotency_key)?);
        Ok(self)
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListingPublishPlan {
    pub public_listing_addr: RadrootsListingAddress,
    pub draft_listing_addr: RadrootsListingAddress,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
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
        request: ListingPreparePublishRequest,
    ) -> Result<ListingPublishPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        listing_publish_plan(&request.actor, request.document, created_at)
    }

    pub async fn enqueue_publish<S>(
        &self,
        request: ListingEnqueuePublishRequest,
        signer: &S,
    ) -> Result<ListingEnqueueReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
        let target_relays = self.resolved_target_relays(&request.target_relays)?;
        let idempotency_key = request.idempotency_key.clone();
        let created_at = self.resolved_created_at(request.created_at)?;
        let plan = listing_publish_plan(&request.actor, request.document, created_at)?;
        let signed_event = sign_authorized_draft(&request.actor, signer, &plan.frozen_draft)?;
        let idempotency_key = match idempotency_key {
            Some(idempotency_key) => idempotency_key,
            None => SdkIdempotencyKey::derive(
                LISTING_PUBLISH_OPERATION_KIND,
                plan.frozen_draft.expected_event_id.as_str(),
                plan.frozen_draft.expected_pubkey.as_str(),
                target_relays.relays(),
            )?,
        };
        let observed_at_ms = i64::from(plan.frozen_draft.created_at) * 1_000;
        let event = event_from_signed(&signed_event);
        let ingest = RadrootsEventIngest::new(event, observed_at_ms)
            .with_raw_json(signed_event.raw_json.clone());
        let ingest_receipt = self.sdk._event_store.ingest_event(ingest).await?;
        let outbox_input = signed_outbox_input(
            &plan,
            signed_event.clone(),
            target_relays.into_vec(),
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
            listing_address: plan.public_listing_addr.into_string(),
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

    fn resolved_target_relays(
        &self,
        target_relays: &SdkRelayTargetPolicy,
    ) -> Result<SdkRelayTargetSet, RadrootsSdkError> {
        match target_relays {
            SdkRelayTargetPolicy::Explicit(target_relays) => Ok(target_relays.clone()),
            SdkRelayTargetPolicy::UseConfiguredRelays => {
                SdkRelayTargetSet::from_normalized_relays(self.sdk.relay_urls().to_vec())
            }
        }
    }

    fn resolved_created_at(
        &self,
        created_at: Option<RadrootsSdkTimestamp>,
    ) -> Result<RadrootsSdkTimestamp, RadrootsSdkError> {
        match created_at {
            Some(created_at) => Ok(created_at),
            None => self.sdk.now(),
        }
    }
}

#[cfg(feature = "runtime")]
fn canonical_listing_draft(
    actor: &RadrootsActorContext,
    document: RadrootsListingDraftDocumentV1,
) -> Result<RadrootsCanonicalListingDraft, RadrootsSdkError> {
    canonicalize_listing_draft(actor, document).map_err(Into::into)
}

#[cfg(feature = "runtime")]
fn listing_publish_plan(
    actor: &RadrootsActorContext,
    document: RadrootsListingDraftDocumentV1,
    created_at: RadrootsSdkTimestamp,
) -> Result<ListingPublishPlan, RadrootsSdkError> {
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let canonical = canonical_listing_draft(actor, document)?;
    let public_listing_addr = canonical.public_listing_addr().clone();
    let draft_listing_addr = canonical.draft_listing_addr().clone();
    let mutation = RadrootsListingMutation::publish(canonical);
    let frozen_draft = build_listing_mutation_draft(&mutation, created_at_nostr)?;
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("listing publish draft produced invalid event id: {error}"),
        })?;
    Ok(ListingPublishPlan {
        public_listing_addr,
        draft_listing_addr,
        expected_event_id,
        frozen_draft,
        created_at,
    })
}

#[cfg(feature = "runtime")]
fn signed_outbox_input(
    plan: &ListingPublishPlan,
    signed_event: RadrootsSignedNostrEvent,
    target_relays: Vec<String>,
    idempotency_key: SdkIdempotencyKey,
    event_store_inserted: bool,
    observed_at_ms: i64,
) -> RadrootsOutboxSignedOperationInput {
    RadrootsOutboxSignedOperationInput::new(
        LISTING_PUBLISH_OPERATION_KIND,
        plan.frozen_draft.clone(),
        signed_event,
        target_relays,
        event_store_inserted,
        observed_at_ms,
        observed_at_ms,
    )
    .with_idempotency_key(idempotency_key.into_string())
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
