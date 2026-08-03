//! Canonical trade planning, commit, query, and private-evidence operations.

use std::{error, fmt};

use radroots_event::{
    EventDraft,
    contract::AuthorRole,
    trade::{TradeMutationEnvelopeV1, TradeProtocolError, canonical_trade_mutation_content},
};
use radroots_event_codec::{encode::EventEncodeError, encode::trade::trade_mutation_event_build};
use radroots_signing::Actor;
use radroots_trade::{Projection, ReductionInput, WorkflowPlan, reducer::reduce_trade_records};

/// Pure inputs for one frozen trade command.
#[derive(Clone, Debug)]
pub struct PrepareRequest {
    actor: Actor,
    mutation: TradeMutationEnvelopeV1,
}

impl PrepareRequest {
    /// Creates explicit inputs for any canonical proposal, revision, decision,
    /// cancellation, or resumable mutation.
    #[must_use]
    pub const fn new(actor: Actor, mutation: TradeMutationEnvelopeV1) -> Self {
        Self { actor, mutation }
    }
}

/// Frozen, replay-stable trade workflow plan.
#[derive(Clone, Debug)]
pub struct Plan {
    actor: Actor,
    workflow: WorkflowPlan,
    draft: EventDraft,
}

impl Plan {
    /// Returns the exact authorized actor carried into signing.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// Returns the lower-owned validated workflow and required host actions.
    pub const fn workflow(&self) -> &WorkflowPlan {
        &self.workflow
    }

    /// Returns the frozen canonical event draft.
    #[must_use]
    pub const fn draft(&self) -> &EventDraft {
        &self.draft
    }
}

/// Trade planning failure stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrepareErrorKind {
    /// The actor identity or role cannot author the supplied mutation.
    UnauthorizedActor,
    /// Event-domain canonicalization rejected the mutation.
    CanonicalMutation,
    /// The lower trade workflow rejected the canonical mutation.
    Workflow,
    /// The canonical event codec rejected the mutation.
    Encode,
    /// The canonical event draft rejected the encoded mutation.
    Draft,
}

/// One secret-safe trade planning failure retaining its lower source.
pub struct PrepareError {
    kind: PrepareErrorKind,
    source: Option<Box<dyn error::Error + Send + Sync>>,
}

impl PrepareError {
    /// Returns the stable client-level planning stage.
    #[must_use]
    pub const fn kind(&self) -> PrepareErrorKind {
        self.kind
    }

    fn unauthorized_actor() -> Self {
        Self {
            kind: PrepareErrorKind::UnauthorizedActor,
            source: None,
        }
    }

    fn canonical(source: TradeProtocolError) -> Self {
        Self::with_source(PrepareErrorKind::CanonicalMutation, source)
    }

    fn workflow(source: radroots_trade::Error) -> Self {
        Self::with_source(PrepareErrorKind::Workflow, source)
    }

    fn encode(source: EventEncodeError) -> Self {
        Self::with_source(PrepareErrorKind::Encode, source)
    }

    fn draft(source: radroots_event::draft::DraftError) -> Self {
        Self::with_source(PrepareErrorKind::Draft, source)
    }

    fn with_source(
        kind: PrepareErrorKind,
        source: impl error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Display for PrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            PrepareErrorKind::UnauthorizedActor => "trade actor is not authorized",
            PrepareErrorKind::CanonicalMutation => "trade mutation is not canonical",
            PrepareErrorKind::Workflow => "trade workflow is invalid",
            PrepareErrorKind::Encode => "trade event encoding failed",
            PrepareErrorKind::Draft => "trade event draft is invalid",
        })
    }
}

impl fmt::Debug for PrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrepareError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl error::Error for PrepareError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn error::Error + 'static))
    }
}

/// Canonicalizes, authorizes, validates, and freezes one trade command.
///
/// This operation performs no signing, persistence, private-artifact access,
/// scheduling, or delivery. Every proposal, revision, decision, cancellation,
/// and resumed command uses the same lower-owned `TradeId` and workflow law.
pub fn prepare(request: PrepareRequest) -> Result<Plan, PrepareError> {
    let canonical = canonical_trade_mutation_content(request.mutation)
        .map_err(PrepareError::canonical)?
        .envelope;
    let required_role = if canonical.author_pubkey == canonical.buyer_pubkey {
        AuthorRole::Buyer
    } else if canonical.author_pubkey == canonical.seller_pubkey {
        AuthorRole::Seller
    } else {
        return Err(PrepareError::unauthorized_actor());
    };
    if request.actor.public_key() != canonical.author_pubkey
        || !request.actor.satisfies(required_role)
    {
        return Err(PrepareError::unauthorized_actor());
    }
    let workflow = WorkflowPlan::prepare(canonical.clone()).map_err(PrepareError::workflow)?;
    let parts = trade_mutation_event_build(canonical.clone()).map_err(PrepareError::encode)?;
    let draft = EventDraft::new(
        canonical.contract_id.clone(),
        parts.kind,
        canonical.authored_at_unix_s,
        parts.tags,
        parts.content,
        canonical.author_pubkey.to_hex(),
    )
    .map_err(PrepareError::draft)?;
    Ok(Plan {
        actor: request.actor,
        workflow,
        draft,
    })
}

/// Deterministically reduces caller-supplied canonical evidence.
#[must_use]
pub fn project(input: ReductionInput) -> Projection {
    reduce_trade_records(input)
}

#[cfg(feature = "sync")]
use radroots_signing::request::CancellationPolicy;
#[cfg(feature = "sync")]
use radroots_storage::{
    event::{EventPage, EventQuery, StoredVisibleEvent},
    journal::IdempotencyKey,
    private_artifact::{PrivateArtifactId, PrivateArtifactMetadata, PrivateArtifactStage},
};
#[cfg(feature = "sync")]
use radroots_sync::{
    PushReceipt,
    policy::{Error as SyncError, SyncId},
};

/// Explicit commit inputs for one prepared trade command.
#[cfg(feature = "sync")]
#[derive(Clone, Debug)]
pub struct EnqueueRequest {
    operation_id: SyncId,
    idempotency_key: IdempotencyKey,
    plan: Plan,
    profile: crate::transport::Profile,
    cancellation: CancellationPolicy,
}

#[cfg(feature = "sync")]
impl EnqueueRequest {
    /// Creates a command request whose transport selection has no fallback.
    #[must_use]
    pub const fn new(
        operation_id: SyncId,
        idempotency_key: IdempotencyKey,
        plan: Plan,
        profile: crate::transport::Profile,
        cancellation: CancellationPolicy,
    ) -> Self {
        Self {
            operation_id,
            idempotency_key,
            plan,
            profile,
            cancellation,
        }
    }
}

/// Private-term metadata verification failure.
#[cfg(feature = "sync")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrivateTermsError {
    /// Canonical storage failed to inspect the requested metadata.
    Storage,
    /// The workflow does not require private terms.
    NotRequired,
    /// Metadata is absent, inactive, or does not match the public commitment.
    EvidenceMismatch,
}

/// Borrowed trade operations over canonical storage and sync capabilities.
#[cfg(feature = "sync")]
#[derive(Clone, Copy)]
pub struct Operations<'a> {
    storage: &'a dyn radroots_storage::Storage,
    sync: crate::sync::Operations<'a>,
}

#[cfg(feature = "sync")]
impl fmt::Debug for Operations<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Operations")
            .field("storage", &"<borrowed canonical storage>")
            .field("sync", &self.sync)
            .finish()
    }
}

#[cfg(feature = "sync")]
impl<'a> Operations<'a> {
    pub(crate) const fn new(
        storage: &'a dyn radroots_storage::Storage,
        sync: crate::sync::Operations<'a>,
    ) -> Self {
        Self { storage, sync }
    }

    /// Signs and atomically enqueues a prepared command. Reusing the same
    /// idempotency input is the canonical resume/replay operation.
    pub async fn enqueue(&self, request: EnqueueRequest) -> Result<PushReceipt, SyncError> {
        let targets = request
            .profile
            .targets()
            .cloned()
            .ok_or(SyncError::InvalidPushRequest)?;
        let satisfaction = request
            .profile
            .satisfaction()
            .cloned()
            .ok_or(SyncError::InvalidPushRequest)?;
        self.sync
            .sign_and_enqueue(radroots_sync::PushRequest::new(
                request.operation_id,
                request.idempotency_key,
                request.plan.actor,
                request.plan.draft,
                targets,
                satisfaction,
                request.cancellation,
            )?)
            .await
    }

    /// Returns one native, bounded, generation-bound page of visible evidence.
    pub async fn query_visible(
        &self,
        query: EventQuery,
    ) -> Result<EventPage<StoredVisibleEvent>, radroots_storage::Error> {
        radroots_storage::event::EventStore::query_visible(self.storage, query).await
    }

    /// Returns native private-artifact metadata without reading secret material.
    pub async fn private_artifact(
        &self,
        artifact_id: PrivateArtifactId,
    ) -> Result<Option<PrivateArtifactMetadata>, radroots_storage::Error> {
        radroots_storage::private_artifact::PrivateArtifactStore::metadata(
            self.storage,
            artifact_id,
        )
        .await
    }

    /// Verifies that canonical active metadata matches a plan's public schema
    /// and ciphertext commitment. Plaintext, ciphertext, and keys never cross
    /// the SDK boundary.
    pub async fn verify_private_terms(
        &self,
        plan: &Plan,
        artifact_id: PrivateArtifactId,
    ) -> Result<PrivateArtifactMetadata, PrivateTermsError> {
        let expected = plan
            .workflow
            .private_terms()
            .ok_or(PrivateTermsError::NotRequired)?;
        let metadata = self
            .private_artifact(artifact_id)
            .await
            .map_err(|_| PrivateTermsError::Storage)?
            .ok_or(PrivateTermsError::EvidenceMismatch)?;
        let commitment = hex_lower(metadata.commitment().as_bytes());
        if metadata.stage() != PrivateArtifactStage::Active
            || metadata.schema_id().as_str() != expected.schema_id()
            || commitment != expected.ciphertext_commitment()
        {
            return Err(PrivateTermsError::EvidenceMismatch);
        }
        Ok(metadata)
    }
}

#[cfg(feature = "sync")]
fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(test)]
mod tests {
    use radroots_event::{
        id::{ClassifiedListingAddress, DTag, EventId, InventoryBinId, MutationId, TradeId},
        trade::{
            FulfillmentProfileV1, RADROOTS_TRADE_CANCELLATION_CONTRACT_ID,
            RADROOTS_TRADE_DECISION_CONTRACT_ID, RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
            RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID,
            RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID, RADROOTS_TRADE_SCHEMA_VERSION,
            TradeCancellationProfileV1, TradeCandidateLineV1, TradeCandidateTermsV1,
            TradeDecisionV1, TradeEconomicAdjustmentV1, TradeEconomicsProfileV1,
            TradeLineTombstoneV1, TradeMutationBodyV1, TradeMutationKindV1, TradePrivateTermsRefV1,
        },
    };
    use radroots_identity::PublicKey;
    use radroots_signing::actor::ActorSource;
    use radroots_trade::workflow::WorkflowAction;

    use super::*;

    const BUYER: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const SELLER: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    fn pubkey(value: &str) -> PublicKey {
        PublicKey::from_hex(value).expect("public key")
    }

    fn actor(public_key: &str, role: AuthorRole) -> Actor {
        Actor::from_public_key_hex(public_key, ActorSource::ExplicitPublicKey, [role])
            .expect("actor")
    }

    fn mutation_id(marker: char) -> MutationId {
        MutationId::parse(std::iter::repeat_n(marker, 64).collect::<String>()).expect("mutation id")
    }

    fn candidate(suffix: &str) -> TradeCandidateTermsV1 {
        TradeCandidateTermsV1 {
            candidate_id: None,
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            base_candidate_id: None,
            supersession_intent: None,
            buyer_pubkey: pubkey(BUYER),
            seller_pubkey: pubkey(SELLER),
            farm_id: DTag::parse("farm-1").expect("farm id"),
            lines: vec![TradeCandidateLineV1 {
                line_id: DTag::parse(format!("line-{suffix}")).expect("line id"),
                listing_addr: ClassifiedListingAddress::parse(format!(
                    "30402:{SELLER}:listing-{suffix}"
                ))
                .expect("listing address"),
                listing_event_id: EventId::parse("cc".repeat(32)).expect("event id"),
                listing_snapshot_sha256: "dd".repeat(32),
                product_id: format!("carrots-{suffix}"),
                option_id: None,
                bin_id: InventoryBinId::parse(format!("bin-{suffix}")).expect("bin id"),
                quantity_mantissa: "2".into(),
                quantity_scale: 0,
                unit_code: "count".into(),
                unit_profile: "mvp-count".into(),
                unit_price_mantissa: "500".into(),
                currency_code: "USD".into(),
                line_subtotal_mantissa: "1000".into(),
                replaces_line_id: None,
            }],
            line_tombstones: Vec::<TradeLineTombstoneV1>::new(),
            economics: TradeEconomicsProfileV1 {
                profile_id: "mvp-fixed".into(),
                currency_code: "USD".into(),
                currency_exponent: 2,
                rounding_profile: "half-even".into(),
                subtotal_mantissa: "1000".into(),
                discount_total_mantissa: "0".into(),
                adjustment_total_mantissa: "0".into(),
                total_mantissa: "1000".into(),
                adjustments: Vec::<TradeEconomicAdjustmentV1>::new(),
            },
            fulfillment: FulfillmentProfileV1 {
                profile_id: "market-pickup".into(),
                method: "pickup".into(),
                starts_at_unix_s: 1_800_000_000,
                ends_at_unix_s: 1_800_003_600,
                timezone: "America/New_York".into(),
                utc_offset_seconds: -18_000,
                fold: 0,
                location_class: "farmstand".into(),
                requires_private_terms: true,
            },
            cancellation: TradeCancellationProfileV1 {
                profile_id: "buyer-pre-agreement".into(),
                buyer_pre_agreement: true,
                post_agreement_cutoff_unix_s: None,
            },
            private_terms: Some(TradePrivateTermsRefV1 {
                artifact_id: "artifact-1".into(),
                schema_id: "radroots.private.fulfillment.v1".into(),
                ciphertext_commitment: "ee".repeat(32),
                required_acknowledgement: true,
            }),
            proposal_expires_at_unix_s: 1_800_010_000,
        }
    }

    fn envelope(contract_id: &str, body: TradeMutationBodyV1) -> TradeMutationEnvelopeV1 {
        let initial = body.mutation_kind() == TradeMutationKindV1::Proposal;
        TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: contract_id.into(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: TradeId::parse("11".repeat(16)).expect("trade id"),
            root_mutation_id: (!initial).then(|| mutation_id('1')),
            buyer_pubkey: pubkey(BUYER),
            seller_pubkey: pubkey(SELLER),
            farm_id: DTag::parse("farm-1").expect("farm id"),
            parent_mutation_ids: if initial {
                vec![]
            } else {
                vec![mutation_id('1')]
            },
            author_pubkey: pubkey(BUYER),
            counterparty_pubkey: pubkey(SELLER),
            authored_at_unix_s: 1_800_000_000,
            body,
        }
    }

    fn all_commands() -> Vec<TradeMutationEnvelopeV1> {
        let proposal = canonical_trade_mutation_content(envelope(
            RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
            TradeMutationBodyV1::Proposal {
                candidate: candidate("1"),
            },
        ))
        .expect("proposal")
        .envelope;
        let proposal_id = proposal.mutation_id.expect("proposal id");
        let candidate_id = match &proposal.body {
            TradeMutationBodyV1::Proposal { candidate } => {
                candidate.candidate_id.expect("candidate id")
            }
            _ => unreachable!(),
        };
        vec![
            proposal,
            envelope(
                RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID,
                TradeMutationBodyV1::RevisionProposal {
                    candidate: candidate("2"),
                },
            ),
            envelope(
                RADROOTS_TRADE_DECISION_CONTRACT_ID,
                TradeMutationBodyV1::Decision {
                    proposal_mutation_id: proposal_id,
                    candidate_id,
                    decision: TradeDecisionV1::Declined {
                        reason: "unavailable".into(),
                    },
                },
            ),
            envelope(
                RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID,
                TradeMutationBodyV1::RevisionDecision {
                    proposal_mutation_id: mutation_id('2'),
                    candidate_id,
                    decision: TradeDecisionV1::Declined {
                        reason: "unavailable".into(),
                    },
                },
            ),
            envelope(
                RADROOTS_TRADE_CANCELLATION_CONTRACT_ID,
                TradeMutationBodyV1::Cancellation {
                    target_candidate_id: Some(candidate_id),
                    target_claim_mutation_id: None,
                    reason: "cancelled".into(),
                },
            ),
        ]
    }

    #[test]
    fn prepare_covers_every_command_with_one_trade_identity_and_private_plan() {
        let plans = all_commands()
            .into_iter()
            .map(|mutation| {
                prepare(PrepareRequest::new(
                    actor(BUYER, AuthorRole::Buyer),
                    mutation,
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("plans");

        assert_eq!(
            plans
                .iter()
                .map(|plan| plan.workflow().kind())
                .collect::<Vec<_>>(),
            [
                TradeMutationKindV1::Proposal,
                TradeMutationKindV1::RevisionProposal,
                TradeMutationKindV1::Decision,
                TradeMutationKindV1::RevisionDecision,
                TradeMutationKindV1::Cancellation,
            ]
        );
        assert!(
            plans
                .iter()
                .all(|plan| plan.workflow().trade_id() == plans[0].workflow().trade_id())
        );
        assert_eq!(
            plans[0].workflow().required_actions()[0],
            WorkflowAction::VerifyPrivateTerms
        );
        assert_eq!(
            plans[0]
                .workflow()
                .private_terms()
                .expect("private terms")
                .artifact_id(),
            "artifact-1"
        );
        for plan in plans {
            assert_eq!(
                plan.draft().contract_id(),
                plan.workflow().kind().contract_id()
            );
            assert_eq!(plan.draft().kind_u32(), plan.workflow().kind().nostr_kind());
        }
    }

    #[test]
    fn prepare_rejects_wrong_identity_role_and_invalid_protocol_once() {
        let mutation = all_commands().remove(0);
        let wrong_role = prepare(PrepareRequest::new(
            actor(BUYER, AuthorRole::Seller),
            mutation.clone(),
        ))
        .expect_err("wrong role");
        assert_eq!(wrong_role.kind(), PrepareErrorKind::UnauthorizedActor);
        assert!(std::error::Error::source(&wrong_role).is_none());

        let wrong_identity = prepare(PrepareRequest::new(
            actor(SELLER, AuthorRole::Buyer),
            mutation,
        ))
        .expect_err("wrong identity");
        assert_eq!(wrong_identity.kind(), PrepareErrorKind::UnauthorizedActor);

        let mut invalid = all_commands().remove(0);
        invalid.contract_id = "radroots.trade.cancellation.v1".into();
        let canonical = prepare(PrepareRequest::new(
            actor(BUYER, AuthorRole::Buyer),
            invalid,
        ))
        .expect_err("invalid contract");
        assert_eq!(canonical.kind(), PrepareErrorKind::CanonicalMutation);
        assert!(std::error::Error::source(&canonical).is_some());
        assert!(!format!("{canonical:?}").contains("artifact-1"));
    }

    #[test]
    fn query_projection_returns_the_lower_projection_and_conflict_evidence_types() {
        let trade_id = TradeId::parse("22".repeat(16)).expect("trade id");
        let projection = project(ReductionInput::new(trade_id));
        assert_eq!(projection.trade_id(), &trade_id);
        assert!(projection.candidate_heads().is_empty());
        assert!(!projection.projection_digest().is_empty());
    }

    #[cfg(all(feature = "sync", feature = "memory", feature = "local-signing"))]
    mod operations {
        use std::sync::{
            Arc,
            atomic::{AtomicU8, Ordering},
        };

        use radroots_nostr::key::SecretKey;
        use radroots_signing::request::CancellationPolicy;
        use radroots_storage::{
            Outbox,
            event::{EventQuery, EventQueryBounds, SourceGeneration},
            journal::IdempotencyKey,
            memory::MemoryStorage,
            private_artifact::{
                ArtifactCommitment, ArtifactKind, ArtifactSchemaId, DurableSecretReference,
                PrivateArtifactId, PrivateArtifactMetadata, PrivateArtifactStore, RetentionPolicy,
            },
        };
        use radroots_sync::{
            Engine,
            policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId, SyncStorage},
        };
        use radroots_transport::{
            DeliveryReceipt, DeliveryRequest, Error as TransportError, EventSink, SinkStatus,
            Target, TargetSet, TransportId,
            capability::{Availability, Maturity, SinkCapabilities},
            policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
        };

        use super::*;
        use crate::{ClientBuilder, transport::Profile};

        const BUYER_SECRET: &str =
            "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";

        struct FixedClock;
        struct SequenceIds(AtomicU8);
        struct NoopSink;

        impl Clock for FixedClock {
            fn now_unix_ms(&self) -> Result<u64, Error> {
                Ok(2_000_000_000_000)
            }
        }
        impl IdSource for SequenceIds {
            fn next_id(&self, _operation: OperationKind) -> Result<SyncId, Error> {
                SyncId::new([self.0.fetch_add(1, Ordering::Relaxed); 16])
            }
        }
        impl EventSink for NoopSink {
            fn status(
                &self,
            ) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
                Box::pin(async {
                    Ok(SinkStatus::new(
                        TransportId::NOSTR,
                        true,
                        Maturity::Stable,
                        Availability::Available,
                        SinkCapabilities::DELIVER,
                        "ready",
                    ))
                })
            }
            fn deliver(
                &self,
                _request: DeliveryRequest,
            ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, TransportError>>
            {
                Box::pin(async { Err(TransportError::UnsupportedOperation) })
            }
        }

        #[tokio::test]
        async fn operations_cover_private_evidence_pagination_commit_replay_and_cancellation() {
            let storage = Arc::new(MemoryStorage::new(
                SourceGeneration::new([6; 32]).expect("generation"),
            ));
            let artifact_id = PrivateArtifactId::new([7; 16]).expect("artifact id");
            let metadata = PrivateArtifactMetadata::new(
                artifact_id,
                ArtifactKind::parse("trade_private_terms").expect("kind"),
                ArtifactSchemaId::parse("radroots.private.fulfillment.v1").expect("schema"),
                ArtifactCommitment::new([0xee; 32]),
                64,
                DurableSecretReference::new("memory", "trade-artifact-1", 1).expect("reference"),
                RetentionPolicy::indefinite(),
                1_800_000_000_000,
            )
            .expect("metadata");
            PrivateArtifactStore::put_metadata(storage.as_ref(), metadata)
                .await
                .expect("store metadata");

            let signer = Arc::new(
                radroots_nostr::signing::LocalSigner::new(
                    SecretKey::parse(BUYER_SECRET).expect("secret"),
                )
                .expect("signer"),
            );
            let capability: Arc<dyn SyncStorage> = storage.clone();
            let engine = Engine::builder(
                capability,
                Arc::new(FixedClock),
                Arc::new(SequenceIds(AtomicU8::new(1))),
                DeadlinePolicy::new(1_000, 1_000, 1_000).expect("deadlines"),
            )
            .sink(Arc::new(NoopSink))
            .signer(signer)
            .build()
            .expect("engine");
            let client = ClientBuilder::new()
                .storage(storage.clone())
                .sync_engine(engine)
                .build()
                .expect("client");
            let operations = client.trade().expect("open").expect("trade operations");
            let plan = prepare(PrepareRequest::new(
                actor(BUYER, AuthorRole::Buyer),
                all_commands().remove(0),
            ))
            .expect("plan");

            let verified = operations
                .verify_private_terms(&plan, artifact_id)
                .await
                .expect("private evidence");
            assert_eq!(verified.artifact_id(), artifact_id);
            let page = operations
                .query_visible(EventQuery::all(EventQueryBounds::first(1).expect("bounds")))
                .await
                .expect("page");
            assert!(page.items().is_empty());
            assert!(page.next_cursor().is_none());

            let targets = TargetSet::new(vec![
                Target::nostr_relay("wss://trade.example").expect("target"),
            ])
            .expect("targets");
            let satisfaction =
                SatisfactionPolicy::new(SatisfactionClass::Delivered, TargetPolicy::all());
            let request = EnqueueRequest::new(
                SyncId::new([11; 16]).expect("operation id"),
                IdempotencyKey::parse("trade-proposal-a").expect("idempotency"),
                plan,
                Profile::delivery(targets.clone(), satisfaction.clone()).expect("profile"),
                CancellationPolicy::PreservePublishedRequest,
            );
            drop(operations.enqueue(request.clone()));
            assert_eq!(
                Outbox::status(storage.as_ref())
                    .await
                    .expect("status")
                    .pending,
                0
            );
            let committed = operations.enqueue(request.clone()).await.expect("commit");
            assert!(!committed.is_replay());
            assert_eq!(committed.outbox().request().target_set(), &targets);
            let replay = operations.enqueue(request).await.expect("resume replay");
            assert!(replay.is_replay());
            assert_eq!(replay.outbox().item_id(), committed.outbox().item_id());
        }
    }
}
