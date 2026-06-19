#[cfg(feature = "runtime")]
use crate::{
    ListingsClient, RadrootsSdkError, RadrootsSdkTimestamp, SdkIdempotencyKey,
    SdkRelayTargetPolicy, SdkRelayUrlPolicy,
    actor_json::SdkActorContextJson,
    workflow_runtime::{SdkWorkflowEnqueueRequest, enqueue_signed_workflow},
};
#[cfg(feature = "runtime")]
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner};
#[cfg(feature = "runtime")]
use radroots_events::{
    draft::RadrootsFrozenEventDraft,
    ids::{RadrootsEventId, RadrootsListingAddress},
    listing::RadrootsListing,
};
#[cfg(feature = "runtime")]
use radroots_outbox::RadrootsOutboxEnqueueStatus;
#[cfg(feature = "runtime")]
use radroots_trade::listing::{
    RadrootsCanonicalListingDraft, RadrootsListingDraftDocumentV1, RadrootsListingMutation,
    build_listing_mutation_draft, canonicalize_listing_draft,
};
#[cfg(feature = "runtime")]
use serde::ser::SerializeStruct;

#[cfg(feature = "runtime")]
pub const LISTING_PUBLISH_OPERATION_KIND: &str = "listing.publish.v1";

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ListingPreparePublishRequest {
    pub actor: RadrootsActorContext,
    pub document: RadrootsListingDraftDocumentV1,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for ListingPreparePublishRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("ListingPreparePublishRequest", 3)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("document", &self.document)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[non_exhaustive]
pub struct ListingEnqueuePublishRequest {
    pub actor: RadrootsActorContext,
    pub document: RadrootsListingDraftDocumentV1,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for ListingEnqueuePublishRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("ListingEnqueuePublishRequest", 5)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("document", &self.document)?;
        state.serialize_field("target_relays", &self.target_relays)?;
        state.serialize_field("idempotency_key", &self.idempotency_key)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ListingPublishPlan {
    pub public_listing_addr: RadrootsListingAddress,
    pub draft_listing_addr: RadrootsListingAddress,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkMutationState {
    StoredAndQueued,
    AlreadyQueued,
}

#[cfg(feature = "runtime")]
impl From<RadrootsOutboxEnqueueStatus> for SdkMutationState {
    fn from(value: RadrootsOutboxEnqueueStatus) -> Self {
        match value {
            RadrootsOutboxEnqueueStatus::Inserted => Self::StoredAndQueued,
            RadrootsOutboxEnqueueStatus::Existing => Self::AlreadyQueued,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ListingEnqueueReceipt {
    pub public_listing_addr: RadrootsListingAddress,
    pub draft_listing_addr: RadrootsListingAddress,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub state: SdkMutationState,
    pub idempotency_digest_prefix: Option<String>,
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
        let ListingEnqueuePublishRequest {
            actor,
            document,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = ListingPreparePublishRequest {
            actor: actor.clone(),
            document,
            created_at,
        };
        let plan = self.prepare_publish(prepare_request)?;
        self.enqueue_prepared_publish(&actor, plan, target_relays, idempotency_key, signer)
            .await
    }

    pub async fn enqueue_prepared_publish<S>(
        &self,
        actor: &RadrootsActorContext,
        plan: ListingPublishPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &S,
    ) -> Result<ListingEnqueueReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: LISTING_PUBLISH_OPERATION_KIND,
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(ListingEnqueueReceipt {
            public_listing_addr: plan.public_listing_addr,
            draft_listing_addr: plan.draft_listing_addr,
            expected_event_id: plan.expected_event_id,
            signed_event_id: enqueue.signed_event_id,
            local_event_seq: enqueue.local_event_seq,
            outbox_operation_id: enqueue.outbox_operation_id,
            outbox_event_id: enqueue.outbox_event_id,
            state: enqueue.state.into(),
            idempotency_digest_prefix: Some(enqueue.idempotency_digest_prefix),
        })
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
