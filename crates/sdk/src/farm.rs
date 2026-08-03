//! Side-effect-free farm planning and canonical durable enqueue operations.

use std::{error, fmt};

use radroots_event::{
    EventDraft,
    contract::AuthorRole,
    envelope::kind::KIND_FARM,
    farm::Farm,
    id::{AddressableCoordinate, ParseError},
};
use radroots_event_codec::{encode::EventEncodeError, encode::farm::to_wire_parts};
use radroots_signing::Actor;

const FARM_PROFILE_CONTRACT_ID: &str = "radroots.farm.profile.v1";

/// Pure inputs for one frozen farm profile plan.
#[derive(Clone, Debug)]
pub struct PrepareRequest {
    actor: Actor,
    farm: Farm,
    created_at_unix: u64,
}

impl PrepareRequest {
    /// Creates explicit canonical planning inputs.
    #[must_use]
    pub const fn new(actor: Actor, farm: Farm, created_at_unix: u64) -> Self {
        Self {
            actor,
            farm,
            created_at_unix,
        }
    }
}

/// Frozen, replay-stable farm publication plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    actor: Actor,
    coordinate: AddressableCoordinate,
    draft: EventDraft,
}

impl Plan {
    /// Returns the exact authorized actor carried into signing.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// Returns the canonical addressable farm coordinate.
    #[must_use]
    pub const fn coordinate(&self) -> &AddressableCoordinate {
        &self.coordinate
    }

    /// Returns the frozen canonical event draft.
    #[must_use]
    pub const fn draft(&self) -> &EventDraft {
        &self.draft
    }
}

/// Farm plan validation stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrepareErrorKind {
    /// The actor does not claim the required farm author role.
    UnauthorizedActor,
    /// The canonical farm codec rejected the native farm model.
    Encode,
    /// The canonical addressable coordinate rejected the farm identity.
    Coordinate,
    /// The canonical event draft rejected the encoded parts.
    Draft,
}

/// One secret-safe farm planning failure retaining its lower source.
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

    fn encode(source: EventEncodeError) -> Self {
        Self::with_source(PrepareErrorKind::Encode, source)
    }

    fn coordinate(source: ParseError) -> Self {
        Self::with_source(PrepareErrorKind::Coordinate, source)
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
            PrepareErrorKind::UnauthorizedActor => "farm actor is not authorized",
            PrepareErrorKind::Encode => "farm model is invalid",
            PrepareErrorKind::Coordinate => "farm coordinate is invalid",
            PrepareErrorKind::Draft => "farm event draft is invalid",
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

/// Validates and freezes one farm profile without storage, signing, or network work.
///
/// `Farm` contains only public profile locality. Exact coordinates and other
/// private farm artifacts are deliberately not accepted here; hosts persist
/// their typed references and metadata through
/// `radroots_storage::private_artifact::PrivateArtifactStore`.
pub fn prepare(request: PrepareRequest) -> Result<Plan, PrepareError> {
    if !request.actor.satisfies(AuthorRole::Farmer) {
        return Err(PrepareError::unauthorized_actor());
    }
    let parts = to_wire_parts(&request.farm).map_err(PrepareError::encode)?;
    let coordinate = AddressableCoordinate::parse(format!(
        "{KIND_FARM}:{}:{}",
        request.actor.public_key(),
        request.farm.d_tag
    ))
    .map_err(PrepareError::coordinate)?;
    let draft = EventDraft::new(
        FARM_PROFILE_CONTRACT_ID,
        parts.kind,
        request.created_at_unix,
        parts.tags,
        parts.content,
        request.actor.public_key().to_hex(),
    )
    .map_err(PrepareError::draft)?;
    Ok(Plan {
        actor: request.actor,
        coordinate,
        draft,
    })
}

#[cfg(feature = "sync")]
use radroots_signing::request::CancellationPolicy;
#[cfg(feature = "sync")]
use radroots_storage::journal::IdempotencyKey;
#[cfg(feature = "sync")]
use radroots_sync::{
    PushReceipt,
    policy::{Error as SyncError, SyncId},
};

/// Explicit commit inputs for one prepared farm publication.
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
    /// Creates an enqueue request whose transport selection has no fallback.
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

/// Borrowed farm commit operations over the canonical sync engine.
#[cfg(feature = "sync")]
#[derive(Clone, Copy, Debug)]
pub struct Operations<'a> {
    sync: crate::sync::Operations<'a>,
}

#[cfg(feature = "sync")]
impl<'a> Operations<'a> {
    pub(crate) const fn new(sync: crate::sync::Operations<'a>) -> Self {
        Self { sync }
    }

    /// Signs and atomically enqueues a prepared farm publication.
    ///
    /// Before the lower atomic enqueue commit, cancellation may leave only
    /// recoverable prepared/signed journal state. After commit, cancellation
    /// cannot claim rollback; replay with the same idempotency input returns
    /// the durable outbox record.
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
}

#[cfg(test)]
mod tests {
    use radroots_event::farm::FarmPublicLocation;
    use radroots_signing::actor::ActorSource;

    use super::*;

    const PUBLIC_KEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    fn actor(role: AuthorRole) -> Actor {
        Actor::from_public_key_hex(PUBLIC_KEY, ActorSource::ExplicitPublicKey, [role])
            .expect("actor")
    }

    fn farm() -> Farm {
        Farm {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_owned(),
            name: "Moss Street Farm".to_owned(),
            about: Some("seasonal vegetables".to_owned()),
            website: None,
            picture: None,
            banner: None,
            location: Some(FarmPublicLocation {
                primary: "Santa Cruz, California".to_owned(),
                city: Some("Santa Cruz".to_owned()),
                region: Some("California".to_owned()),
                country: Some("US".to_owned()),
                geohash: "9q8yy".to_owned(),
            }),
            tags: Some(vec!["vegetables".to_owned()]),
        }
    }

    #[test]
    fn prepare_is_pure_deterministic_and_uses_canonical_public_types() {
        let request = PrepareRequest::new(actor(AuthorRole::Farmer), farm(), 1_800_000_000);
        let first = prepare(request.clone()).expect("first plan");
        let second = prepare(request).expect("second plan");

        assert_eq!(first, second);
        assert_eq!(first.draft().contract_id(), FARM_PROFILE_CONTRACT_ID);
        assert_eq!(first.draft().kind_u32(), KIND_FARM);
        assert_eq!(first.draft().created_at_u64(), 1_800_000_000);
        assert_eq!(
            first.draft().expected_pubkey(),
            &actor(AuthorRole::Farmer).public_key()
        );
        assert_eq!(
            first.coordinate().as_str(),
            format!("{KIND_FARM}:{PUBLIC_KEY}:AAAAAAAAAAAAAAAAAAAAAA")
        );
        assert!(first.draft().content().contains("Moss Street Farm"));
        assert!(!first.draft().content().contains("latitude"));
        assert!(!first.draft().content().contains("longitude"));
    }

    #[test]
    fn prepare_maps_authorization_and_lower_validation_once() {
        let unauthorized = prepare(PrepareRequest::new(
            actor(AuthorRole::Buyer),
            farm(),
            1_800_000_000,
        ))
        .expect_err("unauthorized");
        assert_eq!(unauthorized.kind(), PrepareErrorKind::UnauthorizedActor);
        assert!(std::error::Error::source(&unauthorized).is_none());

        let mut invalid = farm();
        invalid.name.clear();
        let encoded = prepare(PrepareRequest::new(
            actor(AuthorRole::Farmer),
            invalid,
            1_800_000_000,
        ))
        .expect_err("invalid model");
        assert_eq!(encoded.kind(), PrepareErrorKind::Encode);
        assert!(std::error::Error::source(&encoded).is_some());
        assert_eq!(encoded.to_string(), "farm model is invalid");
        assert!(!format!("{encoded:?}").contains("name"));
    }

    #[cfg(all(feature = "sync", feature = "memory", feature = "local-signing"))]
    mod enqueue {
        use std::sync::{
            Arc,
            atomic::{AtomicU8, Ordering},
        };

        use radroots_storage::{
            Outbox, event::SourceGeneration, journal::IdempotencyKey, memory::MemoryStorage,
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

        use crate::{ClientBuilder, transport::Profile};

        use super::*;

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
        async fn enqueue_preserves_commit_cancellation_idempotency_and_delivery_policy() {
            let storage = Arc::new(MemoryStorage::new(
                SourceGeneration::new([4; 32]).expect("generation"),
            ));
            let signer =
                Arc::new(radroots_nostr::signing::LocalSigner::generate().expect("local signer"));
            let farm_actor = Actor::new(
                signer.public_key(),
                ActorSource::ExplicitPublicKey,
                [AuthorRole::Farmer],
            )
            .expect("actor");
            let plan =
                prepare(PrepareRequest::new(farm_actor, farm(), 1_800_000_000)).expect("plan");
            let targets = TargetSet::new(vec![
                Target::nostr_relay("wss://farm.example").expect("target"),
            ])
            .expect("targets");
            let satisfaction =
                SatisfactionPolicy::new(SatisfactionClass::Delivered, TargetPolicy::all());
            let profile = Profile::delivery(targets.clone(), satisfaction.clone())
                .expect("transport profile");
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
            let operations = client.farm().expect("open").expect("farm operations");
            let request = EnqueueRequest::new(
                SyncId::new([7; 16]).expect("operation id"),
                IdempotencyKey::parse("farm-publish-a").expect("idempotency key"),
                plan,
                profile,
                CancellationPolicy::PreservePublishedRequest,
            );

            drop(operations.enqueue(request.clone()));
            assert_eq!(
                Outbox::status(storage.as_ref())
                    .await
                    .expect("outbox status")
                    .pending,
                0
            );

            let committed = operations
                .enqueue(request.clone())
                .await
                .expect("committed enqueue");
            assert!(!committed.is_replay());
            assert_eq!(committed.outbox().request().target_set(), &targets);
            assert_eq!(committed.outbox().request().satisfaction(), &satisfaction);
            let replay = operations.enqueue(request).await.expect("replay");
            assert!(replay.is_replay());
            assert_eq!(replay.outbox().item_id(), committed.outbox().item_id());

            let unavailable = EnqueueRequest::new(
                SyncId::new([8; 16]).expect("operation id"),
                IdempotencyKey::parse("farm-publish-preview").expect("idempotency key"),
                prepare(PrepareRequest::new(
                    actor(AuthorRole::Farmer),
                    farm(),
                    1_800_000_000,
                ))
                .expect("plan"),
                Profile::unavailable_preview(TransportId::RETICULUM),
                CancellationPolicy::PreservePublishedRequest,
            );
            assert_eq!(
                operations.enqueue(unavailable).await,
                Err(Error::InvalidPushRequest)
            );
        }
    }
}
