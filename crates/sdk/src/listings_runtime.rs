#[cfg(feature = "signer-adapters")]
use crate::workflow_runtime::enqueue_configured_signed_workflow;
#[cfg(feature = "runtime")]
use crate::{
    ListingsClient, NostrRelayUrlPolicy, RadrootsSdkError, RadrootsSdkTimestamp,
    SatisfactionPolicy, SdkIdempotencyKey, TargetPolicy,
    workflow_runtime::{SdkWorkflowEnqueueRequest, enqueue_signed_workflow},
};
#[cfg(feature = "runtime")]
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner};
#[cfg(feature = "runtime")]
use radroots_event::{
    draft::RadrootsEventDraft,
    ids::{RadrootsClassifiedListingAddress, RadrootsEventId},
    kinds::KIND_CLASSIFIED_LISTING,
    operational_listing::RadrootsOperationalListing,
};
#[cfg(feature = "runtime")]
use radroots_outbox::RadrootsOutboxEnqueueStatus;
#[cfg(feature = "runtime")]
use radroots_trade::operational_listing::{
    RadrootsOperationalListingCanonicalEdit, RadrootsOperationalListingEditDocumentV1,
    RadrootsOperationalListingMutation, build_operational_listing_mutation_draft,
    canonicalize_operational_listing_edit,
};
#[cfg(feature = "runtime")]
pub const LISTING_PUBLISH_OPERATION_KIND: &str = "listing.publish.v1";

#[cfg(feature = "runtime")]
const OPERATIONAL_LISTING_PUBLISHED_CONTRACT_ID: &str = "radroots.operational_listing.published.v1";

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct ListingPreparePublishRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub document: RadrootsOperationalListingEditDocumentV1,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl ListingPreparePublishRequest {
    pub fn new(actor: RadrootsActorContext, listing: RadrootsOperationalListing) -> Self {
        Self {
            actor,
            document: RadrootsOperationalListingEditDocumentV1::new(listing),
            created_at: None,
        }
    }

    pub fn from_document(
        actor: RadrootsActorContext,
        document: RadrootsOperationalListingEditDocumentV1,
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
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct ListingEnqueuePublishRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub document: RadrootsOperationalListingEditDocumentV1,
    pub target_policy: TargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl ListingEnqueuePublishRequest {
    pub fn new(
        actor: RadrootsActorContext,
        listing: RadrootsOperationalListing,
        target_policy: TargetPolicy,
    ) -> Self {
        Self::from_document(
            actor,
            RadrootsOperationalListingEditDocumentV1::new(listing),
            target_policy,
        )
    }

    pub fn from_document(
        actor: RadrootsActorContext,
        document: RadrootsOperationalListingEditDocumentV1,
        target_policy: TargetPolicy,
    ) -> Self {
        Self {
            actor,
            document,
            target_policy,
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn try_with_nostr_targets<I, S>(
        mut self,
        target_policy: I,
        policy: NostrRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.target_policy = TargetPolicy::try_nostr_relays(target_policy, policy)?;
        Ok(self)
    }

    pub fn with_idempotency_key(mut self, idempotency_key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(idempotency_key);
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
    public_listing_addr: RadrootsClassifiedListingAddress,
    expected_event_id: RadrootsEventId,
    frozen_draft: RadrootsEventDraft,
    created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "runtime")]
impl ListingPublishPlan {
    pub fn public_listing_addr(&self) -> &RadrootsClassifiedListingAddress {
        &self.public_listing_addr
    }

    pub fn expected_event_id(&self) -> &RadrootsEventId {
        &self.expected_event_id
    }

    pub fn frozen_draft(&self) -> &RadrootsEventDraft {
        &self.frozen_draft
    }

    pub fn created_at(&self) -> RadrootsSdkTimestamp {
        self.created_at
    }
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
    pub public_listing_addr: RadrootsClassifiedListingAddress,
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

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_publish(
        &self,
        request: ListingEnqueuePublishRequest,
    ) -> Result<ListingEnqueueReceipt, RadrootsSdkError> {
        let ListingEnqueuePublishRequest {
            actor,
            document,
            target_policy,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = ListingPreparePublishRequest {
            actor: actor.clone(),
            document,
            created_at,
        };
        let plan = self.prepare_publish(prepare_request)?;
        self.enqueue_prepared_publish(&actor, plan, target_policy, idempotency_key)
            .await
    }

    pub async fn enqueue_publish_with_explicit_signer(
        &self,
        request: ListingEnqueuePublishRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<ListingEnqueueReceipt, RadrootsSdkError> {
        let ListingEnqueuePublishRequest {
            actor,
            document,
            target_policy,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = ListingPreparePublishRequest {
            actor: actor.clone(),
            document,
            created_at,
        };
        let plan = self.prepare_publish(prepare_request)?;
        self.enqueue_prepared_publish_with_explicit_signer(
            &actor,
            plan,
            target_policy,
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_prepared_publish(
        &self,
        actor: &RadrootsActorContext,
        plan: ListingPublishPlan,
        target_policy: TargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<ListingEnqueueReceipt, RadrootsSdkError> {
        let metadata = validate_listing_publish_plan(&plan)?;
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: LISTING_PUBLISH_OPERATION_KIND,
                actor,
                frozen_draft: &plan.frozen_draft,
                target_policy,
                satisfaction_policy: SatisfactionPolicy::AllAccepted,
                idempotency_key,
            },
        )
        .await?;
        Ok(listing_enqueue_receipt(metadata, enqueue))
    }

    pub async fn enqueue_prepared_publish_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: ListingPublishPlan,
        target_policy: TargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<ListingEnqueueReceipt, RadrootsSdkError> {
        let metadata = validate_listing_publish_plan(&plan)?;
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: LISTING_PUBLISH_OPERATION_KIND,
                actor,
                frozen_draft: &plan.frozen_draft,
                target_policy,
                satisfaction_policy: SatisfactionPolicy::AllAccepted,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(listing_enqueue_receipt(metadata, enqueue))
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
fn listing_enqueue_receipt(
    metadata: ValidatedListingPublishPlanMetadata,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> ListingEnqueueReceipt {
    ListingEnqueueReceipt {
        public_listing_addr: metadata.public_listing_addr,
        expected_event_id: metadata.expected_event_id,
        signed_event_id: enqueue.signed_event_id,
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state: enqueue.state.into(),
        idempotency_digest_prefix: Some(enqueue.idempotency_digest_prefix),
    }
}

#[cfg(feature = "runtime")]
fn canonical_listing_edit(
    actor: &RadrootsActorContext,
    document: RadrootsOperationalListingEditDocumentV1,
) -> Result<RadrootsOperationalListingCanonicalEdit, RadrootsSdkError> {
    canonicalize_operational_listing_edit(actor, document).map_err(Into::into)
}

#[cfg(feature = "runtime")]
struct ValidatedListingPublishPlanMetadata {
    public_listing_addr: RadrootsClassifiedListingAddress,
    expected_event_id: RadrootsEventId,
}

#[cfg(feature = "runtime")]
fn validate_listing_publish_plan(
    plan: &ListingPublishPlan,
) -> Result<ValidatedListingPublishPlanMetadata, RadrootsSdkError> {
    let invalid = |reason: &str| RadrootsSdkError::InvalidRequest {
        message: format!("invalid prepared listing publish plan: {reason}"),
    };
    plan.frozen_draft
        .validate_for_signing()
        .map_err(|_| invalid("frozen draft is invalid"))?;
    if plan.frozen_draft.contract_id() != OPERATIONAL_LISTING_PUBLISHED_CONTRACT_ID
        || plan.frozen_draft.kind_u32() != KIND_CLASSIFIED_LISTING
    {
        return Err(invalid(
            "contract or kind does not match Operational Listing publish",
        ));
    }

    let expected_event_id = RadrootsEventId::parse(plan.frozen_draft.expected_event_id_str())
        .expect("validated frozen draft has a typed event ID");
    if plan.expected_event_id != expected_event_id {
        return Err(invalid("expected event ID does not match frozen draft"));
    }
    if plan.created_at.unix_seconds() != plan.frozen_draft.created_at_u64() {
        return Err(invalid("created-at timestamp does not match frozen draft"));
    }

    let tags = plan.frozen_draft.tags_as_vec();
    let mut d_tags = tags
        .iter()
        .filter(|tag| tag.first().is_some_and(|value| value == "d"));
    let d_tag = d_tags
        .next()
        .and_then(|tag| tag.get(1))
        .ok_or_else(|| invalid("frozen draft is missing its listing identifier"))?;
    if d_tags.next().is_some() {
        return Err(invalid(
            "frozen draft contains duplicate listing identifiers",
        ));
    }
    let public_listing_addr = RadrootsClassifiedListingAddress::parse(format!(
        "{KIND_CLASSIFIED_LISTING}:{}:{d_tag}",
        plan.frozen_draft.expected_pubkey_str()
    ))
    .map_err(|_| invalid("frozen draft listing address is invalid"))?;
    if plan.public_listing_addr != public_listing_addr {
        return Err(invalid("listing address does not match frozen draft"));
    }
    Ok(ValidatedListingPublishPlanMetadata {
        public_listing_addr,
        expected_event_id,
    })
}

#[cfg(feature = "runtime")]
fn listing_publish_plan(
    actor: &RadrootsActorContext,
    document: RadrootsOperationalListingEditDocumentV1,
    created_at: RadrootsSdkTimestamp,
) -> Result<ListingPublishPlan, RadrootsSdkError> {
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let canonical = canonical_listing_edit(actor, document)?;
    let public_listing_addr = canonical.public_listing_addr().clone();
    let mutation = RadrootsOperationalListingMutation::publish(canonical);
    let frozen_draft =
        build_operational_listing_mutation_draft(&mutation, u64::from(created_at_nostr))?;
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id_str())
        .expect("frozen listing edit produces a valid event id");
    Ok(ListingPublishPlan {
        public_listing_addr,
        expected_event_id,
        frozen_draft,
        created_at,
    })
}

#[cfg(all(test, feature = "runtime", feature = "signer-adapters"))]
#[path = "../tests/unit/listings_runtime_tests.rs"]
mod tests;
