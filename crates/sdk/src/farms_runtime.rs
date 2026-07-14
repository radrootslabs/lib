#[cfg(feature = "signer-adapters")]
use crate::workflow_runtime::enqueue_configured_signed_workflow;
#[cfg(feature = "runtime")]
use crate::{
    FarmsClient, NostrRelayUrlPolicy, RadrootsSdkError, RadrootsSdkTimestamp, SatisfactionPolicy,
    SdkIdempotencyKey, SdkMutationState, TargetPolicy, farm,
    geonames::{
        Geocoder, GeocoderLocalityCandidate, GeocoderLocalityLookup, GeocoderLocalityQuery,
        GeocoderPoint, GeocoderReverseOptions, GeocoderReverseResult,
    },
    private_store::SdkPrivateFarmLocationRecord,
    workflow_runtime::{SdkWorkflowEnqueueRequest, enqueue_signed_workflow},
};
#[cfg(feature = "runtime")]
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner};
#[cfg(feature = "runtime")]
use radroots_event::{
    contract::RadrootsActorRole,
    draft::RadrootsEventDraft,
    farm::{RadrootsFarm, RadrootsFarmPublicLocation},
    ids::{RadrootsAddressableCoordinate, RadrootsEventId},
    kinds::KIND_FARM,
    listing::RadrootsListingPublicLocation,
};
#[cfg(feature = "runtime")]
#[cfg(feature = "runtime")]
pub const FARM_PUBLISH_OPERATION_KIND: &str = "farm.publish.v1";

#[cfg(feature = "runtime")]
const FARM_PROFILE_CONTRACT_ID: &str = "radroots.farm.profile.v1";
#[cfg(feature = "runtime")]
const FARM_PRIVATE_LOCATION_OPERATION: &str = "farm.private_location.upsert";
#[cfg(feature = "runtime")]
const GEOHASH5_LEN: usize = 5;
#[cfg(feature = "runtime")]
const GEOHASH_BASE32: &[u8; 32] = b"0123456789bcdefghjkmnpqrstuvwxyz";

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct FarmPreparePublishRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub farm: RadrootsFarm,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl FarmPreparePublishRequest {
    pub fn new(actor: RadrootsActorContext, farm: RadrootsFarm) -> Self {
        Self {
            actor,
            farm,
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
pub struct FarmEnqueuePublishRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub farm: RadrootsFarm,
    pub target_policy: TargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl FarmEnqueuePublishRequest {
    pub fn new(
        actor: RadrootsActorContext,
        farm: RadrootsFarm,
        target_policy: TargetPolicy,
    ) -> Self {
        Self {
            actor,
            farm,
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
pub struct FarmPublishPlan {
    pub farm_addr: RadrootsAddressableCoordinate,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct FarmEnqueueReceipt {
    pub farm_addr: RadrootsAddressableCoordinate,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub state: SdkMutationState,
    pub idempotency_digest_prefix: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SdkExactLocation {
    pub latitude: f64,
    pub longitude: f64,
}

#[cfg(feature = "runtime")]
impl SdkExactLocation {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SdkPublicLocality {
    pub primary: String,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub geohash5: String,
}

#[cfg(feature = "runtime")]
impl SdkPublicLocality {
    pub fn to_farm_public_location(&self) -> RadrootsFarmPublicLocation {
        RadrootsFarmPublicLocation {
            primary: self.primary.clone(),
            city: self.city.clone(),
            region: self.region.clone(),
            country: self.country.clone(),
            geohash: self.geohash5.clone(),
        }
    }

    pub fn to_listing_public_location(&self) -> RadrootsListingPublicLocation {
        RadrootsListingPublicLocation {
            primary: self.primary.clone(),
            city: self.city.clone(),
            region: self.region.clone(),
            country: self.country.clone(),
            geohash: self.geohash5.clone(),
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct FarmPrivateLocationUpsertRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub farm_d_tag: String,
    pub exact_location: SdkExactLocation,
    pub label: Option<String>,
    pub updated_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl FarmPrivateLocationUpsertRequest {
    pub fn new(
        actor: RadrootsActorContext,
        farm_d_tag: impl Into<String>,
        exact_location: SdkExactLocation,
    ) -> Self {
        Self {
            actor,
            farm_d_tag: farm_d_tag.into(),
            exact_location,
            label: None,
            updated_at: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_updated_at(mut self, updated_at: RadrootsSdkTimestamp) -> Self {
        self.updated_at = Some(updated_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum FarmPrivateLocationInput {
    Exact(SdkExactLocation),
    Locality(GeocoderLocalityQuery),
}

#[cfg(feature = "runtime")]
impl FarmPrivateLocationInput {
    pub fn exact(location: SdkExactLocation) -> Self {
        Self::Exact(location)
    }

    pub fn city(city: impl Into<String>) -> Self {
        Self::Locality(GeocoderLocalityQuery::structured(city))
    }

    pub fn query(query: impl Into<String>) -> Self {
        Self::Locality(GeocoderLocalityQuery::query(query))
    }

    pub fn geonames_id(id: i64) -> Self {
        Self::Locality(GeocoderLocalityQuery::feature_id(id))
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct FarmPrivateLocationSetRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub farm_d_tag: String,
    pub input: FarmPrivateLocationInput,
    pub label: Option<String>,
    pub updated_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl FarmPrivateLocationSetRequest {
    pub fn new(
        actor: RadrootsActorContext,
        farm_d_tag: impl Into<String>,
        input: FarmPrivateLocationInput,
    ) -> Self {
        Self {
            actor,
            farm_d_tag: farm_d_tag.into(),
            input,
            label: None,
            updated_at: None,
        }
    }

    pub fn exact(
        actor: RadrootsActorContext,
        farm_d_tag: impl Into<String>,
        exact_location: SdkExactLocation,
    ) -> Self {
        Self::new(
            actor,
            farm_d_tag,
            FarmPrivateLocationInput::exact(exact_location),
        )
    }

    pub fn city(
        actor: RadrootsActorContext,
        farm_d_tag: impl Into<String>,
        city: impl Into<String>,
    ) -> Self {
        Self::new(actor, farm_d_tag, FarmPrivateLocationInput::city(city))
    }

    pub fn query(
        actor: RadrootsActorContext,
        farm_d_tag: impl Into<String>,
        query: impl Into<String>,
    ) -> Self {
        Self::new(actor, farm_d_tag, FarmPrivateLocationInput::query(query))
    }

    pub fn geonames_id(
        actor: RadrootsActorContext,
        farm_d_tag: impl Into<String>,
        id: i64,
    ) -> Self {
        Self::new(actor, farm_d_tag, FarmPrivateLocationInput::geonames_id(id))
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_updated_at(mut self, updated_at: RadrootsSdkTimestamp) -> Self {
        self.updated_at = Some(updated_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct FarmPrivateLocationClearRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub farm_d_tag: String,
}

#[cfg(feature = "runtime")]
impl FarmPrivateLocationClearRequest {
    pub fn new(actor: RadrootsActorContext, farm_d_tag: impl Into<String>) -> Self {
        Self {
            actor,
            farm_d_tag: farm_d_tag.into(),
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FarmPrivateLocationClearReceipt {
    pub farm_addr: RadrootsAddressableCoordinate,
    pub cleared: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FarmPrivateLocationReceipt {
    pub farm_addr: RadrootsAddressableCoordinate,
    pub farm_pubkey: String,
    pub farm_d_tag: String,
    pub label: Option<String>,
    pub exact_location: SdkExactLocation,
    pub public_locality: SdkPublicLocality,
    pub geonames_feature_id: Option<i64>,
    pub geonames_country_id: Option<String>,
    pub updated_at_ms: i64,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FarmPrivateLocationLookupCandidate {
    pub geonames_feature_id: i64,
    pub geonames_country_id: String,
    pub name: String,
    pub display_name: String,
    pub exact_location: SdkExactLocation,
    pub region: Option<String>,
    pub country: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FarmPrivateLocationLookupReceipt {
    pub farm_addr: RadrootsAddressableCoordinate,
    pub farm_pubkey: String,
    pub farm_d_tag: String,
    pub input: FarmPrivateLocationInput,
    pub candidates: Vec<FarmPrivateLocationLookupCandidate>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "status", content = "receipt")]
pub enum FarmPrivateLocationSetResult {
    Stored(FarmPrivateLocationReceipt),
    NoMatch(FarmPrivateLocationLookupReceipt),
    Ambiguous(FarmPrivateLocationLookupReceipt),
}

#[cfg(feature = "runtime")]
impl<'sdk> FarmsClient<'sdk> {
    pub fn prepare_publish(
        &self,
        request: FarmPreparePublishRequest,
    ) -> Result<FarmPublishPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        farm_publish_plan(&request.actor, request.farm, created_at)
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_publish(
        &self,
        request: FarmEnqueuePublishRequest,
    ) -> Result<FarmEnqueueReceipt, RadrootsSdkError> {
        let FarmEnqueuePublishRequest {
            actor,
            farm,
            target_policy,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = FarmPreparePublishRequest {
            actor: actor.clone(),
            farm,
            created_at,
        };
        let plan = self.prepare_publish(prepare_request)?;
        self.enqueue_prepared_publish(&actor, plan, target_policy, idempotency_key)
            .await
    }

    pub async fn enqueue_publish_with_explicit_signer(
        &self,
        request: FarmEnqueuePublishRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<FarmEnqueueReceipt, RadrootsSdkError> {
        let FarmEnqueuePublishRequest {
            actor,
            farm,
            target_policy,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = FarmPreparePublishRequest {
            actor: actor.clone(),
            farm,
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
        plan: FarmPublishPlan,
        target_policy: TargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<FarmEnqueueReceipt, RadrootsSdkError> {
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: FARM_PUBLISH_OPERATION_KIND,
                actor,
                frozen_draft: &plan.frozen_draft,
                target_policy,
                satisfaction_policy: SatisfactionPolicy::AllAccepted,
                idempotency_key,
            },
        )
        .await?;
        Ok(farm_enqueue_receipt(plan, enqueue))
    }

    pub async fn enqueue_prepared_publish_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: FarmPublishPlan,
        target_policy: TargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<FarmEnqueueReceipt, RadrootsSdkError> {
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: FARM_PUBLISH_OPERATION_KIND,
                actor,
                frozen_draft: &plan.frozen_draft,
                target_policy,
                satisfaction_policy: SatisfactionPolicy::AllAccepted,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(farm_enqueue_receipt(plan, enqueue))
    }

    pub async fn upsert_private_location(
        &self,
        request: FarmPrivateLocationUpsertRequest,
    ) -> Result<FarmPrivateLocationReceipt, RadrootsSdkError> {
        let geocoder = self.sdk.geonames().open_verified()?;
        self.upsert_private_location_with_geocoder(request, &geocoder)
            .await
    }

    pub async fn set_private_location(
        &self,
        request: FarmPrivateLocationSetRequest,
    ) -> Result<FarmPrivateLocationSetResult, RadrootsSdkError> {
        let geocoder = self.sdk.geonames().open_verified()?;
        self.set_private_location_with_geocoder(request, &geocoder)
            .await
    }

    pub async fn set_private_location_with_geocoder(
        &self,
        request: FarmPrivateLocationSetRequest,
        geocoder: &Geocoder,
    ) -> Result<FarmPrivateLocationSetResult, RadrootsSdkError> {
        match request.input {
            FarmPrivateLocationInput::Exact(exact_location) => {
                let upsert = FarmPrivateLocationUpsertRequest {
                    actor: request.actor,
                    farm_d_tag: request.farm_d_tag,
                    exact_location,
                    label: request.label,
                    updated_at: request.updated_at,
                };
                self.upsert_private_location_with_geocoder(upsert, geocoder)
                    .await
                    .map(FarmPrivateLocationSetResult::Stored)
            }
            FarmPrivateLocationInput::Locality(locality_query) => {
                self.set_private_location_from_locality(
                    request.actor,
                    request.farm_d_tag,
                    locality_query,
                    request.label,
                    request.updated_at,
                    geocoder,
                )
                .await
            }
        }
    }

    pub async fn upsert_private_location_with_geocoder(
        &self,
        request: FarmPrivateLocationUpsertRequest,
        geocoder: &Geocoder,
    ) -> Result<FarmPrivateLocationReceipt, RadrootsSdkError> {
        require_farmer_actor(&request.actor, FARM_PRIVATE_LOCATION_OPERATION)?;
        validate_exact_location(request.exact_location)?;
        let label = normalize_private_location_label(request.label)?;
        let updated_at_ms = match request.updated_at {
            Some(updated_at) => sdk_timestamp_ms(updated_at)?,
            None => crate::runtime::sdk_now_ms(self.sdk)?,
        };
        let farm_addr = farm_addr(&request.actor, request.farm_d_tag.as_str())?;
        let reverse = nearest_geonames_locality(geocoder, request.exact_location)?;
        let public_locality = public_locality_from_reverse(request.exact_location, &reverse)?;
        let record = SdkPrivateFarmLocationRecord {
            farm_addr: farm_addr.clone(),
            farm_pubkey: request.actor.pubkey().as_str().to_owned(),
            farm_d_tag: request.farm_d_tag,
            label,
            latitude: request.exact_location.latitude,
            longitude: request.exact_location.longitude,
            locality_primary: public_locality.primary.clone(),
            locality_city: public_locality.city.clone(),
            locality_region: public_locality.region.clone(),
            locality_country: public_locality.country.clone(),
            geohash5: public_locality.geohash5.clone(),
            geonames_feature_id: Some(reverse.id),
            geonames_country_id: Some(reverse.country_id.clone()),
            updated_at_ms,
        };
        self.sdk
            ._private_store
            .upsert_farm_location(&record)
            .await?;
        Ok(private_location_receipt_from_record(record))
    }

    async fn set_private_location_from_locality(
        &self,
        actor: RadrootsActorContext,
        farm_d_tag: String,
        locality_query: GeocoderLocalityQuery,
        label: Option<String>,
        updated_at: Option<RadrootsSdkTimestamp>,
        geocoder: &Geocoder,
    ) -> Result<FarmPrivateLocationSetResult, RadrootsSdkError> {
        require_farmer_actor(&actor, FARM_PRIVATE_LOCATION_OPERATION)?;
        let label = normalize_private_location_label(label)?;
        let farm_addr = farm_addr(&actor, farm_d_tag.as_str())?;
        let lookup = geocoder.locality(&locality_query)?;
        match lookup {
            GeocoderLocalityLookup::Unique { candidate } => {
                let exact_location = exact_location_from_locality_candidate(&candidate);
                validate_exact_location(exact_location)?;
                let public_locality = public_locality_from_candidate(exact_location, &candidate)?;
                let updated_at_ms = match updated_at {
                    Some(updated_at) => sdk_timestamp_ms(updated_at)?,
                    None => crate::runtime::sdk_now_ms(self.sdk)?,
                };
                let record = SdkPrivateFarmLocationRecord {
                    farm_addr: farm_addr.clone(),
                    farm_pubkey: actor.pubkey().as_str().to_owned(),
                    farm_d_tag,
                    label,
                    latitude: exact_location.latitude,
                    longitude: exact_location.longitude,
                    locality_primary: public_locality.primary.clone(),
                    locality_city: public_locality.city.clone(),
                    locality_region: public_locality.region.clone(),
                    locality_country: public_locality.country.clone(),
                    geohash5: public_locality.geohash5.clone(),
                    geonames_feature_id: Some(candidate.id),
                    geonames_country_id: Some(candidate.country_id.clone()),
                    updated_at_ms,
                };
                self.sdk
                    ._private_store
                    .upsert_farm_location(&record)
                    .await?;
                Ok(FarmPrivateLocationSetResult::Stored(
                    private_location_receipt_from_record(record),
                ))
            }
            GeocoderLocalityLookup::NoMatch => Ok(FarmPrivateLocationSetResult::NoMatch(
                farm_private_location_lookup_receipt(
                    farm_addr,
                    actor.pubkey().as_str(),
                    farm_d_tag,
                    FarmPrivateLocationInput::Locality(locality_query),
                    Vec::new(),
                ),
            )),
            GeocoderLocalityLookup::Ambiguous { candidates } => Ok(
                FarmPrivateLocationSetResult::Ambiguous(farm_private_location_lookup_receipt(
                    farm_addr,
                    actor.pubkey().as_str(),
                    farm_d_tag,
                    FarmPrivateLocationInput::Locality(locality_query),
                    candidates
                        .into_iter()
                        .map(farm_private_location_candidate)
                        .collect(),
                )),
            ),
        }
    }

    pub async fn private_location(
        &self,
        farm_addr: &RadrootsAddressableCoordinate,
    ) -> Result<Option<FarmPrivateLocationReceipt>, RadrootsSdkError> {
        self.sdk
            ._private_store
            .farm_location(farm_addr)
            .await?
            .map(private_location_receipt_from_record)
            .map(Ok)
            .transpose()
    }

    pub async fn clear_private_location(
        &self,
        request: FarmPrivateLocationClearRequest,
    ) -> Result<FarmPrivateLocationClearReceipt, RadrootsSdkError> {
        require_farmer_actor(&request.actor, FARM_PRIVATE_LOCATION_OPERATION)?;
        let farm_addr = farm_addr(&request.actor, request.farm_d_tag.as_str())?;
        let cleared = self
            .sdk
            ._private_store
            .delete_farm_location(&farm_addr)
            .await?;
        Ok(FarmPrivateLocationClearReceipt { farm_addr, cleared })
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
fn farm_enqueue_receipt(
    plan: FarmPublishPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> FarmEnqueueReceipt {
    FarmEnqueueReceipt {
        farm_addr: plan.farm_addr,
        expected_event_id: plan.expected_event_id,
        signed_event_id: enqueue.signed_event_id,
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state: enqueue.state.into(),
        idempotency_digest_prefix: Some(enqueue.idempotency_digest_prefix),
    }
}

#[cfg(feature = "runtime")]
fn farm_publish_plan(
    actor: &RadrootsActorContext,
    farm_value: RadrootsFarm,
    created_at: RadrootsSdkTimestamp,
) -> Result<FarmPublishPlan, RadrootsSdkError> {
    require_farmer_actor(actor, "farm.prepare_publish")?;
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let parts =
        farm::build_draft(&farm_value).map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("farm publish draft encode failed: {error}"),
        })?;
    let farm_addr = farm_addr(actor, farm_value.d_tag.as_str())
        .expect("validated farm d tag forms a farm address");
    let frozen_draft = RadrootsEventDraft::new(
        FARM_PROFILE_CONTRACT_ID,
        parts.kind,
        created_at_nostr.into(),
        parts.tags,
        parts.content,
        actor.pubkey().as_str(),
    )
    .expect("validated farm publish draft freezes");
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id_str())
        .expect("frozen farm draft produces a valid event id");
    Ok(FarmPublishPlan {
        farm_addr,
        expected_event_id,
        frozen_draft,
        created_at,
    })
}

#[cfg(feature = "runtime")]
fn require_farmer_actor(
    actor: &RadrootsActorContext,
    operation: &'static str,
) -> Result<(), RadrootsSdkError> {
    if actor.satisfies(RadrootsActorRole::Farmer) {
        Ok(())
    } else {
        Err(RadrootsSdkError::UnauthorizedActor {
            operation: operation.to_owned(),
            reason: "missing role Farmer".to_owned(),
        })
    }
}

#[cfg(feature = "runtime")]
fn farm_addr(
    actor: &RadrootsActorContext,
    d_tag: &str,
) -> Result<RadrootsAddressableCoordinate, RadrootsSdkError> {
    RadrootsAddressableCoordinate::parse(format!("{KIND_FARM}:{}:{d_tag}", actor.pubkey())).map_err(
        |error| RadrootsSdkError::InvalidRequest {
            message: format!("farm address is invalid: {error}"),
        },
    )
}

#[cfg(feature = "runtime")]
fn validate_exact_location(location: SdkExactLocation) -> Result<(), RadrootsSdkError> {
    if !location.latitude.is_finite()
        || !location.longitude.is_finite()
        || location.latitude < -90.0
        || location.latitude > 90.0
        || location.longitude < -180.0
        || location.longitude > 180.0
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm exact location coordinates are outside valid latitude/longitude bounds"
                .to_owned(),
        });
    }
    Ok(())
}

#[cfg(feature = "runtime")]
fn sdk_timestamp_ms(timestamp: RadrootsSdkTimestamp) -> Result<i64, RadrootsSdkError> {
    let seconds = timestamp.unix_seconds();
    let millis = seconds
        .checked_mul(1_000)
        .ok_or(RadrootsSdkError::TimestampOutOfRange { value: seconds })?;
    i64::try_from(millis).map_err(|_| RadrootsSdkError::TimestampOutOfRange { value: seconds })
}

#[cfg(feature = "runtime")]
fn nearest_geonames_locality(
    geocoder: &Geocoder,
    exact_location: SdkExactLocation,
) -> Result<GeocoderReverseResult, RadrootsSdkError> {
    let mut results = geocoder.reverse(
        GeocoderPoint {
            lat: exact_location.latitude,
            lng: exact_location.longitude,
        },
        Some(GeocoderReverseOptions {
            limit: 1,
            degree_offset: 0.5,
        }),
    )?;
    results.pop().ok_or_else(|| RadrootsSdkError::GeoNames {
        kind: crate::RadrootsSdkGeoNamesErrorKind::Lookup,
        message: "GeoNames reverse lookup returned no public locality".to_owned(),
    })
}

#[cfg(feature = "runtime")]
fn public_locality_from_reverse(
    exact_location: SdkExactLocation,
    reverse: &GeocoderReverseResult,
) -> Result<SdkPublicLocality, RadrootsSdkError> {
    let primary = required_public_string(reverse.name.as_str(), "GeoNames locality name")?;
    let country = optional_public_string(reverse.country_name.as_deref())
        .or_else(|| Some(reverse.country_id.clone()));
    Ok(SdkPublicLocality {
        primary: primary.clone(),
        city: Some(primary),
        region: optional_public_string(reverse.admin1_name.as_deref()),
        country,
        geohash5: geohash5(exact_location)?,
    })
}

#[cfg(feature = "runtime")]
fn public_locality_from_candidate(
    exact_location: SdkExactLocation,
    candidate: &GeocoderLocalityCandidate,
) -> Result<SdkPublicLocality, RadrootsSdkError> {
    let primary = required_public_string(candidate.name.as_str(), "GeoNames locality name")?;
    let country = optional_public_string(candidate.country_name.as_deref())
        .or_else(|| Some(candidate.country_id.clone()));
    Ok(SdkPublicLocality {
        primary: primary.clone(),
        city: Some(primary),
        region: optional_public_string(candidate.admin1_name.as_deref()),
        country,
        geohash5: geohash5(exact_location)?,
    })
}

#[cfg(feature = "runtime")]
fn exact_location_from_locality_candidate(
    candidate: &GeocoderLocalityCandidate,
) -> SdkExactLocation {
    SdkExactLocation {
        latitude: candidate.point.lat,
        longitude: candidate.point.lng,
    }
}

#[cfg(feature = "runtime")]
fn normalize_private_location_label(
    label: Option<String>,
) -> Result<Option<String>, RadrootsSdkError> {
    match label {
        Some(label) => {
            let label = label.trim();
            if label.is_empty() {
                return Err(RadrootsSdkError::InvalidRequest {
                    message: "farm private location label must not be empty".to_owned(),
                });
            }
            Ok(Some(label.to_owned()))
        }
        None => Ok(None),
    }
}

#[cfg(feature = "runtime")]
fn required_public_string(value: &str, label: &str) -> Result<String, RadrootsSdkError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RadrootsSdkError::GeoNames {
            kind: crate::RadrootsSdkGeoNamesErrorKind::Lookup,
            message: format!("{label} must not be empty"),
        });
    }
    Ok(trimmed.to_owned())
}

#[cfg(feature = "runtime")]
fn optional_public_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(feature = "runtime")]
fn geohash5(location: SdkExactLocation) -> Result<String, RadrootsSdkError> {
    validate_exact_location(location)?;
    let mut latitude_min = -90.0;
    let mut latitude_max = 90.0;
    let mut longitude_min = -180.0;
    let mut longitude_max = 180.0;
    let mut even = true;
    let mut bit_count = 0;
    let mut value = 0usize;
    let mut geohash = String::with_capacity(GEOHASH5_LEN);
    while geohash.len() < GEOHASH5_LEN {
        value <<= 1;
        if even {
            let middle = (longitude_min + longitude_max) / 2.0;
            if location.longitude >= middle {
                value |= 1;
                longitude_min = middle;
            } else {
                longitude_max = middle;
            }
        } else {
            let middle = (latitude_min + latitude_max) / 2.0;
            if location.latitude >= middle {
                value |= 1;
                latitude_min = middle;
            } else {
                latitude_max = middle;
            }
        }
        even = !even;
        bit_count += 1;
        if bit_count == 5 {
            geohash.push(GEOHASH_BASE32[value] as char);
            bit_count = 0;
            value = 0;
        }
    }
    Ok(geohash)
}

#[cfg(feature = "runtime")]
fn private_location_receipt_from_record(
    record: SdkPrivateFarmLocationRecord,
) -> FarmPrivateLocationReceipt {
    FarmPrivateLocationReceipt {
        farm_addr: record.farm_addr,
        farm_pubkey: record.farm_pubkey,
        farm_d_tag: record.farm_d_tag,
        label: record.label,
        exact_location: SdkExactLocation {
            latitude: record.latitude,
            longitude: record.longitude,
        },
        public_locality: SdkPublicLocality {
            primary: record.locality_primary,
            city: record.locality_city,
            region: record.locality_region,
            country: record.locality_country,
            geohash5: record.geohash5,
        },
        geonames_feature_id: record.geonames_feature_id,
        geonames_country_id: record.geonames_country_id,
        updated_at_ms: record.updated_at_ms,
    }
}

#[cfg(feature = "runtime")]
fn farm_private_location_lookup_receipt(
    farm_addr: RadrootsAddressableCoordinate,
    farm_pubkey: &str,
    farm_d_tag: String,
    input: FarmPrivateLocationInput,
    candidates: Vec<FarmPrivateLocationLookupCandidate>,
) -> FarmPrivateLocationLookupReceipt {
    FarmPrivateLocationLookupReceipt {
        farm_addr,
        farm_pubkey: farm_pubkey.to_owned(),
        farm_d_tag,
        input,
        candidates,
    }
}

#[cfg(feature = "runtime")]
fn farm_private_location_candidate(
    candidate: GeocoderLocalityCandidate,
) -> FarmPrivateLocationLookupCandidate {
    let exact_location = exact_location_from_locality_candidate(&candidate);
    let region = optional_public_string(candidate.admin1_name.as_deref());
    let country = optional_public_string(candidate.country_name.as_deref())
        .or_else(|| Some(candidate.country_id.clone()));
    FarmPrivateLocationLookupCandidate {
        geonames_feature_id: candidate.id,
        geonames_country_id: candidate.country_id,
        name: candidate.name,
        display_name: candidate.display_name,
        exact_location,
        region,
        country,
    }
}

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/farms_runtime_tests.rs"]
mod tests;
