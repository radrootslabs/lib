use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
#[cfg(feature = "transport-workers")]
use std::sync::{Mutex, MutexGuard};

use radroots_event::{draft::RadrootsVerifiedSignedEvent, wire::RadrootsNip01EventWire};
#[cfg(feature = "transport-workers")]
use radroots_transport::RadrootsTransportTargetReceipt;
use radroots_transport::{
    RadrootsTransport, RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
    RadrootsTransportError, RadrootsTransportKind, RadrootsTransportPayload,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportSatisfactionPolicyKind,
    RadrootsTransportTarget, RadrootsTransportTargetSet,
};
use thiserror::Error;

pub type RadrootsRuntimeTransportFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RadrootsRuntimeTransportError>> + Send + 'a>>;

pub const RADROOTS_RUNTIME_DELIVERY_QUEUE_DEFAULT_CAPACITY: usize = 64;
pub const RADROOTS_RUNTIME_DELIVERY_QUEUE_MAX_CAPACITY: usize = 1_024;

#[derive(Debug, Error)]
pub enum RadrootsRuntimeTransportError {
    #[error("transport `{0}` is not registered")]
    TransportNotRegistered(String),

    #[error("transport `{0}` is already registered")]
    TransportAlreadyRegistered(String),

    #[error("transport dispatch received no targets")]
    EmptyDispatchTargets,

    #[error("transport target error: {0}")]
    TransportTarget(String),

    #[error("runtime queue capacity {actual} is outside the supported range 1..={max}")]
    InvalidQueueCapacity { actual: usize, max: usize },

    #[error("runtime queue capacity {capacity} is exhausted")]
    QueueCapacityExhausted { capacity: usize },

    #[error("runtime queue is shut down")]
    QueueShutdown,

    #[error("runtime queue sequence space is exhausted")]
    QueueSequenceExhausted,

    #[error("runtime queue completion has no matching in-flight task")]
    QueueCompletionUnmatched,

    #[error("runtime queue state lock is poisoned")]
    QueueStatePoisoned,

    #[error("runtime {field} id must be positive, received {value}")]
    InvalidIdentifier { field: &'static str, value: i64 },

    #[error("runtime {field} timestamp must not be negative, received {value}")]
    InvalidTimestamp { field: &'static str, value: i64 },

    #[error("runtime delivery job must contain at least one plan")]
    EmptyDeliveryJob,

    #[error("runtime delivery plan must contain at least one target")]
    EmptyDeliveryPlan,

    #[error("runtime delivery job contains duplicate plan id {delivery_plan_id}")]
    DuplicateDeliveryPlanId { delivery_plan_id: i64 },

    #[error("runtime delivery plan contains duplicate target id {delivery_target_id}")]
    DuplicateDeliveryTargetId { delivery_target_id: i64 },

    #[error(
        "runtime delivery target {delivery_target_id} has incoherent initial status {status:?}"
    )]
    InvalidDeliveryTargetInitialStatus {
        delivery_target_id: i64,
        status: RadrootsTransportDeliveryTargetStatus,
    },

    #[error("runtime lease {record_id} has incoherent claimed/recovered state")]
    InvalidLeaseState { record_id: i64 },

    #[error(
        "inbound observation event `{observation_event_id}` does not match signed event `{signed_event_id}`"
    )]
    InboundObservationEventMismatch {
        observation_event_id: String,
        signed_event_id: String,
    },

    #[error("transport `{kind}` failed: {message}")]
    Transport { kind: String, message: String },

    #[cfg(feature = "transport-workers")]
    #[error("transport `{kind}` failed after a partial delivery outcome: {message}")]
    PartialDelivery {
        kind: String,
        message: String,
        partial_receipt: Box<RadrootsRuntimeDeliveryJobReceipt>,
    },
}

impl RadrootsRuntimeTransportError {
    #[cfg(feature = "transport-workers")]
    pub fn partial_delivery_receipt(&self) -> Option<&RadrootsRuntimeDeliveryJobReceipt> {
        match self {
            Self::PartialDelivery {
                partial_receipt, ..
            } => Some(partial_receipt),
            _ => None,
        }
    }
}

impl From<RadrootsTransportError> for RadrootsRuntimeTransportError {
    fn from(value: RadrootsTransportError) -> Self {
        Self::TransportTarget(value.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsRuntimeTransportPayload {
    SignedEvent(Box<RadrootsVerifiedSignedEvent>),
    OpaqueBytes { label: String, bytes: Vec<u8> },
}

impl RadrootsRuntimeTransportPayload {
    pub fn signed_event(event: RadrootsVerifiedSignedEvent) -> Self {
        Self::SignedEvent(Box::new(event))
    }

    pub fn verified_signed_event_json(
        event: &RadrootsVerifiedSignedEvent,
    ) -> Result<RadrootsTransportPayload, RadrootsTransportError> {
        verify_signed_event_raw_json_matches_event(event)?;
        let signed_event = event.signed_event();
        RadrootsTransportPayload::unchecked_signed_event_json(
            signed_event.id_str(),
            signed_event.raw_json(),
        )
    }

    pub fn transport_payload(&self) -> Result<RadrootsTransportPayload, RadrootsTransportError> {
        match self {
            Self::SignedEvent(event) => Self::verified_signed_event_json(event),
            Self::OpaqueBytes { label, bytes } => {
                RadrootsTransportPayload::opaque_bytes(label, bytes)
            }
        }
    }
}

fn verify_signed_event_raw_json_matches_event(
    event: &RadrootsVerifiedSignedEvent,
) -> Result<(), RadrootsTransportError> {
    let signed_event = event.signed_event();
    let wire = RadrootsNip01EventWire::parse_json(signed_event.raw_json())
        .map_err(|_| RadrootsTransportError::InvalidPayloadBytes)?;
    if wire.id.as_str() != signed_event.id_str() {
        return Err(RadrootsTransportError::InvalidPayloadId);
    }
    (wire == *signed_event.wire())
        .then_some(())
        .ok_or(RadrootsTransportError::InvalidPayloadBytes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeTransportDispatchRequest {
    request_id: String,
    payload: RadrootsRuntimeTransportPayload,
    target_set: RadrootsTransportTargetSet,
    satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    now_ms: i64,
}

impl RadrootsRuntimeTransportDispatchRequest {
    pub fn new(
        request_id: impl Into<String>,
        payload: RadrootsRuntimeTransportPayload,
        targets: Vec<RadrootsTransportTarget>,
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
        now_ms: i64,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        if targets.is_empty() {
            return Err(RadrootsRuntimeTransportError::EmptyDispatchTargets);
        }
        let target_set = RadrootsTransportTargetSet::new(targets)?;
        let request_id = request_id.into();
        RadrootsTransportDeliveryRequest::new(
            request_id.clone(),
            payload.transport_payload()?,
            target_set.clone(),
            satisfaction_policy.clone(),
        )?
        .try_with_now_ms(now_ms)?;
        Ok(Self {
            request_id,
            payload,
            target_set,
            satisfaction_policy,
            now_ms,
        })
    }

    pub fn transport_delivery_request(
        &self,
    ) -> Result<RadrootsTransportDeliveryRequest, RadrootsRuntimeTransportError> {
        Ok(RadrootsTransportDeliveryRequest::new(
            self.request_id.clone(),
            self.payload.transport_payload()?,
            self.target_set.clone(),
            self.satisfaction_policy.clone(),
        )?
        .try_with_now_ms(self.now_ms)?)
    }

    pub fn request_id(&self) -> &str {
        self.request_id.as_str()
    }

    pub fn target_set(&self) -> &RadrootsTransportTargetSet {
        &self.target_set
    }

    pub fn satisfaction_policy(&self) -> &RadrootsTransportSatisfactionPolicy {
        &self.satisfaction_policy
    }

    pub fn now_ms(&self) -> i64 {
        self.now_ms
    }
}

#[derive(Clone, Default)]
pub struct RadrootsRuntimeTransportRegistry {
    transports: BTreeMap<RadrootsTransportKind, Arc<dyn RadrootsTransport>>,
}

impl RadrootsRuntimeTransportRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T>(&mut self, transport: T) -> Result<(), RadrootsRuntimeTransportError>
    where
        T: RadrootsTransport + 'static,
    {
        let kind = transport.transport_kind();
        if self.transports.contains_key(&kind) {
            return Err(RadrootsRuntimeTransportError::TransportAlreadyRegistered(
                kind.canonical_label(),
            ));
        }
        self.transports.insert(kind, Arc::new(transport));
        Ok(())
    }

    pub fn transport(
        &self,
        kind: &RadrootsTransportKind,
    ) -> Result<Arc<dyn RadrootsTransport>, RadrootsRuntimeTransportError> {
        self.transports.get(kind).cloned().ok_or_else(|| {
            RadrootsRuntimeTransportError::TransportNotRegistered(kind.canonical_label())
        })
    }

    pub fn registered_kinds(&self) -> Vec<RadrootsTransportKind> {
        self.transports.keys().cloned().collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeQueueStatus {
    capacity: usize,
    queued: usize,
    in_flight: usize,
    shutdown: bool,
}

impl RadrootsRuntimeQueueStatus {
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn queued(&self) -> usize {
        self.queued
    }

    pub fn in_flight(&self) -> usize {
        self.in_flight
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeQueueTask<T> {
    sequence: u64,
    payload: T,
}

impl<T> RadrootsRuntimeQueueTask<T> {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn into_payload(self) -> T {
        self.payload
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeBoundedQueue<T> {
    capacity: usize,
    next_sequence: Option<u64>,
    in_flight: usize,
    shutdown: bool,
    queue: VecDeque<RadrootsRuntimeQueueTask<T>>,
}

impl<T> RadrootsRuntimeBoundedQueue<T> {
    pub fn new(capacity: usize) -> Result<Self, RadrootsRuntimeTransportError> {
        if !(1..=RADROOTS_RUNTIME_DELIVERY_QUEUE_MAX_CAPACITY).contains(&capacity) {
            return Err(RadrootsRuntimeTransportError::InvalidQueueCapacity {
                actual: capacity,
                max: RADROOTS_RUNTIME_DELIVERY_QUEUE_MAX_CAPACITY,
            });
        }
        Ok(Self {
            capacity,
            next_sequence: Some(1),
            in_flight: 0,
            shutdown: false,
            queue: VecDeque::new(),
        })
    }

    pub fn try_enqueue(&mut self, payload: T) -> Result<u64, RadrootsRuntimeTransportError> {
        if self.shutdown {
            return Err(RadrootsRuntimeTransportError::QueueShutdown);
        }
        let admitted = self.queue.len().checked_add(self.in_flight).ok_or(
            RadrootsRuntimeTransportError::QueueCapacityExhausted {
                capacity: self.capacity,
            },
        )?;
        if admitted >= self.capacity {
            return Err(RadrootsRuntimeTransportError::QueueCapacityExhausted {
                capacity: self.capacity,
            });
        }
        let sequence = self
            .next_sequence
            .ok_or(RadrootsRuntimeTransportError::QueueSequenceExhausted)?;
        self.next_sequence = sequence.checked_add(1);
        self.queue
            .push_back(RadrootsRuntimeQueueTask { sequence, payload });
        Ok(sequence)
    }

    pub fn pop(
        &mut self,
    ) -> Result<Option<RadrootsRuntimeQueueTask<T>>, RadrootsRuntimeTransportError> {
        if self.queue.is_empty() {
            return Ok(None);
        }
        let next_in_flight = self.in_flight.checked_add(1).ok_or(
            RadrootsRuntimeTransportError::QueueCapacityExhausted {
                capacity: self.capacity,
            },
        )?;
        let task = self
            .queue
            .pop_front()
            .ok_or(RadrootsRuntimeTransportError::QueueCompletionUnmatched)?;
        self.in_flight = next_in_flight;
        Ok(Some(task))
    }

    pub fn complete_task(&mut self) -> Result<(), RadrootsRuntimeTransportError> {
        self.in_flight = self
            .in_flight
            .checked_sub(1)
            .ok_or(RadrootsRuntimeTransportError::QueueCompletionUnmatched)?;
        Ok(())
    }

    pub fn shutdown(&mut self) {
        self.shutdown = true;
    }

    pub fn status(&self) -> RadrootsRuntimeQueueStatus {
        RadrootsRuntimeQueueStatus {
            capacity: self.capacity,
            queued: self.queue.len(),
            in_flight: self.in_flight,
            shutdown: self.shutdown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryTarget {
    delivery_target_id: i64,
    target: RadrootsTransportTarget,
    status: RadrootsTransportDeliveryTargetStatus,
}

impl RadrootsRuntimeDeliveryTarget {
    pub fn ready(
        delivery_target_id: i64,
        target: RadrootsTransportTarget,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        Self::with_initial_status(
            delivery_target_id,
            target,
            RadrootsTransportDeliveryTargetStatus::Pending,
        )
    }

    pub fn deferred_until_implemented(
        delivery_target_id: i64,
        target: RadrootsTransportTarget,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        Self::with_initial_status(
            delivery_target_id,
            target,
            RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented,
        )
    }

    pub fn retryable(
        delivery_target_id: i64,
        target: RadrootsTransportTarget,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        Self::with_initial_status(
            delivery_target_id,
            target,
            RadrootsTransportDeliveryTargetStatus::FailedRetryable,
        )
    }

    fn with_initial_status(
        delivery_target_id: i64,
        target: RadrootsTransportTarget,
        status: RadrootsTransportDeliveryTargetStatus,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        validate_positive_id("delivery target", delivery_target_id)?;
        if !matches!(
            status,
            RadrootsTransportDeliveryTargetStatus::Pending
                | RadrootsTransportDeliveryTargetStatus::FailedRetryable
                | RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
        ) {
            return Err(
                RadrootsRuntimeTransportError::InvalidDeliveryTargetInitialStatus {
                    delivery_target_id,
                    status,
                },
            );
        }
        Ok(Self {
            delivery_target_id,
            target,
            status,
        })
    }

    pub fn delivery_target_id(&self) -> i64 {
        self.delivery_target_id
    }

    pub fn target(&self) -> &RadrootsTransportTarget {
        &self.target
    }

    pub fn status(&self) -> RadrootsTransportDeliveryTargetStatus {
        self.status
    }

    pub fn is_ready_for_attempt(&self) -> bool {
        self.status.is_ready_for_attempt()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryPlan {
    delivery_plan_id: i64,
    satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    targets: Vec<RadrootsRuntimeDeliveryTarget>,
    required_target_count: usize,
}

impl RadrootsRuntimeDeliveryPlan {
    pub fn new(
        delivery_plan_id: i64,
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
        targets: Vec<RadrootsRuntimeDeliveryTarget>,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        validate_positive_id("delivery plan", delivery_plan_id)?;
        if targets.is_empty() {
            return Err(RadrootsRuntimeTransportError::EmptyDeliveryPlan);
        }
        let mut target_ids = std::collections::BTreeSet::new();
        for target in &targets {
            if !target_ids.insert(target.delivery_target_id) {
                return Err(RadrootsRuntimeTransportError::DuplicateDeliveryTargetId {
                    delivery_target_id: target.delivery_target_id,
                });
            }
        }
        let target_set = RadrootsTransportTargetSet::new(
            targets.iter().map(|target| target.target.clone()).collect(),
        )?;
        satisfaction_policy.validate_for_target_set(&target_set)?;
        let delivery_capable_target_count = targets
            .iter()
            .filter(|target| !target.status.is_deferred_until_implemented())
            .count();
        let satisfaction_target_count = if delivery_capable_target_count == 0 {
            target_set.len()
        } else {
            delivery_capable_target_count
        };
        let required_target_count =
            satisfaction_policy.required_target_count(satisfaction_target_count)?;
        Ok(Self {
            delivery_plan_id,
            satisfaction_policy,
            targets,
            required_target_count,
        })
    }

    pub fn delivery_plan_id(&self) -> i64 {
        self.delivery_plan_id
    }

    pub fn satisfaction_policy(&self) -> &RadrootsTransportSatisfactionPolicy {
        &self.satisfaction_policy
    }

    pub fn targets(&self) -> &[RadrootsRuntimeDeliveryTarget] {
        &self.targets
    }

    pub fn required_target_count(&self) -> usize {
        self.required_target_count
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryJob {
    outbox_event_id: i64,
    payload: RadrootsRuntimeTransportPayload,
    plans: Vec<RadrootsRuntimeDeliveryPlan>,
    now_ms: i64,
}

impl RadrootsRuntimeDeliveryJob {
    pub fn new(
        outbox_event_id: i64,
        payload: RadrootsRuntimeTransportPayload,
        plans: Vec<RadrootsRuntimeDeliveryPlan>,
        now_ms: i64,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        validate_positive_id("outbox event", outbox_event_id)?;
        validate_timestamp("delivery job", now_ms)?;
        payload.transport_payload()?;
        if plans.is_empty() {
            return Err(RadrootsRuntimeTransportError::EmptyDeliveryJob);
        }
        let mut plan_ids = std::collections::BTreeSet::new();
        for plan in &plans {
            if !plan_ids.insert(plan.delivery_plan_id) {
                return Err(RadrootsRuntimeTransportError::DuplicateDeliveryPlanId {
                    delivery_plan_id: plan.delivery_plan_id,
                });
            }
        }
        Ok(Self {
            outbox_event_id,
            payload,
            plans,
            now_ms,
        })
    }

    pub fn outbox_event_id(&self) -> i64 {
        self.outbox_event_id
    }

    pub fn payload(&self) -> &RadrootsRuntimeTransportPayload {
        &self.payload
    }

    pub fn plans(&self) -> &[RadrootsRuntimeDeliveryPlan] {
        &self.plans
    }

    pub fn now_ms(&self) -> i64 {
        self.now_ms
    }
}

fn validate_positive_id(
    field: &'static str,
    value: i64,
) -> Result<(), RadrootsRuntimeTransportError> {
    if value <= 0 {
        return Err(RadrootsRuntimeTransportError::InvalidIdentifier { field, value });
    }
    Ok(())
}

fn validate_timestamp(
    field: &'static str,
    value: i64,
) -> Result<(), RadrootsRuntimeTransportError> {
    if value < 0 {
        return Err(RadrootsRuntimeTransportError::InvalidTimestamp { field, value });
    }
    Ok(())
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryWorkerConfig {
    bounded_queue_capacity: usize,
}

#[cfg(feature = "transport-workers")]
impl RadrootsRuntimeDeliveryWorkerConfig {
    pub fn new(bounded_queue_capacity: usize) -> Result<Self, RadrootsRuntimeTransportError> {
        RadrootsRuntimeBoundedQueue::<()>::new(bounded_queue_capacity)?;
        Ok(Self {
            bounded_queue_capacity,
        })
    }

    pub fn bounded_queue_capacity(&self) -> usize {
        self.bounded_queue_capacity
    }
}

#[cfg(feature = "transport-workers")]
impl Default for RadrootsRuntimeDeliveryWorkerConfig {
    fn default() -> Self {
        Self {
            bounded_queue_capacity: RADROOTS_RUNTIME_DELIVERY_QUEUE_DEFAULT_CAPACITY,
        }
    }
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsRuntimeDeliveryPlanSatisfactionState {
    Satisfied,
    Unsatisfied,
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryPlanReceipt {
    delivery_plan_id: i64,
    satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    targets: Vec<RadrootsRuntimeDeliveryTarget>,
    attempted_target_count: usize,
    required_target_count: usize,
    satisfied_target_count: usize,
    satisfaction_state: RadrootsRuntimeDeliveryPlanSatisfactionState,
    target_receipts: Vec<RadrootsTransportTargetReceipt>,
}

#[cfg(feature = "transport-workers")]
impl RadrootsRuntimeDeliveryPlanReceipt {
    fn from_target_states(
        plan: &RadrootsRuntimeDeliveryPlan,
        targets: Vec<RadrootsRuntimeDeliveryTarget>,
        target_receipts: Vec<RadrootsTransportTargetReceipt>,
    ) -> Self {
        let satisfied_target_count =
            satisfied_target_count_for_policy(&plan.satisfaction_policy, &targets);
        let satisfaction_state = if target_states_satisfy_policy(
            &plan.satisfaction_policy,
            plan.required_target_count,
            &targets,
        ) {
            RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
        } else {
            RadrootsRuntimeDeliveryPlanSatisfactionState::Unsatisfied
        };
        Self {
            delivery_plan_id: plan.delivery_plan_id,
            satisfaction_policy: plan.satisfaction_policy.clone(),
            targets,
            attempted_target_count: target_receipts.len(),
            required_target_count: plan.required_target_count,
            satisfied_target_count,
            satisfaction_state,
            target_receipts,
        }
    }

    pub fn is_satisfied(&self) -> bool {
        self.satisfaction_state == RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
    }

    pub fn delivery_plan_id(&self) -> i64 {
        self.delivery_plan_id
    }

    pub fn satisfaction_policy(&self) -> &RadrootsTransportSatisfactionPolicy {
        &self.satisfaction_policy
    }

    pub fn targets(&self) -> &[RadrootsRuntimeDeliveryTarget] {
        &self.targets
    }

    pub fn target_count(&self) -> usize {
        self.targets.len()
    }

    pub fn attempted_target_count(&self) -> usize {
        self.attempted_target_count
    }

    pub fn required_target_count(&self) -> usize {
        self.required_target_count
    }

    pub fn satisfied_target_count(&self) -> usize {
        self.satisfied_target_count
    }

    pub fn satisfaction_state(&self) -> &RadrootsRuntimeDeliveryPlanSatisfactionState {
        &self.satisfaction_state
    }

    pub fn target_receipts(&self) -> &[RadrootsTransportTargetReceipt] {
        &self.target_receipts
    }
}

#[cfg(feature = "transport-workers")]
fn satisfied_target_count_for_policy(
    policy: &RadrootsTransportSatisfactionPolicy,
    targets: &[RadrootsRuntimeDeliveryTarget],
) -> usize {
    let Some(class) = policy.target_satisfaction_class() else {
        return 0;
    };
    match policy.required_target_fingerprints() {
        None => targets
            .iter()
            .filter(|target| target.status.counts_as_satisfied(class))
            .count(),
        Some(required_targets) => required_targets
            .iter()
            .filter(|required| {
                targets.iter().any(|target| {
                    target.target.fingerprint() == *required
                        && target.status.counts_as_satisfied(class)
                })
            })
            .count(),
    }
}

#[cfg(feature = "transport-workers")]
fn target_states_satisfy_policy(
    policy: &RadrootsTransportSatisfactionPolicy,
    required_target_count: usize,
    targets: &[RadrootsRuntimeDeliveryTarget],
) -> bool {
    if policy.kind() == RadrootsTransportSatisfactionPolicyKind::NoWait {
        return true;
    }
    match policy.required_target_fingerprints() {
        None => satisfied_target_count_for_policy(policy, targets) >= required_target_count,
        Some(required) => satisfied_target_count_for_policy(policy, targets) == required.len(),
    }
}

#[cfg(feature = "transport-workers")]
fn dispatch_satisfaction_policy(
    policy: &RadrootsTransportSatisfactionPolicy,
) -> RadrootsTransportSatisfactionPolicy {
    match policy.target_satisfaction_class() {
        Some(class) => RadrootsTransportSatisfactionPolicy::all(class),
        None => RadrootsTransportSatisfactionPolicy::no_wait(),
    }
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryJobReceipt {
    outbox_event_id: i64,
    dispatch_count: usize,
    plan_receipts: Vec<RadrootsRuntimeDeliveryPlanReceipt>,
    all_plans_satisfied: bool,
    target_receipts: Vec<RadrootsTransportTargetReceipt>,
}

#[cfg(feature = "transport-workers")]
impl RadrootsRuntimeDeliveryJobReceipt {
    fn new(
        outbox_event_id: i64,
        dispatch_count: usize,
        plan_receipts: Vec<RadrootsRuntimeDeliveryPlanReceipt>,
    ) -> Self {
        let all_plans_satisfied = !plan_receipts.is_empty()
            && plan_receipts
                .iter()
                .all(RadrootsRuntimeDeliveryPlanReceipt::is_satisfied);
        let target_receipts = plan_receipts
            .iter()
            .flat_map(|receipt| receipt.target_receipts.iter().cloned())
            .collect();
        Self {
            outbox_event_id,
            dispatch_count,
            plan_receipts,
            all_plans_satisfied,
            target_receipts,
        }
    }

    pub fn outbox_event_id(&self) -> i64 {
        self.outbox_event_id
    }

    pub fn dispatch_count(&self) -> usize {
        self.dispatch_count
    }

    pub fn plan_receipts(&self) -> &[RadrootsRuntimeDeliveryPlanReceipt] {
        &self.plan_receipts
    }

    pub fn all_plans_satisfied(&self) -> bool {
        self.all_plans_satisfied
    }

    pub fn target_receipts(&self) -> &[RadrootsTransportTargetReceipt] {
        &self.target_receipts
    }
}

#[cfg(feature = "transport-workers")]
pub struct RadrootsRuntimeDeliveryWorker<'a> {
    registry: &'a RadrootsRuntimeTransportRegistry,
    queue_capacity: usize,
    queue: Mutex<RadrootsRuntimeBoundedQueue<()>>,
}

#[cfg(feature = "transport-workers")]
struct RadrootsRuntimeQueueAdmission<'a> {
    queue: &'a Mutex<RadrootsRuntimeBoundedQueue<()>>,
    active: bool,
}

#[cfg(feature = "transport-workers")]
impl RadrootsRuntimeQueueAdmission<'_> {
    fn complete(mut self) -> Result<(), RadrootsRuntimeTransportError> {
        lock_runtime_queue(self.queue)?.complete_task()?;
        self.active = false;
        Ok(())
    }
}

#[cfg(feature = "transport-workers")]
impl Drop for RadrootsRuntimeQueueAdmission<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        match self.queue.lock() {
            Ok(mut queue) => {
                let _ = queue.complete_task();
            }
            Err(poisoned) => {
                let mut queue = poisoned.into_inner();
                let _ = queue.complete_task();
            }
        }
    }
}

#[cfg(feature = "transport-workers")]
fn lock_runtime_queue(
    queue: &Mutex<RadrootsRuntimeBoundedQueue<()>>,
) -> Result<MutexGuard<'_, RadrootsRuntimeBoundedQueue<()>>, RadrootsRuntimeTransportError> {
    queue
        .lock()
        .map_err(|_| RadrootsRuntimeTransportError::QueueStatePoisoned)
}

#[cfg(feature = "transport-workers")]
impl<'a> RadrootsRuntimeDeliveryWorker<'a> {
    pub fn new(
        registry: &'a RadrootsRuntimeTransportRegistry,
        config: RadrootsRuntimeDeliveryWorkerConfig,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        let queue = RadrootsRuntimeBoundedQueue::new(config.bounded_queue_capacity)?;
        Ok(Self {
            registry,
            queue_capacity: config.bounded_queue_capacity,
            queue: Mutex::new(queue),
        })
    }

    pub fn queue_capacity(&self) -> usize {
        self.queue_capacity
    }

    pub fn queue_status(
        &self,
    ) -> Result<RadrootsRuntimeQueueStatus, RadrootsRuntimeTransportError> {
        Ok(lock_runtime_queue(&self.queue)?.status())
    }

    fn admit(&self) -> Result<RadrootsRuntimeQueueAdmission<'_>, RadrootsRuntimeTransportError> {
        let mut queue = lock_runtime_queue(&self.queue)?;
        let sequence = queue.try_enqueue(())?;
        let task = queue
            .pop()?
            .ok_or(RadrootsRuntimeTransportError::QueueCompletionUnmatched)?;
        if task.sequence() != sequence {
            queue.complete_task()?;
            return Err(RadrootsRuntimeTransportError::QueueCompletionUnmatched);
        }
        drop(queue);
        Ok(RadrootsRuntimeQueueAdmission {
            queue: &self.queue,
            active: true,
        })
    }

    pub async fn execute_job(
        &self,
        job: RadrootsRuntimeDeliveryJob,
    ) -> Result<RadrootsRuntimeDeliveryJobReceipt, RadrootsRuntimeTransportError> {
        let admission = self.admit()?;
        let result = self.execute_admitted_job(job).await;
        admission.complete()?;
        result
    }

    async fn execute_admitted_job(
        &self,
        job: RadrootsRuntimeDeliveryJob,
    ) -> Result<RadrootsRuntimeDeliveryJobReceipt, RadrootsRuntimeTransportError> {
        let outbox_event_id = job.outbox_event_id;
        let payload = job.payload;
        let now_ms = job.now_ms;
        let mut plan_receipts = Vec::new();
        let mut dispatch_count = 0usize;
        for plan in job.plans {
            let dispatch_satisfaction_policy =
                dispatch_satisfaction_policy(&plan.satisfaction_policy);
            let mut target_states = plan.targets.clone();
            let target_positions = target_states
                .iter()
                .enumerate()
                .map(|(index, target)| (target.target.fingerprint().as_str().to_owned(), index))
                .collect::<BTreeMap<_, _>>();
            let mut by_kind =
                BTreeMap::<RadrootsTransportKind, Vec<RadrootsRuntimeDeliveryTarget>>::new();
            if plan.satisfaction_policy.kind() != RadrootsTransportSatisfactionPolicyKind::NoWait {
                for target in plan
                    .targets
                    .iter()
                    .filter(|target| target.is_ready_for_attempt())
                    .cloned()
                {
                    by_kind
                        .entry(target.target.kind().clone())
                        .or_default()
                        .push(target);
                }
            }
            let mut plan_target_receipts = Vec::new();
            for (kind, targets) in by_kind {
                let transport = match self.registry.transport(&kind) {
                    Ok(transport) => transport,
                    Err(error) => {
                        return Err(partial_delivery_error(
                            &kind,
                            error.to_string(),
                            &plan,
                            PartialDeliveryState {
                                outbox_event_id,
                                dispatch_count,
                                completed_plan_receipts: plan_receipts,
                                current_targets: target_states,
                                current_target_receipts: plan_target_receipts,
                            },
                        ));
                    }
                };
                let transport_targets = targets
                    .iter()
                    .map(|target| target.target.clone())
                    .collect::<Vec<_>>();
                let request = match RadrootsRuntimeTransportDispatchRequest::new(
                    format!(
                        "outbox-event-{}-plan-{}",
                        outbox_event_id, plan.delivery_plan_id
                    ),
                    payload.clone(),
                    transport_targets,
                    dispatch_satisfaction_policy.clone(),
                    now_ms,
                ) {
                    Ok(request) => request,
                    Err(error) => {
                        return Err(partial_delivery_error(
                            &kind,
                            error.to_string(),
                            &plan,
                            PartialDeliveryState {
                                outbox_event_id,
                                dispatch_count,
                                completed_plan_receipts: plan_receipts,
                                current_targets: target_states,
                                current_target_receipts: plan_target_receipts,
                            },
                        ));
                    }
                };
                let delivery_request = match request.transport_delivery_request() {
                    Ok(request) => request,
                    Err(error) => {
                        return Err(partial_delivery_error(
                            &kind,
                            error.to_string(),
                            &plan,
                            PartialDeliveryState {
                                outbox_event_id,
                                dispatch_count,
                                completed_plan_receipts: plan_receipts,
                                current_targets: target_states,
                                current_target_receipts: plan_target_receipts,
                            },
                        ));
                    }
                };
                dispatch_count += 1;
                let receipt = match transport.deliver(delivery_request.clone()).await {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        return Err(partial_delivery_error(
                            &kind,
                            error.to_string(),
                            &plan,
                            PartialDeliveryState {
                                outbox_event_id,
                                dispatch_count,
                                completed_plan_receipts: plan_receipts,
                                current_targets: target_states,
                                current_target_receipts: plan_target_receipts,
                            },
                        ));
                    }
                };
                if let Err(error) = receipt.validate_for_request(&delivery_request) {
                    return Err(partial_delivery_error(
                        &kind,
                        error.to_string(),
                        &plan,
                        PartialDeliveryState {
                            outbox_event_id,
                            dispatch_count,
                            completed_plan_receipts: plan_receipts,
                            current_targets: target_states,
                            current_target_receipts: plan_target_receipts,
                        },
                    ));
                }
                for target_receipt in receipt.target_receipts().iter().cloned() {
                    let Some(index) = target_positions
                        .get(target_receipt.target().fingerprint().as_str())
                        .copied()
                    else {
                        return Err(partial_delivery_error(
                            &kind,
                            "delivery receipt target is not present in the validated plan"
                                .to_owned(),
                            &plan,
                            PartialDeliveryState {
                                outbox_event_id,
                                dispatch_count,
                                completed_plan_receipts: plan_receipts,
                                current_targets: target_states,
                                current_target_receipts: plan_target_receipts,
                            },
                        ));
                    };
                    target_states[index].status = target_receipt.status();
                    plan_target_receipts.push(target_receipt);
                }
            }
            let plan_receipt = RadrootsRuntimeDeliveryPlanReceipt::from_target_states(
                &plan,
                target_states,
                plan_target_receipts,
            );
            plan_receipts.push(plan_receipt);
        }
        Ok(RadrootsRuntimeDeliveryJobReceipt::new(
            outbox_event_id,
            dispatch_count,
            plan_receipts,
        ))
    }
}

#[cfg(feature = "transport-workers")]
struct PartialDeliveryState {
    outbox_event_id: i64,
    dispatch_count: usize,
    completed_plan_receipts: Vec<RadrootsRuntimeDeliveryPlanReceipt>,
    current_targets: Vec<RadrootsRuntimeDeliveryTarget>,
    current_target_receipts: Vec<RadrootsTransportTargetReceipt>,
}

#[cfg(feature = "transport-workers")]
fn partial_delivery_error(
    kind: &RadrootsTransportKind,
    message: String,
    current_plan: &RadrootsRuntimeDeliveryPlan,
    mut state: PartialDeliveryState,
) -> RadrootsRuntimeTransportError {
    state
        .completed_plan_receipts
        .push(RadrootsRuntimeDeliveryPlanReceipt::from_target_states(
            current_plan,
            state.current_targets,
            state.current_target_receipts,
        ));
    RadrootsRuntimeTransportError::PartialDelivery {
        kind: kind.canonical_label(),
        message,
        partial_receipt: Box::new(RadrootsRuntimeDeliveryJobReceipt::new(
            state.outbox_event_id,
            state.dispatch_count,
            state.completed_plan_receipts,
        )),
    }
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeLeaseRecord {
    record_id: i64,
    claimed: bool,
    claim_expires_at_ms: i64,
    recovered: bool,
}

#[cfg(feature = "transport-workers")]
impl RadrootsRuntimeLeaseRecord {
    pub fn claimed(
        record_id: i64,
        claim_expires_at_ms: i64,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        validate_positive_id("lease record", record_id)?;
        validate_timestamp("lease expiry", claim_expires_at_ms)?;
        Ok(Self {
            record_id,
            claimed: true,
            claim_expires_at_ms,
            recovered: false,
        })
    }

    pub fn record_id(&self) -> i64 {
        self.record_id
    }

    pub fn is_claimed(&self) -> bool {
        self.claimed
    }

    pub fn claim_expires_at_ms(&self) -> i64 {
        self.claim_expires_at_ms
    }

    pub fn is_recovered(&self) -> bool {
        self.recovered
    }

    fn validate(&self) -> Result<(), RadrootsRuntimeTransportError> {
        validate_positive_id("lease record", self.record_id)?;
        validate_timestamp("lease expiry", self.claim_expires_at_ms)?;
        if self.claimed && self.recovered {
            return Err(RadrootsRuntimeTransportError::InvalidLeaseState {
                record_id: self.record_id,
            });
        }
        Ok(())
    }
}

#[cfg(feature = "transport-workers")]
pub fn recover_expired_leases(
    leases: &mut [RadrootsRuntimeLeaseRecord],
    now_ms: i64,
) -> Result<usize, RadrootsRuntimeTransportError> {
    validate_timestamp("lease recovery", now_ms)?;
    for lease in leases.iter() {
        lease.validate()?;
    }
    let mut recovered = 0usize;
    for lease in leases {
        if lease.claimed && lease.claim_expires_at_ms <= now_ms {
            lease.claimed = false;
            lease.recovered = true;
            recovered += 1;
        }
    }
    Ok(recovered)
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeInboundObservation {
    event_id: String,
    transport_target: RadrootsTransportTarget,
    observed_at_ms: i64,
}

#[cfg(feature = "transport-workers")]
impl RadrootsRuntimeInboundObservation {
    pub fn verified_signed_event(
        event: &RadrootsVerifiedSignedEvent,
        transport_target: RadrootsTransportTarget,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        validate_timestamp("inbound observation", observed_at_ms)?;
        Ok(Self {
            event_id: event.signed_event().id_str().to_owned(),
            transport_target,
            observed_at_ms,
        })
    }

    pub fn event_id(&self) -> &str {
        self.event_id.as_str()
    }

    pub fn transport_target(&self) -> &RadrootsTransportTarget {
        &self.transport_target
    }

    pub fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    pub fn require_verified_for_signed_event(
        &self,
        event: &RadrootsVerifiedSignedEvent,
    ) -> Result<(), RadrootsRuntimeTransportError> {
        if self.event_id != event.signed_event().id_str() {
            return Err(
                RadrootsRuntimeTransportError::InboundObservationEventMismatch {
                    observation_event_id: self.event_id.clone(),
                    signed_event_id: event.signed_event().id_str().to_owned(),
                },
            );
        }
        Ok(())
    }
}

#[cfg(feature = "transport-workers")]
pub trait RadrootsRuntimeInboundObservationSink: Send + Sync {
    fn record_verified_observation<'a>(
        &'a self,
        event: RadrootsVerifiedSignedEvent,
        observation: RadrootsRuntimeInboundObservation,
    ) -> RadrootsRuntimeTransportFuture<'a, ()>;
}

#[cfg(feature = "transport-workers")]
pub fn record_verified_inbound_observation<'a, S>(
    sink: &'a S,
    event: RadrootsVerifiedSignedEvent,
    observation: RadrootsRuntimeInboundObservation,
) -> RadrootsRuntimeTransportFuture<'a, ()>
where
    S: RadrootsRuntimeInboundObservationSink + ?Sized,
{
    Box::pin(async move {
        observation.require_verified_for_signed_event(&event)?;
        sink.record_verified_observation(event, observation).await
    })
}

#[cfg(test)]
mod tests {
    use super::{
        RADROOTS_RUNTIME_DELIVERY_QUEUE_DEFAULT_CAPACITY,
        RADROOTS_RUNTIME_DELIVERY_QUEUE_MAX_CAPACITY, RadrootsRuntimeBoundedQueue,
        RadrootsRuntimeTransportDispatchRequest, RadrootsRuntimeTransportError,
        RadrootsRuntimeTransportFuture, RadrootsRuntimeTransportPayload,
        RadrootsRuntimeTransportRegistry,
    };
    #[cfg(feature = "transport-workers")]
    use super::{
        RadrootsRuntimeDeliveryJob, RadrootsRuntimeDeliveryPlan,
        RadrootsRuntimeDeliveryPlanSatisfactionState, RadrootsRuntimeDeliveryTarget,
        RadrootsRuntimeDeliveryWorker, RadrootsRuntimeDeliveryWorkerConfig,
        RadrootsRuntimeInboundObservation, RadrootsRuntimeInboundObservationSink,
        RadrootsRuntimeLeaseRecord, record_verified_inbound_observation, recover_expired_leases,
    };
    #[cfg(feature = "transport-workers")]
    use radroots_event::{
        draft::{RadrootsSignedEvent, RadrootsVerifiedSignedEvent},
        wire::RadrootsNip01EventWire,
    };
    use radroots_transport::{
        RadrootsTransport, RadrootsTransportCapabilities, RadrootsTransportDeliveryReceipt,
        RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
        RadrootsTransportError, RadrootsTransportFetchReceipt, RadrootsTransportFetchRequest,
        RadrootsTransportFuture, RadrootsTransportImplementationState, RadrootsTransportKind,
        RadrootsTransportOutcome, RadrootsTransportOutcomeKind, RadrootsTransportSatisfactionClass,
        RadrootsTransportSatisfactionPolicy, RadrootsTransportStatus, RadrootsTransportTarget,
        RadrootsTransportTargetReceipt, RadrootsTransportTargetSet,
    };
    #[cfg(feature = "transport-workers")]
    use std::sync::{Arc, Mutex};

    struct StaticTransport {
        kind: RadrootsTransportKind,
        outcome_kind: RadrootsTransportOutcomeKind,
        #[cfg(feature = "transport-workers")]
        captured_now_ms: Option<Arc<Mutex<Vec<i64>>>>,
    }

    impl StaticTransport {
        fn new(kind: RadrootsTransportKind, outcome_kind: RadrootsTransportOutcomeKind) -> Self {
            Self {
                kind,
                outcome_kind,
                #[cfg(feature = "transport-workers")]
                captured_now_ms: None,
            }
        }

        #[cfg(feature = "transport-workers")]
        fn recording_now_ms(
            kind: RadrootsTransportKind,
            outcome_kind: RadrootsTransportOutcomeKind,
        ) -> (Self, Arc<Mutex<Vec<i64>>>) {
            let captured_now_ms = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    kind,
                    outcome_kind,
                    captured_now_ms: Some(captured_now_ms.clone()),
                },
                captured_now_ms,
            )
        }
    }

    impl RadrootsTransport for StaticTransport {
        fn transport_kind(&self) -> RadrootsTransportKind {
            self.kind.clone()
        }

        fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus> {
            Box::pin(async move {
                RadrootsTransportStatus::new(
                    self.kind.clone(),
                    true,
                    RadrootsTransportImplementationState::Real,
                    true,
                    "ready",
                )
                .map(|status| {
                    status.with_capabilities(RadrootsTransportCapabilities::deliver_and_fetch())
                })
            })
        }

        fn deliver<'a>(
            &'a self,
            request: RadrootsTransportDeliveryRequest,
        ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt> {
            Box::pin(async move {
                #[cfg(feature = "transport-workers")]
                if let Some(captured_now_ms) = &self.captured_now_ms {
                    captured_now_ms
                        .lock()
                        .expect("now_ms capture")
                        .push(request.now_ms());
                }
                RadrootsTransportDeliveryReceipt::for_request(
                    &request,
                    request
                        .target_set()
                        .targets()
                        .iter()
                        .cloned()
                        .map(|target| {
                            RadrootsTransportTargetReceipt::new(
                                target,
                                RadrootsTransportOutcome::new(self.outcome_kind),
                            )
                        })
                        .collect(),
                )
            })
        }

        fn fetch<'a>(
            &'a self,
            request: RadrootsTransportFetchRequest,
        ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
            Box::pin(async move {
                RadrootsTransportFetchReceipt::for_request(
                    &request,
                    request
                        .target_set()
                        .targets()
                        .iter()
                        .cloned()
                        .map(|target| {
                            RadrootsTransportTargetReceipt::new(
                                target,
                                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen),
                            )
                        })
                        .collect(),
                    0,
                )
            })
        }
    }

    #[cfg(feature = "transport-workers")]
    #[derive(Clone, Copy)]
    enum ForgedDeliveryReceipt {
        RequestId,
        TargetSet,
    }

    #[cfg(feature = "transport-workers")]
    struct ForgedReceiptTransport {
        forged: ForgedDeliveryReceipt,
    }

    #[cfg(feature = "transport-workers")]
    impl RadrootsTransport for ForgedReceiptTransport {
        fn transport_kind(&self) -> RadrootsTransportKind {
            RadrootsTransportKind::Nostr
        }

        fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus> {
            Box::pin(async {
                RadrootsTransportStatus::new(
                    RadrootsTransportKind::Nostr,
                    true,
                    RadrootsTransportImplementationState::Real,
                    true,
                    "forged receipt fixture",
                )
            })
        }

        fn deliver<'a>(
            &'a self,
            request: RadrootsTransportDeliveryRequest,
        ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt> {
            Box::pin(async move {
                match self.forged {
                    ForgedDeliveryReceipt::RequestId => RadrootsTransportDeliveryReceipt::new(
                        "forged-request",
                        request.target_set().clone(),
                        request
                            .target_set()
                            .targets()
                            .iter()
                            .cloned()
                            .map(|target| {
                                RadrootsTransportTargetReceipt::new(
                                    target,
                                    RadrootsTransportOutcome::new(
                                        RadrootsTransportOutcomeKind::Accepted,
                                    ),
                                )
                            })
                            .collect(),
                    ),
                    ForgedDeliveryReceipt::TargetSet => {
                        let target =
                            RadrootsTransportTarget::nostr_relay("wss://forged-relay.example")?;
                        RadrootsTransportDeliveryReceipt::new(
                            request.request_id(),
                            RadrootsTransportTargetSet::new(vec![target.clone()])?,
                            vec![RadrootsTransportTargetReceipt::new(
                                target,
                                RadrootsTransportOutcome::new(
                                    RadrootsTransportOutcomeKind::Accepted,
                                ),
                            )],
                        )
                    }
                }
            })
        }

        fn fetch<'a>(
            &'a self,
            _request: RadrootsTransportFetchRequest,
        ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
            Box::pin(async { Err(RadrootsTransportError::UnsupportedOperation) })
        }
    }

    #[cfg(feature = "transport-workers")]
    struct PendingTransport;

    #[cfg(feature = "transport-workers")]
    impl RadrootsTransport for PendingTransport {
        fn transport_kind(&self) -> RadrootsTransportKind {
            RadrootsTransportKind::Nostr
        }

        fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus> {
            Box::pin(async {
                RadrootsTransportStatus::new(
                    RadrootsTransportKind::Nostr,
                    true,
                    RadrootsTransportImplementationState::Real,
                    true,
                    "pending transport fixture",
                )
            })
        }

        fn deliver<'a>(
            &'a self,
            _request: RadrootsTransportDeliveryRequest,
        ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt> {
            Box::pin(std::future::pending())
        }

        fn fetch<'a>(
            &'a self,
            _request: RadrootsTransportFetchRequest,
        ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
            Box::pin(async { Err(RadrootsTransportError::UnsupportedOperation) })
        }
    }

    fn target(kind: RadrootsTransportKind, uri: &str) -> RadrootsTransportTarget {
        RadrootsTransportTarget::new(kind, uri).expect("target")
    }

    fn opaque_payload() -> RadrootsRuntimeTransportPayload {
        RadrootsRuntimeTransportPayload::OpaqueBytes {
            label: "runtime-test-payload".to_owned(),
            bytes: b"runtime payload".to_vec(),
        }
    }

    #[cfg(feature = "transport-workers")]
    fn delivery_target(
        delivery_target_id: i64,
        kind: RadrootsTransportKind,
        uri: &str,
    ) -> RadrootsRuntimeDeliveryTarget {
        RadrootsRuntimeDeliveryTarget::ready(delivery_target_id, target(kind, uri))
            .expect("delivery target")
    }

    #[cfg(feature = "transport-workers")]
    fn deferred_delivery_target(
        delivery_target_id: i64,
        kind: RadrootsTransportKind,
        uri: &str,
    ) -> RadrootsRuntimeDeliveryTarget {
        RadrootsRuntimeDeliveryTarget::deferred_until_implemented(
            delivery_target_id,
            target(kind, uri),
        )
        .expect("deferred delivery target")
    }

    #[cfg(feature = "transport-workers")]
    fn delivery_plan(
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
        targets: Vec<RadrootsRuntimeDeliveryTarget>,
    ) -> RadrootsRuntimeDeliveryPlan {
        RadrootsRuntimeDeliveryPlan::new(7, satisfaction_policy, targets).expect("delivery plan")
    }

    #[cfg(feature = "transport-workers")]
    fn delivery_job(
        plans: Vec<RadrootsRuntimeDeliveryPlan>,
        now_ms: i64,
    ) -> RadrootsRuntimeDeliveryJob {
        RadrootsRuntimeDeliveryJob::new(42, opaque_payload(), plans, now_ms).expect("delivery job")
    }

    #[cfg(feature = "transport-workers")]
    fn delivery_worker(
        registry: &RadrootsRuntimeTransportRegistry,
        capacity: usize,
    ) -> RadrootsRuntimeDeliveryWorker<'_> {
        RadrootsRuntimeDeliveryWorker::new(
            registry,
            RadrootsRuntimeDeliveryWorkerConfig::new(capacity).expect("worker config"),
        )
        .expect("delivery worker")
    }

    #[cfg(feature = "transport-workers")]
    fn signed_event() -> RadrootsVerifiedSignedEvent {
        let raw_json = r#"{"id":"fb3f42caf9db337a7f1c0d49cd8ba5191f08dc1c419ed0640f7ea48a924e3bf3","pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","created_at":1781632860,"kind":1,"tags":[],"content":"The first strawberries are ready.","sig":"dba0a86fee54304c2b419742f186e74d7edca5fc7234c8aa294651de9bc2f16bf829d46f36ec759a767c4ccd1841a73243eae89afd5f6c89b2243491bfbb5f50"}"#.to_owned();
        let wire =
            RadrootsNip01EventWire::parse_json(raw_json.as_str()).expect("signed event wire");
        RadrootsSignedEvent::from_wire_verified_id(wire, raw_json)
            .expect("signed event identity")
            .verify_signature()
            .expect("signed event signature")
    }

    #[cfg(feature = "transport-workers")]
    #[test]
    fn runtime_verified_signed_event_payload_uses_structured_signed_event() {
        let event = signed_event();
        let payload = RadrootsRuntimeTransportPayload::verified_signed_event_json(&event)
            .expect("verified payload");
        let via_variant = RadrootsRuntimeTransportPayload::signed_event(event.clone())
            .transport_payload()
            .expect("transport payload");
        let (event_id, raw_json) = payload
            .signed_event_json_parts()
            .expect("signed event payload");

        assert_eq!(payload, via_variant);
        assert_eq!(event_id, event.signed_event().id_str());
        assert_eq!(raw_json, event.signed_event().raw_json());
    }

    #[cfg(feature = "transport-workers")]
    struct RecordingInboundSink {
        expected_event_id: String,
    }

    #[cfg(feature = "transport-workers")]
    impl RadrootsRuntimeInboundObservationSink for RecordingInboundSink {
        fn record_verified_observation<'a>(
            &'a self,
            event: RadrootsVerifiedSignedEvent,
            observation: RadrootsRuntimeInboundObservation,
        ) -> RadrootsRuntimeTransportFuture<'a, ()> {
            Box::pin(async move {
                assert_eq!(event.signed_event().id_str(), self.expected_event_id);
                assert_eq!(observation.event_id(), self.expected_event_id);
                assert_eq!(
                    observation.transport_target().uri().as_str(),
                    "wss://relay.example"
                );
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn registry_dispatches_transport_by_transport_kind() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        registry
            .register(StaticTransport::new(
                RadrootsTransportKind::Nostr,
                RadrootsTransportOutcomeKind::Accepted,
            ))
            .expect("register");
        assert_eq!(
            registry.registered_kinds(),
            vec![RadrootsTransportKind::Nostr]
        );
        assert!(matches!(
            registry.register(StaticTransport::new(
                RadrootsTransportKind::Nostr,
                RadrootsTransportOutcomeKind::Accepted,
            )),
            Err(RadrootsRuntimeTransportError::TransportAlreadyRegistered(_))
        ));

        let transport = registry
            .transport(&RadrootsTransportKind::Nostr)
            .expect("nostr transport");
        let request = RadrootsRuntimeTransportDispatchRequest::new(
            "nostr-delivery",
            opaque_payload(),
            vec![target(RadrootsTransportKind::Nostr, "wss://relay.example")],
            RadrootsTransportSatisfactionPolicy::any_accepted(),
            1_000,
        )
        .expect("request");
        let receipt = transport
            .deliver(
                request
                    .transport_delivery_request()
                    .expect("delivery request"),
            )
            .await
            .expect("receipt");
        let status = transport.status().await.expect("status");
        assert_eq!(status.kind(), &RadrootsTransportKind::Nostr);
        assert_eq!(
            status.capabilities(),
            &RadrootsTransportCapabilities::deliver_and_fetch()
        );
        let fetch = transport
            .fetch(
                RadrootsTransportFetchRequest::new(
                    "nostr-fetch",
                    RadrootsTransportTargetSet::new(vec![target(
                        RadrootsTransportKind::Nostr,
                        "wss://relay.example",
                    )])
                    .expect("target set"),
                )
                .expect("fetch request"),
            )
            .await
            .expect("fetch");
        assert_eq!(fetch.fetched_count(), 0);

        assert_eq!(
            receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
            1
        );
        assert_eq!(
            receipt.target_receipts()[0].status(),
            RadrootsTransportDeliveryTargetStatus::Accepted
        );
    }

    #[test]
    fn dispatch_request_preserves_transport_now_ms() {
        let request = RadrootsRuntimeTransportDispatchRequest::new(
            "nostr-delivery",
            opaque_payload(),
            vec![target(RadrootsTransportKind::Nostr, "wss://relay.example")],
            RadrootsTransportSatisfactionPolicy::any_accepted(),
            123_456,
        )
        .expect("request");

        let delivery_request = request
            .transport_delivery_request()
            .expect("delivery request");

        assert_eq!(delivery_request.now_ms(), 123_456);
    }

    #[test]
    fn bounded_queue_tracks_capacity_inflight_and_shutdown() {
        let mut queue = RadrootsRuntimeBoundedQueue::new(2).expect("queue");
        assert_eq!(queue.try_enqueue("a").expect("enqueue"), 1);
        assert_eq!(queue.try_enqueue("b").expect("enqueue"), 2);
        assert!(matches!(
            queue.try_enqueue("c"),
            Err(RadrootsRuntimeTransportError::QueueCapacityExhausted { capacity: 2 })
        ));
        let task = queue.pop().expect("pop").expect("task");
        assert_eq!(task.sequence(), 1);
        assert_eq!(task.payload(), &"a");
        assert_eq!(queue.status().in_flight(), 1);
        assert!(matches!(
            queue.try_enqueue("c"),
            Err(RadrootsRuntimeTransportError::QueueCapacityExhausted { capacity: 2 })
        ));
        queue.complete_task().expect("complete");
        assert_eq!(queue.try_enqueue("c").expect("enqueue"), 3);
        queue.shutdown();
        assert!(matches!(
            queue.try_enqueue("d"),
            Err(RadrootsRuntimeTransportError::QueueShutdown)
        ));
        assert!(queue.status().is_shutdown());
        assert!(matches!(
            RadrootsRuntimeBoundedQueue::<()>::new(0),
            Err(RadrootsRuntimeTransportError::InvalidQueueCapacity { actual: 0, .. })
        ));
        assert!(matches!(
            RadrootsRuntimeBoundedQueue::<()>::new(
                RADROOTS_RUNTIME_DELIVERY_QUEUE_MAX_CAPACITY + 1
            ),
            Err(RadrootsRuntimeTransportError::InvalidQueueCapacity { .. })
        ));
        RadrootsRuntimeBoundedQueue::<()>::new(RADROOTS_RUNTIME_DELIVERY_QUEUE_MAX_CAPACITY)
            .expect("maximum queue");
    }

    #[test]
    fn bounded_queue_reports_sequence_exhaustion_and_unmatched_completion() {
        let mut queue = RadrootsRuntimeBoundedQueue::new(1).expect("queue");
        assert!(matches!(
            queue.complete_task(),
            Err(RadrootsRuntimeTransportError::QueueCompletionUnmatched)
        ));
        queue.next_sequence = Some(u64::MAX);
        assert_eq!(queue.try_enqueue("last").expect("last sequence"), u64::MAX);
        let task = queue.pop().expect("pop").expect("task");
        assert_eq!(task.sequence(), u64::MAX);
        queue.complete_task().expect("complete");
        assert!(matches!(
            queue.try_enqueue("overflow"),
            Err(RadrootsRuntimeTransportError::QueueSequenceExhausted)
        ));
    }

    #[cfg(feature = "transport-workers")]
    #[test]
    fn delivery_worker_config_uses_governed_capacity_range() {
        assert_eq!(
            RadrootsRuntimeDeliveryWorkerConfig::default().bounded_queue_capacity(),
            RADROOTS_RUNTIME_DELIVERY_QUEUE_DEFAULT_CAPACITY
        );
        RadrootsRuntimeDeliveryWorkerConfig::new(RADROOTS_RUNTIME_DELIVERY_QUEUE_MAX_CAPACITY)
            .expect("maximum worker capacity");
        assert!(matches!(
            RadrootsRuntimeDeliveryWorkerConfig::new(0),
            Err(RadrootsRuntimeTransportError::InvalidQueueCapacity { actual: 0, .. })
        ));
        assert!(matches!(
            RadrootsRuntimeDeliveryWorkerConfig::new(
                RADROOTS_RUNTIME_DELIVERY_QUEUE_MAX_CAPACITY + 1,
            ),
            Err(RadrootsRuntimeTransportError::InvalidQueueCapacity { .. })
        ));
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_dispatches_ready_targets_and_skips_deferred() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        registry
            .register(StaticTransport::new(
                RadrootsTransportKind::Nostr,
                RadrootsTransportOutcomeKind::Accepted,
            ))
            .expect("register");
        let worker = delivery_worker(&registry, 8);
        let ready = delivery_target(1, RadrootsTransportKind::Nostr, "wss://relay.example");
        let deferred =
            deferred_delivery_target(2, RadrootsTransportKind::Reticulum, "reticulum:local");
        let receipt = worker
            .execute_job(delivery_job(
                vec![delivery_plan(
                    RadrootsTransportSatisfactionPolicy::any_accepted(),
                    vec![ready, deferred],
                )],
                1_000,
            ))
            .await
            .expect("worker receipt");

        assert_eq!(worker.queue_capacity(), 8);
        assert_eq!(receipt.dispatch_count(), 1);
        assert!(receipt.all_plans_satisfied());
        assert_eq!(receipt.plan_receipts().len(), 1);
        let plan_receipt = &receipt.plan_receipts()[0];
        assert_eq!(plan_receipt.delivery_plan_id(), 7);
        assert_eq!(plan_receipt.target_count(), 2);
        assert_eq!(plan_receipt.targets()[0].delivery_target_id(), 1);
        assert_eq!(plan_receipt.targets()[1].delivery_target_id(), 2);
        assert_eq!(plan_receipt.attempted_target_count(), 1);
        assert_eq!(plan_receipt.required_target_count(), 1);
        assert_eq!(plan_receipt.satisfied_target_count(), 1);
        assert_eq!(
            plan_receipt.satisfaction_state(),
            &RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
        );
        assert_eq!(receipt.target_receipts().len(), 1);
        assert_eq!(
            receipt.target_receipts()[0].status(),
            RadrootsTransportDeliveryTargetStatus::Accepted
        );
        assert_eq!(worker.queue_status().expect("queue status").in_flight(), 0);
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_passes_job_now_ms_to_registered_transport() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        let (transport, captured_now_ms) = StaticTransport::recording_now_ms(
            RadrootsTransportKind::Nostr,
            RadrootsTransportOutcomeKind::Accepted,
        );
        registry.register(transport).expect("register");
        let worker = delivery_worker(&registry, 8);
        let receipt = worker
            .execute_job(delivery_job(
                vec![delivery_plan(
                    RadrootsTransportSatisfactionPolicy::any_accepted(),
                    vec![delivery_target(
                        1,
                        RadrootsTransportKind::Nostr,
                        "wss://relay.example",
                    )],
                )],
                987_654,
            ))
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count(), 1);
        assert_eq!(
            captured_now_ms.lock().expect("now_ms capture").as_slice(),
            &[987_654]
        );
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_rejects_receipts_forged_for_another_request() {
        for forged in [
            ForgedDeliveryReceipt::RequestId,
            ForgedDeliveryReceipt::TargetSet,
        ] {
            let mut registry = RadrootsRuntimeTransportRegistry::new();
            registry
                .register(ForgedReceiptTransport { forged })
                .expect("register");
            let worker = delivery_worker(&registry, 8);

            let error = worker
                .execute_job(delivery_job(
                    vec![delivery_plan(
                        RadrootsTransportSatisfactionPolicy::any_accepted(),
                        vec![delivery_target(
                            1,
                            RadrootsTransportKind::Nostr,
                            "wss://relay.example",
                        )],
                    )],
                    1_000,
                ))
                .await
                .expect_err("forged receipt rejected");
            assert!(matches!(
                error,
                RadrootsRuntimeTransportError::PartialDelivery { .. }
            ));
            let partial = error.partial_delivery_receipt().expect("partial receipt");
            assert_eq!(partial.dispatch_count(), 1);
            assert!(partial.target_receipts().is_empty());
        }
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_reports_no_wait_satisfied_without_dispatch() {
        let registry = RadrootsRuntimeTransportRegistry::new();
        let worker = delivery_worker(&registry, 8);
        let receipt = worker
            .execute_job(delivery_job(
                vec![delivery_plan(
                    RadrootsTransportSatisfactionPolicy::no_wait(),
                    vec![delivery_target(
                        1,
                        RadrootsTransportKind::Nostr,
                        "wss://relay.example",
                    )],
                )],
                1_000,
            ))
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count(), 0);
        assert!(receipt.all_plans_satisfied());
        let plan_receipt = &receipt.plan_receipts()[0];
        assert_eq!(plan_receipt.target_count(), 1);
        assert_eq!(plan_receipt.attempted_target_count(), 0);
        assert_eq!(plan_receipt.required_target_count(), 0);
        assert_eq!(plan_receipt.satisfied_target_count(), 0);
        assert_eq!(
            plan_receipt.satisfaction_state(),
            &RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
        );
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_required_targets_use_fingerprints() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        registry
            .register(StaticTransport::new(
                RadrootsTransportKind::Nostr,
                RadrootsTransportOutcomeKind::Accepted,
            ))
            .expect("register");
        let worker = delivery_worker(&registry, 8);
        let required_target = target(RadrootsTransportKind::Reticulum, "reticulum:local");
        let required_fingerprint = required_target.fingerprint().clone();
        let receipt = worker
            .execute_job(delivery_job(
                vec![delivery_plan(
                    RadrootsTransportSatisfactionPolicy::required_targets(
                        RadrootsTransportSatisfactionClass::Accepted,
                        vec![required_fingerprint],
                    )
                    .expect("required target policy"),
                    vec![
                        delivery_target(1, RadrootsTransportKind::Nostr, "wss://relay.example"),
                        RadrootsRuntimeDeliveryTarget::deferred_until_implemented(
                            2,
                            required_target,
                        )
                        .expect("deferred target"),
                    ],
                )],
                1_000,
            ))
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count(), 1);
        assert!(!receipt.all_plans_satisfied());
        let plan_receipt = &receipt.plan_receipts()[0];
        assert_eq!(plan_receipt.target_count(), 2);
        assert_eq!(plan_receipt.attempted_target_count(), 1);
        assert_eq!(plan_receipt.required_target_count(), 1);
        assert_eq!(plan_receipt.satisfied_target_count(), 0);
        assert_eq!(
            plan_receipt.satisfaction_state(),
            &RadrootsRuntimeDeliveryPlanSatisfactionState::Unsatisfied
        );
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_keeps_accepted_and_delivered_satisfaction_distinct() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        registry
            .register(StaticTransport::new(
                RadrootsTransportKind::Nostr,
                RadrootsTransportOutcomeKind::Accepted,
            ))
            .expect("register");
        let worker = delivery_worker(&registry, 8);
        let receipt = worker
            .execute_job(delivery_job(
                vec![delivery_plan(
                    RadrootsTransportSatisfactionPolicy::any_delivered(),
                    vec![delivery_target(
                        1,
                        RadrootsTransportKind::Nostr,
                        "wss://relay.example",
                    )],
                )],
                1_000,
            ))
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count(), 1);
        assert!(!receipt.all_plans_satisfied());
        assert_eq!(receipt.plan_receipts()[0].required_target_count(), 1);
        assert_eq!(receipt.plan_receipts()[0].satisfied_target_count(), 0);
        assert_eq!(
            receipt.plan_receipts()[0].satisfaction_state(),
            &RadrootsRuntimeDeliveryPlanSatisfactionState::Unsatisfied
        );
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_reports_quorum_satisfaction() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        registry
            .register(StaticTransport::new(
                RadrootsTransportKind::Nostr,
                RadrootsTransportOutcomeKind::Accepted,
            ))
            .expect("register");
        let worker = delivery_worker(&registry, 8);
        let receipt = worker
            .execute_job(delivery_job(
                vec![delivery_plan(
                    RadrootsTransportSatisfactionPolicy::quorum_accepted(2).expect("valid quorum"),
                    vec![
                        delivery_target(1, RadrootsTransportKind::Nostr, "wss://relay-a.example"),
                        delivery_target(2, RadrootsTransportKind::Nostr, "wss://relay-b.example"),
                    ],
                )],
                1_000,
            ))
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count(), 1);
        assert!(receipt.all_plans_satisfied());
        let plan_receipt = &receipt.plan_receipts()[0];
        assert_eq!(plan_receipt.target_count(), 2);
        assert_eq!(plan_receipt.attempted_target_count(), 2);
        assert_eq!(plan_receipt.required_target_count(), 2);
        assert_eq!(plan_receipt.satisfied_target_count(), 2);
        assert_eq!(
            plan_receipt.satisfaction_state(),
            &RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
        );
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_evaluates_cross_transport_quorum_globally() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        for kind in [
            RadrootsTransportKind::Nostr,
            RadrootsTransportKind::Reticulum,
        ] {
            registry
                .register(StaticTransport::new(
                    kind,
                    RadrootsTransportOutcomeKind::Accepted,
                ))
                .expect("register");
        }
        let worker = delivery_worker(&registry, 8);
        let receipt = worker
            .execute_job(delivery_job(
                vec![delivery_plan(
                    RadrootsTransportSatisfactionPolicy::quorum_accepted(2).expect("valid quorum"),
                    vec![
                        delivery_target(1, RadrootsTransportKind::Nostr, "wss://relay.example"),
                        delivery_target(2, RadrootsTransportKind::Reticulum, "reticulum:local"),
                    ],
                )],
                1_000,
            ))
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count(), 2);
        assert!(receipt.all_plans_satisfied());
        let plan_receipt = &receipt.plan_receipts()[0];
        assert_eq!(plan_receipt.target_count(), 2);
        assert_eq!(plan_receipt.attempted_target_count(), 2);
        assert_eq!(plan_receipt.required_target_count(), 2);
        assert_eq!(plan_receipt.satisfied_target_count(), 2);
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_reports_reticulum_targets_unsatisfied_without_retry_failure() {
        let registry = RadrootsRuntimeTransportRegistry::new();
        let worker = delivery_worker(&registry, 8);
        let reticulum_target =
            deferred_delivery_target(1, RadrootsTransportKind::Reticulum, "reticulum:local");
        let receipt = worker
            .execute_job(delivery_job(
                vec![delivery_plan(
                    RadrootsTransportSatisfactionPolicy::any_accepted(),
                    vec![reticulum_target],
                )],
                1_000,
            ))
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count(), 0);
        assert!(!receipt.all_plans_satisfied());
        let plan_receipt = &receipt.plan_receipts()[0];
        assert_eq!(plan_receipt.target_count(), 1);
        assert_eq!(plan_receipt.attempted_target_count(), 0);
        assert_eq!(plan_receipt.required_target_count(), 1);
        assert_eq!(plan_receipt.satisfied_target_count(), 0);
        assert_eq!(
            plan_receipt.satisfaction_state(),
            &RadrootsRuntimeDeliveryPlanSatisfactionState::Unsatisfied
        );
        assert!(receipt.target_receipts().is_empty());
    }

    #[cfg(feature = "transport-workers")]
    #[test]
    fn delivery_topology_rejects_invalid_ids_empty_and_duplicate_authority() {
        let nostr = target(RadrootsTransportKind::Nostr, "wss://relay.example");
        assert!(matches!(
            RadrootsRuntimeDeliveryTarget::ready(0, nostr.clone()),
            Err(RadrootsRuntimeTransportError::InvalidIdentifier { .. })
        ));
        assert!(matches!(
            RadrootsRuntimeDeliveryTarget::with_initial_status(
                1,
                nostr.clone(),
                RadrootsTransportDeliveryTargetStatus::Accepted,
            ),
            Err(RadrootsRuntimeTransportError::InvalidDeliveryTargetInitialStatus { .. })
        ));
        assert!(matches!(
            RadrootsRuntimeDeliveryPlan::new(
                7,
                RadrootsTransportSatisfactionPolicy::no_wait(),
                Vec::new(),
            ),
            Err(RadrootsRuntimeTransportError::EmptyDeliveryPlan)
        ));

        let first = RadrootsRuntimeDeliveryTarget::ready(1, nostr.clone()).expect("target");
        let duplicate_id = RadrootsRuntimeDeliveryTarget::ready(
            1,
            target(RadrootsTransportKind::Nostr, "wss://relay-2.example"),
        )
        .expect("target");
        assert!(matches!(
            RadrootsRuntimeDeliveryPlan::new(
                7,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                vec![first.clone(), duplicate_id],
            ),
            Err(RadrootsRuntimeTransportError::DuplicateDeliveryTargetId { .. })
        ));

        let duplicate_fingerprint =
            RadrootsRuntimeDeliveryTarget::ready(2, nostr.clone()).expect("target");
        assert!(matches!(
            RadrootsRuntimeDeliveryPlan::new(
                7,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                vec![first.clone(), duplicate_fingerprint],
            ),
            Err(RadrootsRuntimeTransportError::TransportTarget(_))
        ));

        let absent = target(RadrootsTransportKind::Nostr, "wss://absent.example");
        let required_absent = RadrootsTransportSatisfactionPolicy::required_targets(
            RadrootsTransportSatisfactionClass::Accepted,
            vec![absent.fingerprint().clone()],
        )
        .expect("required policy");
        assert!(matches!(
            RadrootsRuntimeDeliveryPlan::new(7, required_absent, vec![first.clone()]),
            Err(RadrootsRuntimeTransportError::TransportTarget(_))
        ));

        let plan = RadrootsRuntimeDeliveryPlan::new(
            7,
            RadrootsTransportSatisfactionPolicy::any_accepted(),
            vec![first],
        )
        .expect("plan");
        let ready = delivery_target(1, RadrootsTransportKind::Nostr, "wss://relay.example");
        let deferred =
            deferred_delivery_target(2, RadrootsTransportKind::Reticulum, "reticulum:local");
        assert_eq!(
            RadrootsRuntimeDeliveryPlan::new(
                8,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                vec![ready.clone(), deferred.clone()],
            )
            .expect("delivery-capable all policy")
            .required_target_count(),
            1
        );
        assert!(matches!(
            RadrootsRuntimeDeliveryPlan::new(
                8,
                RadrootsTransportSatisfactionPolicy::quorum_accepted(2).expect("quorum policy"),
                vec![ready, deferred],
            ),
            Err(RadrootsRuntimeTransportError::TransportTarget(_))
        ));
        assert!(matches!(
            RadrootsRuntimeDeliveryJob::new(42, opaque_payload(), Vec::new(), 1_000),
            Err(RadrootsRuntimeTransportError::EmptyDeliveryJob)
        ));
        assert!(matches!(
            RadrootsRuntimeDeliveryJob::new(42, opaque_payload(), vec![plan.clone(), plan], 1_000,),
            Err(RadrootsRuntimeTransportError::DuplicateDeliveryPlanId { .. })
        ));
        assert!(matches!(
            RadrootsRuntimeDeliveryJob::new(0, opaque_payload(), Vec::new(), 1_000),
            Err(RadrootsRuntimeTransportError::InvalidIdentifier { .. })
        ));
        assert!(matches!(
            RadrootsRuntimeDeliveryJob::new(42, opaque_payload(), Vec::new(), -1),
            Err(RadrootsRuntimeTransportError::InvalidTimestamp { .. })
        ));
    }

    #[cfg(feature = "transport-workers")]
    #[test]
    fn delivery_topology_accepts_exact_target_boundary_and_rejects_one_over() {
        let targets = (0..radroots_transport::RADROOTS_TRANSPORT_TARGET_MAX_COUNT)
            .map(|index| {
                delivery_target(
                    i64::try_from(index + 1).expect("target id"),
                    RadrootsTransportKind::Nostr,
                    format!("wss://relay-{index}.example").as_str(),
                )
            })
            .collect::<Vec<_>>();
        RadrootsRuntimeDeliveryPlan::new(
            7,
            RadrootsTransportSatisfactionPolicy::all_accepted(),
            targets.clone(),
        )
        .expect("exact target boundary");

        let mut one_over = targets;
        one_over.push(delivery_target(
            17,
            RadrootsTransportKind::Nostr,
            "wss://relay-over.example",
        ));
        assert!(matches!(
            RadrootsRuntimeDeliveryPlan::new(
                7,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                one_over,
            ),
            Err(RadrootsRuntimeTransportError::TransportTarget(_))
        ));
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_returns_completed_receipts_after_later_transport_failure() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        registry
            .register(StaticTransport::new(
                RadrootsTransportKind::Nostr,
                RadrootsTransportOutcomeKind::Accepted,
            ))
            .expect("register");
        let worker = delivery_worker(&registry, 8);
        let job = delivery_job(
            vec![delivery_plan(
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                vec![
                    delivery_target(1, RadrootsTransportKind::Nostr, "wss://relay.example"),
                    delivery_target(2, RadrootsTransportKind::Reticulum, "reticulum:local"),
                ],
            )],
            1_000,
        );

        let error = worker
            .execute_job(job)
            .await
            .expect_err("later transport failure");
        let partial = error.partial_delivery_receipt().expect("partial receipt");
        assert_eq!(partial.outbox_event_id(), 42);
        assert_eq!(partial.dispatch_count(), 1);
        assert_eq!(partial.plan_receipts().len(), 1);
        assert_eq!(partial.target_receipts().len(), 1);
        assert_eq!(
            partial.target_receipts()[0].status(),
            RadrootsTransportDeliveryTargetStatus::Accepted
        );
        assert_eq!(partial.plan_receipts()[0].targets().len(), 2);
        assert!(!partial.all_plans_satisfied());
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_queue_bounds_admission_and_releases_cancelled_work() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        registry.register(PendingTransport).expect("register");
        let worker = delivery_worker(&registry, 1);
        let job = || {
            delivery_job(
                vec![delivery_plan(
                    RadrootsTransportSatisfactionPolicy::any_accepted(),
                    vec![delivery_target(
                        1,
                        RadrootsTransportKind::Nostr,
                        "wss://relay.example",
                    )],
                )],
                1_000,
            )
        };
        let mut first = Box::pin(worker.execute_job(job()));
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        assert!(matches!(
            std::future::Future::poll(first.as_mut(), &mut context),
            std::task::Poll::Pending
        ));
        assert_eq!(worker.queue_status().expect("queue status").in_flight(), 1);

        assert!(matches!(
            worker.execute_job(job()).await,
            Err(RadrootsRuntimeTransportError::QueueCapacityExhausted { capacity: 1 })
        ));
        drop(first);
        assert_eq!(worker.queue_status().expect("queue status").in_flight(), 0);
        assert_eq!(worker.queue_status().expect("queue status").queued(), 0);
    }

    #[cfg(feature = "transport-workers")]
    #[test]
    fn lease_recovery_marks_expired_claims() {
        let mut leases = vec![
            RadrootsRuntimeLeaseRecord::claimed(1, 900).expect("lease"),
            RadrootsRuntimeLeaseRecord::claimed(2, 1_100).expect("lease"),
        ];
        assert_eq!(
            recover_expired_leases(&mut leases, 1_000).expect("recovery"),
            1
        );
        assert!(!leases[0].is_claimed());
        assert!(leases[0].is_recovered());
        assert!(leases[1].is_claimed());
        assert!(matches!(
            recover_expired_leases(&mut leases, -1),
            Err(RadrootsRuntimeTransportError::InvalidTimestamp { .. })
        ));

        let mut invalid = vec![
            RadrootsRuntimeLeaseRecord::claimed(3, 500).expect("lease"),
            RadrootsRuntimeLeaseRecord::claimed(4, 600).expect("lease"),
        ];
        invalid[1].recovered = true;
        assert!(matches!(
            recover_expired_leases(&mut invalid, 1_000),
            Err(RadrootsRuntimeTransportError::InvalidLeaseState { record_id: 4 })
        ));
        assert!(invalid[0].is_claimed());
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn inbound_observation_sink_requires_verified_signed_events() {
        let event = signed_event();
        let sink = RecordingInboundSink {
            expected_event_id: event.signed_event().id_str().to_owned(),
        };

        let observation = RadrootsRuntimeInboundObservation::verified_signed_event(
            &event,
            target(RadrootsTransportKind::Nostr, "wss://relay.example"),
            1_000,
        )
        .expect("verified observation");
        record_verified_inbound_observation(&sink, event.clone(), observation)
            .await
            .expect("record observation");

        assert!(matches!(
            RadrootsRuntimeInboundObservation::verified_signed_event(
                &event,
                target(RadrootsTransportKind::Nostr, "wss://relay.example"),
                -1,
            ),
            Err(RadrootsRuntimeTransportError::InvalidTimestamp { .. })
        ));

        let exact_endpoint =
            "x".repeat(radroots_transport::RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES);
        RadrootsRuntimeInboundObservation::verified_signed_event(
            &event,
            target(RadrootsTransportKind::Local, exact_endpoint.as_str()),
            1_000,
        )
        .expect("exact endpoint boundary");
        assert!(RadrootsTransportTarget::new(RadrootsTransportKind::Local, "").is_err());
        assert!(
            RadrootsTransportTarget::new(
                RadrootsTransportKind::Local,
                "x".repeat(radroots_transport::RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES + 1),
            )
            .is_err()
        );

        let mut mismatched = RadrootsRuntimeInboundObservation::verified_signed_event(
            &event,
            target(RadrootsTransportKind::Nostr, "wss://relay.example"),
            1_000,
        )
        .expect("verified observation");
        mismatched.event_id = "a".repeat(64);
        assert!(matches!(
            record_verified_inbound_observation(&sink, event.clone(), mismatched).await,
            Err(RadrootsRuntimeTransportError::InboundObservationEventMismatch { .. })
        ));

        let invalid_raw = event
            .signed_event()
            .raw_json()
            .replace(event.signed_event().wire().sig.as_str(), &"0".repeat(128));
        let invalid_wire =
            RadrootsNip01EventWire::parse_json(&invalid_raw).expect("invalid signature wire");
        let invalid_signed = RadrootsSignedEvent::from_wire_verified_id(invalid_wire, invalid_raw)
            .expect("id-valid event");
        assert!(invalid_signed.verify_signature().is_err());
    }
}
