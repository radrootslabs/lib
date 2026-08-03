//! Side-effect-free listing planning and canonical durable enqueue operations.

use std::{error, fmt};

use radroots_event::{EventDraft, contract::AuthorRole, id::ClassifiedListingAddress};
use radroots_signing::Actor;
use radroots_trade::operational_listing::{
    RadrootsOperationalListingEditDocumentV1, RadrootsOperationalListingEditError,
    RadrootsOperationalListingLifecycleState, RadrootsOperationalListingMutation,
    RadrootsOperationalListingMutationError, build_operational_listing_mutation_draft,
    canonicalize_operational_listing_edit,
};

/// Supported public listing mutation intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Action {
    /// Publish the first public version of a listing.
    Publish,
    /// Replace an existing addressable listing with a new public version.
    Update,
}

/// Pure inputs for one frozen public listing plan.
#[derive(Clone, Debug)]
pub struct PrepareRequest {
    actor: Actor,
    document: RadrootsOperationalListingEditDocumentV1,
    action: Action,
    created_at_unix: u64,
}

impl PrepareRequest {
    /// Creates a public listing publication request.
    #[must_use]
    pub const fn publish(
        actor: Actor,
        document: RadrootsOperationalListingEditDocumentV1,
        created_at_unix: u64,
    ) -> Self {
        Self {
            actor,
            document,
            action: Action::Publish,
            created_at_unix,
        }
    }

    /// Creates a public listing replacement request.
    #[must_use]
    pub const fn update(
        actor: Actor,
        document: RadrootsOperationalListingEditDocumentV1,
        created_at_unix: u64,
    ) -> Self {
        Self {
            actor,
            document,
            action: Action::Update,
            created_at_unix,
        }
    }
}

/// Frozen, replay-stable public listing mutation plan.
#[derive(Clone, Debug)]
pub struct Plan {
    actor: Actor,
    action: Action,
    address: ClassifiedListingAddress,
    lifecycle: RadrootsOperationalListingLifecycleState,
    draft: EventDraft,
}

impl Plan {
    /// Returns the exact authorized actor carried into signing.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// Returns the requested listing mutation intent.
    #[must_use]
    pub const fn action(&self) -> Action {
        self.action
    }

    /// Returns the canonical addressable listing identity.
    #[must_use]
    pub const fn address(&self) -> &ClassifiedListingAddress {
        &self.address
    }

    /// Returns the lower-owned lifecycle state resulting from the mutation.
    #[must_use]
    pub const fn lifecycle(&self) -> RadrootsOperationalListingLifecycleState {
        self.lifecycle
    }

    /// Returns the frozen canonical event draft.
    #[must_use]
    pub const fn draft(&self) -> &EventDraft {
        &self.draft
    }
}

/// Listing plan validation stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrepareErrorKind {
    /// The actor does not claim the seller author role.
    UnauthorizedActor,
    /// The lower trade boundary rejected the untrusted edit document.
    Edit,
    /// The lower trade/event boundary rejected mutation preparation.
    Mutation,
}

/// One secret-safe listing planning failure retaining its lower source.
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

    fn edit(source: RadrootsOperationalListingEditError) -> Self {
        Self::with_source(PrepareErrorKind::Edit, source)
    }

    fn mutation(source: RadrootsOperationalListingMutationError) -> Self {
        Self::with_source(PrepareErrorKind::Mutation, source)
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
            PrepareErrorKind::UnauthorizedActor => "listing actor is not authorized",
            PrepareErrorKind::Edit => "listing edit is invalid",
            PrepareErrorKind::Mutation => "listing mutation is invalid",
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

/// Validates and freezes one listing mutation without storage, signing, or network work.
///
/// The canonical public model permits only coarse public locality. Exact
/// coordinates and other private artifacts are deliberately absent; hosts
/// persist those through `radroots_storage::private_artifact::PrivateArtifactStore`.
pub fn prepare(request: PrepareRequest) -> Result<Plan, PrepareError> {
    if !request.actor.satisfies(AuthorRole::Seller) {
        return Err(PrepareError::unauthorized_actor());
    }
    let canonical =
        canonicalize_operational_listing_edit(request.actor.public_key(), request.document)
            .map_err(PrepareError::edit)?;
    let address = canonical.public_listing_addr().clone();
    let mutation = match request.action {
        Action::Publish => RadrootsOperationalListingMutation::publish(canonical),
        Action::Update => RadrootsOperationalListingMutation::update(canonical),
    };
    let lifecycle = mutation.lifecycle_state().map_err(PrepareError::mutation)?;
    let draft = build_operational_listing_mutation_draft(&mutation, request.created_at_unix)
        .map_err(PrepareError::mutation)?;
    Ok(Plan {
        actor: request.actor,
        action: request.action,
        address,
        lifecycle,
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

/// Explicit commit inputs for one prepared public listing mutation.
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

/// Borrowed listing commit operations over the canonical sync engine.
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

    /// Signs and atomically enqueues a prepared public listing mutation.
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
    use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
    use radroots_event::{
        envelope::kind::KIND_CLASSIFIED_LISTING,
        farm::FarmRef,
        id::{DTag, InventoryBinId},
        listing::operational::{
            OperationalListing, OperationalListingAvailability, OperationalListingBin,
            OperationalListingDeliveryMethod, OperationalListingProduct,
            OperationalListingPublicLocation, OperationalListingStatus,
        },
    };
    use radroots_signing::actor::ActorSource;

    use super::*;

    const PUBLIC_KEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    fn actor(role: AuthorRole) -> Actor {
        Actor::from_public_key_hex(PUBLIC_KEY, ActorSource::ExplicitPublicKey, [role])
            .expect("actor")
    }

    fn listing(seller: &str) -> OperationalListing {
        OperationalListing {
            d_tag: DTag::parse("AAAAAAAAAAAAAAAAAAAAAg").expect("d tag"),
            published_at: None,
            farm: FarmRef {
                pubkey: seller.to_owned(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_owned(),
            },
            product: OperationalListingProduct {
                key: "coffee".to_owned(),
                title: "Coffee".to_owned(),
                category: "coffee".to_owned(),
                summary: Some("Single origin coffee".to_owned()),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: InventoryBinId::parse("bin-1").expect("bin id"),
            bins: vec![OperationalListingBin {
                bin_id: InventoryBinId::parse("bin-1").expect("bin id"),
                quantity: Quantity::try_new(Decimal::from(1000_u32), Unit::MassG)
                    .expect("quantity"),
                price_per_canonical_unit: QuantityPrice::try_new(
                    Money::try_new(Decimal::from(20_u32), Currency::USD).expect("money"),
                    Quantity::try_new(Decimal::from(1_u32), Unit::MassG).expect("unit"),
                )
                .expect("price"),
                display_amount: None,
                display_unit: None,
                display_label: None,
                display_price: None,
                display_price_unit: None,
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: Some(Decimal::from(5_u32)),
            availability: Some(OperationalListingAvailability::Status {
                status: OperationalListingStatus::Active,
            }),
            delivery_method: Some(OperationalListingDeliveryMethod::Pickup),
            location: Some(OperationalListingPublicLocation {
                primary: "Santa Cruz, California".to_owned(),
                city: Some("Santa Cruz".to_owned()),
                region: Some("California".to_owned()),
                country: Some("US".to_owned()),
                geohash: "9q8yy".to_owned(),
            }),
            images: None,
        }
    }

    fn document(seller: &str) -> RadrootsOperationalListingEditDocumentV1 {
        RadrootsOperationalListingEditDocumentV1::new(listing(seller))
    }

    #[test]
    fn prepare_publish_and_update_are_pure_canonical_and_privacy_bounded() {
        let publish_request = PrepareRequest::publish(
            actor(AuthorRole::Seller),
            document(PUBLIC_KEY),
            1_800_000_000,
        );
        let first = prepare(publish_request.clone()).expect("publish plan");
        let replay = prepare(publish_request).expect("replayed plan");
        let update = prepare(PrepareRequest::update(
            actor(AuthorRole::Seller),
            document(PUBLIC_KEY),
            1_800_000_001,
        ))
        .expect("update plan");

        assert_eq!(first.action(), Action::Publish);
        assert_eq!(update.action(), Action::Update);
        assert_eq!(
            first.lifecycle(),
            RadrootsOperationalListingLifecycleState::Published
        );
        assert_eq!(
            update.lifecycle(),
            RadrootsOperationalListingLifecycleState::Published
        );
        assert_eq!(first.address(), replay.address());
        assert_eq!(first.draft(), replay.draft());
        assert_eq!(first.address(), update.address());
        assert_eq!(first.draft().kind_u32(), KIND_CLASSIFIED_LISTING);
        assert_eq!(first.draft().created_at_u64(), 1_800_000_000);
        assert!(!first.draft().content().contains("latitude"));
        assert!(!first.draft().content().contains("longitude"));
    }

    #[test]
    fn prepare_maps_authorization_and_lower_validation_classes_once() {
        let unauthorized = prepare(PrepareRequest::publish(
            actor(AuthorRole::Buyer),
            document(PUBLIC_KEY),
            1_800_000_000,
        ))
        .expect_err("unauthorized");
        assert_eq!(unauthorized.kind(), PrepareErrorKind::UnauthorizedActor);
        assert!(std::error::Error::source(&unauthorized).is_none());

        let mut invalid = listing(PUBLIC_KEY);
        invalid.product.title.clear();
        let edit = prepare(PrepareRequest::publish(
            actor(AuthorRole::Seller),
            RadrootsOperationalListingEditDocumentV1::new(invalid),
            1_800_000_000,
        ))
        .expect_err("invalid listing");
        assert_eq!(edit.kind(), PrepareErrorKind::Edit);
        assert!(std::error::Error::source(&edit).is_some());
        assert_eq!(edit.to_string(), "listing edit is invalid");
        assert!(!format!("{edit:?}").contains("title"));

        let mismatch = prepare(PrepareRequest::update(
            actor(AuthorRole::Seller),
            document("8f"),
            1_800_000_000,
        ))
        .expect_err("invalid seller identity");
        assert_eq!(mismatch.kind(), PrepareErrorKind::Edit);
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

        use super::*;
        use crate::{ClientBuilder, transport::Profile};

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
        async fn enqueue_preserves_commit_cancellation_idempotency_and_transport_outcomes() {
            let storage = Arc::new(MemoryStorage::new(
                SourceGeneration::new([5; 32]).expect("generation"),
            ));
            let signer =
                Arc::new(radroots_nostr::signing::LocalSigner::generate().expect("local signer"));
            let seller = Actor::new(
                signer.public_key(),
                ActorSource::ExplicitPublicKey,
                [AuthorRole::Seller],
            )
            .expect("actor");
            let plan = prepare(PrepareRequest::publish(
                seller,
                document(&signer.public_key().to_hex()),
                1_800_000_000,
            ))
            .expect("plan");
            let targets = TargetSet::new(vec![
                Target::nostr_relay("wss://listing.example").expect("target"),
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
            let operations = client.listing().expect("open").expect("listing operations");
            let request = EnqueueRequest::new(
                SyncId::new([9; 16]).expect("operation id"),
                IdempotencyKey::parse("listing-publish-a").expect("idempotency key"),
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
                SyncId::new([10; 16]).expect("operation id"),
                IdempotencyKey::parse("listing-update-preview").expect("idempotency key"),
                prepare(PrepareRequest::update(
                    actor(AuthorRole::Seller),
                    document(PUBLIC_KEY),
                    1_800_000_001,
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
