#![forbid(unsafe_code)]

use crate::generated::nip09_reconciliation_manifest::{
    NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION,
    NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION,
    NIP09_RECONCILIATION_HOOK_ID as NIP09_HOOK_ID, NIP09_RECONCILIATION_MANIFEST_SHA256,
    NIP09_RECONCILIATION_VERSION,
};
use crate::migrations::{EVENT_STORE_MIGRATIONS, is_event_store_owned_table_name};
use crate::model::reconciliation_v1::{
    RadrootsEventAdmissionStatus, RadrootsEventIngest, RadrootsEventStoreSourceGeneration,
    RadrootsRawHeadDecision, StoredEventClass, tag_semantic_name, tag_value_type_name,
};
use crate::{
    RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1,
    RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1,
    RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1,
    RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1, RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1,
    RadrootsEventStoreError, RadrootsEventStoreSourceCapacityResourceV1,
};
use radroots_event::contract::registry_v7::RadrootsEventContract;
use radroots_event::envelope::{RadrootsEventEnvelope, RadrootsEventKindClass};
use radroots_event::event_head::v1::{
    RadrootsCurrentEventHead, RadrootsEventHeadCandidate, RadrootsEventHeadCandidateResult,
    RadrootsEventHeadCoordinate, RadrootsEventHeadDecision,
    event_head_candidate_for_nip01_event_v1, select_event_head_v1,
};
use radroots_event::ids::RadrootsNip01Coordinate;
use radroots_event_codec::admission::registry_v7::{
    RadrootsRegistryV7AdmissionDecision, admit_verified_event_registry_v7,
};
use radroots_event_codec::deletion::reconciliation_v1::admission::{
    RadrootsAdmittedNip09DeletionRequestEventV1, admit_verified_nip09_deletion_request_event_v1,
};
#[cfg(test)]
use radroots_event_codec::deletion::reconciliation_v1::evaluator::evaluate_nip09_suppression_from_borrowed_requests_v1;
use radroots_event_codec::deletion::reconciliation_v1::evaluator::{
    RadrootsNip09SuppressionOutcome, RadrootsNip09SuppressionReason,
};
use radroots_event_codec::verification::v1::RadrootsSignatureVerifiedEvent;
#[cfg(test)]
use sqlx::SqlitePool;
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

mod raw_source_rebuild;
#[cfg(test)]
mod result_vector_executor;
mod visibility_oracle_v1;

#[cfg(test)]
pub(crate) use raw_source_rebuild::{
    RawSourceRebuildFailpointV1, preserve_raw_source_rebuild_primary_failure_for_test,
    rebuild_from_raw_v1_in_transaction_for_test, rebuild_from_raw_v1_on_pool_for_test,
    rebuild_from_raw_v1_on_pool_with_caller_schema_limits_for_test,
};
pub(crate) use raw_source_rebuild::{
    rebuild_from_raw_v1_in_existing_transaction, rebuild_from_raw_v1_on_pool,
};

const RECONCILIATION_SNAPSHOT_BATCH_SIZE: i64 = 512;
const RECONCILIATION_SNAPSHOT_BATCH_LEN: usize = 512;
const RECONCILIATION_SNAPSHOT_BATCH_COUNT: u64 = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReconciliationCapacityLimits {
    pub(crate) raw_events: u64,
    pub(crate) raw_tags: u64,
    pub(crate) raw_event_bytes: u64,
    pub(crate) raw_tag_bytes: u64,
}

impl ReconciliationCapacityLimits {
    pub(crate) const fn production() -> Self {
        Self {
            raw_events: RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1,
            raw_tags: RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1,
            raw_event_bytes: RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1,
            raw_tag_bytes: RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1,
        }
    }

    pub(crate) const fn limit(self, resource: RadrootsEventStoreSourceCapacityResourceV1) -> u64 {
        match resource {
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents => self.raw_events,
            RadrootsEventStoreSourceCapacityResourceV1::RawTags => self.raw_tags,
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes => self.raw_event_bytes,
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes => self.raw_tag_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReconciliationCapacity {
    pub(crate) raw_events: u64,
    pub(crate) raw_tags: u64,
    pub(crate) raw_event_bytes: u64,
    pub(crate) raw_tag_bytes: u64,
}

impl ReconciliationCapacity {
    pub(crate) fn value(self, resource: RadrootsEventStoreSourceCapacityResourceV1) -> u64 {
        match resource {
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents => self.raw_events,
            RadrootsEventStoreSourceCapacityResourceV1::RawTags => self.raw_tags,
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes => self.raw_event_bytes,
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes => self.raw_tag_bytes,
        }
    }

    fn value_mut(&mut self, resource: RadrootsEventStoreSourceCapacityResourceV1) -> &mut u64 {
        match resource {
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents => &mut self.raw_events,
            RadrootsEventStoreSourceCapacityResourceV1::RawTags => &mut self.raw_tags,
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes => &mut self.raw_event_bytes,
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes => &mut self.raw_tag_bytes,
        }
    }

    fn checked_add(
        &mut self,
        limits: ReconciliationCapacityLimits,
        resource: RadrootsEventStoreSourceCapacityResourceV1,
        amount: u64,
    ) -> Result<(), RadrootsEventStoreError> {
        let limit = limits.limit(resource);
        let actual = self.value(resource).checked_add(amount).ok_or(
            RadrootsEventStoreError::SourceCapacityExceeded {
                resource,
                current: self.value(resource),
                requested: amount,
                limit,
            },
        )?;
        if actual > limit {
            return Err(RadrootsEventStoreError::SourceCapacityExceeded {
                resource,
                current: self.value(resource),
                requested: amount,
                limit,
            });
        }
        *self.value_mut(resource) = actual;
        Ok(())
    }

    fn validate(self, limits: ReconciliationCapacityLimits) -> Result<(), RadrootsEventStoreError> {
        for resource in [
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
            RadrootsEventStoreSourceCapacityResourceV1::RawTags,
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
        ] {
            let actual = self.value(resource);
            let limit = limits.limit(resource);
            if actual > limit {
                return Err(RadrootsEventStoreError::SourceCapacityExceeded {
                    resource,
                    current: actual,
                    requested: 0,
                    limit,
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReconciliationProfile {
    Nip09V1RegistryV7,
}

pub(crate) trait SourceGenerationProvider {
    fn fill_generation(&self, generation: &mut [u8; 32]) -> Result<(), RadrootsEventStoreError>;
}

pub(crate) struct OsSourceGenerationProvider;

impl SourceGenerationProvider for OsSourceGenerationProvider {
    fn fill_generation(&self, generation: &mut [u8; 32]) -> Result<(), RadrootsEventStoreError> {
        getrandom::getrandom(generation)
            .map_err(|_| RadrootsEventStoreError::SourceGenerationEntropyUnavailable)
    }
}

#[derive(Clone)]
pub(crate) struct EventAdmission {
    pub(crate) status: RadrootsEventAdmissionStatus,
    pub(crate) code: Option<String>,
    pub(crate) contract: Option<&'static RadrootsEventContract>,
}

impl EventAdmission {
    pub(crate) fn for_profile(
        profile: ReconciliationProfile,
        event: &RadrootsSignatureVerifiedEvent,
    ) -> Result<Self, RadrootsEventStoreError> {
        match profile {
            ReconciliationProfile::Nip09V1RegistryV7 => {
                Self::from_registry_v7(admit_verified_event_registry_v7(event))
            }
        }
    }

    fn from_registry_v7(
        decision: RadrootsRegistryV7AdmissionDecision,
    ) -> Result<Self, RadrootsEventStoreError> {
        Ok(match decision {
            RadrootsRegistryV7AdmissionDecision::Admitted { contract } => Self {
                status: RadrootsEventAdmissionStatus::Admitted,
                code: None,
                contract: Some(contract),
            },
            RadrootsRegistryV7AdmissionDecision::Unsupported { code } => Self {
                status: RadrootsEventAdmissionStatus::Unsupported,
                code: Some(code.to_owned()),
                contract: None,
            },
            RadrootsRegistryV7AdmissionDecision::Invalid { code } => Self {
                status: RadrootsEventAdmissionStatus::Invalid,
                code: Some(code.to_owned()),
                contract: None,
            },
            RadrootsRegistryV7AdmissionDecision::Defect { code } => {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!("registry-v7 admission defect `{code}`"),
                });
            }
        })
    }

    pub(crate) fn valid_stream_eligible(&self, kind_class: RadrootsEventKindClass) -> bool {
        self.status == RadrootsEventAdmissionStatus::Admitted
            && kind_class != RadrootsEventKindClass::Ephemeral
    }
}

#[derive(Clone)]
struct ReconciledEvent {
    seq: i64,
    inserted_at_ms: i64,
    verified_event: RadrootsSignatureVerifiedEvent,
    admission: EventAdmission,
}

struct StoredRawTag {
    tag_index: i64,
    tag_name: String,
    tag_value: Option<String>,
    tag_json: String,
}

struct ReconciliationSnapshot {
    events: Vec<ReconciledEvent>,
    capacity: ReconciliationCapacity,
}

#[derive(Clone)]
struct RawHeadWinner {
    candidate: RadrootsEventHeadCandidate,
    event_seq: i64,
    updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AddressableHeadState {
    kind: i64,
    pubkey: String,
    d_tag: String,
    raw_head_event_id: String,
    raw_head_event_seq: i64,
    raw_head_created_at: i64,
    admission_status: String,
    admission_code: Option<String>,
    contract_id: Option<String>,
    visibility: String,
    nip09_outcome: Option<String>,
    nip09_reason: Option<String>,
    event_reference_request_id: Option<String>,
    address_reference_request_id: Option<String>,
    address_reference_cutoff: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AddressableTransitionFact {
    transition_seq: i64,
    origin: String,
    kind: i64,
    pubkey: String,
    d_tag: String,
    raw_head_event_id: String,
    raw_head_event_seq: i64,
    raw_head_created_at: i64,
    visible_event_id: Option<String>,
    visible_event_seq: Option<i64>,
    retracted_event_id: Option<String>,
    retracted_event_seq: Option<i64>,
    admission_status: String,
    admission_code: Option<String>,
    contract_id: Option<String>,
    visibility: String,
    nip09_outcome: Option<String>,
    nip09_reason: Option<String>,
    event_reference_request_id: Option<String>,
    address_reference_request_id: Option<String>,
    address_reference_cutoff: Option<i64>,
    cause_event_seq: Option<i64>,
    cause_event_id: Option<String>,
    raw_head_decision: String,
}

#[derive(Clone)]
struct SourceState {
    generation: RadrootsEventStoreSourceGeneration,
    profile: ReconciliationProfile,
    raw_event_count: i64,
    raw_tag_count: i64,
    raw_high_water_seq: i64,
    last_transition_seq: i64,
    transition_floor_seq: i64,
    baseline_raw_event_count: i64,
    baseline_raw_tag_count: i64,
    baseline_raw_high_water_seq: i64,
}

struct SourceRebuildPlan {
    generation: RadrootsEventStoreSourceGeneration,
    generation_ordinal: i64,
    transition_floor_seq: i64,
    raw_event_count: i64,
    raw_tag_count: i64,
    raw_high_water_seq: i64,
    prior: Option<SourceState>,
}

struct SourceRebuildMarkerTokenV1 {
    generation: RadrootsEventStoreSourceGeneration,
}

struct RequestIndex {
    event_targets: BTreeMap<String, BTreeMap<String, String>>,
    address_targets: BTreeMap<(i64, String, String), AddressRequestEvidence>,
}

#[derive(Clone)]
struct IndexedRequestEvidence {
    request_id: String,
    created_at: u64,
}

#[derive(Default)]
struct AddressRequestEvidence {
    authorized: Option<IndexedRequestEvidence>,
    unauthorized: bool,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RequestFact {
    request_event_id: String,
    request_event_seq: i64,
    request_pubkey: String,
    request_created_at: i64,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EventTargetFact {
    request_event_id: String,
    target_event_id: String,
    source_tag_index: i64,
    source_tag_value: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct AddressTargetFact {
    request_event_id: String,
    target_kind: i64,
    target_pubkey: String,
    target_d_tag: String,
    inclusive_cutoff: i64,
    source_tag_index: i64,
    source_tag_value: String,
    source_kind_text: String,
    source_pubkey_text: String,
    source_d_tag: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RawHeadFact {
    coordinate_type: String,
    kind: i64,
    pubkey: String,
    d_tag: Option<String>,
    event_id: String,
    created_at: i64,
    updated_at_ms: i64,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EventCoordinateFact {
    event_id: String,
    event_seq: i64,
    coordinate_type: String,
    kind: i64,
    pubkey: String,
    created_at: i64,
    inserted_at_ms: i64,
    admission_status: String,
    admission_code: Option<String>,
    contract_id: Option<String>,
    raw_d_tag: String,
    nip09_matchable: i64,
    nip09_d_tag: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct StoredSuppressionDecision {
    outcome: RadrootsNip09SuppressionOutcome,
    reason: &'static str,
    event_reference_request_id: Option<String>,
    address_reference_request_id: Option<String>,
    address_reference_cutoff: Option<i64>,
}

impl RequestIndex {
    fn new(requests: &[RadrootsAdmittedNip09DeletionRequestEventV1]) -> Self {
        let mut index = Self {
            event_targets: BTreeMap::new(),
            address_targets: BTreeMap::new(),
        };
        for request in requests {
            index.insert(request);
        }
        index
    }

    fn insert(&mut self, request: &RadrootsAdmittedNip09DeletionRequestEventV1) {
        let request_event = request.event();
        let request_author = request_event.author_str();
        let request_id = request_event.id_str();
        for target in request.projection().event_targets() {
            self.event_targets
                .entry(target.event_id().as_str().to_owned())
                .or_default()
                .entry(request_author.to_owned())
                .and_modify(|current| {
                    if request_id < current.as_str() {
                        *current = request_id.to_owned();
                    }
                })
                .or_insert_with(|| request_id.to_owned());
        }
        for target in request.projection().address_targets() {
            let coordinate = (
                i64::from(target.coordinate().kind()),
                target.coordinate().pubkey().as_str().to_owned(),
                target.coordinate().identifier().to_owned(),
            );
            let evidence = self.address_targets.entry(coordinate.clone()).or_default();
            if request_author == coordinate.1 {
                let replace = evidence.authorized.as_ref().is_none_or(|current| {
                    request_event.created_at_u64() > current.created_at
                        || (request_event.created_at_u64() == current.created_at
                            && request_id < current.request_id.as_str())
                });
                if replace {
                    evidence.authorized = Some(IndexedRequestEvidence {
                        request_id: request_id.to_owned(),
                        created_at: request_event.created_at_u64(),
                    });
                }
            } else {
                evidence.unauthorized = true;
            }
        }
    }

    fn decision(
        &self,
        event: &RadrootsEventEnvelope,
    ) -> Result<StoredSuppressionDecision, RadrootsEventStoreError> {
        if event.kind_u32() == 5 {
            return Ok(StoredSuppressionDecision {
                outcome: RadrootsNip09SuppressionOutcome::Visible,
                reason: RadrootsNip09SuppressionReason::DeletionRequestImmune.code(),
                event_reference_request_id: None,
                address_reference_request_id: None,
                address_reference_cutoff: None,
            });
        }

        let (event_reference_request_id, unauthorized_event_reference) = self
            .event_targets
            .get(event.id_str())
            .map_or((None, false), |by_author| {
                let authorized = by_author.get(event.author_str()).cloned();
                let unauthorized = by_author.len() > usize::from(authorized.is_some());
                (authorized, unauthorized)
            });
        let address_evidence = nip01_coordinate_key(event)
            .as_ref()
            .and_then(|coordinate| self.address_targets.get(coordinate));
        let address_reference = address_evidence.and_then(|evidence| evidence.authorized.as_ref());
        let has_unauthorized_reference = unauthorized_event_reference
            || address_evidence.is_some_and(|evidence| evidence.unauthorized);
        let address_applies =
            address_reference.is_some_and(|evidence| event.created_at_u64() <= evidence.created_at);
        let (outcome, reason) = match (event_reference_request_id.is_some(), address_applies) {
            (true, true) => (
                RadrootsNip09SuppressionOutcome::Suppressed,
                RadrootsNip09SuppressionReason::EventIdAndAddressReference,
            ),
            (true, false) => (
                RadrootsNip09SuppressionOutcome::Suppressed,
                RadrootsNip09SuppressionReason::EventIdReference,
            ),
            (false, true) => (
                RadrootsNip09SuppressionOutcome::Suppressed,
                RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff,
            ),
            (false, false) if address_reference.is_some() => (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget,
            ),
            (false, false) if has_unauthorized_reference => (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::RequestAuthorMismatch,
            ),
            (false, false) => (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::NoAuthorizedReference,
            ),
        };
        Ok(StoredSuppressionDecision {
            outcome,
            reason: reason.code(),
            event_reference_request_id,
            address_reference_request_id: address_reference
                .map(|evidence| evidence.request_id.clone()),
            address_reference_cutoff: address_reference
                .map(|evidence| i64_from_u64("address_reference_cutoff", evidence.created_at))
                .transpose()?,
        })
    }
}

pub(crate) async fn validate_reconciliation_capacity(
    connection: &mut SqliteConnection,
    limits: ReconciliationCapacityLimits,
) -> Result<(), RadrootsEventStoreError> {
    measure_reconciliation_capacity_bounded(connection, limits)
        .await?
        .validate(limits)
}

fn reconciliation_capacity_value(
    resource: RadrootsEventStoreSourceCapacityResourceV1,
    value: i64,
) -> Result<u64, RadrootsEventStoreError> {
    u64::try_from(value).map_err(|_| RadrootsEventStoreError::MigrationHookStateDrift {
        hook_id: NIP09_HOOK_ID,
        reason: format!("measured {resource} is outside the unsigned capacity range: {value}"),
    })
}

pub(crate) async fn apply_reconciliation_hook(
    connection: &mut SqliteConnection,
    generation_provider: &dyn SourceGenerationProvider,
    limits: ReconciliationCapacityLimits,
) -> Result<(), RadrootsEventStoreError> {
    crate::source_maintenance_v1::preflight_source_generation_append_v1(connection).await?;
    validate_rebuild_marker_absent(connection).await?;
    validate_projection_cursor_authority(connection).await?;
    let snapshot = load_reconciliation_snapshot(connection, limits).await?;
    let events = snapshot.events;
    let raw_event_count = i64::try_from(snapshot.capacity.raw_events).map_err(|_| {
        RadrootsEventStoreError::MigrationHookStateDrift {
            hook_id: NIP09_HOOK_ID,
            reason: "raw event count exceeds SQLite integer range".to_owned(),
        }
    })?;
    let raw_tag_count = i64::try_from(snapshot.capacity.raw_tags).map_err(|_| {
        RadrootsEventStoreError::MigrationHookStateDrift {
            hook_id: NIP09_HOOK_ID,
            reason: "raw tag count exceeds SQLite integer range".to_owned(),
        }
    })?;
    let raw_high_water_seq = events.last().map(|event| event.seq).unwrap_or(0);
    let source_state_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_source_state")
            .fetch_one(&mut *connection)
            .await?;
    let prior = match source_state_count {
        0 => None,
        1 => {
            validate_applied_hook_state_with_events(connection, &events).await?;
            Some(read_source_state(connection).await?)
        }
        count => {
            return hook_drift(format!(
                "expected zero or one source state before rebuild, found {count}"
            ));
        }
    };

    let mut generation_bytes = [0_u8; 32];
    generation_provider.fill_generation(&mut generation_bytes)?;
    let generation = RadrootsEventStoreSourceGeneration::from_bytes(generation_bytes);
    let generation_exists: i64 = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM radroots_event_store_source_generation WHERE source_generation = ?)",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await?;
    if generation_exists != 0 {
        return hook_drift(
            "fresh source generation collided with existing generation history".to_owned(),
        );
    }

    let transition_floor_seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(transition_seq), 0) FROM radroots_event_store_addressable_head_transition",
    )
    .fetch_one(&mut *connection)
    .await?;
    let prior_generation_ordinal: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(generation_ordinal), 0) FROM radroots_event_store_source_generation",
    )
    .fetch_one(&mut *connection)
    .await?;
    let generation_ordinal =
        checked_authority_add(prior_generation_ordinal, 1, "source generation ordinal")?;
    if let Some(prior) = prior.as_ref()
        && (prior.last_transition_seq != transition_floor_seq
            || prior.raw_event_count != raw_event_count
            || prior.raw_tag_count != raw_tag_count
            || prior.raw_high_water_seq != raw_high_water_seq)
    {
        return hook_drift(
            "prior source authority does not bind the immutable rebuild baseline".to_owned(),
        );
    }
    let plan = SourceRebuildPlan {
        generation,
        generation_ordinal,
        transition_floor_seq,
        raw_event_count,
        raw_tag_count,
        raw_high_water_seq,
        prior,
    };

    let marker = open_source_rebuild_marker(connection, &plan).await?;
    append_source_generation(connection, &plan).await?;
    rotate_source_state(connection, &plan).await?;
    reconcile_raw_events(connection, &events).await?;
    persist_event_coordinate_facts(connection, plan.generation, &events).await?;
    rebuild_raw_heads(connection, &events).await?;
    let requests = persist_nip09_facts(connection, plan.generation, &events).await?;
    synchronize_addressable_heads(
        connection,
        plan.generation,
        &events,
        &requests,
        TransitionOrigin::Baseline,
        None,
        "baseline_rebuild",
    )
    .await?;
    update_source_authority(
        connection,
        plan.raw_event_count,
        plan.raw_tag_count,
        plan.raw_high_water_seq,
    )
    .await?;
    validate_rebuild_hook_state_with_events(connection, plan.generation, &events).await?;
    if crate::source_maintenance_v1::bind_source_capacity_to_generation_v1(
        connection,
        plan.generation,
    )
    .await?
    {
        crate::store::food_availability_projection_v1::apply_food_availability_projection_hook_v1(
            connection,
        )
        .await?;
    }
    close_source_rebuild_marker(connection, marker).await?;
    validate_sqlite_integrity_after_rebuild(connection).await?;
    validate_active_hook_state_fast(connection).await
}

async fn open_source_rebuild_marker(
    connection: &mut SqliteConnection,
    plan: &SourceRebuildPlan,
) -> Result<SourceRebuildMarkerTokenV1, RadrootsEventStoreError> {
    let prior_generation = plan
        .prior
        .as_ref()
        .map(|state| state.generation.as_bytes().as_slice());
    let inserted = sqlx::query(
        "INSERT INTO radroots_event_store_source_rebuild_marker(singleton, barrier_key, target_generation, target_generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq, prior_active_generation, prior_raw_event_count, prior_raw_tag_count, prior_raw_high_water_seq, prior_last_transition_seq) VALUES (1, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(plan.generation.as_bytes().as_slice())
    .bind(plan.generation_ordinal)
    .bind(NIP09_RECONCILIATION_VERSION)
    .bind(NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION)
    .bind(i64::from(
        NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION,
    ))
    .bind(NIP09_HOOK_ID)
    .bind(NIP09_RECONCILIATION_MANIFEST_SHA256)
    .bind(plan.transition_floor_seq)
    .bind(plan.raw_event_count)
    .bind(plan.raw_tag_count)
    .bind(plan.raw_high_water_seq)
    .bind(prior_generation)
    .bind(plan.prior.as_ref().map(|state| state.raw_event_count))
    .bind(plan.prior.as_ref().map(|state| state.raw_tag_count))
    .bind(plan.prior.as_ref().map(|state| state.raw_high_water_seq))
    .bind(plan.prior.as_ref().map(|state| state.last_transition_seq))
    .execute(&mut *connection)
    .await?;
    require_expected_insert(inserted.rows_affected(), "source rebuild marker")?;
    Ok(SourceRebuildMarkerTokenV1 {
        generation: plan.generation,
    })
}

async fn append_source_generation(
    connection: &mut SqliteConnection,
    plan: &SourceRebuildPlan,
) -> Result<(), RadrootsEventStoreError> {
    let inserted = sqlx::query(
        "INSERT INTO radroots_event_store_source_generation(source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(plan.generation.as_bytes().as_slice())
    .bind(plan.generation_ordinal)
    .bind(NIP09_RECONCILIATION_VERSION)
    .bind(NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION)
    .bind(i64::from(
        NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION,
    ))
    .bind(NIP09_HOOK_ID)
    .bind(NIP09_RECONCILIATION_MANIFEST_SHA256)
    .bind(plan.transition_floor_seq)
    .bind(plan.raw_event_count)
    .bind(plan.raw_tag_count)
    .bind(plan.raw_high_water_seq)
    .execute(&mut *connection)
    .await?;
    require_expected_insert(inserted.rows_affected(), "source generation")
}

async fn rotate_source_state(
    connection: &mut SqliteConnection,
    plan: &SourceRebuildPlan,
) -> Result<(), RadrootsEventStoreError> {
    let changed = if let Some(prior) = plan.prior.as_ref() {
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET active_generation = ?, raw_event_count = 0, raw_tag_count = 0, raw_high_water_seq = 0, last_transition_seq = ? WHERE singleton = 1 AND active_generation = ? AND raw_event_count = ? AND raw_tag_count = ? AND raw_high_water_seq = ? AND last_transition_seq = ?",
        )
        .bind(plan.generation.as_bytes().as_slice())
        .bind(plan.transition_floor_seq)
        .bind(prior.generation.as_bytes().as_slice())
        .bind(prior.raw_event_count)
        .bind(prior.raw_tag_count)
        .bind(prior.raw_high_water_seq)
        .bind(prior.last_transition_seq)
        .execute(&mut *connection)
        .await?
    } else {
        sqlx::query(
            "INSERT INTO radroots_event_store_source_state(singleton, active_generation, raw_event_count, raw_tag_count, raw_high_water_seq, last_transition_seq) VALUES (1, ?, 0, 0, 0, ?)",
        )
        .bind(plan.generation.as_bytes().as_slice())
        .bind(plan.transition_floor_seq)
        .execute(&mut *connection)
        .await?
    };
    if changed.rows_affected() != 1 {
        return hook_drift(format!(
            "source state rebuild transition affected {} rows",
            changed.rows_affected()
        ));
    }
    Ok(())
}

async fn close_source_rebuild_marker(
    connection: &mut SqliteConnection,
    marker: SourceRebuildMarkerTokenV1,
) -> Result<(), RadrootsEventStoreError> {
    let deleted = sqlx::query(
        "DELETE FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1 AND target_generation = ?",
    )
    .bind(marker.generation.as_bytes().as_slice())
    .execute(&mut *connection)
    .await?;
    if deleted.rows_affected() != 1 {
        return hook_drift(format!(
            "source rebuild marker close affected {} rows",
            deleted.rows_affected()
        ));
    }
    Ok(())
}

async fn validate_sqlite_integrity_after_rebuild(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let foreign_key_violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut *connection)
        .await?;
    for row in foreign_key_violations {
        let table: String = row.try_get("table")?;
        if is_event_store_owned_table_name(EVENT_STORE_MIGRATIONS, &table) {
            return Err(RadrootsEventStoreError::ForeignKeyViolation {
                table,
                rowid: row.try_get("rowid")?,
                parent: row.try_get("parent")?,
                foreign_key_index: row.try_get("fkid")?,
            });
        }
    }
    let integrity_rows = sqlx::query("PRAGMA integrity_check")
        .fetch_all(&mut *connection)
        .await?;
    if integrity_rows.len() != 1 || integrity_rows[0].try_get::<String, _>(0)?.as_str() != "ok" {
        return hook_drift("SQLite integrity validation failed after source rebuild".to_owned());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) async fn validate_applied_hook_state(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let snapshot =
        load_reconciliation_snapshot(connection, ReconciliationCapacityLimits::production())
            .await?;
    validate_applied_hook_state_with_events(connection, &snapshot.events).await
}

async fn validate_applied_hook_state_with_events(
    connection: &mut SqliteConnection,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    let state = validate_structural_hook_state(connection).await?;
    validate_hook_state_with_events(connection, &state, events).await
}

async fn validate_rebuild_hook_state_with_events(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    validate_active_rebuild_marker(connection, generation).await?;
    let state = validate_structural_source_state(connection).await?;
    if state.generation != generation {
        return hook_drift(
            "open rebuild marker target does not match active source generation".to_owned(),
        );
    }
    validate_hook_state_with_events(connection, &state, events).await
}

async fn validate_raw_source_rebuild_core_with_events_v1(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    validate_active_rebuild_marker(connection, generation).await?;
    let state = read_source_state(connection).await?;
    if state.generation != generation {
        return hook_drift(
            "open rebuild marker target does not match active source generation".to_owned(),
        );
    }
    validate_source_raw_authority_with_state(connection, &state).await?;
    validate_transition_interval_full(connection, &state).await?;
    validate_derived_event_storage(connection, events).await?;
    validate_raw_heads(connection, events).await?;
    validate_event_coordinate_facts(connection, state.generation, events).await?;
    let requests = validate_nip09_fact_graph(connection, state.generation, events).await?;
    validate_addressable_state(connection, state.generation, events, &requests).await?;
    validate_transition_history(connection, &state, events).await?;
    validate_latest_transitions_match_state(connection, state.generation).await
}

async fn validate_hook_state_with_events(
    connection: &mut SqliteConnection,
    state: &SourceState,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    validate_source_raw_authority_with_state(connection, state).await?;
    validate_transition_interval_full(connection, state).await?;
    validate_derived_event_storage(connection, events).await?;
    validate_raw_heads(connection, events).await?;
    validate_event_coordinate_facts(connection, state.generation, events).await?;
    let requests = validate_nip09_fact_graph(connection, state.generation, events).await?;
    validate_addressable_state(connection, state.generation, events, &requests).await?;
    validate_transition_history(connection, state, events).await?;
    validate_latest_transitions_match_state(connection, state.generation).await
}

pub(crate) async fn validate_active_hook_state_fast(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    // Supported writes are guarded transactionally. Reopen validates only
    // constant-cost authority bounds; full history/state checks remain part of
    // migration and rebuild audits, while cursor inventory is migration-only.
    validate_rebuild_marker_absent(connection).await?;
    validate_structural_source_state_fast(connection)
        .await
        .map(|_| ())
}

async fn validate_structural_hook_state(
    connection: &mut SqliteConnection,
) -> Result<SourceState, RadrootsEventStoreError> {
    validate_rebuild_marker_absent(connection).await?;
    validate_structural_source_state(connection).await
}

async fn validate_structural_source_state(
    connection: &mut SqliteConnection,
) -> Result<SourceState, RadrootsEventStoreError> {
    validate_projection_cursor_authority(connection).await?;
    validate_structural_source_state_fast(connection).await
}

async fn validate_structural_source_state_fast(
    connection: &mut SqliteConnection,
) -> Result<SourceState, RadrootsEventStoreError> {
    let rows = sqlx::query(
        "SELECT state.active_generation, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, state.last_transition_seq, generation.generation_ordinal, (SELECT MAX(candidate.generation_ordinal) FROM radroots_event_store_source_generation AS candidate) AS max_generation_ordinal, generation.reconciliation_version, generation.addressable_feed_version, generation.event_contract_registry_version, generation.hook_id, generation.hook_manifest_sha256, generation.transition_floor_seq, generation.baseline_raw_event_count, generation.baseline_raw_tag_count, generation.baseline_raw_high_water_seq FROM radroots_event_store_source_state AS state JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = state.active_generation WHERE state.singleton = 1",
    )
    .fetch_all(&mut *connection)
    .await?;
    if rows.len() != 1 {
        return hook_drift(format!(
            "expected one active source state, found {}",
            rows.len()
        ));
    }
    let row = &rows[0];
    let generation = generation_from_blob(row.try_get("active_generation")?)?;
    let profile = reconciliation_profile(
        row.try_get("reconciliation_version")?,
        row.try_get("addressable_feed_version")?,
        row.try_get("event_contract_registry_version")?,
        row.try_get::<String, _>("hook_id")?.as_str(),
        row.try_get::<String, _>("hook_manifest_sha256")?.as_str(),
    )?;
    let state = SourceState {
        generation,
        profile,
        raw_event_count: row.try_get("raw_event_count")?,
        raw_tag_count: row.try_get("raw_tag_count")?,
        raw_high_water_seq: row.try_get("raw_high_water_seq")?,
        last_transition_seq: row.try_get("last_transition_seq")?,
        transition_floor_seq: row.try_get("transition_floor_seq")?,
        baseline_raw_event_count: row.try_get("baseline_raw_event_count")?,
        baseline_raw_tag_count: row.try_get("baseline_raw_tag_count")?,
        baseline_raw_high_water_seq: row.try_get("baseline_raw_high_water_seq")?,
    };
    let generation_ordinal: i64 = row.try_get("generation_ordinal")?;
    let max_generation_ordinal: i64 = row.try_get("max_generation_ordinal")?;
    if generation_ordinal != max_generation_ordinal {
        return hook_drift("active generation contract metadata is inconsistent".to_owned());
    }
    if state.baseline_raw_event_count > state.raw_event_count
        || state.baseline_raw_tag_count > state.raw_tag_count
        || state.baseline_raw_high_water_seq > state.raw_high_water_seq
        || state.transition_floor_seq > state.last_transition_seq
    {
        return hook_drift("active generation baseline exceeds current authority".to_owned());
    }
    let actual_high_water: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) FROM event_envelopes")
            .fetch_one(&mut *connection)
            .await?;
    if actual_high_water != state.raw_high_water_seq {
        return hook_drift("raw high-water does not match active source authority".to_owned());
    }
    let first_transition_seq: Option<i64> = sqlx::query_scalar(
        "SELECT transition_seq FROM radroots_event_store_addressable_head_transition WHERE source_generation = ? ORDER BY transition_seq ASC LIMIT 1",
    )
    .bind(state.generation.as_bytes().as_slice())
    .fetch_optional(&mut *connection)
    .await?;
    let transition_high_water: Option<i64> = sqlx::query_scalar(
        "SELECT transition_seq FROM radroots_event_store_addressable_head_transition WHERE source_generation = ? ORDER BY transition_seq DESC LIMIT 1",
    )
    .bind(state.generation.as_bytes().as_slice())
    .fetch_optional(&mut *connection)
    .await?;
    let expected_count = state.last_transition_seq - state.transition_floor_seq;
    let expected_first = if expected_count > 0 {
        Some(checked_authority_add(
            state.transition_floor_seq,
            1,
            "first transition sequence",
        )?)
    } else {
        None
    };
    let expected_last = (expected_count > 0).then_some(state.last_transition_seq);
    if expected_count < 0
        || first_transition_seq != expected_first
        || transition_high_water != expected_last
    {
        return hook_drift(format!(
            "active transition bounds are inconsistent: floor={}, last={}, first={first_transition_seq:?}, high-water={transition_high_water:?}",
            state.transition_floor_seq, state.last_transition_seq
        ));
    }

    Ok(state)
}

async fn validate_rebuild_marker_absent(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let marker_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker")
            .fetch_one(&mut *connection)
            .await?;
    if marker_count != 0 {
        return hook_drift(format!(
            "source rebuild marker residue is present outside reconciliation: {marker_count} row(s)"
        ));
    }
    Ok(())
}

async fn validate_active_rebuild_marker(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
) -> Result<(), RadrootsEventStoreError> {
    let valid_marker_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker AS marker JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = marker.target_generation JOIN radroots_event_store_source_state AS state ON state.singleton = marker.singleton AND state.active_generation = marker.target_generation WHERE marker.singleton = 1 AND marker.barrier_key = 1 AND marker.target_generation = ? AND marker.target_generation_ordinal = generation.generation_ordinal AND marker.reconciliation_version = generation.reconciliation_version AND marker.addressable_feed_version = generation.addressable_feed_version AND marker.event_contract_registry_version = generation.event_contract_registry_version AND marker.hook_id = generation.hook_id AND marker.hook_manifest_sha256 = generation.hook_manifest_sha256 AND marker.transition_floor_seq = generation.transition_floor_seq AND marker.baseline_raw_event_count = generation.baseline_raw_event_count AND marker.baseline_raw_tag_count = generation.baseline_raw_tag_count AND marker.baseline_raw_high_water_seq = generation.baseline_raw_high_water_seq AND state.raw_event_count = marker.baseline_raw_event_count AND state.raw_tag_count = marker.baseline_raw_tag_count AND state.raw_high_water_seq = marker.baseline_raw_high_water_seq",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await?;
    if valid_marker_count != 1 {
        return hook_drift(
            "open source rebuild marker does not bind completed active authority".to_owned(),
        );
    }
    Ok(())
}

async fn validate_projection_cursor_authority(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let probe_limit = i64::from(RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1) + 1;
    let cursor_probe = sqlx::query("SELECT 1 FROM projection_cursor LIMIT ?")
        .bind(probe_limit)
        .fetch_all(&mut *connection)
        .await?;
    validate_projection_cursor_cardinality_v1(cursor_probe.len())?;
    let identity_probe =
        sqlx::query("SELECT 1 FROM radroots_event_store_projection_cursor_source LIMIT ?")
            .bind(probe_limit)
            .fetch_all(&mut *connection)
            .await?;
    validate_projection_cursor_cardinality_v1(identity_probe.len())?;

    let raw_high_water: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) FROM event_envelopes")
            .fetch_one(&mut *connection)
            .await?;
    let invalid: Option<i64> = sqlx::query_scalar(
        "SELECT 1
         FROM projection_cursor AS cursor
         LEFT JOIN radroots_event_store_projection_cursor_source AS source
           ON source.projection_id = cursor.projection_id
         WHERE typeof(cursor.projection_id) != 'text'
            OR length(cursor.projection_id) = 0
            OR typeof(cursor.projection_version) != 'integer'
            OR cursor.projection_version < 1
            OR cursor.projection_version > 4294967295
            OR typeof(cursor.last_event_seq) != 'integer'
            OR cursor.last_event_seq < 0
            OR cursor.last_event_seq > ?
            OR typeof(cursor.updated_at_ms) != 'integer'
            OR source.projection_id IS NULL
            OR typeof(source.source_revision) != 'integer'
            OR source.source_revision <= 0
            OR source.source_revision >= 9223372036854775807
            OR (
              source.source_generation IS NOT NULL
              AND (
                typeof(source.source_generation) != 'blob'
                OR length(source.source_generation) != 32
              )
            )
         LIMIT 1",
    )
    .bind(raw_high_water)
    .fetch_optional(&mut *connection)
    .await?;
    if invalid.is_some() {
        return hook_drift(
            "a projection cursor identity is invalid or ahead of raw source authority".to_owned(),
        );
    }
    let orphan_identity: Option<i64> = sqlx::query_scalar(
        "SELECT 1
         FROM radroots_event_store_projection_cursor_source AS source
         LEFT JOIN projection_cursor AS cursor
           ON cursor.projection_id = source.projection_id
         WHERE cursor.projection_id IS NULL
         LIMIT 1",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if orphan_identity.is_some() {
        return hook_drift("a projection cursor source identity has no cursor".to_owned());
    }
    Ok(())
}

pub(crate) async fn preflight_projection_cursor_insert_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let rows = sqlx::query("SELECT 1 FROM projection_cursor LIMIT ?")
        .bind(i64::from(
            RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1,
        ))
        .fetch_all(&mut *connection)
        .await?;
    if rows.len()
        >= usize::try_from(RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1)
            .unwrap_or(usize::MAX)
    {
        return Err(RadrootsEventStoreError::ProjectionCursorCapacityExceeded {
            current: RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1,
            limit: RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1,
        });
    }
    Ok(())
}

fn validate_projection_cursor_cardinality_v1(
    observed: usize,
) -> Result<(), RadrootsEventStoreError> {
    let limit = RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1;
    let current = u32::try_from(observed).unwrap_or(u32::MAX);
    if current > limit {
        return Err(RadrootsEventStoreError::ProjectionCursorCapacityExceeded { current, limit });
    }
    Ok(())
}

async fn validate_transition_interval_full(
    connection: &mut SqliteConnection,
    state: &SourceState,
) -> Result<(), RadrootsEventStoreError> {
    let transition_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition WHERE source_generation = ?",
    )
    .bind(state.generation.as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await?;
    let expected_count = state.last_transition_seq - state.transition_floor_seq;
    let foreign_transition_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition WHERE transition_seq > ? AND transition_seq <= ? AND source_generation != ?",
    )
    .bind(state.transition_floor_seq)
    .bind(state.last_transition_seq)
    .bind(state.generation.as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await?;
    let pre_floor_active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition WHERE source_generation = ? AND transition_seq <= ?",
    )
    .bind(state.generation.as_bytes().as_slice())
    .bind(state.transition_floor_seq)
    .fetch_one(&mut *connection)
    .await?;
    if expected_count < 0
        || transition_count != expected_count
        || foreign_transition_count != 0
        || pre_floor_active_count != 0
    {
        return hook_drift(format!(
            "active transition interval is not contiguous: floor={}, last={}, count={}, foreign={foreign_transition_count}, pre-floor={pre_floor_active_count}",
            state.transition_floor_seq, state.last_transition_seq, transition_count
        ));
    }
    Ok(())
}

pub(crate) async fn validate_source_raw_authority(
    connection: &mut SqliteConnection,
) -> Result<ReconciliationProfile, RadrootsEventStoreError> {
    validate_rebuild_marker_absent(connection).await?;
    let state = read_source_state(connection).await?;
    let actual_high_water: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) FROM event_envelopes")
            .fetch_one(&mut *connection)
            .await?;
    if actual_high_water != state.raw_high_water_seq {
        validate_source_raw_authority_with_state(connection, &state).await?;
        return hook_drift("raw high-water drift was not explained by raw counts".to_owned());
    }
    Ok(state.profile)
}

pub(crate) async fn synchronize_after_insert(
    connection: &mut SqliteConnection,
    ingest: &RadrootsEventIngest,
    admission: &EventAdmission,
    inserted_seq: i64,
    inserted_event_id: &str,
    inserted_tag_count: usize,
    raw_head_decision: &RadrootsRawHeadDecision,
) -> Result<(), RadrootsEventStoreError> {
    validate_rebuild_marker_absent(connection).await?;
    if inserted_seq == i64::MAX {
        return hook_drift(
            "raw source sequence space is exhausted at SQLite INTEGER maximum".to_owned(),
        );
    }
    let prior = read_source_state(connection).await?;
    let actual_high_water: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) FROM event_envelopes")
            .fetch_one(&mut *connection)
            .await?;
    let actual_inserted_tag_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM event_envelope_tags WHERE event_id = ?")
            .bind(inserted_event_id)
            .fetch_one(&mut *connection)
            .await?;
    let inserted_tag_count = i64::try_from(inserted_tag_count).map_err(|_| {
        RadrootsEventStoreError::MigrationHookStateDrift {
            hook_id: NIP09_HOOK_ID,
            reason: "inserted tag count exceeds SQLite integer range".to_owned(),
        }
    })?;
    if actual_inserted_tag_count != inserted_tag_count
        || inserted_seq <= prior.raw_high_water_seq
        || actual_high_water != inserted_seq
    {
        let expected = SourceState {
            generation: prior.generation,
            profile: prior.profile,
            raw_event_count: checked_authority_add(prior.raw_event_count, 1, "raw event count")?,
            raw_tag_count: checked_authority_add(
                prior.raw_tag_count,
                inserted_tag_count,
                "raw tag count",
            )?,
            raw_high_water_seq: inserted_seq,
            last_transition_seq: prior.last_transition_seq,
            transition_floor_seq: prior.transition_floor_seq,
            baseline_raw_event_count: prior.baseline_raw_event_count,
            baseline_raw_tag_count: prior.baseline_raw_tag_count,
            baseline_raw_high_water_seq: prior.baseline_raw_high_water_seq,
        };
        validate_source_raw_authority_with_state(connection, &expected).await?;
        return hook_drift(
            "pending raw append failed its indexed envelope/tag delta checks".to_owned(),
        );
    }
    let next_event_count = checked_authority_add(prior.raw_event_count, 1, "raw event count")?;
    let next_tag_count =
        checked_authority_add(prior.raw_tag_count, inserted_tag_count, "raw tag count")?;
    match prior.profile {
        ReconciliationProfile::Nip09V1RegistryV7 => {
            synchronize_insert_delta(
                connection,
                prior.generation,
                ingest,
                admission,
                inserted_seq,
                inserted_event_id,
                raw_head_decision,
            )
            .await?;
        }
    }
    update_source_authority(connection, next_event_count, next_tag_count, inserted_seq).await
}

pub(crate) async fn persist_event_coordinate_after_insert(
    connection: &mut SqliteConnection,
    ingest: &RadrootsEventIngest,
    admission: &EventAdmission,
    inserted_seq: i64,
) -> Result<(), RadrootsEventStoreError> {
    validate_rebuild_marker_absent(connection).await?;
    let generation = active_source_generation(connection).await?;
    persist_event_coordinate_fact(
        connection,
        generation,
        &ReconciledEvent {
            seq: inserted_seq,
            inserted_at_ms: ingest.observed_at_ms(),
            verified_event: ingest.verified_event().clone(),
            admission: admission.clone(),
        },
    )
    .await
}

pub(crate) async fn active_source_generation(
    connection: &mut SqliteConnection,
) -> Result<RadrootsEventStoreSourceGeneration, RadrootsEventStoreError> {
    read_source_state(connection)
        .await
        .map(|state| state.generation)
}

async fn stored_suppression_decision(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    target: &EventCoordinateFact,
) -> Result<StoredSuppressionDecision, RadrootsEventStoreError> {
    if target.kind == 5 {
        return Ok(StoredSuppressionDecision {
            outcome: RadrootsNip09SuppressionOutcome::Visible,
            reason: "deletion_request_immune",
            event_reference_request_id: None,
            address_reference_request_id: None,
            address_reference_cutoff: None,
        });
    }
    let event_reference_request_id = sqlx::query_scalar::<_, String>(
        "SELECT request.request_event_id
         FROM radroots_event_store_nip09_event_target AS target
         JOIN radroots_event_store_nip09_request AS request
           ON request.source_generation = target.source_generation
          AND request.request_event_id = target.request_event_id
         WHERE target.source_generation = ?
           AND target.target_event_id = ?
           AND request.request_pubkey = ?
         ORDER BY request.request_event_id
         LIMIT 1",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(target.event_id.as_str())
    .bind(target.pubkey.as_str())
    .fetch_optional(&mut *connection)
    .await?;
    let address_reference = sqlx::query(
        "SELECT request.request_event_id, target.inclusive_cutoff
         FROM radroots_event_store_event_coordinate AS coordinate
         JOIN radroots_event_store_nip09_address_target AS target
           ON target.source_generation = coordinate.source_generation
          AND target.target_kind = coordinate.kind
          AND target.target_pubkey = coordinate.pubkey
          AND target.target_d_tag = coordinate.nip09_d_tag
         JOIN radroots_event_store_nip09_request AS request
           ON request.source_generation = target.source_generation
          AND request.request_event_id = target.request_event_id
         WHERE coordinate.source_generation = ?
           AND coordinate.event_id = ?
           AND coordinate.nip09_matchable = 1
           AND request.request_pubkey = coordinate.pubkey
         ORDER BY target.inclusive_cutoff DESC, request.request_event_id
         LIMIT 1",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(target.event_id.as_str())
    .fetch_optional(&mut *connection)
    .await?;
    let (address_reference_request_id, address_reference_cutoff) =
        if let Some(row) = address_reference {
            (
                Some(row.try_get("request_event_id")?),
                Some(row.try_get("inclusive_cutoff")?),
            )
        } else {
            (None, None)
        };
    let has_unauthorized_reference: i64 = sqlx::query_scalar(
        "SELECT
           EXISTS (
             SELECT 1
             FROM radroots_event_store_nip09_event_target AS target
             JOIN radroots_event_store_nip09_request AS request
               ON request.source_generation = target.source_generation
              AND request.request_event_id = target.request_event_id
             WHERE target.source_generation = ?
               AND target.target_event_id = ?
               AND request.request_pubkey != ?
           )
           OR EXISTS (
             SELECT 1
             FROM radroots_event_store_event_coordinate AS coordinate
             JOIN radroots_event_store_nip09_address_target AS target
               ON target.source_generation = coordinate.source_generation
              AND target.target_kind = coordinate.kind
              AND target.target_pubkey = coordinate.pubkey
              AND target.target_d_tag = coordinate.nip09_d_tag
             JOIN radroots_event_store_nip09_request AS request
               ON request.source_generation = target.source_generation
              AND request.request_event_id = target.request_event_id
             WHERE coordinate.source_generation = ?
               AND coordinate.event_id = ?
               AND coordinate.nip09_matchable = 1
               AND request.request_pubkey != coordinate.pubkey
           )",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(target.event_id.as_str())
    .bind(target.pubkey.as_str())
    .bind(generation.as_bytes().as_slice())
    .bind(target.event_id.as_str())
    .fetch_one(&mut *connection)
    .await?;
    let address_applies =
        address_reference_cutoff.is_some_and(|cutoff| target.created_at <= cutoff);
    let (outcome, reason) = match (event_reference_request_id.is_some(), address_applies) {
        (true, true) => (
            RadrootsNip09SuppressionOutcome::Suppressed,
            "deletion_event_id_and_address_reference",
        ),
        (true, false) => (
            RadrootsNip09SuppressionOutcome::Suppressed,
            "deletion_event_id_reference",
        ),
        (false, true) => (
            RadrootsNip09SuppressionOutcome::Suppressed,
            "deletion_address_reference",
        ),
        (false, false) if address_reference_request_id.is_some() => (
            RadrootsNip09SuppressionOutcome::Visible,
            "deletion_address_cutoff_precedes_target",
        ),
        (false, false) if has_unauthorized_reference != 0 => (
            RadrootsNip09SuppressionOutcome::Visible,
            "deletion_request_author_mismatch",
        ),
        (false, false) => (
            RadrootsNip09SuppressionOutcome::Visible,
            "deletion_no_authorized_reference",
        ),
    };
    Ok(StoredSuppressionDecision {
        outcome,
        reason,
        event_reference_request_id,
        address_reference_request_id,
        address_reference_cutoff,
    })
}

async fn reconcile_raw_events(
    connection: &mut SqliteConnection,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    for event in events {
        let envelope = event.verified_event.event();
        let event_class = StoredEventClass::from_event_kind_class(envelope.kind_class());
        let valid_stream_eligible = event.admission.valid_stream_eligible(envelope.kind_class());
        sqlx::query(
            "UPDATE event_envelopes SET verification_status = 'verified', contract_status = ?, contract_id = ?, event_class = ?, projection_eligible = ?, updated_at_ms = ? WHERE seq = ?",
        )
        .bind(event.admission.status.as_str())
        .bind(event.admission.contract.map(|contract| contract.id))
        .bind(event_class.as_str())
        .bind(i64::from(valid_stream_eligible))
        .bind(event.inserted_at_ms)
        .bind(event.seq)
        .execute(&mut *connection)
        .await?;
        update_derived_tags(connection, event).await?;
    }
    Ok(())
}

async fn load_reconciliation_snapshot(
    connection: &mut SqliteConnection,
    limits: ReconciliationCapacityLimits,
) -> Result<ReconciliationSnapshot, RadrootsEventStoreError> {
    let measured_capacity = measure_reconciliation_capacity_bounded(connection, limits).await?;
    let mut loaded_capacity = ReconciliationCapacity::default();
    let mut tags_by_event = BTreeMap::<String, Vec<StoredRawTag>>::new();
    let mut next_tag_rowid = i64::MIN;
    loop {
        let rows = sqlx::query(
            "SELECT rowid AS tag_rowid, event_id, tag_index, tag_name, tag_value, tag_json FROM event_envelope_tags WHERE rowid >= ? ORDER BY rowid LIMIT ?",
        )
        .bind(next_tag_rowid)
        .bind(RECONCILIATION_SNAPSHOT_BATCH_SIZE)
        .fetch_all(&mut *connection)
        .await?;
        let row_count = rows.len();
        for row in rows {
            let tag_rowid: i64 = row.try_get("tag_rowid")?;
            let event_id: String = row.try_get("event_id")?;
            let tag_index: i64 = row.try_get("tag_index")?;
            let tag_name: String = row.try_get("tag_name")?;
            let tag_value: Option<String> = row.try_get("tag_value")?;
            let tag_json: String = row.try_get("tag_json")?;
            loaded_capacity.checked_add(
                limits,
                RadrootsEventStoreSourceCapacityResourceV1::RawTags,
                1,
            )?;
            loaded_capacity.checked_add(
                limits,
                RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
                text_payload_bytes(
                    RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
                    limits,
                    [
                        event_id.len(),
                        tag_name.len(),
                        tag_value.as_ref().map_or(0, String::len),
                        tag_json.len(),
                    ],
                )?,
            )?;
            next_tag_rowid = tag_rowid.checked_add(1).ok_or_else(|| {
                RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: NIP09_HOOK_ID,
                    reason: "raw tag rowid exhausts bounded snapshot pagination".to_owned(),
                }
            })?;
            tags_by_event
                .entry(event_id)
                .or_default()
                .push(StoredRawTag {
                    tag_index,
                    tag_name,
                    tag_value,
                    tag_json,
                });
        }
        if row_count < RECONCILIATION_SNAPSHOT_BATCH_LEN {
            break;
        }
    }
    for tags in tags_by_event.values_mut() {
        tags.sort_unstable_by_key(|tag| tag.tag_index);
    }

    let mut events = Vec::new();
    let mut next_event_seq = i64::MIN;
    loop {
        let rows = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, inserted_at_ms FROM event_envelopes WHERE seq >= ? ORDER BY seq LIMIT ?",
        )
        .bind(next_event_seq)
        .bind(RECONCILIATION_SNAPSHOT_BATCH_SIZE)
        .fetch_all(&mut *connection)
        .await?;
        let row_count = rows.len();
        for row in rows {
            let seq: i64 = row.try_get("seq")?;
            let event_id: String = row.try_get("event_id")?;
            let pubkey: String = row.try_get("pubkey")?;
            let stored_tags_json: String = row.try_get("tags_json")?;
            let content: String = row.try_get("content")?;
            let sig: String = row.try_get("sig")?;
            let raw_json: String = row.try_get("raw_json")?;
            loaded_capacity.checked_add(
                limits,
                RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                1,
            )?;
            loaded_capacity.checked_add(
                limits,
                RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
                text_payload_bytes(
                    RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
                    limits,
                    [
                        event_id.len(),
                        pubkey.len(),
                        stored_tags_json.len(),
                        content.len(),
                        sig.len(),
                        raw_json.len(),
                    ],
                )?,
            )?;
            if seq <= 0 {
                return hook_drift(format!(
                    "raw source event `{event_id}` has nonpositive sequence {seq}"
                ));
            }
            next_event_seq = seq.checked_add(1).ok_or_else(|| {
                RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: NIP09_HOOK_ID,
                    reason: format!(
                        "raw source event `{event_id}` exhausts the SQLite INTEGER sequence space"
                    ),
                }
            })?;
            let inserted_at_ms: i64 = row.try_get("inserted_at_ms")?;
            let ingest =
                RadrootsEventIngest::from_raw_json_reconciliation_v1(raw_json, inserted_at_ms)
                    .map_err(|_| raw_mismatch(event_id.as_str(), "raw_json"))?;
            let event = ingest.event();
            compare_raw_field(event.id_str() == event_id, event_id.as_str(), "event_id")?;
            compare_raw_field(event.author_str() == pubkey, event_id.as_str(), "pubkey")?;
            let created_at: i64 = row.try_get("created_at")?;
            compare_raw_field(
                i64::try_from(event.created_at_u64()).ok() == Some(created_at),
                event_id.as_str(),
                "created_at",
            )?;
            compare_raw_field(
                i64::from(event.kind_u32()) == row.try_get::<i64, _>("kind")?,
                event_id.as_str(),
                "kind",
            )?;
            let tags_json = serde_json::to_string(&event.tags_as_vec())?;
            compare_raw_field(
                tags_json == stored_tags_json,
                event_id.as_str(),
                "tags_json",
            )?;
            compare_raw_field(event.content() == content, event_id.as_str(), "content")?;
            compare_raw_field(event.sig_str() == sig, event_id.as_str(), "sig")?;
            compare_raw_tags(
                event_id.as_str(),
                event.tags_as_vec().as_slice(),
                tags_by_event.remove(event_id.as_str()).unwrap_or_default(),
            )?;
            let admission = EventAdmission::for_profile(
                ReconciliationProfile::Nip09V1RegistryV7,
                ingest.verified_event(),
            )?;
            events.push(ReconciledEvent {
                seq,
                inserted_at_ms,
                verified_event: ingest.verified_event().clone(),
                admission,
            });
        }
        if row_count < RECONCILIATION_SNAPSHOT_BATCH_LEN {
            break;
        }
    }
    if let Some(event_id) = tags_by_event.keys().next() {
        return Err(raw_mismatch(event_id, "tag_rows"));
    }
    if loaded_capacity != measured_capacity {
        return hook_drift(
            "raw source changed while the bounded reconciliation snapshot was loaded".to_owned(),
        );
    }
    Ok(ReconciliationSnapshot {
        events,
        capacity: loaded_capacity,
    })
}

fn compare_raw_tags(
    event_id: &str,
    tags: &[Vec<String>],
    rows: Vec<StoredRawTag>,
) -> Result<(), RadrootsEventStoreError> {
    if rows.len() != tags.len() {
        return Err(raw_mismatch(event_id, "tag_rows"));
    }
    for (index, (row, tag)) in rows.into_iter().zip(tags).enumerate() {
        let expected_index =
            i64::try_from(index).map_err(|_| raw_mismatch(event_id, "tag_index"))?;
        let expected_name = tag.first().map(String::as_str).unwrap_or("");
        let expected_value = tag.get(1).map(String::as_str);
        let expected_json = serde_json::to_string(tag)?;
        if row.tag_index != expected_index
            || row.tag_name != expected_name
            || row.tag_value.as_deref() != expected_value
            || row.tag_json != expected_json
        {
            return Err(raw_mismatch(event_id, "tag_rows"));
        }
    }
    Ok(())
}

pub(crate) async fn measure_reconciliation_capacity_bounded(
    connection: &mut SqliteConnection,
    limits: ReconciliationCapacityLimits,
) -> Result<ReconciliationCapacity, RadrootsEventStoreError> {
    let mut capacity = ReconciliationCapacity::default();
    let mut next_event_seq = i64::MIN;
    loop {
        let (page_size, page_len) =
            bounded_capacity_page_len(capacity.raw_events, limits.raw_events);
        let rows = sqlx::query(
            "SELECT seq, length(CAST(event_id AS BLOB)) + length(CAST(pubkey AS BLOB)) + length(CAST(tags_json AS BLOB)) + length(CAST(content AS BLOB)) + length(CAST(sig AS BLOB)) + length(CAST(raw_json AS BLOB)) AS raw_bytes FROM event_envelopes WHERE seq >= ? ORDER BY seq LIMIT ?",
        )
        .bind(next_event_seq)
        .bind(page_size)
        .fetch_all(&mut *connection)
        .await?;
        let row_count = rows.len();
        for row in rows {
            let seq: i64 = row.try_get("seq")?;
            capacity.checked_add(
                limits,
                RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                1,
            )?;
            capacity.checked_add(
                limits,
                RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
                reconciliation_capacity_value(
                    RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
                    row.try_get("raw_bytes")?,
                )?,
            )?;
            next_event_seq = seq.checked_add(1).ok_or_else(|| {
                RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: NIP09_HOOK_ID,
                    reason: format!(
                        "raw source event sequence {seq} exhausts bounded snapshot pagination"
                    ),
                }
            })?;
        }
        if row_count < page_len {
            break;
        }
    }

    let mut next_tag_rowid = i64::MIN;
    loop {
        let (page_size, page_len) = bounded_capacity_page_len(capacity.raw_tags, limits.raw_tags);
        let rows = sqlx::query(
            "SELECT rowid AS tag_rowid, length(CAST(event_id AS BLOB)) + length(CAST(tag_name AS BLOB)) + COALESCE(length(CAST(tag_value AS BLOB)), 0) + length(CAST(tag_json AS BLOB)) AS raw_bytes FROM event_envelope_tags WHERE rowid >= ? ORDER BY rowid LIMIT ?",
        )
        .bind(next_tag_rowid)
        .bind(page_size)
        .fetch_all(&mut *connection)
        .await?;
        let row_count = rows.len();
        for row in rows {
            capacity.checked_add(
                limits,
                RadrootsEventStoreSourceCapacityResourceV1::RawTags,
                1,
            )?;
            capacity.checked_add(
                limits,
                RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
                reconciliation_capacity_value(
                    RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
                    row.try_get("raw_bytes")?,
                )?,
            )?;
            let tag_rowid: i64 = row.try_get("tag_rowid")?;
            next_tag_rowid = tag_rowid.checked_add(1).ok_or_else(|| {
                RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: NIP09_HOOK_ID,
                    reason: "raw tag rowid exhausts bounded capacity pagination".to_owned(),
                }
            })?;
        }
        if row_count < page_len {
            break;
        }
    }
    Ok(capacity)
}

fn bounded_capacity_page_len(current: u64, limit: u64) -> (i64, usize) {
    let page_count = limit
        .saturating_sub(current)
        .saturating_add(1)
        .min(RECONCILIATION_SNAPSHOT_BATCH_COUNT);
    (
        i64::try_from(page_count).unwrap_or(RECONCILIATION_SNAPSHOT_BATCH_SIZE),
        usize::try_from(page_count).unwrap_or(RECONCILIATION_SNAPSHOT_BATCH_LEN),
    )
}

async fn update_derived_tags(
    connection: &mut SqliteConnection,
    event: &ReconciledEvent,
) -> Result<(), RadrootsEventStoreError> {
    for (index, tag) in event.verified_event.event().tag_slices().iter().enumerate() {
        let name = tag.as_slice().first().map(String::as_str).unwrap_or("");
        let contract_tag = event.admission.contract.and_then(|contract| {
            contract
                .tags
                .iter()
                .find(|candidate| candidate.name == name)
        });
        let tag_index = i64::try_from(index)
            .map_err(|_| raw_mismatch(event.verified_event.event().id_str(), "tag_index"))?;
        sqlx::query(
            "UPDATE event_envelope_tags SET contract_semantic = ?, contract_value_type = ?, relay_indexed = ? WHERE event_id = ? AND tag_index = ?",
        )
        .bind(contract_tag.map(|tag| tag_semantic_name(tag.semantic)))
        .bind(contract_tag.map(|tag| tag_value_type_name(tag.value_type)))
        .bind(i64::from(
            contract_tag.map(|tag| tag.relay_indexed).unwrap_or(false),
        ))
        .bind(event.verified_event.event().id_str())
        .bind(tag_index)
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

async fn validate_derived_event_storage(
    connection: &mut SqliteConnection,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    let mut next_event_seq = i64::MIN;
    let mut event_index = 0_usize;
    loop {
        let rows = sqlx::query(
            "SELECT seq, verification_status, contract_status, contract_id, event_class, projection_eligible, updated_at_ms FROM event_envelopes WHERE seq >= ? ORDER BY seq LIMIT ?",
        )
        .bind(next_event_seq)
        .bind(RECONCILIATION_SNAPSHOT_BATCH_SIZE)
        .fetch_all(&mut *connection)
        .await?;
        let row_count = rows.len();
        for row in rows {
            let Some(event) = events.get(event_index) else {
                return hook_drift("derived envelope row count exceeds raw events".to_owned());
            };
            let seq: i64 = row.try_get("seq")?;
            next_event_seq = seq.checked_add(1).ok_or_else(|| {
                RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: NIP09_HOOK_ID,
                    reason: "derived envelope sequence exhausts bounded pagination".to_owned(),
                }
            })?;
            event_index = event_index.checked_add(1).ok_or_else(|| {
                RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: NIP09_HOOK_ID,
                    reason: "derived envelope count exceeds addressable memory".to_owned(),
                }
            })?;
            let envelope = event.verified_event.event();
            let expected_class = StoredEventClass::from_event_kind_class(envelope.kind_class());
            let expected_projection =
                i64::from(event.admission.valid_stream_eligible(envelope.kind_class()));
            if seq != event.seq
                || row.try_get::<String, _>("verification_status")? != "verified"
                || row.try_get::<String, _>("contract_status")? != event.admission.status.as_str()
                || row.try_get::<Option<String>, _>("contract_id")?.as_deref()
                    != event.admission.contract.map(|contract| contract.id)
                || row.try_get::<Option<String>, _>("event_class")?.as_deref()
                    != Some(expected_class.as_str())
                || row.try_get::<i64, _>("projection_eligible")? != expected_projection
                || row.try_get::<i64, _>("updated_at_ms")? != event.inserted_at_ms
            {
                return hook_drift(format!(
                    "derived envelope fields disagree for `{}`",
                    envelope.id_str()
                ));
            }
        }
        if row_count < RECONCILIATION_SNAPSHOT_BATCH_LEN {
            break;
        }
    }
    if event_index != events.len() {
        return hook_drift("derived envelope row count differs from raw events".to_owned());
    }

    let mut expected_tags = BTreeSet::new();
    for event in events {
        for (index, tag) in event.verified_event.event().tag_slices().iter().enumerate() {
            let name = tag.as_slice().first().map(String::as_str).unwrap_or("");
            let contract_tag = event.admission.contract.and_then(|contract| {
                contract
                    .tags
                    .iter()
                    .find(|candidate| candidate.name == name)
            });
            expected_tags.insert((
                event.verified_event.event().id_str().to_owned(),
                i64_from_usize("tag_index", index)?,
                contract_tag.map(|tag| tag_semantic_name(tag.semantic).to_owned()),
                contract_tag.map(|tag| tag_value_type_name(tag.value_type).to_owned()),
                i64::from(contract_tag.map(|tag| tag.relay_indexed).unwrap_or(false)),
            ));
        }
    }
    let mut next_tag_rowid = i64::MIN;
    loop {
        let rows = sqlx::query(
            "SELECT rowid AS tag_rowid, event_id, tag_index, contract_semantic, contract_value_type, relay_indexed FROM event_envelope_tags WHERE rowid >= ? ORDER BY rowid LIMIT ?",
        )
        .bind(next_tag_rowid)
        .bind(RECONCILIATION_SNAPSHOT_BATCH_SIZE)
        .fetch_all(&mut *connection)
        .await?;
        let row_count = rows.len();
        for row in rows {
            let tag_rowid: i64 = row.try_get("tag_rowid")?;
            let actual_tag = (
                row.try_get::<String, _>("event_id")?,
                row.try_get::<i64, _>("tag_index")?,
                row.try_get::<Option<String>, _>("contract_semantic")?,
                row.try_get::<Option<String>, _>("contract_value_type")?,
                row.try_get::<i64, _>("relay_indexed")?,
            );
            next_tag_rowid = tag_rowid.checked_add(1).ok_or_else(|| {
                RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: NIP09_HOOK_ID,
                    reason: "derived tag rowid exhausts bounded validation pagination".to_owned(),
                }
            })?;
            if !expected_tags.remove(&actual_tag) {
                return hook_drift(
                    "derived tag fields disagree with admitted contracts".to_owned(),
                );
            }
        }
        if row_count < RECONCILIATION_SNAPSHOT_BATCH_LEN {
            break;
        }
    }
    if !expected_tags.is_empty() {
        return hook_drift("derived tag row count differs from raw tags".to_owned());
    }
    Ok(())
}

async fn validate_raw_heads(
    connection: &mut SqliteConnection,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    let mut expected = BTreeSet::new();
    for winner in select_raw_head_winners(events).into_values() {
        let (coordinate_type, kind, pubkey, d_tag) = match winner.candidate.coordinate {
            RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => (
                "replaceable".to_owned(),
                i64::from(kind),
                pubkey.to_string(),
                None,
            ),
            RadrootsEventHeadCoordinate::Addressable {
                kind,
                pubkey,
                d_tag,
            } => (
                "addressable".to_owned(),
                i64::from(kind),
                pubkey.to_string(),
                Some(d_tag),
            ),
        };
        expected.insert(RawHeadFact {
            coordinate_type,
            kind,
            pubkey,
            d_tag,
            event_id: winner.candidate.event_id.to_string(),
            created_at: i64_from_u64("raw_head_created_at", winner.candidate.created_at)?,
            updated_at_ms: winner.updated_at_ms,
        });
    }
    let rows = sqlx::query(
        "SELECT coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms FROM event_envelope_head ORDER BY coordinate_type, kind, pubkey, d_tag",
    )
    .fetch_all(&mut *connection)
    .await?;
    let actual = rows
        .into_iter()
        .map(|row| {
            Ok(RawHeadFact {
                coordinate_type: row.try_get("coordinate_type")?,
                kind: row.try_get("kind")?,
                pubkey: row.try_get("pubkey")?,
                d_tag: row.try_get("d_tag")?,
                event_id: row.try_get("event_id")?,
                created_at: row.try_get("created_at")?,
                updated_at_ms: row.try_get("updated_at_ms")?,
            })
        })
        .collect::<Result<BTreeSet<_>, sqlx::Error>>()?;
    if actual != expected {
        return hook_drift("raw head rows disagree with deterministic NIP-01 selection".to_owned());
    }
    Ok(())
}

async fn rebuild_raw_heads(
    connection: &mut SqliteConnection,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    let winners = select_raw_head_winners(events);
    sqlx::query("DELETE FROM event_envelope_head")
        .execute(&mut *connection)
        .await?;
    for winner in winners.into_values() {
        match &winner.candidate.coordinate {
            RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
                sqlx::query(
                    "INSERT INTO event_envelope_head(coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms) VALUES ('replaceable', ?, ?, NULL, ?, ?, ?)",
                )
                .bind(i64::from(*kind))
                .bind(pubkey.as_str())
                .bind(winner.candidate.event_id.as_str())
                .bind(i64_from_u64(
                    "raw_head_created_at",
                    winner.candidate.created_at,
                )?)
                .bind(winner.updated_at_ms)
                .execute(&mut *connection)
                .await?;
            }
            RadrootsEventHeadCoordinate::Addressable {
                kind,
                pubkey,
                d_tag,
            } => {
                sqlx::query(
                    "INSERT INTO event_envelope_head(coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms) VALUES ('addressable', ?, ?, ?, ?, ?, ?)",
                )
                .bind(i64::from(*kind))
                .bind(pubkey.as_str())
                .bind(d_tag)
                .bind(winner.candidate.event_id.as_str())
                .bind(i64_from_u64(
                    "raw_head_created_at",
                    winner.candidate.created_at,
                )?)
                .bind(winner.updated_at_ms)
                .execute(&mut *connection)
                .await?;
            }
        }
    }
    Ok(())
}

fn select_raw_head_winners(
    events: &[ReconciledEvent],
) -> BTreeMap<RadrootsEventHeadCoordinate, RawHeadWinner> {
    let mut winners = BTreeMap::new();
    for event in events {
        apply_raw_head_to_winners(&mut winners, event);
    }
    winners
}

fn apply_raw_head_to_winners(
    winners: &mut BTreeMap<RadrootsEventHeadCoordinate, RawHeadWinner>,
    event: &ReconciledEvent,
) -> RadrootsRawHeadDecision {
    let candidate = match event_head_candidate_for_nip01_event_v1(event.verified_event.event()) {
        RadrootsEventHeadCandidateResult::Candidate(candidate) => candidate,
        RadrootsEventHeadCandidateResult::NotHeadSelected => {
            return RadrootsRawHeadDecision::NotHeadSelected;
        }
        RadrootsEventHeadCandidateResult::NotPersisted => {
            return RadrootsRawHeadDecision::NotPersisted;
        }
        RadrootsEventHeadCandidateResult::Malformed(_) => {
            return RadrootsRawHeadDecision::MalformedCoordinate;
        }
    };
    let current = winners
        .get(&candidate.coordinate)
        .map(|winner| RadrootsCurrentEventHead {
            coordinate: winner.candidate.coordinate.clone(),
            event_id: winner.candidate.event_id.clone(),
            created_at: winner.candidate.created_at,
        });
    let decision = select_event_head_v1(candidate.clone(), current.as_ref());
    if matches!(decision, RadrootsEventHeadDecision::Applied(_)) {
        winners.insert(
            candidate.coordinate.clone(),
            RawHeadWinner {
                candidate,
                event_seq: event.seq,
                updated_at_ms: event.inserted_at_ms,
            },
        );
    }
    RadrootsRawHeadDecision::from_protocol(&decision)
}

async fn persist_event_coordinate_facts(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    for event in events {
        persist_event_coordinate_fact(connection, generation, event).await?;
    }
    Ok(())
}

async fn persist_event_coordinate_fact(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    event: &ReconciledEvent,
) -> Result<(), RadrootsEventStoreError> {
    let Some(fact) = event_coordinate_fact(event)? else {
        return Ok(());
    };
    let inserted = sqlx::query(
        "INSERT INTO radroots_event_store_event_coordinate(source_generation, event_id, event_seq, coordinate_type, kind, pubkey, created_at, inserted_at_ms, admission_status, admission_code, contract_id, raw_d_tag, nip09_matchable, nip09_d_tag) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(fact.event_id.as_str())
    .bind(fact.event_seq)
    .bind(fact.coordinate_type.as_str())
    .bind(fact.kind)
    .bind(fact.pubkey.as_str())
    .bind(fact.created_at)
    .bind(fact.inserted_at_ms)
    .bind(fact.admission_status.as_str())
    .bind(fact.admission_code.as_deref())
    .bind(fact.contract_id.as_deref())
    .bind(fact.raw_d_tag.as_str())
    .bind(fact.nip09_matchable)
    .bind(fact.nip09_d_tag.as_deref())
    .execute(&mut *connection)
    .await?;
    require_expected_insert(inserted.rows_affected(), "event coordinate fact")
}

async fn validate_event_coordinate_facts(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    let expected = events
        .iter()
        .map(event_coordinate_fact)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    let actual = sqlx::query(
        "SELECT event_id, event_seq, coordinate_type, kind, pubkey, created_at, inserted_at_ms, admission_status, admission_code, contract_id, raw_d_tag, nip09_matchable, nip09_d_tag FROM radroots_event_store_event_coordinate WHERE source_generation = ? ORDER BY event_seq, event_id",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(event_coordinate_fact_from_row)
    .collect::<Result<BTreeSet<_>, sqlx::Error>>()?;
    if actual != expected {
        return hook_drift(
            "persisted NIP-01 coordinate facts differ from immutable raw events".to_owned(),
        );
    }
    Ok(())
}

fn event_coordinate_fact(
    event: &ReconciledEvent,
) -> Result<Option<EventCoordinateFact>, RadrootsEventStoreError> {
    let envelope = event.verified_event.event();
    let kind = envelope.kind_u32();
    let (coordinate_type, raw_d_tag, nip09_d_tag) =
        if matches!(kind, 0 | 3) || (10_000..=19_999).contains(&kind) {
            ("replaceable", String::new(), Some(String::new()))
        } else if (30_000..=39_999).contains(&kind) {
            let raw_d_tag_value = envelope
                .tag_slices()
                .iter()
                .find(|tag| tag.as_slice().first().is_some_and(|name| name == "d"))
                .and_then(|tag| tag.as_slice().get(1))
                .cloned();
            let nip09_d_tag = raw_d_tag_value.as_ref().and_then(|d_tag| {
                RadrootsNip01Coordinate::parse(format!("{kind}:{}:{d_tag}", envelope.author_str()))
                    .ok()
                    .map(|coordinate| coordinate.identifier().to_owned())
            });
            (
                "addressable",
                raw_d_tag_value.unwrap_or_default(),
                nip09_d_tag,
            )
        } else {
            return Ok(None);
        };
    Ok(Some(EventCoordinateFact {
        event_id: envelope.id_str().to_owned(),
        event_seq: event.seq,
        coordinate_type: coordinate_type.to_owned(),
        kind: i64::from(kind),
        pubkey: envelope.author_str().to_owned(),
        created_at: i64_from_u64("created_at", envelope.created_at_u64())?,
        inserted_at_ms: event.inserted_at_ms,
        admission_status: event.admission.status.as_str().to_owned(),
        admission_code: event.admission.code.clone(),
        contract_id: event
            .admission
            .contract
            .map(|contract| contract.id.to_owned()),
        raw_d_tag,
        nip09_matchable: i64::from(nip09_d_tag.is_some()),
        nip09_d_tag,
    }))
}

fn event_coordinate_fact_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<EventCoordinateFact, sqlx::Error> {
    Ok(EventCoordinateFact {
        event_id: row.try_get("event_id")?,
        event_seq: row.try_get("event_seq")?,
        coordinate_type: row.try_get("coordinate_type")?,
        kind: row.try_get("kind")?,
        pubkey: row.try_get("pubkey")?,
        created_at: row.try_get("created_at")?,
        inserted_at_ms: row.try_get("inserted_at_ms")?,
        admission_status: row.try_get("admission_status")?,
        admission_code: row.try_get("admission_code")?,
        contract_id: row.try_get("contract_id")?,
        raw_d_tag: row.try_get("raw_d_tag")?,
        nip09_matchable: row.try_get("nip09_matchable")?,
        nip09_d_tag: row.try_get("nip09_d_tag")?,
    })
}

async fn persist_nip09_facts(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    events: &[ReconciledEvent],
) -> Result<Vec<RadrootsAdmittedNip09DeletionRequestEventV1>, RadrootsEventStoreError> {
    let mut requests = Vec::new();
    for event in events {
        if event.admission.status != RadrootsEventAdmissionStatus::Admitted
            || event.verified_event.event().kind_u32() != 5
        {
            continue;
        }
        let request = admit_verified_nip09_deletion_request_event_v1(event.verified_event.clone())
            .map_err(|error| RadrootsEventStoreError::MigrationHookStateDrift {
                hook_id: NIP09_HOOK_ID,
                reason: format!(
                    "centrally admitted deletion request `{}` failed typed admission: {}",
                    event.verified_event.event().id_str(),
                    error
                ),
            })?;
        persist_request_fact(connection, generation, event.seq, &request).await?;
        requests.push(request);
    }

    requests.sort_by(|left, right| left.event().id().cmp(right.event().id()));
    Ok(requests)
}

async fn validate_nip09_fact_graph(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    events: &[ReconciledEvent],
) -> Result<Vec<RadrootsAdmittedNip09DeletionRequestEventV1>, RadrootsEventStoreError> {
    let mut requests = Vec::new();
    let mut expected_requests = BTreeSet::new();
    let mut expected_event_targets = BTreeSet::new();
    let mut expected_address_targets = BTreeSet::new();
    for event in events {
        if event.admission.status != RadrootsEventAdmissionStatus::Admitted
            || event.verified_event.event().kind_u32() != 5
        {
            continue;
        }
        let request = admit_verified_nip09_deletion_request_event_v1(event.verified_event.clone())
            .map_err(|error| RadrootsEventStoreError::MigrationHookStateDrift {
                hook_id: NIP09_HOOK_ID,
                reason: format!(
                    "centrally admitted deletion request `{}` failed typed admission: {error}",
                    event.verified_event.event().id_str()
                ),
            })?;
        expected_requests.insert(RequestFact {
            request_event_id: request.event().id_str().to_owned(),
            request_event_seq: event.seq,
            request_pubkey: request.event().author_str().to_owned(),
            request_created_at: i64_from_u64(
                "request_created_at",
                request.event().created_at_u64(),
            )?,
        });
        for target in request.projection().event_targets() {
            let source_tag_value =
                request_source_tag_value(request.event(), target.tag_index(), "e")?;
            expected_event_targets.insert(EventTargetFact {
                request_event_id: request.event().id_str().to_owned(),
                target_event_id: target.event_id().as_str().to_owned(),
                source_tag_index: i64_from_usize("source_tag_index", target.tag_index())?,
                source_tag_value: source_tag_value.to_owned(),
            });
        }
        for target in request.projection().address_targets() {
            let source_tag_value =
                request_source_tag_value(request.event(), target.tag_index(), "a")?;
            let (source_kind_text, source_pubkey_text, source_d_tag) =
                raw_coordinate_parts(source_tag_value)?;
            expected_address_targets.insert(AddressTargetFact {
                request_event_id: request.event().id_str().to_owned(),
                target_kind: i64::from(target.coordinate().kind()),
                target_pubkey: target.coordinate().pubkey().as_str().to_owned(),
                target_d_tag: target.coordinate().identifier().to_owned(),
                inclusive_cutoff: i64_from_u64(
                    "inclusive_cutoff",
                    request.event().created_at_u64(),
                )?,
                source_tag_index: i64_from_usize("source_tag_index", target.tag_index())?,
                source_tag_value: source_tag_value.to_owned(),
                source_kind_text: source_kind_text.to_owned(),
                source_pubkey_text: source_pubkey_text.to_owned(),
                source_d_tag: source_d_tag.to_owned(),
            });
        }
        requests.push(request);
    }
    requests.sort_by(|left, right| left.event().id().cmp(right.event().id()));

    let actual_requests = sqlx::query(
        "SELECT request_event_id, request_event_seq, request_pubkey, request_created_at FROM radroots_event_store_nip09_request WHERE source_generation = ? ORDER BY request_event_id",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(|row| {
        Ok(RequestFact {
            request_event_id: row.try_get("request_event_id")?,
            request_event_seq: row.try_get("request_event_seq")?,
            request_pubkey: row.try_get("request_pubkey")?,
            request_created_at: row.try_get("request_created_at")?,
        })
    })
    .collect::<Result<BTreeSet<_>, sqlx::Error>>()?;
    if actual_requests != expected_requests {
        return hook_drift("persisted NIP-09 request facts are incomplete or forged".to_owned());
    }

    let actual_event_targets = sqlx::query(
        "SELECT request_event_id, target_event_id, source_tag_index, source_tag_value FROM radroots_event_store_nip09_event_target WHERE source_generation = ? ORDER BY request_event_id, target_event_id",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(|row| {
        Ok(EventTargetFact {
            request_event_id: row.try_get("request_event_id")?,
            target_event_id: row.try_get("target_event_id")?,
            source_tag_index: row.try_get("source_tag_index")?,
            source_tag_value: row.try_get("source_tag_value")?,
        })
    })
    .collect::<Result<BTreeSet<_>, sqlx::Error>>()?;
    if actual_event_targets != expected_event_targets {
        return hook_drift("persisted NIP-09 event targets are incomplete or forged".to_owned());
    }

    let actual_address_targets = sqlx::query(
        "SELECT request_event_id, target_kind, target_pubkey, target_d_tag, inclusive_cutoff, source_tag_index, source_tag_value, source_kind_text, source_pubkey_text, source_d_tag FROM radroots_event_store_nip09_address_target WHERE source_generation = ? ORDER BY request_event_id, target_kind, target_pubkey, target_d_tag",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(|row| {
        Ok(AddressTargetFact {
            request_event_id: row.try_get("request_event_id")?,
            target_kind: row.try_get("target_kind")?,
            target_pubkey: row.try_get("target_pubkey")?,
            target_d_tag: row.try_get("target_d_tag")?,
            inclusive_cutoff: row.try_get("inclusive_cutoff")?,
            source_tag_index: row.try_get("source_tag_index")?,
            source_tag_value: row.try_get("source_tag_value")?,
            source_kind_text: row.try_get("source_kind_text")?,
            source_pubkey_text: row.try_get("source_pubkey_text")?,
            source_d_tag: row.try_get("source_d_tag")?,
        })
    })
    .collect::<Result<BTreeSet<_>, sqlx::Error>>()?;
    if actual_address_targets != expected_address_targets {
        return hook_drift("persisted NIP-09 address targets are incomplete or forged".to_owned());
    }

    Ok(requests)
}

async fn synchronize_insert_delta(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    ingest: &RadrootsEventIngest,
    admission: &EventAdmission,
    inserted_seq: i64,
    inserted_event_id: &str,
    raw_head_decision: &RadrootsRawHeadDecision,
) -> Result<(), RadrootsEventStoreError> {
    let mut affected_coordinates = BTreeSet::new();

    if admission.status == RadrootsEventAdmissionStatus::Admitted && ingest.event().kind_u32() == 5
    {
        let request = admit_verified_nip09_deletion_request_event_v1(ingest.verified_event().clone())
            .map_err(|error| RadrootsEventStoreError::MigrationHookStateDrift {
                hook_id: NIP09_HOOK_ID,
                reason: format!(
                    "centrally admitted deletion request `{inserted_event_id}` failed typed admission: {error}"
                ),
            })?;
        persist_request_fact(connection, generation, inserted_seq, &request).await?;
        affected_coordinates.extend(
            load_request_affected_coordinates(connection, generation, request.event().id_str())
                .await?,
        );
    }

    if matches!(raw_head_decision, RadrootsRawHeadDecision::Applied)
        && let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_nip01_event_v1(ingest.event())
        && let RadrootsEventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag,
        } = candidate.coordinate
    {
        affected_coordinates.insert((i64::from(kind), pubkey.to_string(), d_tag));
    }

    if !affected_coordinates.is_empty() {
        synchronize_addressable_coordinates(
            connection,
            generation,
            &affected_coordinates,
            (inserted_seq, inserted_event_id),
            raw_head_decision_code(raw_head_decision),
        )
        .await?;
    }
    Ok(())
}

async fn persist_request_fact(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    request_seq: i64,
    request: &RadrootsAdmittedNip09DeletionRequestEventV1,
) -> Result<(), RadrootsEventStoreError> {
    let event = request.event();
    let inserted = sqlx::query(
        "INSERT INTO radroots_event_store_nip09_request(source_generation, request_event_id, request_event_seq, request_pubkey, request_created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(event.id_str())
    .bind(request_seq)
    .bind(event.author_str())
    .bind(i64_from_u64(
        "request_created_at",
        event.created_at_u64(),
    )?)
    .execute(&mut *connection)
    .await?;
    require_expected_insert(inserted.rows_affected(), "NIP-09 request")?;
    for target in request.projection().event_targets() {
        let source_tag_value = request_source_tag_value(event, target.tag_index(), "e")?;
        let inserted = sqlx::query(
            "INSERT INTO radroots_event_store_nip09_event_target(source_generation, request_event_id, target_event_id, source_tag_index, source_tag_value) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(generation.as_bytes().as_slice())
        .bind(event.id_str())
        .bind(target.event_id().as_str())
        .bind(i64_from_usize("source_tag_index", target.tag_index())?)
        .bind(source_tag_value)
        .execute(&mut *connection)
        .await?;
        require_expected_insert(inserted.rows_affected(), "NIP-09 event target")?;
    }
    for target in request.projection().address_targets() {
        let source_tag_value = request_source_tag_value(event, target.tag_index(), "a")?;
        let (source_kind_text, source_pubkey_text, source_d_tag) =
            raw_coordinate_parts(source_tag_value)?;
        let inserted = sqlx::query(
            "INSERT INTO radroots_event_store_nip09_address_target(source_generation, request_event_id, target_kind, target_pubkey, target_d_tag, inclusive_cutoff, source_tag_index, source_tag_value, source_kind_text, source_pubkey_text, source_d_tag) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(generation.as_bytes().as_slice())
        .bind(event.id_str())
        .bind(i64::from(target.coordinate().kind()))
        .bind(target.coordinate().pubkey().as_str())
        .bind(target.coordinate().identifier())
        .bind(i64_from_u64("inclusive_cutoff", event.created_at_u64())?)
        .bind(i64_from_usize("source_tag_index", target.tag_index())?)
        .bind(source_tag_value)
        .bind(source_kind_text)
        .bind(source_pubkey_text)
        .bind(source_d_tag)
        .execute(&mut *connection)
        .await?;
        require_expected_insert(inserted.rows_affected(), "NIP-09 address target")?;
    }
    Ok(())
}

async fn load_request_affected_coordinates(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    request_event_id: &str,
) -> Result<BTreeSet<(i64, String, String)>, RadrootsEventStoreError> {
    let rows = sqlx::query(
        "SELECT head.kind, head.pubkey, head.d_tag
         FROM radroots_event_store_nip09_event_target AS target
         JOIN event_envelope_head AS head
           ON head.coordinate_type = 'addressable'
          AND head.event_id = target.target_event_id
         WHERE target.source_generation = ?
           AND target.request_event_id = ?
         UNION
         SELECT head.kind, head.pubkey, head.d_tag
         FROM radroots_event_store_nip09_address_target AS target
         JOIN event_envelope_head AS head
           ON head.coordinate_type = 'addressable'
          AND head.kind = target.target_kind
          AND head.pubkey = target.target_pubkey
          AND head.d_tag = target.target_d_tag
         WHERE target.source_generation = ?
           AND target.request_event_id = ?
           AND target.target_kind BETWEEN 30000 AND 39999
         ORDER BY 1, 2, 3",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(request_event_id)
    .bind(generation.as_bytes().as_slice())
    .bind(request_event_id)
    .fetch_all(&mut *connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get("kind")?,
                row.try_get("pubkey")?,
                row.try_get("d_tag")?,
            ))
        })
        .collect::<Result<BTreeSet<_>, sqlx::Error>>()
        .map_err(Into::into)
}

async fn synchronize_addressable_coordinates(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    coordinates: &BTreeSet<(i64, String, String)>,
    cause: (i64, &str),
    raw_head_decision: &str,
) -> Result<(), RadrootsEventStoreError> {
    for (kind, pubkey, d_tag) in coordinates {
        let row = sqlx::query(
            "SELECT coordinate.event_id, coordinate.event_seq, coordinate.coordinate_type, coordinate.kind, coordinate.pubkey, coordinate.created_at, coordinate.inserted_at_ms, coordinate.admission_status, coordinate.admission_code, coordinate.contract_id, coordinate.raw_d_tag, coordinate.nip09_matchable, coordinate.nip09_d_tag
             FROM event_envelope_head AS head
             JOIN radroots_event_store_event_coordinate AS coordinate
               ON coordinate.source_generation = ?
              AND coordinate.event_id = head.event_id
              AND coordinate.coordinate_type = head.coordinate_type
              AND coordinate.kind = head.kind
              AND coordinate.pubkey = head.pubkey
              AND coordinate.raw_d_tag = head.d_tag
             WHERE head.coordinate_type = 'addressable'
               AND head.kind = ?
               AND head.pubkey = ?
               AND head.d_tag = ?",
        )
        .bind(generation.as_bytes().as_slice())
        .bind(*kind)
        .bind(pubkey)
        .bind(d_tag)
        .fetch_optional(&mut *connection)
        .await?;
        let Some(row) = row else {
            return hook_drift(format!(
                "affected addressable coordinate {kind}:{pubkey}:{d_tag} has no raw head"
            ));
        };
        let fact = event_coordinate_fact_from_row(row)?;
        let desired = addressable_state_for_stored_facts(connection, generation, &fact).await?;
        let prior = read_addressable_state(connection, generation, *kind, pubkey, d_tag).await?;
        if prior.as_ref() == Some(&desired) {
            continue;
        }
        let visible =
            (desired.visibility == "visible").then_some(desired.raw_head_event_id.as_str());
        let retracted = prior
            .as_ref()
            .filter(|prior| {
                prior.visibility == "visible" && visible != Some(prior.raw_head_event_id.as_str())
            })
            .map(|prior| (prior.raw_head_event_seq, prior.raw_head_event_id.as_str()));
        insert_addressable_transition(
            connection,
            generation,
            &desired,
            TransitionOrigin::Incremental,
            Some(cause),
            retracted,
            raw_head_decision,
        )
        .await?;
        write_addressable_state(
            connection,
            generation,
            &desired,
            TransitionOrigin::Incremental,
            Some(cause),
        )
        .await?;
    }
    Ok(())
}

async fn read_addressable_state(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    kind: i64,
    pubkey: &str,
    d_tag: &str,
) -> Result<Option<AddressableHeadState>, RadrootsEventStoreError> {
    let row = sqlx::query(
        "SELECT kind, pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff FROM radroots_event_store_addressable_head_state WHERE source_generation = ? AND kind = ? AND pubkey = ? AND d_tag = ?",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(kind)
    .bind(pubkey)
    .bind(d_tag)
    .fetch_optional(&mut *connection)
    .await?;
    row.map(|row| {
        Ok(AddressableHeadState {
            kind: row.try_get("kind")?,
            pubkey: row.try_get("pubkey")?,
            d_tag: row.try_get("d_tag")?,
            raw_head_event_id: row.try_get("raw_head_event_id")?,
            raw_head_event_seq: row.try_get("raw_head_event_seq")?,
            raw_head_created_at: row.try_get("raw_head_created_at")?,
            admission_status: row.try_get("admission_status")?,
            admission_code: row.try_get("admission_code")?,
            contract_id: row.try_get("contract_id")?,
            visibility: row.try_get("visibility")?,
            nip09_outcome: row.try_get("nip09_outcome")?,
            nip09_reason: row.try_get("nip09_reason")?,
            event_reference_request_id: row.try_get("event_reference_request_id")?,
            address_reference_request_id: row.try_get("address_reference_request_id")?,
            address_reference_cutoff: row.try_get("address_reference_cutoff")?,
        })
    })
    .transpose()
}

#[derive(Clone, Copy)]
enum TransitionOrigin {
    Baseline,
    Incremental,
}

impl TransitionOrigin {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Incremental => "incremental",
        }
    }
}

async fn synchronize_addressable_heads(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    events: &[ReconciledEvent],
    requests: &[RadrootsAdmittedNip09DeletionRequestEventV1],
    origin: TransitionOrigin,
    cause: Option<(i64, &str)>,
    raw_head_decision: &str,
) -> Result<(), RadrootsEventStoreError> {
    let desired = desired_addressable_states(events, requests)?;
    let current = read_addressable_states(connection, generation).await?;
    for (key, state) in desired {
        let prior = current.get(&key);
        if prior == Some(&state) {
            continue;
        }
        let retracted = prior
            .filter(|prior| prior.visibility == "visible")
            .map(|prior| (prior.raw_head_event_seq, prior.raw_head_event_id.as_str()));
        insert_addressable_transition(
            connection,
            generation,
            &state,
            origin,
            cause,
            retracted,
            raw_head_decision,
        )
        .await?;
        write_addressable_state(connection, generation, &state, origin, cause).await?;
    }
    Ok(())
}

fn desired_addressable_states(
    events: &[ReconciledEvent],
    requests: &[RadrootsAdmittedNip09DeletionRequestEventV1],
) -> Result<BTreeMap<(i64, String, String), AddressableHeadState>, RadrootsEventStoreError> {
    let event_by_id = events
        .iter()
        .map(|event| (event.verified_event.event().id_str(), event))
        .collect::<BTreeMap<_, _>>();
    let winners = select_raw_head_winners(events);
    let request_index = RequestIndex::new(requests);
    let mut desired = BTreeMap::new();
    for (coordinate, winner) in winners {
        let RadrootsEventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag,
        } = coordinate
        else {
            continue;
        };
        let event = event_by_id
            .get(winner.candidate.event_id.as_str())
            .ok_or_else(|| RadrootsEventStoreError::MigrationHookStateDrift {
                hook_id: NIP09_HOOK_ID,
                reason: format!(
                    "raw head `{}` has no reconciled event",
                    winner.candidate.event_id
                ),
            })?;
        let state = addressable_state_for_event(
            i64::from(kind),
            pubkey.as_str(),
            d_tag.as_str(),
            winner.event_seq,
            event,
            &request_index,
        )?;
        desired.insert((i64::from(kind), pubkey.to_string(), d_tag), state);
    }
    Ok(desired)
}

async fn validate_addressable_state(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    events: &[ReconciledEvent],
    requests: &[RadrootsAdmittedNip09DeletionRequestEventV1],
) -> Result<(), RadrootsEventStoreError> {
    let expected = desired_addressable_states(events, requests)?;
    let actual = read_addressable_states(connection, generation).await?;
    if actual != expected {
        return hook_drift(
            "active addressable head state disagrees with canonical replay".to_owned(),
        );
    }
    Ok(())
}

async fn validate_latest_transitions_match_state(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
) -> Result<(), RadrootsEventStoreError> {
    let mismatch_count: i64 = sqlx::query_scalar(
        "WITH latest AS (
           SELECT kind, pubkey, d_tag, MAX(transition_seq) AS transition_seq
           FROM radroots_event_store_addressable_head_transition
           WHERE source_generation = ?
           GROUP BY kind, pubkey, d_tag
         ),
         snapshot AS (
           SELECT transition.*
           FROM latest
           JOIN radroots_event_store_addressable_head_transition AS transition
             ON transition.transition_seq = latest.transition_seq
         )
         SELECT
           (
             SELECT COUNT(*)
             FROM radroots_event_store_addressable_head_state AS state
             LEFT JOIN snapshot
               ON snapshot.kind = state.kind
              AND snapshot.pubkey = state.pubkey
              AND snapshot.d_tag = state.d_tag
             WHERE state.source_generation = ?
               AND (
                 snapshot.transition_seq IS NULL
                 OR snapshot.source_generation IS NOT state.source_generation
                 OR snapshot.raw_head_event_id IS NOT state.raw_head_event_id
                 OR snapshot.raw_head_event_seq IS NOT state.raw_head_event_seq
                 OR snapshot.raw_head_created_at IS NOT state.raw_head_created_at
                 OR snapshot.admission_status IS NOT state.admission_status
                 OR snapshot.admission_code IS NOT state.admission_code
                 OR snapshot.contract_id IS NOT state.contract_id
                 OR snapshot.visibility IS NOT state.visibility
                 OR snapshot.nip09_outcome IS NOT state.nip09_outcome
                 OR snapshot.nip09_reason IS NOT state.nip09_reason
                 OR snapshot.event_reference_request_id IS NOT state.event_reference_request_id
                 OR snapshot.address_reference_request_id IS NOT state.address_reference_request_id
                 OR snapshot.address_reference_cutoff IS NOT state.address_reference_cutoff
                 OR snapshot.origin IS NOT state.last_origin
                 OR snapshot.cause_event_seq IS NOT state.last_cause_event_seq
                 OR snapshot.cause_event_id IS NOT state.last_cause_event_id
                 OR snapshot.visible_event_id IS NOT
                    CASE WHEN state.visibility = 'visible' THEN state.raw_head_event_id END
                 OR snapshot.visible_event_seq IS NOT
                    CASE WHEN state.visibility = 'visible' THEN state.raw_head_event_seq END
               )
           )
           +
           (
             SELECT COUNT(*)
             FROM snapshot
             LEFT JOIN radroots_event_store_addressable_head_state AS state
               ON state.source_generation = snapshot.source_generation
              AND state.kind = snapshot.kind
              AND state.pubkey = snapshot.pubkey
              AND state.d_tag = snapshot.d_tag
             WHERE snapshot.source_generation = ?
               AND state.kind IS NULL
           )",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(generation.as_bytes().as_slice())
    .bind(generation.as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await?;
    if mismatch_count != 0 {
        return hook_drift(format!(
            "{mismatch_count} latest addressable transition snapshots disagree with current state"
        ));
    }
    Ok(())
}

async fn validate_transition_history(
    connection: &mut SqliteConnection,
    source: &SourceState,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    validate_baseline_authority(connection, source, events).await?;
    let expected = expected_transition_history(source, events)?;
    let actual = sqlx::query(
        "SELECT transition_seq, origin, kind, pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at, visible_event_id, visible_event_seq, retracted_event_id, retracted_event_seq, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff, cause_event_seq, cause_event_id, raw_head_decision FROM radroots_event_store_addressable_head_transition WHERE source_generation = ? ORDER BY transition_seq",
    )
    .bind(source.generation.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(|row| {
        Ok(AddressableTransitionFact {
            transition_seq: row.try_get("transition_seq")?,
            origin: row.try_get("origin")?,
            kind: row.try_get("kind")?,
            pubkey: row.try_get("pubkey")?,
            d_tag: row.try_get("d_tag")?,
            raw_head_event_id: row.try_get("raw_head_event_id")?,
            raw_head_event_seq: row.try_get("raw_head_event_seq")?,
            raw_head_created_at: row.try_get("raw_head_created_at")?,
            visible_event_id: row.try_get("visible_event_id")?,
            visible_event_seq: row.try_get("visible_event_seq")?,
            retracted_event_id: row.try_get("retracted_event_id")?,
            retracted_event_seq: row.try_get("retracted_event_seq")?,
            admission_status: row.try_get("admission_status")?,
            admission_code: row.try_get("admission_code")?,
            contract_id: row.try_get("contract_id")?,
            visibility: row.try_get("visibility")?,
            nip09_outcome: row.try_get("nip09_outcome")?,
            nip09_reason: row.try_get("nip09_reason")?,
            event_reference_request_id: row.try_get("event_reference_request_id")?,
            address_reference_request_id: row.try_get("address_reference_request_id")?,
            address_reference_cutoff: row.try_get("address_reference_cutoff")?,
            cause_event_seq: row.try_get("cause_event_seq")?,
            cause_event_id: row.try_get("cause_event_id")?,
            raw_head_decision: row.try_get("raw_head_decision")?,
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()?;
    if actual != expected {
        return hook_drift(
            "addressable transition history disagrees with deterministic arrival replay".to_owned(),
        );
    }
    Ok(())
}

async fn validate_baseline_authority(
    connection: &mut SqliteConnection,
    source: &SourceState,
    events: &[ReconciledEvent],
) -> Result<(), RadrootsEventStoreError> {
    let baseline_events = events
        .iter()
        .filter(|event| event.seq <= source.baseline_raw_high_water_seq)
        .collect::<Vec<_>>();
    let baseline_event_count = i64_from_usize("baseline_raw_event_count", baseline_events.len())?;
    let baseline_high_water = baseline_events.last().map(|event| event.seq).unwrap_or(0);
    let baseline_tag_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM event_envelope_tags AS tag JOIN event_envelopes AS event ON event.event_id = tag.event_id WHERE event.seq <= ?",
    )
    .bind(source.baseline_raw_high_water_seq)
    .fetch_one(&mut *connection)
    .await?;
    if baseline_event_count != source.baseline_raw_event_count
        || baseline_tag_count != source.baseline_raw_tag_count
        || baseline_high_water != source.baseline_raw_high_water_seq
    {
        return hook_drift(
            "active generation baseline raw authority disagrees with canonical replay".to_owned(),
        );
    }
    Ok(())
}

fn expected_transition_history(
    source: &SourceState,
    events: &[ReconciledEvent],
) -> Result<Vec<AddressableTransitionFact>, RadrootsEventStoreError> {
    let baseline_events = events
        .iter()
        .filter(|event| event.seq <= source.baseline_raw_high_water_seq)
        .cloned()
        .collect::<Vec<_>>();
    let incremental_events = events
        .iter()
        .filter(|event| event.seq > source.baseline_raw_high_water_seq);
    let event_by_id = events
        .iter()
        .map(|event| (event.verified_event.event().id_str(), event))
        .collect::<BTreeMap<_, _>>();
    let requests = admitted_nip09_requests(&baseline_events)?;
    let mut request_index = RequestIndex::new(&requests);
    let mut winners = select_raw_head_winners(&baseline_events);
    let mut states = desired_addressable_states(&baseline_events, &requests)?;
    let mut transitions = Vec::new();
    let mut next_transition_seq = source.transition_floor_seq;

    for state in states.values() {
        next_transition_seq = checked_authority_add(next_transition_seq, 1, "transition sequence")?;
        transitions.push(addressable_transition_fact(
            next_transition_seq,
            state,
            TransitionOrigin::Baseline,
            None,
            None,
            "baseline_rebuild",
        ));
    }

    for event in incremental_events {
        let raw_head_decision = apply_raw_head_to_winners(&mut winners, event);
        let mut affected_coordinates = BTreeSet::new();
        if event.admission.status == RadrootsEventAdmissionStatus::Admitted
            && event.verified_event.event().kind_u32() == 5
        {
            let request = admitted_nip09_request(event)?;
            affected_coordinates.extend(request_affected_addressable_coordinates(
                &request,
                &winners,
                &event_by_id,
            ));
            request_index.insert(&request);
        } else if matches!(raw_head_decision, RadrootsRawHeadDecision::Applied)
            && let RadrootsEventHeadCandidateResult::Candidate(candidate) =
                event_head_candidate_for_nip01_event_v1(event.verified_event.event())
            && let RadrootsEventHeadCoordinate::Addressable {
                kind,
                pubkey,
                d_tag,
            } = candidate.coordinate
        {
            affected_coordinates.insert((i64::from(kind), pubkey.to_string(), d_tag));
        }

        for (kind, pubkey, d_tag) in affected_coordinates {
            let key = (kind, pubkey.clone(), d_tag.clone());
            let coordinate = RadrootsEventHeadCoordinate::Addressable {
                kind: u32::try_from(kind).map_err(|_| RadrootsEventStoreError::IntegerRange {
                    field: "transition.kind",
                    value: kind,
                })?,
                pubkey: radroots_event::ids::RadrootsPublicKey::parse(pubkey.as_str())?,
                d_tag: d_tag.clone(),
            };
            let winner = winners.get(&coordinate).ok_or_else(|| {
                RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: NIP09_HOOK_ID,
                    reason: format!(
                        "affected addressable coordinate {kind}:{pubkey}:{d_tag} has no replay head"
                    ),
                }
            })?;
            let target = event_by_id
                .get(winner.candidate.event_id.as_str())
                .ok_or_else(|| RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: NIP09_HOOK_ID,
                    reason: format!(
                        "raw head `{}` has no reconciled event",
                        winner.candidate.event_id
                    ),
                })?;
            let desired = addressable_state_for_event(
                kind,
                &pubkey,
                &d_tag,
                winner.event_seq,
                target,
                &request_index,
            )?;
            let prior = states.get(&key);
            if prior == Some(&desired) {
                continue;
            }
            let visible =
                (desired.visibility == "visible").then_some(desired.raw_head_event_id.as_str());
            let retracted = prior
                .filter(|prior| {
                    prior.visibility == "visible"
                        && visible != Some(prior.raw_head_event_id.as_str())
                })
                .map(|prior| (prior.raw_head_event_seq, prior.raw_head_event_id.as_str()));
            next_transition_seq =
                checked_authority_add(next_transition_seq, 1, "transition sequence")?;
            transitions.push(addressable_transition_fact(
                next_transition_seq,
                &desired,
                TransitionOrigin::Incremental,
                Some((event.seq, event.verified_event.event().id_str())),
                retracted,
                raw_head_decision_code(&raw_head_decision),
            ));
            states.insert(key, desired);
        }
    }
    Ok(transitions)
}

fn admitted_nip09_requests(
    events: &[ReconciledEvent],
) -> Result<Vec<RadrootsAdmittedNip09DeletionRequestEventV1>, RadrootsEventStoreError> {
    let mut requests = events
        .iter()
        .filter(|event| {
            event.admission.status == RadrootsEventAdmissionStatus::Admitted
                && event.verified_event.event().kind_u32() == 5
        })
        .map(admitted_nip09_request)
        .collect::<Result<Vec<_>, _>>()?;
    requests.sort_by(|left, right| left.event().id().cmp(right.event().id()));
    Ok(requests)
}

fn admitted_nip09_request(
    event: &ReconciledEvent,
) -> Result<RadrootsAdmittedNip09DeletionRequestEventV1, RadrootsEventStoreError> {
    admit_verified_nip09_deletion_request_event_v1(event.verified_event.clone()).map_err(|error| {
        RadrootsEventStoreError::MigrationHookStateDrift {
            hook_id: NIP09_HOOK_ID,
            reason: format!(
                "centrally admitted deletion request `{}` failed typed admission: {error}",
                event.verified_event.event().id_str()
            ),
        }
    })
}

fn request_affected_addressable_coordinates<'a>(
    request: &RadrootsAdmittedNip09DeletionRequestEventV1,
    winners: &BTreeMap<RadrootsEventHeadCoordinate, RawHeadWinner>,
    event_by_id: &BTreeMap<&'a str, &'a ReconciledEvent>,
) -> BTreeSet<(i64, String, String)> {
    let mut affected = BTreeSet::new();
    for target in request.projection().event_targets() {
        let Some(event) = event_by_id.get(target.event_id().as_str()) else {
            continue;
        };
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_nip01_event_v1(event.verified_event.event())
        else {
            continue;
        };
        let coordinate = &candidate.coordinate;
        let RadrootsEventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag,
        } = coordinate
        else {
            continue;
        };
        if winners
            .get(coordinate)
            .is_some_and(|winner| winner.candidate.event_id == candidate.event_id)
        {
            affected.insert((i64::from(*kind), pubkey.to_string(), d_tag.clone()));
        }
    }
    for target in request.projection().address_targets() {
        let kind = target.coordinate().kind();
        if !(30_000..=39_999).contains(&kind) {
            continue;
        }
        let coordinate = RadrootsEventHeadCoordinate::Addressable {
            kind,
            pubkey: target.coordinate().pubkey().clone(),
            d_tag: target.coordinate().identifier().to_owned(),
        };
        if winners.contains_key(&coordinate) {
            affected.insert((
                i64::from(kind),
                target.coordinate().pubkey().as_str().to_owned(),
                target.coordinate().identifier().to_owned(),
            ));
        }
    }
    affected
}

fn addressable_transition_fact(
    transition_seq: i64,
    state: &AddressableHeadState,
    origin: TransitionOrigin,
    cause: Option<(i64, &str)>,
    retracted: Option<(i64, &str)>,
    raw_head_decision: &str,
) -> AddressableTransitionFact {
    let visible = (state.visibility == "visible")
        .then_some((state.raw_head_event_seq, state.raw_head_event_id.clone()));
    AddressableTransitionFact {
        transition_seq,
        origin: origin.as_str().to_owned(),
        kind: state.kind,
        pubkey: state.pubkey.clone(),
        d_tag: state.d_tag.clone(),
        raw_head_event_id: state.raw_head_event_id.clone(),
        raw_head_event_seq: state.raw_head_event_seq,
        raw_head_created_at: state.raw_head_created_at,
        visible_event_id: visible.as_ref().map(|visible| visible.1.clone()),
        visible_event_seq: visible.map(|visible| visible.0),
        retracted_event_id: retracted.map(|retracted| retracted.1.to_owned()),
        retracted_event_seq: retracted.map(|retracted| retracted.0),
        admission_status: state.admission_status.clone(),
        admission_code: state.admission_code.clone(),
        contract_id: state.contract_id.clone(),
        visibility: state.visibility.clone(),
        nip09_outcome: state.nip09_outcome.clone(),
        nip09_reason: state.nip09_reason.clone(),
        event_reference_request_id: state.event_reference_request_id.clone(),
        address_reference_request_id: state.address_reference_request_id.clone(),
        address_reference_cutoff: state.address_reference_cutoff,
        cause_event_seq: cause.map(|cause| cause.0),
        cause_event_id: cause.map(|cause| cause.1.to_owned()),
        raw_head_decision: raw_head_decision.to_owned(),
    }
}

fn addressable_state_for_event(
    kind: i64,
    pubkey: &str,
    d_tag: &str,
    event_seq: i64,
    event: &ReconciledEvent,
    request_index: &RequestIndex,
) -> Result<AddressableHeadState, RadrootsEventStoreError> {
    let mut state = addressable_state_base(kind, pubkey, d_tag, event_seq, event)?;
    if event.admission.status != RadrootsEventAdmissionStatus::Admitted {
        return Ok(state);
    }

    let decision = request_index.decision(event.verified_event.event())?;
    state.nip09_outcome = Some(decision.outcome.code().to_owned());
    state.nip09_reason = Some(decision.reason.to_owned());
    state.event_reference_request_id = decision.event_reference_request_id;
    state.address_reference_request_id = decision.address_reference_request_id;
    state.address_reference_cutoff = decision.address_reference_cutoff;
    state.visibility = match decision.outcome {
        RadrootsNip09SuppressionOutcome::Visible => "visible",
        RadrootsNip09SuppressionOutcome::Suppressed => "suppressed",
    }
    .to_owned();
    Ok(state)
}

async fn addressable_state_for_stored_facts(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    event: &EventCoordinateFact,
) -> Result<AddressableHeadState, RadrootsEventStoreError> {
    if event.coordinate_type != "addressable" {
        return hook_drift(format!(
            "event coordinate `{}` is not addressable",
            event.event_id
        ));
    }
    let mut state = AddressableHeadState {
        kind: event.kind,
        pubkey: event.pubkey.clone(),
        d_tag: event.raw_d_tag.clone(),
        raw_head_event_id: event.event_id.clone(),
        raw_head_event_seq: event.event_seq,
        raw_head_created_at: event.created_at,
        admission_status: event.admission_status.clone(),
        admission_code: event.admission_code.clone(),
        contract_id: event.contract_id.clone(),
        visibility: "not_admitted".to_owned(),
        nip09_outcome: None,
        nip09_reason: None,
        event_reference_request_id: None,
        address_reference_request_id: None,
        address_reference_cutoff: None,
    };
    if event.admission_status != RadrootsEventAdmissionStatus::Admitted.as_str() {
        return Ok(state);
    }
    let decision = stored_suppression_decision(connection, generation, event).await?;
    state.nip09_outcome = Some(decision.outcome.code().to_owned());
    state.nip09_reason = Some(decision.reason.to_owned());
    state.event_reference_request_id = decision.event_reference_request_id;
    state.address_reference_request_id = decision.address_reference_request_id;
    state.address_reference_cutoff = decision.address_reference_cutoff;
    state.visibility = match decision.outcome {
        RadrootsNip09SuppressionOutcome::Visible => "visible",
        RadrootsNip09SuppressionOutcome::Suppressed => "suppressed",
    }
    .to_owned();
    Ok(state)
}

fn addressable_state_base(
    kind: i64,
    pubkey: &str,
    d_tag: &str,
    event_seq: i64,
    event: &ReconciledEvent,
) -> Result<AddressableHeadState, RadrootsEventStoreError> {
    let envelope = event.verified_event.event();
    Ok(AddressableHeadState {
        kind,
        pubkey: pubkey.to_owned(),
        d_tag: d_tag.to_owned(),
        raw_head_event_id: envelope.id_str().to_owned(),
        raw_head_event_seq: event_seq,
        raw_head_created_at: i64_from_u64("raw_head_created_at", envelope.created_at_u64())?,
        admission_status: event.admission.status.as_str().to_owned(),
        admission_code: event.admission.code.clone(),
        contract_id: event
            .admission
            .contract
            .map(|contract| contract.id.to_owned()),
        visibility: "not_admitted".to_owned(),
        nip09_outcome: None,
        nip09_reason: None,
        event_reference_request_id: None,
        address_reference_request_id: None,
        address_reference_cutoff: None,
    })
}

async fn read_addressable_states(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
) -> Result<BTreeMap<(i64, String, String), AddressableHeadState>, RadrootsEventStoreError> {
    let rows = sqlx::query(
        "SELECT kind, pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff FROM radroots_event_store_addressable_head_state WHERE source_generation = ? ORDER BY kind, pubkey, d_tag",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            let state = AddressableHeadState {
                kind: row.try_get("kind")?,
                pubkey: row.try_get("pubkey")?,
                d_tag: row.try_get("d_tag")?,
                raw_head_event_id: row.try_get("raw_head_event_id")?,
                raw_head_event_seq: row.try_get("raw_head_event_seq")?,
                raw_head_created_at: row.try_get("raw_head_created_at")?,
                admission_status: row.try_get("admission_status")?,
                admission_code: row.try_get("admission_code")?,
                contract_id: row.try_get("contract_id")?,
                visibility: row.try_get("visibility")?,
                nip09_outcome: row.try_get("nip09_outcome")?,
                nip09_reason: row.try_get("nip09_reason")?,
                event_reference_request_id: row.try_get("event_reference_request_id")?,
                address_reference_request_id: row.try_get("address_reference_request_id")?,
                address_reference_cutoff: row.try_get("address_reference_cutoff")?,
            };
            Ok((
                (state.kind, state.pubkey.clone(), state.d_tag.clone()),
                state,
            ))
        })
        .collect()
}

async fn write_addressable_state(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    state: &AddressableHeadState,
    origin: TransitionOrigin,
    cause: Option<(i64, &str)>,
) -> Result<(), RadrootsEventStoreError> {
    let updated = sqlx::query(
        "UPDATE radroots_event_store_addressable_head_state SET raw_head_event_id = ?, raw_head_event_seq = ?, raw_head_created_at = ?, admission_status = ?, admission_code = ?, contract_id = ?, visibility = ?, nip09_outcome = ?, nip09_reason = ?, event_reference_request_id = ?, address_reference_request_id = ?, address_reference_cutoff = ?, last_origin = ?, last_cause_event_seq = ?, last_cause_event_id = ? WHERE source_generation = ? AND kind = ? AND pubkey = ? AND d_tag = ?",
    )
    .bind(state.raw_head_event_id.as_str())
    .bind(state.raw_head_event_seq)
    .bind(state.raw_head_created_at)
    .bind(state.admission_status.as_str())
    .bind(state.admission_code.as_deref())
    .bind(state.contract_id.as_deref())
    .bind(state.visibility.as_str())
    .bind(state.nip09_outcome.as_deref())
    .bind(state.nip09_reason.as_deref())
    .bind(state.event_reference_request_id.as_deref())
    .bind(state.address_reference_request_id.as_deref())
    .bind(state.address_reference_cutoff)
    .bind(origin.as_str())
    .bind(cause.map(|cause| cause.0))
    .bind(cause.map(|cause| cause.1))
    .bind(generation.as_bytes().as_slice())
    .bind(state.kind)
    .bind(state.pubkey.as_str())
    .bind(state.d_tag.as_str())
    .execute(&mut *connection)
    .await?;
    if updated.rows_affected() == 0 {
        sqlx::query(
            "INSERT INTO radroots_event_store_addressable_head_state(source_generation, kind, pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff, last_origin, last_cause_event_seq, last_cause_event_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(generation.as_bytes().as_slice())
        .bind(state.kind)
        .bind(state.pubkey.as_str())
        .bind(state.d_tag.as_str())
        .bind(state.raw_head_event_id.as_str())
        .bind(state.raw_head_event_seq)
        .bind(state.raw_head_created_at)
        .bind(state.admission_status.as_str())
        .bind(state.admission_code.as_deref())
        .bind(state.contract_id.as_deref())
        .bind(state.visibility.as_str())
        .bind(state.nip09_outcome.as_deref())
        .bind(state.nip09_reason.as_deref())
        .bind(state.event_reference_request_id.as_deref())
        .bind(state.address_reference_request_id.as_deref())
        .bind(state.address_reference_cutoff)
        .bind(origin.as_str())
        .bind(cause.map(|cause| cause.0))
        .bind(cause.map(|cause| cause.1))
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

async fn insert_addressable_transition(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    state: &AddressableHeadState,
    origin: TransitionOrigin,
    cause: Option<(i64, &str)>,
    retracted: Option<(i64, &str)>,
    raw_head_decision: &str,
) -> Result<(), RadrootsEventStoreError> {
    let visible = (state.visibility == "visible")
        .then_some((state.raw_head_event_seq, state.raw_head_event_id.as_str()));
    sqlx::query(
        "INSERT INTO radroots_event_store_addressable_head_transition(source_generation, origin, kind, pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at, visible_event_id, visible_event_seq, retracted_event_id, retracted_event_seq, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff, cause_event_seq, cause_event_id, raw_head_decision) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(origin.as_str())
    .bind(state.kind)
    .bind(state.pubkey.as_str())
    .bind(state.d_tag.as_str())
    .bind(state.raw_head_event_id.as_str())
    .bind(state.raw_head_event_seq)
    .bind(state.raw_head_created_at)
    .bind(visible.map(|visible| visible.1))
    .bind(visible.map(|visible| visible.0))
    .bind(retracted.map(|retracted| retracted.1))
    .bind(retracted.map(|retracted| retracted.0))
    .bind(state.admission_status.as_str())
    .bind(state.admission_code.as_deref())
    .bind(state.contract_id.as_deref())
    .bind(state.visibility.as_str())
    .bind(state.nip09_outcome.as_deref())
    .bind(state.nip09_reason.as_deref())
    .bind(state.event_reference_request_id.as_deref())
    .bind(state.address_reference_request_id.as_deref())
    .bind(state.address_reference_cutoff)
    .bind(cause.map(|cause| cause.0))
    .bind(cause.map(|cause| cause.1))
    .bind(raw_head_decision)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn read_source_state(
    connection: &mut SqliteConnection,
) -> Result<SourceState, RadrootsEventStoreError> {
    let rows = sqlx::query(
        "SELECT state.active_generation, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, state.last_transition_seq, generation.generation_ordinal, (SELECT MAX(candidate.generation_ordinal) FROM radroots_event_store_source_generation AS candidate) AS max_generation_ordinal, generation.transition_floor_seq, generation.baseline_raw_event_count, generation.baseline_raw_tag_count, generation.baseline_raw_high_water_seq, generation.reconciliation_version, generation.addressable_feed_version, generation.event_contract_registry_version, generation.hook_id, generation.hook_manifest_sha256 FROM radroots_event_store_source_state AS state JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = state.active_generation WHERE state.singleton = 1",
    )
    .fetch_all(&mut *connection)
    .await?;
    if rows.len() != 1 {
        return hook_drift(format!(
            "expected one active source state, found {}",
            rows.len()
        ));
    }
    let row = &rows[0];
    let generation_ordinal: i64 = row.try_get("generation_ordinal")?;
    let max_generation_ordinal: i64 = row.try_get("max_generation_ordinal")?;
    if generation_ordinal != max_generation_ordinal {
        return hook_drift("active source generation is not the newest generation".to_owned());
    }
    let hook_id: String = row.try_get("hook_id")?;
    let hook_manifest_sha256: String = row.try_get("hook_manifest_sha256")?;
    Ok(SourceState {
        generation: generation_from_blob(row.try_get("active_generation")?)?,
        profile: reconciliation_profile(
            row.try_get("reconciliation_version")?,
            row.try_get("addressable_feed_version")?,
            row.try_get("event_contract_registry_version")?,
            hook_id.as_str(),
            hook_manifest_sha256.as_str(),
        )?,
        raw_event_count: row.try_get("raw_event_count")?,
        raw_tag_count: row.try_get("raw_tag_count")?,
        raw_high_water_seq: row.try_get("raw_high_water_seq")?,
        last_transition_seq: row.try_get("last_transition_seq")?,
        transition_floor_seq: row.try_get("transition_floor_seq")?,
        baseline_raw_event_count: row.try_get("baseline_raw_event_count")?,
        baseline_raw_tag_count: row.try_get("baseline_raw_tag_count")?,
        baseline_raw_high_water_seq: row.try_get("baseline_raw_high_water_seq")?,
    })
}

fn reconciliation_profile(
    reconciliation_version: i64,
    addressable_feed_version: i64,
    event_contract_registry_version: i64,
    hook_id: &str,
    hook_manifest_sha256: &str,
) -> Result<ReconciliationProfile, RadrootsEventStoreError> {
    match (
        reconciliation_version,
        addressable_feed_version,
        event_contract_registry_version,
        hook_id,
        hook_manifest_sha256,
    ) {
        (
            NIP09_RECONCILIATION_VERSION,
            NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION,
            registry_version,
            NIP09_HOOK_ID,
            NIP09_RECONCILIATION_MANIFEST_SHA256,
        ) if registry_version
            == i64::from(NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION) =>
        {
            Ok(ReconciliationProfile::Nip09V1RegistryV7)
        }
        _ => hook_drift("active generation contract metadata is unsupported".to_owned()),
    }
}

async fn validate_source_raw_authority_with_state(
    connection: &mut SqliteConnection,
    state: &SourceState,
) -> Result<(), RadrootsEventStoreError> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS raw_event_count, COALESCE(MAX(seq), 0) AS raw_high_water_seq FROM event_envelopes",
    )
    .fetch_one(&mut *connection)
    .await?;
    let actual_count: i64 = row.try_get("raw_event_count")?;
    let actual_high_water: i64 = row.try_get("raw_high_water_seq")?;
    let actual_tag_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_envelope_tags")
        .fetch_one(&mut *connection)
        .await?;
    if actual_count != state.raw_event_count
        || actual_high_water != state.raw_high_water_seq
        || actual_tag_count != state.raw_tag_count
    {
        return Err(RadrootsEventStoreError::RawEventSourceDrift {
            expected_count: state.raw_event_count,
            expected_tag_count: state.raw_tag_count,
            expected_high_water: state.raw_high_water_seq,
            actual_count,
            actual_tag_count,
            actual_high_water,
        });
    }
    Ok(())
}

async fn update_source_authority(
    connection: &mut SqliteConnection,
    raw_event_count: i64,
    raw_tag_count: i64,
    raw_high_water_seq: i64,
) -> Result<(), RadrootsEventStoreError> {
    let last_transition_seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(transition.transition_seq), generation.transition_floor_seq) FROM radroots_event_store_source_state AS state JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = state.active_generation LEFT JOIN radroots_event_store_addressable_head_transition AS transition ON transition.source_generation = state.active_generation WHERE state.singleton = 1",
    )
    .fetch_one(&mut *connection)
    .await?;
    let updated = sqlx::query(
        "UPDATE radroots_event_store_source_state SET raw_event_count = ?, raw_tag_count = ?, raw_high_water_seq = ?, last_transition_seq = ? WHERE singleton = 1",
    )
    .bind(raw_event_count)
    .bind(raw_tag_count)
    .bind(raw_high_water_seq)
    .bind(last_transition_seq)
    .execute(&mut *connection)
    .await?;
    if updated.rows_affected() != 1 {
        return hook_drift(format!(
            "source authority update affected {} rows",
            updated.rows_affected()
        ));
    }
    Ok(())
}

pub(crate) fn generation_from_blob(
    value: Vec<u8>,
) -> Result<RadrootsEventStoreSourceGeneration, RadrootsEventStoreError> {
    let bytes: [u8; 32] = value.try_into().map_err(|value: Vec<u8>| {
        RadrootsEventStoreError::MigrationHookStateDrift {
            hook_id: NIP09_HOOK_ID,
            reason: format!("source generation has {} bytes instead of 32", value.len()),
        }
    })?;
    Ok(RadrootsEventStoreSourceGeneration::from_bytes(bytes))
}

fn raw_head_decision_code(decision: &RadrootsRawHeadDecision) -> &'static str {
    match decision {
        RadrootsRawHeadDecision::Applied => "applied",
        RadrootsRawHeadDecision::NotHeadSelected | RadrootsRawHeadDecision::NotPersisted => {
            "not_head_selected"
        }
        RadrootsRawHeadDecision::SkippedDuplicate => "not_head_selected",
        RadrootsRawHeadDecision::SkippedOlder => "skipped_older",
        RadrootsRawHeadDecision::SkippedSameTimestampHigherEventId => {
            "skipped_same_timestamp_higher_event_id"
        }
        RadrootsRawHeadDecision::MalformedCoordinate => "malformed_coordinate",
    }
}

fn nip01_coordinate_key(event: &RadrootsEventEnvelope) -> Option<(i64, String, String)> {
    let RadrootsEventHeadCandidateResult::Candidate(candidate) =
        event_head_candidate_for_nip01_event_v1(event)
    else {
        return None;
    };
    match candidate.coordinate {
        RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
            Some((i64::from(kind), pubkey.to_string(), String::new()))
        }
        RadrootsEventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag,
        } => Some((i64::from(kind), pubkey.to_string(), d_tag)),
    }
}

fn request_source_tag_value<'a>(
    event: &'a RadrootsEventEnvelope,
    tag_index: usize,
    expected_name: &'static str,
) -> Result<&'a str, RadrootsEventStoreError> {
    let tag = event
        .tag_slices()
        .get(tag_index)
        .map(|tag| tag.as_slice())
        .filter(|tag| tag.first().is_some_and(|name| name == expected_name))
        .and_then(|tag| tag.get(1))
        .ok_or_else(|| RadrootsEventStoreError::MigrationHookStateDrift {
            hook_id: NIP09_HOOK_ID,
            reason: format!(
                "admitted deletion request `{}` has no `{expected_name}` value at source tag {tag_index}",
                event.id_str()
            ),
        })?;
    Ok(tag.as_str())
}

fn raw_coordinate_parts(value: &str) -> Result<(&str, &str, &str), RadrootsEventStoreError> {
    let (kind, remainder) =
        value
            .split_once(':')
            .ok_or_else(|| RadrootsEventStoreError::MigrationHookStateDrift {
                hook_id: NIP09_HOOK_ID,
                reason: "admitted deletion request has an unparseable raw address target"
                    .to_owned(),
            })?;
    let (pubkey, d_tag) = remainder.split_once(':').ok_or_else(|| {
        RadrootsEventStoreError::MigrationHookStateDrift {
            hook_id: NIP09_HOOK_ID,
            reason: "admitted deletion request has an unparseable raw address target".to_owned(),
        }
    })?;
    Ok((kind, pubkey, d_tag))
}

fn require_expected_insert(
    rows_affected: u64,
    entity: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    if rows_affected == 1 {
        return Ok(());
    }
    hook_drift(format!(
        "expected one new {entity} row, inserted {rows_affected}"
    ))
}

fn checked_authority_add(
    current: i64,
    delta: i64,
    field: &'static str,
) -> Result<i64, RadrootsEventStoreError> {
    current
        .checked_add(delta)
        .ok_or_else(|| RadrootsEventStoreError::MigrationHookStateDrift {
            hook_id: NIP09_HOOK_ID,
            reason: format!("{field} exhausted the SQLite INTEGER range"),
        })
}

fn compare_raw_field(
    matches: bool,
    event_id: &str,
    field: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    if matches {
        Ok(())
    } else {
        Err(raw_mismatch(event_id, field))
    }
}

fn raw_mismatch(event_id: &str, field: &'static str) -> RadrootsEventStoreError {
    RadrootsEventStoreError::RawEventReconciliationMismatch {
        event_id: event_id.to_owned(),
        field,
    }
}

fn hook_drift<T>(reason: String) -> Result<T, RadrootsEventStoreError> {
    Err(RadrootsEventStoreError::MigrationHookStateDrift {
        hook_id: NIP09_HOOK_ID,
        reason,
    })
}

fn i64_from_u64(field: &'static str, value: u64) -> Result<i64, RadrootsEventStoreError> {
    i64::try_from(value).map_err(|_| RadrootsEventStoreError::UnsignedIntegerRange { field, value })
}

fn i64_from_usize(field: &'static str, value: usize) -> Result<i64, RadrootsEventStoreError> {
    i64::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange {
        field,
        value: i64::MAX,
    })
}

fn text_payload_bytes<const N: usize>(
    resource: RadrootsEventStoreSourceCapacityResourceV1,
    limits: ReconciliationCapacityLimits,
    lengths: [usize; N],
) -> Result<u64, RadrootsEventStoreError> {
    let limit = limits.limit(resource);
    lengths.into_iter().try_fold(0_u64, |total, length| {
        let length =
            u64::try_from(length).map_err(|_| RadrootsEventStoreError::SourceCapacityExceeded {
                resource,
                current: u64::MAX,
                requested: 0,
                limit,
            })?;
        total
            .checked_add(length)
            .ok_or(RadrootsEventStoreError::SourceCapacityExceeded {
                resource,
                current: u64::MAX,
                requested: 0,
                limit,
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{Keys, SECP256K1, secp256k1::Message};
    use radroots_event::wire::v1::RadrootsNip01EventWire;
    use radroots_event::{
        deletion::RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
        envelope::RadrootsEventEnvelopeParts,
        kinds::{KIND_DELETION_REQUEST, KIND_LIST_SET_RELAY, KIND_POST},
        wire::compute_canonical_nip01_event_id,
    };
    use radroots_event_codec::verification::v1::verify_nip01_event_v1;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    const FIXTURE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const TARGET_CREATED_AT: u64 = 1_800_100_100;
    const REQUEST_CREATED_AT: u64 = TARGET_CREATED_AT + 1;
    const EVENT_STORE_V1_UP_SQL: &str = include_str!("../../migrations/0001_event_store.up.sql");
    const EVENT_STORE_V2_UP_SQL: &str = include_str!("../../migrations/0002_nip09.up.sql");

    struct FixedTestGeneration([u8; 32]);

    impl SourceGenerationProvider for FixedTestGeneration {
        fn fill_generation(
            &self,
            generation: &mut [u8; 32],
        ) -> Result<(), RadrootsEventStoreError> {
            generation.copy_from_slice(&self.0);
            Ok(())
        }
    }

    struct FailingTestGeneration;

    impl SourceGenerationProvider for FailingTestGeneration {
        fn fill_generation(
            &self,
            _generation: &mut [u8; 32],
        ) -> Result<(), RadrootsEventStoreError> {
            Err(RadrootsEventStoreError::SourceGenerationEntropyUnavailable)
        }
    }

    #[test]
    fn bounded_capacity_page_len_caps_gross_source_probe_at_one_over() {
        for limit in [25_000_u64, 250_000_u64] {
            let mut current = 0_u64;
            loop {
                let (sqlite_limit, fetched_len) = bounded_capacity_page_len(current, limit);
                assert_eq!(
                    usize::try_from(sqlite_limit).expect("positive bounded page limit"),
                    fetched_len
                );
                assert!((1..=RECONCILIATION_SNAPSHOT_BATCH_LEN).contains(&fetched_len));
                let fetched = current + u64::try_from(fetched_len).expect("bounded page length");
                if fetched > limit {
                    assert_eq!(fetched, limit + 1);
                    break;
                }
                current = fetched;
            }
        }
        assert_eq!(bounded_capacity_page_len(24_576, 25_000), (425, 425));
        assert_eq!(bounded_capacity_page_len(249_856, 250_000), (145, 145));
        assert_eq!(bounded_capacity_page_len(25_000, 25_000), (1, 1));
    }

    #[tokio::test]
    async fn scalar_reconciliation_authority_rejects_every_boundary() {
        let limits = ReconciliationCapacityLimits {
            raw_events: 1,
            raw_tags: 1,
            raw_event_bytes: 1,
            raw_tag_bytes: 1,
        };
        for resource in [
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
            RadrootsEventStoreSourceCapacityResourceV1::RawTags,
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
        ] {
            let mut exact = ReconciliationCapacity::default();
            exact
                .checked_add(limits, resource, 1)
                .expect("exact capacity");
            assert_eq!(exact.value(resource), 1);
            assert!(matches!(
                exact.checked_add(limits, resource, 1),
                Err(RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: actual_resource,
                    current: 1,
                    requested: 1,
                    limit: 1,
                }) if actual_resource == resource
            ));

            let mut overflow = ReconciliationCapacity::default();
            *overflow.value_mut(resource) = u64::MAX;
            assert!(matches!(
                overflow.checked_add(limits, resource, 1),
                Err(RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: actual_resource,
                    current: u64::MAX,
                    requested: 1,
                    limit: 1,
                }) if actual_resource == resource
            ));
            assert!(matches!(
                overflow.validate(limits),
                Err(RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: actual_resource,
                    current: u64::MAX,
                    requested: 0,
                    limit: 1,
                }) if actual_resource == resource
            ));
        }

        assert!(matches!(
            reconciliation_capacity_value(
                RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                -1,
            ),
            Err(RadrootsEventStoreError::MigrationHookStateDrift { .. })
        ));
        assert!(matches!(
            EventAdmission::from_registry_v7(RadrootsRegistryV7AdmissionDecision::Defect {
                code: "fixture_defect",
            }),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { ref reason })
                if reason.contains("fixture_defect")
        ));

        let deletion = signed_event(
            REQUEST_CREATED_AT,
            KIND_DELETION_REQUEST,
            Vec::new(),
            "immune",
        );
        let index = RequestIndex::new(&[]);
        let indexed_decision = index.decision(&deletion).expect("deletion immunity");
        assert_eq!(
            indexed_decision.reason,
            RadrootsNip09SuppressionReason::DeletionRequestImmune.code()
        );

        let pool = open_v1_test_pool().await;
        let mut connection = pool.acquire().await.expect("connection");
        let stored_decision = stored_suppression_decision(
            &mut connection,
            RadrootsEventStoreSourceGeneration::from_bytes([1; 32]),
            &EventCoordinateFact {
                event_id: deletion.id_str().to_owned(),
                event_seq: 1,
                coordinate_type: "regular".to_owned(),
                kind: i64::from(KIND_DELETION_REQUEST),
                pubkey: deletion.author_str().to_owned(),
                created_at: i64::try_from(deletion.created_at_u64()).expect("created_at"),
                inserted_at_ms: 1,
                admission_status: "admitted".to_owned(),
                admission_code: None,
                contract_id: None,
                raw_d_tag: String::new(),
                nip09_matchable: 0,
                nip09_d_tag: None,
            },
        )
        .await
        .expect("stored deletion immunity");
        assert_eq!(stored_decision.reason, "deletion_request_immune");

        assert_eq!(
            reconciliation_profile(
                NIP09_RECONCILIATION_VERSION,
                NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION,
                i64::from(NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION),
                NIP09_HOOK_ID,
                NIP09_RECONCILIATION_MANIFEST_SHA256,
            )
            .expect("supported profile"),
            ReconciliationProfile::Nip09V1RegistryV7
        );
        assert!(reconciliation_profile(0, 0, 0, "invalid", "invalid").is_err());
        assert!(generation_from_blob(vec![0; 31]).is_err());

        for (decision, code) in [
            (RadrootsRawHeadDecision::Applied, "applied"),
            (
                RadrootsRawHeadDecision::NotHeadSelected,
                "not_head_selected",
            ),
            (RadrootsRawHeadDecision::NotPersisted, "not_head_selected"),
            (
                RadrootsRawHeadDecision::SkippedDuplicate,
                "not_head_selected",
            ),
            (RadrootsRawHeadDecision::SkippedOlder, "skipped_older"),
            (
                RadrootsRawHeadDecision::SkippedSameTimestampHigherEventId,
                "skipped_same_timestamp_higher_event_id",
            ),
            (
                RadrootsRawHeadDecision::MalformedCoordinate,
                "malformed_coordinate",
            ),
        ] {
            assert_eq!(raw_head_decision_code(&decision), code);
        }

        let regular = signed_event(TARGET_CREATED_AT, KIND_POST, Vec::new(), "regular");
        assert!(nip01_coordinate_key(&regular).is_none());
        let tagged = signed_event(
            TARGET_CREATED_AT,
            KIND_DELETION_REQUEST,
            vec![vec!["e".to_owned(), "value".to_owned()]],
            "tagged",
        );
        assert_eq!(
            request_source_tag_value(&tagged, 0, "e").expect("tag value"),
            "value"
        );
        assert!(request_source_tag_value(&tagged, 0, "a").is_err());
        assert!(request_source_tag_value(&tagged, 1, "e").is_err());

        assert_eq!(
            raw_coordinate_parts("30402:author:opaque:value").expect("coordinate parts"),
            ("30402", "author", "opaque:value")
        );
        assert!(raw_coordinate_parts("invalid").is_err());
        assert!(raw_coordinate_parts("30402:invalid").is_err());
        require_expected_insert(1, "fixture").expect("one insert");
        assert!(require_expected_insert(0, "fixture").is_err());
        assert!(checked_authority_add(i64::MAX, 1, "fixture").is_err());
        compare_raw_field(true, "fixture", "content").expect("matching field");
        assert!(compare_raw_field(false, "fixture", "content").is_err());
        assert!(i64_from_u64("fixture", i64::MAX as u64 + 1).is_err());
        assert!(i64_from_usize("fixture", i64::MAX as usize + 1).is_err());
        assert!(
            text_payload_bytes(
                RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
                limits,
                [usize::MAX, 1],
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn deep_reconciliation_rejects_each_persisted_fact_drift() {
        for (label, guards, mutation, expected_reason) in [
            (
                "projection cursor orphan",
                &["DROP TRIGGER radroots_event_store_projection_cursor_source_insert_guard"][..],
                "INSERT INTO radroots_event_store_projection_cursor_source(projection_id, source_generation, source_revision) SELECT 'orphan', active_generation, 1 FROM radroots_event_store_source_state WHERE singleton = 1",
                "projection cursor source identity has no cursor",
            ),
            (
                "raw head",
                &["DROP TRIGGER radroots_event_store_event_head_update_guard"][..],
                "UPDATE event_envelope_head SET updated_at_ms = updated_at_ms + 1",
                "raw head rows disagree",
            ),
            (
                "derived envelope",
                &["DROP TRIGGER radroots_event_store_event_envelopes_derived_update_guard"][..],
                "UPDATE event_envelopes SET projection_eligible = 1 - projection_eligible",
                "derived envelope fields disagree",
            ),
            (
                "derived tag",
                &["DROP TRIGGER radroots_event_store_event_tags_derived_update_guard"][..],
                "UPDATE event_envelope_tags SET relay_indexed = 1 - relay_indexed WHERE rowid = (SELECT MIN(rowid) FROM event_envelope_tags)",
                "derived tag fields disagree",
            ),
            (
                "event coordinate",
                &["DROP TRIGGER radroots_event_store_event_coordinate_update_guard"][..],
                "UPDATE radroots_event_store_event_coordinate SET inserted_at_ms = inserted_at_ms + 1",
                "coordinate facts differ",
            ),
            (
                "request",
                &["DROP TRIGGER radroots_event_store_nip09_request_update_guard"][..],
                "UPDATE radroots_event_store_nip09_request SET request_created_at = request_created_at + 1",
                "request facts are incomplete or forged",
            ),
            (
                "event target",
                &["DROP TRIGGER radroots_event_store_nip09_event_target_update_guard"][..],
                "UPDATE radroots_event_store_nip09_event_target SET source_tag_value = 'forged'",
                "event targets are incomplete or forged",
            ),
            (
                "address target",
                &["DROP TRIGGER radroots_event_store_nip09_address_target_update_guard"][..],
                "UPDATE radroots_event_store_nip09_address_target SET source_d_tag = 'forged'",
                "address targets are incomplete or forged",
            ),
            (
                "addressable state",
                &[
                    "DROP TRIGGER radroots_event_store_addressable_state_identity_update_guard",
                    "DROP TRIGGER radroots_event_store_addressable_state_old_update_guard",
                ][..],
                "UPDATE radroots_event_store_addressable_head_state SET raw_head_created_at = raw_head_created_at + 1",
                "addressable head state disagrees",
            ),
            (
                "transition history",
                &["DROP TRIGGER radroots_event_store_addressable_transition_update_guard"][..],
                "UPDATE radroots_event_store_addressable_head_transition SET raw_head_created_at = raw_head_created_at + 1",
                "transition history disagrees",
            ),
        ] {
            let error = deep_reconciliation_error_after_corruption(guards, mutation).await;
            assert!(
                matches!(
                    error,
                    RadrootsEventStoreError::MigrationHookStateDrift { ref reason, .. }
                        if reason.contains(expected_reason)
                ),
                "{label}: {error}",
            );
        }
    }

    #[tokio::test]
    async fn deep_reconciliation_rejects_source_and_raw_authority_drift() {
        for (label, guard_drops, mutation, expected_reason) in [
            (
                "active generation metadata",
                &["DROP TRIGGER radroots_event_store_source_generation_update_guard"][..],
                "UPDATE radroots_event_store_source_generation SET hook_id = 'invalid' WHERE source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1)",
                "generation contract metadata is unsupported",
            ),
            (
                "active generation ordinal",
                &[
                    "DROP TRIGGER radroots_event_store_source_generation_insert_conflict_guard",
                    "DROP TRIGGER radroots_event_store_source_generation_append_guard",
                ][..],
                "INSERT INTO radroots_event_store_source_generation(source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq) SELECT zeroblob(32), generation_ordinal + 1, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq FROM radroots_event_store_source_generation WHERE source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1)",
                "active generation contract metadata is inconsistent",
            ),
            (
                "generation baseline",
                &["DROP TRIGGER radroots_event_store_source_generation_update_guard"][..],
                "UPDATE radroots_event_store_source_generation SET baseline_raw_event_count = baseline_raw_event_count + 1 WHERE source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1)",
                "active generation baseline exceeds current authority",
            ),
            (
                "raw high water",
                &["DROP TRIGGER radroots_event_store_source_state_authority_update_guard"][..],
                "UPDATE radroots_event_store_source_state SET raw_high_water_seq = raw_high_water_seq + 1 WHERE singleton = 1",
                "raw high-water does not match active source authority",
            ),
            (
                "transition bounds",
                &["DROP TRIGGER radroots_event_store_source_state_authority_update_guard"][..],
                "UPDATE radroots_event_store_source_state SET last_transition_seq = last_transition_seq + 1 WHERE singleton = 1",
                "active transition bounds are inconsistent",
            ),
            (
                "signed created_at",
                &["DROP TRIGGER radroots_event_store_event_envelopes_raw_update_guard"][..],
                "UPDATE event_envelopes SET created_at = created_at + 1 WHERE seq = 1",
                "signed raw JSON field `created_at`",
            ),
            (
                "signed kind",
                &["DROP TRIGGER radroots_event_store_event_envelopes_raw_update_guard"][..],
                "UPDATE event_envelopes SET kind = kind + 1 WHERE seq = 1",
                "signed raw JSON field `kind`",
            ),
            (
                "signed tags",
                &["DROP TRIGGER radroots_event_store_event_envelopes_raw_update_guard"][..],
                "UPDATE event_envelopes SET tags_json = '[]' WHERE kind = 30002",
                "signed raw JSON field `tags_json`",
            ),
            (
                "missing raw tag",
                &["DROP TRIGGER radroots_event_store_event_tags_delete_guard"][..],
                "DELETE FROM event_envelope_tags WHERE event_id = (SELECT event_id FROM event_envelopes WHERE kind = 30002)",
                "signed raw JSON field `tag_rows`",
            ),
            (
                "forged raw tag",
                &["DROP TRIGGER radroots_event_store_event_tags_raw_update_guard"][..],
                "UPDATE event_envelope_tags SET tag_name = 'forged' WHERE rowid = (SELECT MIN(rowid) FROM event_envelope_tags)",
                "signed raw JSON field `tag_rows`",
            ),
        ] {
            let error = deep_reconciliation_error_after_corruption(guard_drops, mutation).await;
            assert!(
                matches!(
                    &error,
                    RadrootsEventStoreError::MigrationHookStateDrift { .. }
                        | RadrootsEventStoreError::RawEventReconciliationMismatch { .. }
                ) && error.to_string().contains(expected_reason),
                "{label}: {error}",
            );
        }
    }

    #[tokio::test]
    async fn source_rebuild_rotates_three_generations_with_deterministic_parity() {
        let pool = open_v1_test_pool().await;
        install_unrelated_foreign_key_violation(&pool).await;
        let author = fixture_author();
        let target = signed_event(
            TARGET_CREATED_AT,
            KIND_LIST_SET_RELAY,
            vec![vec!["d".to_owned(), "rotation".to_owned()]],
            "{}",
        );
        let target_id = target.id_str().to_owned();
        seed_v1_raw_event(&pool, target, 1_000).await;
        seed_v1_raw_event(
            &pool,
            signed_event(
                REQUEST_CREATED_AT,
                KIND_DELETION_REQUEST,
                vec![vec!["e".to_owned(), target_id]],
                "remove",
            ),
            2_000,
        )
        .await;
        let raw_before = raw_authority_rows(&pool).await;

        install_v2_with_generation(&pool, [0x11; 32]).await;
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('stale-after-rebuild', 1, 0, 1)",
        )
        .execute(&pool)
        .await
        .expect("first-generation cursor");
        let first_history = generation_history_counts(&pool, [0x11; 32]).await;
        let first_state =
            addressable_state_for_generation(&pool, [0x11; 32], author.as_str(), "rotation").await;

        rotate_with_generation(&pool, [0x22; 32]).await;
        let second_state =
            addressable_state_for_generation(&pool, [0x22; 32], author.as_str(), "rotation").await;
        rotate_with_generation(&pool, [0x33; 32]).await;
        let third_state =
            addressable_state_for_generation(&pool, [0x33; 32], author.as_str(), "rotation").await;

        assert_eq!(first_state, second_state);
        assert_eq!(second_state, third_state);
        assert_eq!(raw_authority_rows(&pool).await, raw_before);
        assert_eq!(
            generation_history_counts(&pool, [0x11; 32]).await,
            first_history
        );
        let generations: Vec<(Vec<u8>, i64, i64)> = sqlx::query_as(
            "SELECT source_generation, generation_ordinal, transition_floor_seq FROM radroots_event_store_source_generation ORDER BY generation_ordinal",
        )
        .fetch_all(&pool)
        .await
        .expect("generation history");
        assert_eq!(
            generations,
            vec![
                (vec![0x11; 32], 1, 0),
                (vec![0x22; 32], 2, 1),
                (vec![0x33; 32], 3, 2),
            ]
        );
        let transitions: Vec<(i64, Vec<u8>)> = sqlx::query_as(
            "SELECT transition_seq, source_generation FROM radroots_event_store_addressable_head_transition ORDER BY transition_seq",
        )
        .fetch_all(&pool)
        .await
        .expect("transition history");
        assert_eq!(
            transitions,
            vec![
                (1, vec![0x11; 32]),
                (2, vec![0x22; 32]),
                (3, vec![0x33; 32]),
            ]
        );
        let cursor_generation: Vec<u8> = sqlx::query_scalar(
            "SELECT source_generation FROM radroots_event_store_projection_cursor_source WHERE projection_id = 'stale-after-rebuild'",
        )
        .fetch_one(&pool)
        .await
        .expect("stale cursor generation");
        let active_generation: Vec<u8> = sqlx::query_scalar(
            "SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("active generation");
        assert_eq!(cursor_generation, vec![0x11; 32]);
        assert_eq!(active_generation, vec![0x33; 32]);
        assert_ne!(cursor_generation, active_generation);
        assert_eq!(rebuild_marker_count(&pool).await, 0);

        let mut connection = pool.acquire().await.expect("validation connection");
        validate_applied_hook_state(&mut connection)
            .await
            .expect("third-generation deep validation");
        let foreign_key_rows = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut *connection)
            .await
            .expect("full foreign-key report");
        assert_eq!(foreign_key_rows.len(), 1);
        assert_eq!(
            foreign_key_rows[0]
                .try_get::<String, _>("table")
                .expect("child table"),
            "caller_child"
        );
    }

    #[tokio::test]
    async fn rebuild_integrity_ignores_unrelated_foreign_keys_and_types_owned_violations() {
        let pool = open_v1_test_pool().await;
        install_unrelated_foreign_key_violation(&pool).await;
        let mut connection = pool.acquire().await.expect("validation connection");
        validate_sqlite_integrity_after_rebuild(&mut connection)
            .await
            .expect("unrelated violation is outside event-store ownership");
        drop(connection);

        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .expect("disable foreign keys");
        sqlx::raw_sql(
            "CREATE TABLE radroots_event_store_owned_parent_probe (
  id INTEGER PRIMARY KEY
) STRICT;
CREATE TABLE radroots_event_store_owned_child_probe (
  id INTEGER PRIMARY KEY,
  parent_id INTEGER NOT NULL REFERENCES radroots_event_store_owned_parent_probe(id)
) STRICT;
INSERT INTO radroots_event_store_owned_child_probe(id, parent_id) VALUES (1, 999);",
        )
        .execute(&pool)
        .await
        .expect("owned orphan");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("enable foreign keys");

        let mut connection = pool.acquire().await.expect("validation connection");
        assert!(matches!(
            validate_sqlite_integrity_after_rebuild(&mut connection).await,
            Err(RadrootsEventStoreError::ForeignKeyViolation {
                table,
                ..
            }) if table == "radroots_event_store_owned_child_probe"
        ));
    }

    #[tokio::test]
    async fn source_rebuild_barrier_rolls_back_partial_state_and_guards_dml() {
        let pool = open_v1_test_pool().await;
        seed_v1_raw_event(
            &pool,
            signed_event(
                TARGET_CREATED_AT,
                KIND_LIST_SET_RELAY,
                vec![vec!["d".to_owned(), "barrier".to_owned()]],
                "{}",
            ),
            1_000,
        )
        .await;
        install_v2_with_generation(&pool, [0x41; 32]).await;
        let raw_before = raw_authority_rows(&pool).await;
        let history_before = generation_history_counts(&pool, [0x41; 32]).await;

        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("partial rebuild transaction");
        let plan = pending_rebuild_plan(&mut transaction, [0x42; 32]).await;
        open_source_rebuild_marker(&mut transaction, &plan)
            .await
            .expect("open marker");
        append_source_generation(&mut transaction, &plan)
            .await
            .expect("append generation");
        rotate_source_state(&mut transaction, &plan)
            .await
            .expect("rotate source state");

        assert!(matches!(
            validate_active_hook_state_fast(&mut transaction).await,
            Err(RadrootsEventStoreError::MigrationHookStateDrift { .. })
        ));
        let foreign_key_violations = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut *transaction)
            .await
            .expect("open-marker foreign key check");
        assert!(
            !foreign_key_violations.is_empty(),
            "open marker must hold the deferred commit barrier"
        );
        assert_database_error(
            sqlx::query(
                "DELETE FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1",
            )
            .execute(&mut *transaction)
            .await,
        );
        assert_database_error(
            sqlx::query(
                "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('blocked-during-rebuild', 1, 0, 1)",
            )
            .execute(&mut *transaction)
            .await,
        );
        assert_database_error(
            sqlx::query(
                "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) SELECT ?, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes ORDER BY seq LIMIT 1",
            )
            .bind("f".repeat(64))
            .execute(&mut *transaction)
            .await,
        );
        assert_database_error(
            sqlx::query(
                "INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed) SELECT event_id, 999, 'x', 'blocked', '[\"x\",\"blocked\"]', NULL, NULL, 0 FROM event_envelopes ORDER BY seq LIMIT 1",
            )
            .execute(&mut *transaction)
            .await,
        );
        sqlx::query("UPDATE event_envelopes SET updated_at_ms = updated_at_ms")
            .execute(&mut *transaction)
            .await
            .expect("derived reclassification is open during rebuild");
        let deleted_heads = sqlx::query("DELETE FROM event_envelope_head")
            .execute(&mut *transaction)
            .await
            .expect("raw head rebuild delete");
        assert_eq!(deleted_heads.rows_affected(), 1);
        assert_database_error(
            sqlx::query(
                "UPDATE radroots_event_store_event_coordinate SET event_seq = event_seq WHERE source_generation = ?",
            )
            .bind(vec![0x41; 32])
            .execute(&mut *transaction)
            .await,
        );
        assert_database_error(sqlx::query("COMMIT").execute(&mut *transaction).await);
        transaction
            .rollback()
            .await
            .expect("rollback rejected partial rebuild");

        assert_eq!(raw_authority_rows(&pool).await, raw_before);
        assert_eq!(
            generation_history_counts(&pool, [0x41; 32]).await,
            history_before
        );
        let generation_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_source_generation")
                .fetch_one(&pool)
                .await
                .expect("generation count after rollback");
        assert_eq!(generation_count, 1);
        assert_eq!(rebuild_marker_count(&pool).await, 0);

        assert_database_error(
            sqlx::query("UPDATE event_envelopes SET updated_at_ms = updated_at_ms")
                .execute(&pool)
                .await,
        );
        assert_database_error(
            sqlx::query("DELETE FROM event_envelope_head")
                .execute(&pool)
                .await,
        );
        assert_database_error(
            sqlx::query(
                "UPDATE radroots_event_store_source_state SET active_generation = active_generation WHERE singleton = 1",
            )
            .execute(&pool)
            .await,
        );
        assert_database_error(
            sqlx::query(
                "INSERT INTO radroots_event_store_source_generation(source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq) VALUES (?, 2, ?, ?, ?, ?, ?, 1, 1, 1, 1)",
            )
            .bind(vec![0x43; 32])
            .bind(NIP09_RECONCILIATION_VERSION)
            .bind(NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION)
            .bind(i64::from(
                NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION,
            ))
            .bind(NIP09_HOOK_ID)
            .bind(NIP09_RECONCILIATION_MANIFEST_SHA256)
            .execute(&pool)
            .await,
        );
    }

    #[tokio::test]
    async fn source_rebuild_entropy_collision_and_empty_ingest_are_atomic() {
        let pool = open_v1_test_pool().await;
        install_v2_with_generation(&pool, [0x71; 32]).await;
        rotate_with_generation(&pool, [0x72; 32]).await;
        let authority_before = raw_authority_rows(&pool).await;

        let mut entropy_transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("entropy transaction");
        assert!(matches!(
            apply_reconciliation_hook(
                &mut entropy_transaction,
                &FailingTestGeneration,
                ReconciliationCapacityLimits::production(),
            )
            .await,
            Err(RadrootsEventStoreError::SourceGenerationEntropyUnavailable)
        ));
        entropy_transaction
            .rollback()
            .await
            .expect("entropy rollback");

        let mut collision_transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("collision transaction");
        assert!(matches!(
            apply_reconciliation_hook(
                &mut collision_transaction,
                &FixedTestGeneration([0x72; 32]),
                ReconciliationCapacityLimits::production(),
            )
            .await,
            Err(RadrootsEventStoreError::MigrationHookStateDrift { .. })
        ));
        collision_transaction
            .rollback()
            .await
            .expect("collision rollback");
        assert_eq!(raw_authority_rows(&pool).await, authority_before);
        assert_eq!(rebuild_marker_count(&pool).await, 0);
        let generation_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_source_generation")
                .fetch_one(&pool)
                .await
                .expect("generation count");
        assert_eq!(generation_count, 2);

        append_regular_event_after_rebuild(&pool).await;
        let authority: (i64, i64, i64) = sqlx::query_as(
            "SELECT raw_event_count, raw_tag_count, raw_high_water_seq FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("post-rebuild authority");
        assert_eq!(authority, (1, 0, 1));
        let mut connection = pool.acquire().await.expect("validation connection");
        validate_applied_hook_state(&mut connection)
            .await
            .expect("empty-store post-rebuild ingest validation");
    }

    #[test]
    fn request_index_reduces_maximum_shape_requests_once_and_decides_by_lookup() {
        let author = fixture_author();
        let address_tags = (0..RADROOTS_NIP09_DELETION_TAG_MAX_COUNT)
            .map(|index| {
                vec![
                    "a".to_owned(),
                    coordinate(author.as_str(), fanout_identifier(index).as_str()),
                ]
            })
            .collect::<Vec<_>>();
        let address_request =
            admitted_request(REQUEST_CREATED_AT, address_tags, "maximum address fanout");
        let event_tags = (0..RADROOTS_NIP09_DELETION_TAG_MAX_COUNT)
            .map(|index| vec!["e".to_owned(), format!("{:064x}", index + 1)])
            .collect::<Vec<_>>();
        let event_request =
            admitted_request(REQUEST_CREATED_AT + 1, event_tags, "maximum event fanout");
        assert_eq!(
            address_request.event().tag_slices().len(),
            RADROOTS_NIP09_DELETION_TAG_MAX_COUNT
        );
        assert_eq!(
            address_request.projection().address_targets().len(),
            RADROOTS_NIP09_DELETION_TAG_MAX_COUNT
        );
        assert_eq!(
            event_request.projection().event_targets().len(),
            RADROOTS_NIP09_DELETION_TAG_MAX_COUNT
        );
        let request_id = address_request.event().id_str().to_owned();
        let requests = vec![address_request, event_request];
        let request_index = RequestIndex::new(&requests);

        assert_eq!(
            request_index.event_targets.len(),
            RADROOTS_NIP09_DELETION_TAG_MAX_COUNT
        );
        assert_eq!(
            request_index.address_targets.len(),
            RADROOTS_NIP09_DELETION_TAG_MAX_COUNT
        );
        assert!(
            request_index
                .address_targets
                .values()
                .all(|evidence| { evidence.authorized.is_some() && !evidence.unauthorized })
        );

        for index in 0..RADROOTS_NIP09_DELETION_TAG_MAX_COUNT {
            let target = unsigned_addressable_target(author.as_str(), index);
            let decision = request_index.decision(&target).expect("indexed decision");
            assert_eq!(
                decision.outcome,
                RadrootsNip09SuppressionOutcome::Suppressed
            );
            assert_eq!(
                decision.reason,
                RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff.code()
            );
            assert_eq!(
                decision.address_reference_request_id.as_deref(),
                Some(request_id.as_str())
            );
        }

        let identifier = fanout_identifier(0);
        let target = verify_nip01_event_v1(signed_event(
            TARGET_CREATED_AT,
            KIND_LIST_SET_RELAY,
            vec![vec!["d".to_owned(), identifier.clone()]],
            "{}",
        ))
        .expect("verified target");
        let admission =
            EventAdmission::for_profile(ReconciliationProfile::Nip09V1RegistryV7, &target)
                .expect("target admission");
        assert_eq!(admission.status, RadrootsEventAdmissionStatus::Admitted);
        let event = ReconciledEvent {
            seq: 1,
            inserted_at_ms: 1,
            verified_event: target,
            admission,
        };
        let state = addressable_state_for_event(
            i64::from(KIND_LIST_SET_RELAY),
            author.as_str(),
            identifier.as_str(),
            event.seq,
            &event,
            &request_index,
        )
        .expect("suppressed addressable state");

        assert_eq!(state.visibility, "suppressed");
        assert_eq!(
            state.nip09_reason.as_deref(),
            Some("deletion_address_reference")
        );
        assert_eq!(
            state.address_reference_request_id.as_deref(),
            Some(request_id.as_str())
        );
        assert_eq!(
            state.address_reference_cutoff,
            Some(i64::try_from(REQUEST_CREATED_AT).expect("request timestamp"))
        );
    }

    #[test]
    fn request_index_reduces_event_and_address_evidence_canonically() {
        let author = fixture_author();
        let target = verify_nip01_event_v1(signed_event(
            TARGET_CREATED_AT,
            KIND_LIST_SET_RELAY,
            vec![vec!["d".to_owned(), fanout_identifier(0)]],
            "{}",
        ))
        .expect("verified target");
        let target_coordinate = coordinate(author.as_str(), fanout_identifier(0).as_str());
        let mut requests = vec![
            admitted_request(
                REQUEST_CREATED_AT,
                vec![
                    vec!["e".to_owned(), target.event().id_str().to_owned()],
                    vec!["a".to_owned(), target_coordinate.clone()],
                ],
                "both",
            ),
            admitted_request(
                REQUEST_CREATED_AT + 1,
                vec![vec!["e".to_owned(), target.event().id_str().to_owned()]],
                "event",
            ),
            admitted_request(
                REQUEST_CREATED_AT + 2,
                vec![vec!["a".to_owned(), target_coordinate]],
                "address",
            ),
        ];
        requests.sort_by(|left, right| left.event().id().cmp(right.event().id()));
        let request_index = RequestIndex::new(&requests);
        let coordinate_key = nip01_coordinate_key(target.event()).expect("target coordinate");

        assert_eq!(
            request_index
                .event_targets
                .get(target.event().id_str())
                .expect("event indices")
                .len(),
            1
        );
        let address_evidence = request_index
            .address_targets
            .get(&coordinate_key)
            .expect("address evidence");
        assert!(address_evidence.authorized.is_some());
        assert!(!address_evidence.unauthorized);
        let expected_event_request_id = requests
            .iter()
            .filter(|request| {
                request
                    .projection()
                    .event_targets()
                    .iter()
                    .any(|event_target| event_target.event_id().as_str() == target.event().id_str())
            })
            .map(|request| request.event().id_str())
            .min()
            .expect("event evidence");
        let expected_address_request_id = requests
            .iter()
            .filter(|request| {
                request
                    .projection()
                    .address_targets()
                    .iter()
                    .any(|address_target| {
                        i64::from(address_target.coordinate().kind()) == coordinate_key.0
                            && address_target.coordinate().pubkey().as_str()
                                == coordinate_key.1.as_str()
                            && address_target.coordinate().identifier() == coordinate_key.2.as_str()
                    })
            })
            .max_by(|left, right| {
                left.event()
                    .created_at_u64()
                    .cmp(&right.event().created_at_u64())
                    .then_with(|| right.event().id().cmp(left.event().id()))
            })
            .map(|request| request.event().id_str())
            .expect("address evidence");
        let decision = request_index
            .decision(target.event())
            .expect("indexed decision");
        assert_eq!(
            decision.reason,
            RadrootsNip09SuppressionReason::EventIdAndAddressReference.code()
        );
        assert_eq!(
            decision.event_reference_request_id.as_deref(),
            Some(expected_event_request_id)
        );
        assert_eq!(
            decision.address_reference_request_id.as_deref(),
            Some(expected_address_request_id)
        );

        let mut reversed = requests.clone();
        reversed.reverse();
        reversed.push(requests[0].clone());
        assert_eq!(
            RequestIndex::new(&reversed)
                .decision(target.event())
                .expect("reversed repeated decision"),
            decision
        );
        assert_request_index_matches_protocol(&target, &requests, &request_index);
        assert_request_index_matches_protocol(&target, &reversed, &RequestIndex::new(&reversed));
    }

    #[test]
    fn expected_transition_history_replays_incremental_heads_and_requests() {
        fn reconciled(
            seq: i64,
            observed_at_ms: i64,
            envelope: RadrootsEventEnvelope,
        ) -> ReconciledEvent {
            let ingest = ingest_for_test(envelope, observed_at_ms);
            let admission = EventAdmission::for_profile(
                ReconciliationProfile::Nip09V1RegistryV7,
                ingest.verified_event(),
            )
            .expect("fixture admission");
            ReconciledEvent {
                seq,
                inserted_at_ms: observed_at_ms,
                verified_event: ingest.verified_event().clone(),
                admission,
            }
        }

        let author = fixture_author();
        let identifier = "incremental-history";
        let baseline = signed_event(
            TARGET_CREATED_AT,
            KIND_LIST_SET_RELAY,
            vec![vec!["d".to_owned(), identifier.to_owned()]],
            "{}",
        );
        let replacement = signed_event(
            TARGET_CREATED_AT + 10,
            KIND_LIST_SET_RELAY,
            vec![vec!["d".to_owned(), identifier.to_owned()]],
            "{}",
        );
        let replacement_id = replacement.id_str().to_owned();
        let older = signed_event(
            TARGET_CREATED_AT - 1,
            KIND_LIST_SET_RELAY,
            vec![vec!["d".to_owned(), identifier.to_owned()]],
            "{}",
        );
        let replaceable = signed_event(TARGET_CREATED_AT + 20, 0, Vec::new(), "{}");
        let replaceable_id = replaceable.id_str().to_owned();
        let deletion = signed_event(
            TARGET_CREATED_AT + 30,
            KIND_DELETION_REQUEST,
            vec![
                vec!["e".to_owned(), "0".repeat(64)],
                vec!["e".to_owned(), replacement_id.clone()],
                vec!["e".to_owned(), replaceable_id],
                vec!["a".to_owned(), coordinate(author.as_str(), identifier)],
            ],
            "remove current head",
        );
        let deletion_id = deletion.id_str().to_owned();
        let events = vec![
            reconciled(1, 1_000, baseline),
            reconciled(2, 2_000, replacement),
            reconciled(3, 3_000, older),
            reconciled(4, 4_000, replaceable),
            reconciled(5, 5_000, deletion),
        ];
        let source = SourceState {
            generation: RadrootsEventStoreSourceGeneration::from_bytes([0x71; 32]),
            profile: ReconciliationProfile::Nip09V1RegistryV7,
            raw_event_count: 5,
            raw_tag_count: 7,
            raw_high_water_seq: 5,
            last_transition_seq: 3,
            transition_floor_seq: 0,
            baseline_raw_event_count: 1,
            baseline_raw_tag_count: 1,
            baseline_raw_high_water_seq: 1,
        };

        let transitions =
            expected_transition_history(&source, &events).expect("incremental history");
        assert_eq!(transitions.len(), 3, "{transitions:#?}");
        assert_eq!(transitions[0].origin, "baseline");
        assert_eq!(transitions[0].raw_head_decision, "baseline_rebuild");
        assert_eq!(transitions[1].origin, "incremental");
        assert_eq!(transitions[1].raw_head_decision, "applied");
        assert_eq!(transitions[1].raw_head_event_id, replacement_id);
        assert_eq!(transitions[2].origin, "incremental");
        assert_eq!(transitions[2].raw_head_decision, "not_head_selected");
        assert_eq!(transitions[2].visibility, "suppressed");
        assert_eq!(
            transitions[2].event_reference_request_id.as_deref(),
            Some(deletion_id.as_str())
        );
        assert_eq!(
            transitions[2].address_reference_request_id.as_deref(),
            Some(deletion_id.as_str())
        );
        assert_eq!(
            transitions[2].retracted_event_id.as_deref(),
            Some(replacement_id.as_str())
        );
    }

    type RawEventRows = Vec<(
        i64,
        String,
        String,
        i64,
        i64,
        String,
        String,
        String,
        String,
        i64,
    )>;
    type RawTagRows = Vec<(String, i64, String, Option<String>, String)>;

    async fn open_v1_test_pool() -> SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("memory options")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("memory pool");
        sqlx::raw_sql(EVENT_STORE_V1_UP_SQL)
            .execute(&pool)
            .await
            .expect("v1 schema");
        pool
    }

    async fn install_unrelated_foreign_key_violation(pool: &SqlitePool) {
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(pool)
            .await
            .expect("disable foreign keys");
        sqlx::raw_sql(
            "CREATE TABLE caller_parent (id INTEGER PRIMARY KEY) STRICT;
CREATE TABLE caller_child (
  id INTEGER PRIMARY KEY,
  parent_id INTEGER NOT NULL REFERENCES caller_parent(id)
) STRICT;
INSERT INTO caller_child(id, parent_id) VALUES (1, 999);",
        )
        .execute(pool)
        .await
        .expect("unrelated orphan");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(pool)
            .await
            .expect("enable foreign keys");
    }

    async fn install_v2_with_generation(pool: &SqlitePool, generation: [u8; 32]) {
        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("v2 transaction");
        sqlx::raw_sql(EVENT_STORE_V2_UP_SQL)
            .execute(&mut *transaction)
            .await
            .expect("v2 schema");
        apply_reconciliation_hook(
            &mut transaction,
            &FixedTestGeneration(generation),
            ReconciliationCapacityLimits::production(),
        )
        .await
        .expect("initial reconciliation");
        validate_applied_hook_state(&mut transaction)
            .await
            .expect("initial deep validation");
        transaction.commit().await.expect("v2 commit");
    }

    async fn populated_v2_validation_pool() -> SqlitePool {
        let pool = open_v1_test_pool().await;
        let author = fixture_author();
        seed_v1_raw_event(
            &pool,
            signed_event(TARGET_CREATED_AT - 3, 0, Vec::new(), "{}"),
            500,
        )
        .await;
        seed_v1_raw_event(
            &pool,
            signed_event(TARGET_CREATED_AT - 2, KIND_POST, Vec::new(), "regular"),
            600,
        )
        .await;
        seed_v1_raw_event(
            &pool,
            signed_event(TARGET_CREATED_AT - 1, 20_001, Vec::new(), "ephemeral"),
            700,
        )
        .await;
        let target = signed_event(
            TARGET_CREATED_AT,
            KIND_LIST_SET_RELAY,
            vec![vec!["d".to_owned(), "deep-validation".to_owned()]],
            "{}",
        );
        let target_id = target.id_str().to_owned();
        seed_v1_raw_event(&pool, target, 1_000).await;
        seed_v1_raw_event(
            &pool,
            signed_event(
                REQUEST_CREATED_AT,
                KIND_DELETION_REQUEST,
                vec![
                    vec!["e".to_owned(), target_id],
                    vec![
                        "a".to_owned(),
                        coordinate(author.as_str(), "deep-validation"),
                    ],
                ],
                "remove",
            ),
            2_000,
        )
        .await;
        install_v2_with_generation(&pool, [0x61; 32]).await;
        pool
    }

    async fn deep_reconciliation_error_after_corruption(
        guard_drops: &[&'static str],
        mutation: &'static str,
    ) -> RadrootsEventStoreError {
        let pool = populated_v2_validation_pool().await;
        let mut connection = pool.acquire().await.expect("corruption connection");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("disable foreign keys");
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *connection)
            .await
            .expect("disable check constraints");
        for guard_drop in guard_drops {
            sqlx::raw_sql(*guard_drop)
                .execute(&mut *connection)
                .await
                .expect("drop trusted mutation guard");
        }
        sqlx::raw_sql(mutation)
            .execute(&mut *connection)
            .await
            .expect("apply trusted corruption");
        sqlx::query("PRAGMA ignore_check_constraints = OFF")
            .execute(&mut *connection)
            .await
            .expect("restore check constraints");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("restore foreign keys");
        validate_applied_hook_state(&mut connection)
            .await
            .expect_err("deep validation must reject trusted corruption")
    }

    async fn rotate_with_generation(pool: &SqlitePool, generation: [u8; 32]) {
        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("rotation transaction");
        apply_reconciliation_hook(
            &mut transaction,
            &FixedTestGeneration(generation),
            ReconciliationCapacityLimits::production(),
        )
        .await
        .expect("reconciliation rotation");
        transaction.commit().await.expect("rotation commit");
    }

    async fn seed_v1_raw_event(
        pool: &SqlitePool,
        envelope: RadrootsEventEnvelope,
        observed_at_ms: i64,
    ) {
        let ingest = ingest_for_test(envelope, observed_at_ms);
        let event = ingest.event();
        let admission = EventAdmission::for_profile(
            ReconciliationProfile::Nip09V1RegistryV7,
            ingest.verified_event(),
        )
        .expect("test admission");
        let tags = event.tags_as_vec();
        let tags_json = serde_json::to_string(&tags).expect("tags JSON");
        let event_class = StoredEventClass::from_event_kind_class(event.kind_class());
        let inserted = sqlx::query(
            "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'verified', ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.id_str())
        .bind(event.author_str())
        .bind(i64::try_from(event.created_at_u64()).expect("created_at"))
        .bind(i64::from(event.kind_u32()))
        .bind(tags_json)
        .bind(event.content())
        .bind(event.sig_str())
        .bind(ingest.raw_json())
        .bind(admission.status.as_str())
        .bind(admission.contract.map(|contract| contract.id))
        .bind(event_class.as_str())
        .bind(i64::from(
            admission.valid_stream_eligible(event.kind_class()),
        ))
        .bind(observed_at_ms)
        .bind(observed_at_ms)
        .execute(pool)
        .await
        .expect("v1 event seed");
        assert_eq!(inserted.rows_affected(), 1);

        for (index, tag) in tags.iter().enumerate() {
            let name = tag.first().map(String::as_str).unwrap_or("");
            let value = tag.get(1).map(String::as_str);
            let contract_tag = admission.contract.and_then(|contract| {
                contract
                    .tags
                    .iter()
                    .find(|candidate| candidate.name == name)
            });
            let inserted = sqlx::query(
                "INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(event.id_str())
            .bind(i64::try_from(index).expect("tag index"))
            .bind(name)
            .bind(value)
            .bind(serde_json::to_string(tag).expect("tag JSON"))
            .bind(contract_tag.map(|tag| tag_semantic_name(tag.semantic)))
            .bind(contract_tag.map(|tag| tag_value_type_name(tag.value_type)))
            .bind(i64::from(
                contract_tag.map(|tag| tag.relay_indexed).unwrap_or(false),
            ))
            .execute(pool)
            .await
            .expect("v1 tag seed");
            assert_eq!(inserted.rows_affected(), 1);
        }
    }

    fn ingest_for_test(
        envelope: RadrootsEventEnvelope,
        observed_at_ms: i64,
    ) -> RadrootsEventIngest {
        let wire = RadrootsNip01EventWire {
            id: envelope.id_str().to_owned(),
            pubkey: envelope.author_str().to_owned(),
            created_at: envelope.created_at_u64(),
            kind: envelope.kind_u32(),
            tags: envelope.tags_as_vec(),
            content: envelope.content().to_owned(),
            sig: envelope.sig_str().to_owned(),
            extra: Default::default(),
        };
        let raw_json = serde_json::to_string(&wire).expect("wire JSON");
        RadrootsEventIngest::from_raw_json(raw_json, observed_at_ms).expect("verified test ingest")
    }

    async fn raw_authority_rows(pool: &SqlitePool) -> (RawEventRows, RawTagRows) {
        let events = sqlx::query_as(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, inserted_at_ms FROM event_envelopes ORDER BY seq",
        )
        .fetch_all(pool)
        .await
        .expect("raw event rows");
        let tags = sqlx::query_as(
            "SELECT event_id, tag_index, tag_name, tag_value, tag_json FROM event_envelope_tags ORDER BY event_id, tag_index",
        )
        .fetch_all(pool)
        .await
        .expect("raw tag rows");
        (events, tags)
    }

    async fn generation_history_counts(
        pool: &SqlitePool,
        generation: [u8; 32],
    ) -> (i64, i64, i64, i64, i64, i64) {
        sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM radroots_event_store_event_coordinate WHERE source_generation = ?), (SELECT COUNT(*) FROM radroots_event_store_nip09_request WHERE source_generation = ?), (SELECT COUNT(*) FROM radroots_event_store_nip09_event_target WHERE source_generation = ?), (SELECT COUNT(*) FROM radroots_event_store_nip09_address_target WHERE source_generation = ?), (SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition WHERE source_generation = ?), (SELECT COUNT(*) FROM radroots_event_store_addressable_head_state WHERE source_generation = ?)",
        )
        .bind(generation.as_slice())
        .bind(generation.as_slice())
        .bind(generation.as_slice())
        .bind(generation.as_slice())
        .bind(generation.as_slice())
        .bind(generation.as_slice())
        .fetch_one(pool)
        .await
        .expect("generation history counts")
    }

    async fn addressable_state_for_generation(
        pool: &SqlitePool,
        generation: [u8; 32],
        author: &str,
        d_tag: &str,
    ) -> AddressableHeadState {
        let mut connection = pool.acquire().await.expect("state connection");
        read_addressable_state(
            &mut connection,
            RadrootsEventStoreSourceGeneration::from_bytes(generation),
            i64::from(KIND_LIST_SET_RELAY),
            author,
            d_tag,
        )
        .await
        .expect("addressable state read")
        .expect("addressable state")
    }

    async fn pending_rebuild_plan(
        connection: &mut SqliteConnection,
        generation: [u8; 32],
    ) -> SourceRebuildPlan {
        let prior = read_source_state(connection)
            .await
            .expect("prior source state");
        let generation_ordinal: i64 = sqlx::query_scalar(
            "SELECT MAX(generation_ordinal) + 1 FROM radroots_event_store_source_generation",
        )
        .fetch_one(&mut *connection)
        .await
        .expect("next generation ordinal");
        let transition_floor_seq: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(transition_seq), 0) FROM radroots_event_store_addressable_head_transition",
        )
        .fetch_one(&mut *connection)
        .await
        .expect("transition floor");
        SourceRebuildPlan {
            generation: RadrootsEventStoreSourceGeneration::from_bytes(generation),
            generation_ordinal,
            transition_floor_seq,
            raw_event_count: prior.raw_event_count,
            raw_tag_count: prior.raw_tag_count,
            raw_high_water_seq: prior.raw_high_water_seq,
            prior: Some(prior),
        }
    }

    async fn rebuild_marker_count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker")
            .fetch_one(pool)
            .await
            .expect("rebuild marker count")
    }

    fn assert_database_error<T>(result: Result<T, sqlx::Error>) {
        assert!(
            matches!(result, Err(sqlx::Error::Database(_))),
            "expected SQLite to reject direct DML"
        );
    }

    async fn append_regular_event_after_rebuild(pool: &SqlitePool) {
        let ingest = ingest_for_test(
            signed_event(
                REQUEST_CREATED_AT + 10,
                KIND_POST,
                Vec::new(),
                "post-rebuild",
            ),
            3_000,
        );
        let admission = EventAdmission::for_profile(
            ReconciliationProfile::Nip09V1RegistryV7,
            ingest.verified_event(),
        )
        .expect("post admission");
        let event = ingest.event();
        let event_class = StoredEventClass::from_event_kind_class(event.kind_class());
        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("post-rebuild append transaction");
        validate_source_raw_authority(&mut transaction)
            .await
            .expect("pre-append authority");
        let inserted = sqlx::query(
            "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, '[]', ?, ?, ?, 'verified', ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.id_str())
        .bind(event.author_str())
        .bind(i64::try_from(event.created_at_u64()).expect("created_at"))
        .bind(i64::from(event.kind_u32()))
        .bind(event.content())
        .bind(event.sig_str())
        .bind(ingest.raw_json())
        .bind(admission.status.as_str())
        .bind(admission.contract.map(|contract| contract.id))
        .bind(event_class.as_str())
        .bind(i64::from(
            admission.valid_stream_eligible(event.kind_class()),
        ))
        .bind(ingest.observed_at_ms())
        .bind(ingest.observed_at_ms())
        .execute(&mut *transaction)
        .await
        .expect("post-rebuild raw append");
        let inserted_seq = inserted.last_insert_rowid();
        persist_event_coordinate_after_insert(&mut transaction, &ingest, &admission, inserted_seq)
            .await
            .expect("post-rebuild coordinate");
        synchronize_after_insert(
            &mut transaction,
            &ingest,
            &admission,
            inserted_seq,
            event.id_str(),
            0,
            &RadrootsRawHeadDecision::NotPersisted,
        )
        .await
        .expect("post-rebuild source synchronization");
        transaction.commit().await.expect("post-rebuild commit");
    }

    fn fixture_author() -> String {
        Keys::parse(FIXTURE_SECRET_KEY_HEX)
            .expect("fixture key")
            .public_key()
            .to_string()
    }

    fn fanout_identifier(index: usize) -> String {
        format!("fanout-{index:04}")
    }

    fn coordinate(author: &str, identifier: &str) -> String {
        format!("{KIND_LIST_SET_RELAY}:{author}:{identifier}")
    }

    fn admitted_request(
        created_at: u64,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> RadrootsAdmittedNip09DeletionRequestEventV1 {
        let verified = verify_nip01_event_v1(signed_event(
            created_at,
            KIND_DELETION_REQUEST,
            tags,
            content,
        ))
        .expect("verified request");
        admit_verified_nip09_deletion_request_event_v1(verified).expect("admitted request")
    }

    fn assert_request_index_matches_protocol(
        target: &RadrootsSignatureVerifiedEvent,
        requests: &[RadrootsAdmittedNip09DeletionRequestEventV1],
        index: &RequestIndex,
    ) {
        let expected = evaluate_nip09_suppression_from_borrowed_requests_v1(target, requests);
        let actual = index.decision(target.event()).expect("indexed decision");
        assert_eq!(actual.outcome, expected.outcome());
        assert_eq!(actual.reason, expected.reason().code());
        assert_eq!(
            actual.event_reference_request_id.as_deref(),
            expected
                .event_reference()
                .map(|evidence| evidence.request_id().as_str())
        );
        assert_eq!(
            actual.address_reference_request_id.as_deref(),
            expected
                .address_reference()
                .map(|evidence| evidence.request_id().as_str())
        );
        assert_eq!(
            actual.address_reference_cutoff,
            expected
                .address_reference()
                .map(|evidence| i64::try_from(evidence.inclusive_cutoff()).expect("cutoff range"))
        );
    }

    fn unsigned_addressable_target(author: &str, index: usize) -> RadrootsEventEnvelope {
        let tags = vec![vec!["d".to_owned(), fanout_identifier(index)]];
        let id = compute_canonical_nip01_event_id(
            author,
            TARGET_CREATED_AT,
            KIND_LIST_SET_RELAY,
            &tags,
            "{}",
        )
        .expect("target id");
        RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: id.into_string(),
            author: author.to_owned(),
            created_at: TARGET_CREATED_AT,
            kind: KIND_LIST_SET_RELAY,
            tags,
            content: "{}".to_owned(),
            sig: "0".repeat(128),
        })
        .expect("target envelope")
    }

    fn signed_event(
        created_at: u64,
        kind: u32,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> RadrootsEventEnvelope {
        let keys = Keys::parse(FIXTURE_SECRET_KEY_HEX).expect("fixture key");
        let author = keys.public_key().to_string();
        let id =
            compute_canonical_nip01_event_id(author.as_str(), created_at, kind, &tags, content)
                .expect("event id");
        let nostr_id = nostr::EventId::from_hex(id.as_str()).expect("Nostr event id");
        let message = Message::from_digest(nostr_id.to_bytes());
        let signature = SECP256K1.sign_schnorr_no_aux_rand(&message, keys.key_pair(SECP256K1));
        RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: id.into_string(),
            author,
            created_at,
            kind,
            tags,
            content: content.to_owned(),
            sig: signature.to_string(),
        })
        .expect("signed event")
    }
}
