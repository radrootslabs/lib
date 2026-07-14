use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use radroots_event::{draft::RadrootsSignedEvent, wire::RadrootsNip01EventWire};
#[cfg(feature = "transport-workers")]
use radroots_transport::RadrootsTransportTargetReceipt;
use radroots_transport::{
    RadrootsTransport, RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
    RadrootsTransportError, RadrootsTransportKind, RadrootsTransportPayload,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget, RadrootsTransportTargetSet,
};
use thiserror::Error;

pub type RadrootsRuntimeTransportFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RadrootsRuntimeTransportError>> + Send + 'a>>;

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

    #[error("runtime queue capacity must be greater than zero")]
    InvalidQueueCapacity,

    #[error("inbound observation for event `{event_id}` is not verified")]
    InboundObservationUnverified { event_id: String },

    #[error(
        "inbound observation event `{observation_event_id}` does not match signed event `{signed_event_id}`"
    )]
    InboundObservationEventMismatch {
        observation_event_id: String,
        signed_event_id: String,
    },

    #[error("transport `{kind}` failed: {message}")]
    Transport { kind: String, message: String },
}

impl From<RadrootsTransportError> for RadrootsRuntimeTransportError {
    fn from(value: RadrootsTransportError) -> Self {
        Self::TransportTarget(value.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsRuntimeTransportPayload {
    SignedEvent(RadrootsSignedEvent),
    OpaqueBytes { label: String, bytes: Vec<u8> },
}

impl RadrootsRuntimeTransportPayload {
    pub fn verified_signed_event_json(
        event: &RadrootsSignedEvent,
    ) -> Result<RadrootsTransportPayload, RadrootsTransportError> {
        verify_signed_event_raw_json_matches_event(event)?;
        RadrootsTransportPayload::unchecked_signed_event_json(event.id_str(), event.raw_json())
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
    event: &RadrootsSignedEvent,
) -> Result<(), RadrootsTransportError> {
    let wire = RadrootsNip01EventWire::parse_json(event.raw_json())
        .map_err(|_| RadrootsTransportError::InvalidPayloadBytes)?;
    if wire.id.as_str() != event.id_str() {
        return Err(RadrootsTransportError::InvalidPayloadId);
    }
    (wire == *event.wire())
        .then_some(())
        .ok_or(RadrootsTransportError::InvalidPayloadBytes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeTransportDispatchRequest {
    pub request_id: String,
    pub payload: RadrootsRuntimeTransportPayload,
    pub target_set: RadrootsTransportTargetSet,
    pub satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    pub now_ms: i64,
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
        Ok(Self {
            request_id: request_id.into(),
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
        )
        .with_now_ms(self.now_ms))
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
    pub capacity: usize,
    pub queued: usize,
    pub in_flight: usize,
    pub shutdown: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeQueueTask<T> {
    pub sequence: u64,
    pub payload: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeBoundedQueue<T> {
    capacity: usize,
    next_sequence: u64,
    in_flight: usize,
    shutdown: bool,
    queue: VecDeque<RadrootsRuntimeQueueTask<T>>,
}

impl<T> RadrootsRuntimeBoundedQueue<T> {
    pub fn new(capacity: usize) -> Result<Self, RadrootsRuntimeTransportError> {
        if capacity == 0 {
            return Err(RadrootsRuntimeTransportError::InvalidQueueCapacity);
        }
        Ok(Self {
            capacity,
            next_sequence: 1,
            in_flight: 0,
            shutdown: false,
            queue: VecDeque::new(),
        })
    }

    pub fn try_enqueue(&mut self, payload: T) -> Option<u64> {
        if self.shutdown || self.queue.len() >= self.capacity {
            return None;
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.queue
            .push_back(RadrootsRuntimeQueueTask { sequence, payload });
        Some(sequence)
    }

    pub fn pop(&mut self) -> Option<RadrootsRuntimeQueueTask<T>> {
        let task = self.queue.pop_front()?;
        self.in_flight += 1;
        Some(task)
    }

    pub fn complete_task(&mut self) {
        self.in_flight = self.in_flight.saturating_sub(1);
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
    pub delivery_target_id: i64,
    pub target: RadrootsTransportTarget,
    pub status: RadrootsTransportDeliveryTargetStatus,
}

impl RadrootsRuntimeDeliveryTarget {
    pub fn ready(delivery_target_id: i64, target: RadrootsTransportTarget) -> Self {
        Self {
            delivery_target_id,
            target,
            status: RadrootsTransportDeliveryTargetStatus::Pending,
        }
    }

    pub fn deferred_until_implemented(
        delivery_target_id: i64,
        target: RadrootsTransportTarget,
    ) -> Self {
        Self {
            delivery_target_id,
            target,
            status: RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented,
        }
    }

    pub fn is_ready_for_attempt(&self) -> bool {
        self.status.is_ready_for_attempt()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryPlan {
    pub delivery_plan_id: i64,
    pub satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    pub targets: Vec<RadrootsRuntimeDeliveryTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryJob {
    pub outbox_event_id: i64,
    pub payload: RadrootsRuntimeTransportPayload,
    pub plans: Vec<RadrootsRuntimeDeliveryPlan>,
    pub now_ms: i64,
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryWorkerConfig {
    pub bounded_queue_capacity: usize,
}

#[cfg(feature = "transport-workers")]
impl Default for RadrootsRuntimeDeliveryWorkerConfig {
    fn default() -> Self {
        Self {
            bounded_queue_capacity: 64,
        }
    }
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct RadrootsRuntimeDeliveryTargetState {
    target: RadrootsTransportTarget,
    status: RadrootsTransportDeliveryTargetStatus,
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
    pub delivery_plan_id: i64,
    pub satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    pub target_count: usize,
    pub attempted_target_count: usize,
    pub required_target_count: usize,
    pub satisfied_target_count: usize,
    pub satisfaction_state: RadrootsRuntimeDeliveryPlanSatisfactionState,
    pub target_receipts: Vec<RadrootsTransportTargetReceipt>,
}

#[cfg(feature = "transport-workers")]
impl RadrootsRuntimeDeliveryPlanReceipt {
    fn from_target_states(
        delivery_plan_id: i64,
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
        target_states: &[RadrootsRuntimeDeliveryTargetState],
        target_receipts: Vec<RadrootsTransportTargetReceipt>,
    ) -> Result<Self, RadrootsRuntimeTransportError> {
        let required_target_count =
            satisfaction_policy.required_target_count(target_states.len())?;
        let satisfied_target_count =
            satisfied_target_count_for_policy(&satisfaction_policy, target_states);
        let satisfaction_state =
            if target_states_satisfy_policy(&satisfaction_policy, target_states)? {
                RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
            } else {
                RadrootsRuntimeDeliveryPlanSatisfactionState::Unsatisfied
            };
        Ok(Self {
            delivery_plan_id,
            satisfaction_policy,
            target_count: target_states.len(),
            attempted_target_count: target_receipts.len(),
            required_target_count,
            satisfied_target_count,
            satisfaction_state,
            target_receipts,
        })
    }

    pub fn is_satisfied(&self) -> bool {
        self.satisfaction_state == RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
    }
}

#[cfg(feature = "transport-workers")]
fn satisfied_target_count_for_policy(
    policy: &RadrootsTransportSatisfactionPolicy,
    target_states: &[RadrootsRuntimeDeliveryTargetState],
) -> usize {
    match policy {
        RadrootsTransportSatisfactionPolicy::NoWait => 0,
        RadrootsTransportSatisfactionPolicy::Any { class }
        | RadrootsTransportSatisfactionPolicy::All { class }
        | RadrootsTransportSatisfactionPolicy::Quorum { class, .. } => target_states
            .iter()
            .filter(|state| state.status.counts_as_satisfied(*class))
            .count(),
        RadrootsTransportSatisfactionPolicy::RequiredTargets { class, targets } => targets
            .iter()
            .filter(|required| {
                target_states.iter().any(|state| {
                    state.target.fingerprint == **required
                        && state.status.counts_as_satisfied(*class)
                })
            })
            .count(),
    }
}

#[cfg(feature = "transport-workers")]
fn target_states_satisfy_policy(
    policy: &RadrootsTransportSatisfactionPolicy,
    target_states: &[RadrootsRuntimeDeliveryTargetState],
) -> Result<bool, RadrootsRuntimeTransportError> {
    match policy {
        RadrootsTransportSatisfactionPolicy::NoWait => Ok(true),
        RadrootsTransportSatisfactionPolicy::Any { .. }
        | RadrootsTransportSatisfactionPolicy::All { .. }
        | RadrootsTransportSatisfactionPolicy::Quorum { .. } => Ok(policy.is_satisfied_by(
            target_states.len(),
            satisfied_target_count_for_policy(policy, target_states),
        )?),
        RadrootsTransportSatisfactionPolicy::RequiredTargets { targets, .. } => {
            policy.required_target_count(target_states.len())?;
            Ok(satisfied_target_count_for_policy(policy, target_states) == targets.len())
        }
    }
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeDeliveryJobReceipt {
    pub outbox_event_id: i64,
    pub dispatch_count: usize,
    pub plan_receipts: Vec<RadrootsRuntimeDeliveryPlanReceipt>,
    pub all_plans_satisfied: bool,
    pub target_receipts: Vec<RadrootsTransportTargetReceipt>,
}

#[cfg(feature = "transport-workers")]
pub struct RadrootsRuntimeDeliveryWorker<'a> {
    registry: &'a RadrootsRuntimeTransportRegistry,
    config: RadrootsRuntimeDeliveryWorkerConfig,
}

#[cfg(feature = "transport-workers")]
impl<'a> RadrootsRuntimeDeliveryWorker<'a> {
    pub fn new(
        registry: &'a RadrootsRuntimeTransportRegistry,
        config: RadrootsRuntimeDeliveryWorkerConfig,
    ) -> Self {
        Self { registry, config }
    }

    pub fn queue_capacity(&self) -> usize {
        self.config.bounded_queue_capacity
    }

    pub async fn execute_job(
        &self,
        job: RadrootsRuntimeDeliveryJob,
    ) -> Result<RadrootsRuntimeDeliveryJobReceipt, RadrootsRuntimeTransportError> {
        let mut target_receipts = Vec::new();
        let mut plan_receipts = Vec::new();
        let mut dispatch_count = 0usize;
        for plan in job.plans {
            let delivery_plan_id = plan.delivery_plan_id;
            let satisfaction_policy = plan.satisfaction_policy.clone();
            let mut target_states = plan
                .targets
                .iter()
                .map(|target| {
                    (
                        target.target.fingerprint.as_str().to_owned(),
                        RadrootsRuntimeDeliveryTargetState {
                            target: target.target.clone(),
                            status: target.status,
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let mut by_kind =
                BTreeMap::<RadrootsTransportKind, Vec<RadrootsRuntimeDeliveryTarget>>::new();
            for target in plan
                .targets
                .into_iter()
                .filter(RadrootsRuntimeDeliveryTarget::is_ready_for_attempt)
            {
                by_kind
                    .entry(target.target.kind.clone())
                    .or_default()
                    .push(target);
            }
            let mut plan_target_receipts = Vec::new();
            for (kind, targets) in by_kind {
                let transport = self.registry.transport(&kind)?;
                let transport_targets = targets
                    .iter()
                    .map(|target| target.target.clone())
                    .collect::<Vec<_>>();
                let request = RadrootsRuntimeTransportDispatchRequest::new(
                    format!(
                        "outbox-event-{}-plan-{}",
                        job.outbox_event_id, plan.delivery_plan_id
                    ),
                    job.payload.clone(),
                    transport_targets,
                    satisfaction_policy.clone(),
                    job.now_ms,
                )?;
                let receipt = transport
                    .deliver(request.transport_delivery_request()?)
                    .await
                    .map_err(|error| RadrootsRuntimeTransportError::Transport {
                        kind: kind.canonical_label(),
                        message: error.to_string(),
                    })?;
                for target_receipt in receipt.target_receipts {
                    target_states.insert(
                        target_receipt.target.fingerprint.as_str().to_owned(),
                        RadrootsRuntimeDeliveryTargetState {
                            target: target_receipt.target.clone(),
                            status: target_receipt.status,
                        },
                    );
                    plan_target_receipts.push(target_receipt);
                }
                dispatch_count += 1;
            }
            let final_target_states = target_states.into_values().collect::<Vec<_>>();
            let plan_receipt = RadrootsRuntimeDeliveryPlanReceipt::from_target_states(
                delivery_plan_id,
                satisfaction_policy,
                &final_target_states,
                plan_target_receipts,
            )?;
            target_receipts.extend(plan_receipt.target_receipts.iter().cloned());
            plan_receipts.push(plan_receipt);
        }
        let all_plans_satisfied = plan_receipts
            .iter()
            .all(RadrootsRuntimeDeliveryPlanReceipt::is_satisfied);
        Ok(RadrootsRuntimeDeliveryJobReceipt {
            outbox_event_id: job.outbox_event_id,
            dispatch_count,
            plan_receipts,
            all_plans_satisfied,
            target_receipts,
        })
    }
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeLeaseRecord {
    pub record_id: i64,
    pub claimed: bool,
    pub claim_expires_at_ms: i64,
    pub recovered: bool,
}

#[cfg(feature = "transport-workers")]
pub fn recover_expired_leases(leases: &mut [RadrootsRuntimeLeaseRecord], now_ms: i64) -> usize {
    let mut recovered = 0usize;
    for lease in leases {
        if lease.claimed && lease.claim_expires_at_ms <= now_ms {
            lease.claimed = false;
            lease.recovered = true;
            recovered += 1;
        }
    }
    recovered
}

#[cfg(feature = "transport-workers")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRuntimeInboundObservation {
    pub event_id: String,
    pub verified: bool,
    pub transport_kind: RadrootsTransportKind,
    pub endpoint_uri: String,
    pub observed_at_ms: i64,
}

#[cfg(feature = "transport-workers")]
impl RadrootsRuntimeInboundObservation {
    pub fn verified_signed_event(
        event: &RadrootsSignedEvent,
        transport_kind: RadrootsTransportKind,
        endpoint_uri: impl Into<String>,
        observed_at_ms: i64,
    ) -> Self {
        Self {
            event_id: event.id_str().to_owned(),
            verified: true,
            transport_kind,
            endpoint_uri: endpoint_uri.into(),
            observed_at_ms,
        }
    }

    pub fn require_verified_for_signed_event(
        &self,
        event: &RadrootsSignedEvent,
    ) -> Result<(), RadrootsRuntimeTransportError> {
        if !self.verified {
            return Err(
                RadrootsRuntimeTransportError::InboundObservationUnverified {
                    event_id: self.event_id.clone(),
                },
            );
        }
        if self.event_id != event.id_str() {
            return Err(
                RadrootsRuntimeTransportError::InboundObservationEventMismatch {
                    observation_event_id: self.event_id.clone(),
                    signed_event_id: event.id_str().to_owned(),
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
        event: RadrootsSignedEvent,
        observation: RadrootsRuntimeInboundObservation,
    ) -> RadrootsRuntimeTransportFuture<'a, ()>;
}

#[cfg(feature = "transport-workers")]
pub fn record_verified_inbound_observation<'a, S>(
    sink: &'a S,
    event: RadrootsSignedEvent,
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
        RadrootsRuntimeBoundedQueue, RadrootsRuntimeTransportDispatchRequest,
        RadrootsRuntimeTransportError, RadrootsRuntimeTransportFuture,
        RadrootsRuntimeTransportPayload, RadrootsRuntimeTransportRegistry,
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
    use radroots_event::{draft::RadrootsSignedEvent, wire::RadrootsNip01EventWire};
    use radroots_transport::{
        RadrootsTransport, RadrootsTransportCapabilities, RadrootsTransportDeliveryReceipt,
        RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
        RadrootsTransportError, RadrootsTransportFetchReceipt, RadrootsTransportFetchRequest,
        RadrootsTransportFuture, RadrootsTransportImplementationState, RadrootsTransportKind,
        RadrootsTransportOutcome, RadrootsTransportOutcomeKind, RadrootsTransportPayload,
        RadrootsTransportSatisfactionClass, RadrootsTransportSatisfactionPolicy,
        RadrootsTransportStatus, RadrootsTransportTarget, RadrootsTransportTargetReceipt,
        RadrootsTransportTargetSet,
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
                Ok(RadrootsTransportStatus::new(
                    self.kind.clone(),
                    true,
                    RadrootsTransportImplementationState::Real,
                    true,
                    "ready",
                )
                .with_capabilities(RadrootsTransportCapabilities::deliver_and_fetch()))
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
                        .push(request.now_ms);
                }
                Ok(RadrootsTransportDeliveryReceipt {
                    request_id: request.request_id,
                    target_receipts: request
                        .target_set
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
                })
            })
        }

        fn fetch<'a>(
            &'a self,
            request: RadrootsTransportFetchRequest,
        ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
            Box::pin(async move {
                Ok(RadrootsTransportFetchReceipt::new(
                    request.request_id,
                    request
                        .target_set
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
                ))
            })
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
    fn signed_event() -> RadrootsSignedEvent {
        let pubkey = "e".repeat(64);
        let sig = "f".repeat(128);
        let mut wire = RadrootsNip01EventWire {
            id: String::new(),
            pubkey: pubkey.clone(),
            created_at: 10,
            kind: 1,
            tags: Vec::new(),
            content: "hello".to_owned(),
            sig: sig.clone(),
            extra: Default::default(),
        };
        wire.id = wire
            .computed_event_id()
            .expect("computed event id")
            .into_string();
        let raw_json = format!(
            "{{\"id\":\"{id}\",\"pubkey\":\"{pubkey}\",\"created_at\":10,\"kind\":1,\"tags\":[],\"content\":\"hello\",\"sig\":\"{sig}\"}}",
            id = wire.id.as_str()
        );
        RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    #[cfg(feature = "transport-workers")]
    #[test]
    fn runtime_verified_signed_event_payload_uses_structured_signed_event() {
        let event = signed_event();
        let payload = RadrootsRuntimeTransportPayload::verified_signed_event_json(&event)
            .expect("verified payload");
        let via_variant = RadrootsRuntimeTransportPayload::SignedEvent(event.clone())
            .transport_payload()
            .expect("transport payload");
        let RadrootsTransportPayload::SignedEventJson {
            event_id, raw_json, ..
        } = payload.clone()
        else {
            panic!("signed event payload expected");
        };

        assert_eq!(payload, via_variant);
        assert_eq!(event_id, event.id_str());
        assert_eq!(raw_json, event.raw_json());

        let mismatched = RadrootsSignedEvent::from_wire_unchecked(event.wire().clone(), "{}")
            .expect("mismatched raw event");
        assert_eq!(
            RadrootsRuntimeTransportPayload::verified_signed_event_json(&mismatched)
                .expect_err("mismatched raw json"),
            RadrootsTransportError::InvalidPayloadBytes
        );
    }

    #[cfg(feature = "transport-workers")]
    struct RecordingInboundSink {
        expected_event_id: String,
    }

    #[cfg(feature = "transport-workers")]
    impl RadrootsRuntimeInboundObservationSink for RecordingInboundSink {
        fn record_verified_observation<'a>(
            &'a self,
            event: RadrootsSignedEvent,
            observation: RadrootsRuntimeInboundObservation,
        ) -> RadrootsRuntimeTransportFuture<'a, ()> {
            Box::pin(async move {
                assert_eq!(event.id_str(), self.expected_event_id);
                assert_eq!(observation.event_id, self.expected_event_id);
                assert!(observation.verified);
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
        assert_eq!(status.kind, RadrootsTransportKind::Nostr);
        assert_eq!(
            status.capabilities,
            RadrootsTransportCapabilities::deliver_and_fetch()
        );
        let fetch = transport
            .fetch(RadrootsTransportFetchRequest::new(
                "nostr-fetch",
                RadrootsTransportTargetSet::new(vec![target(
                    RadrootsTransportKind::Nostr,
                    "wss://relay.example",
                )])
                .expect("target set"),
            ))
            .await
            .expect("fetch");
        assert_eq!(fetch.fetched_count, 0);

        assert_eq!(
            receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
            1
        );
        assert_eq!(
            receipt.target_receipts[0].status,
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

        assert_eq!(delivery_request.now_ms, 123_456);
    }

    #[cfg(feature = "transport-reticulum")]
    #[tokio::test]
    async fn registry_dispatches_reticulum_preview_transport_without_success() {
        let mut registry = RadrootsRuntimeTransportRegistry::new();
        registry
            .register(radroots_transport_reticulum::RadrootsReticulumPreviewTransport::default())
            .expect("register");
        let transport = registry
            .transport(&RadrootsTransportKind::Reticulum)
            .expect("reticulum transport");
        let request = RadrootsRuntimeTransportDispatchRequest::new(
            "reticulum-delivery",
            opaque_payload(),
            vec![target(
                RadrootsTransportKind::Reticulum,
                "reticulum:preview-unavailable",
            )],
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
        assert_eq!(
            status.implementation,
            RadrootsTransportImplementationState::PreviewUnavailable
        );
        assert!(!status.capabilities.deliver);
        assert!(!status.capabilities.fetch);
        let fetch = transport
            .fetch(RadrootsTransportFetchRequest::new(
                "reticulum-fetch",
                RadrootsTransportTargetSet::new(vec![target(
                    RadrootsTransportKind::Reticulum,
                    "reticulum:preview-unavailable",
                )])
                .expect("target set"),
            ))
            .await
            .expect("fetch");
        assert_eq!(fetch.fetched_count, 0);

        assert_eq!(
            receipt.satisfied_target_count(RadrootsTransportSatisfactionClass::Accepted),
            0
        );
        assert_eq!(
            receipt.target_receipts[0].status,
            RadrootsTransportDeliveryTargetStatus::PreviewUnavailable
        );
    }

    #[test]
    fn bounded_queue_tracks_capacity_inflight_and_shutdown() {
        let mut queue = RadrootsRuntimeBoundedQueue::new(2).expect("queue");
        assert_eq!(queue.try_enqueue("a"), Some(1));
        assert_eq!(queue.try_enqueue("b"), Some(2));
        assert_eq!(queue.try_enqueue("c"), None);
        let task = queue.pop().expect("task");
        assert_eq!(task.sequence, 1);
        assert_eq!(queue.status().in_flight, 1);
        queue.complete_task();
        queue.shutdown();
        assert_eq!(queue.try_enqueue("d"), None);
        assert!(queue.status().shutdown);
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
        let worker = RadrootsRuntimeDeliveryWorker::new(
            &registry,
            RadrootsRuntimeDeliveryWorkerConfig {
                bounded_queue_capacity: 8,
            },
        );
        let ready = RadrootsRuntimeDeliveryTarget::ready(
            1,
            target(RadrootsTransportKind::Nostr, "wss://relay.example"),
        );
        let deferred = RadrootsRuntimeDeliveryTarget::deferred_until_implemented(
            2,
            target(
                RadrootsTransportKind::Reticulum,
                "reticulum:preview-unavailable",
            ),
        );
        let receipt = worker
            .execute_job(RadrootsRuntimeDeliveryJob {
                outbox_event_id: 42,
                payload: opaque_payload(),
                plans: vec![RadrootsRuntimeDeliveryPlan {
                    delivery_plan_id: 7,
                    satisfaction_policy: RadrootsTransportSatisfactionPolicy::any_accepted(),
                    targets: vec![ready, deferred],
                }],
                now_ms: 1_000,
            })
            .await
            .expect("worker receipt");

        assert_eq!(worker.queue_capacity(), 8);
        assert_eq!(receipt.dispatch_count, 1);
        assert!(receipt.all_plans_satisfied);
        assert_eq!(receipt.plan_receipts.len(), 1);
        assert_eq!(receipt.plan_receipts[0].delivery_plan_id, 7);
        assert_eq!(receipt.plan_receipts[0].target_count, 2);
        assert_eq!(receipt.plan_receipts[0].attempted_target_count, 1);
        assert_eq!(receipt.plan_receipts[0].required_target_count, 1);
        assert_eq!(receipt.plan_receipts[0].satisfied_target_count, 1);
        assert_eq!(
            receipt.plan_receipts[0].satisfaction_state,
            RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
        );
        assert_eq!(receipt.target_receipts.len(), 1);
        assert_eq!(
            receipt.target_receipts[0].status,
            RadrootsTransportDeliveryTargetStatus::Accepted
        );
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
        let worker = RadrootsRuntimeDeliveryWorker::new(
            &registry,
            RadrootsRuntimeDeliveryWorkerConfig {
                bounded_queue_capacity: 8,
            },
        );
        let receipt = worker
            .execute_job(RadrootsRuntimeDeliveryJob {
                outbox_event_id: 42,
                payload: opaque_payload(),
                plans: vec![RadrootsRuntimeDeliveryPlan {
                    delivery_plan_id: 7,
                    satisfaction_policy: RadrootsTransportSatisfactionPolicy::any_accepted(),
                    targets: vec![RadrootsRuntimeDeliveryTarget::ready(
                        1,
                        target(RadrootsTransportKind::Nostr, "wss://relay.example"),
                    )],
                }],
                now_ms: 987_654,
            })
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count, 1);
        assert_eq!(
            captured_now_ms.lock().expect("now_ms capture").as_slice(),
            &[987_654]
        );
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_reports_no_wait_satisfied_without_dispatch() {
        let registry = RadrootsRuntimeTransportRegistry::new();
        let worker = RadrootsRuntimeDeliveryWorker::new(
            &registry,
            RadrootsRuntimeDeliveryWorkerConfig {
                bounded_queue_capacity: 8,
            },
        );
        let receipt = worker
            .execute_job(RadrootsRuntimeDeliveryJob {
                outbox_event_id: 42,
                payload: opaque_payload(),
                plans: vec![RadrootsRuntimeDeliveryPlan {
                    delivery_plan_id: 7,
                    satisfaction_policy: RadrootsTransportSatisfactionPolicy::no_wait(),
                    targets: Vec::new(),
                }],
                now_ms: 1_000,
            })
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count, 0);
        assert!(receipt.all_plans_satisfied);
        assert_eq!(receipt.plan_receipts[0].target_count, 0);
        assert_eq!(receipt.plan_receipts[0].attempted_target_count, 0);
        assert_eq!(receipt.plan_receipts[0].required_target_count, 0);
        assert_eq!(receipt.plan_receipts[0].satisfied_target_count, 0);
        assert_eq!(
            receipt.plan_receipts[0].satisfaction_state,
            RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
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
        let worker = RadrootsRuntimeDeliveryWorker::new(
            &registry,
            RadrootsRuntimeDeliveryWorkerConfig {
                bounded_queue_capacity: 8,
            },
        );
        let required_target = target(
            RadrootsTransportKind::Reticulum,
            "reticulum:preview-unavailable",
        );
        let required_fingerprint = required_target.fingerprint.clone();
        let receipt = worker
            .execute_job(RadrootsRuntimeDeliveryJob {
                outbox_event_id: 42,
                payload: opaque_payload(),
                plans: vec![RadrootsRuntimeDeliveryPlan {
                    delivery_plan_id: 7,
                    satisfaction_policy: RadrootsTransportSatisfactionPolicy::required_targets(
                        RadrootsTransportSatisfactionClass::Accepted,
                        vec![required_fingerprint],
                    )
                    .expect("required target policy"),
                    targets: vec![
                        RadrootsRuntimeDeliveryTarget::ready(
                            1,
                            target(RadrootsTransportKind::Nostr, "wss://relay.example"),
                        ),
                        RadrootsRuntimeDeliveryTarget::deferred_until_implemented(
                            2,
                            required_target,
                        ),
                    ],
                }],
                now_ms: 1_000,
            })
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count, 1);
        assert!(!receipt.all_plans_satisfied);
        assert_eq!(receipt.plan_receipts[0].target_count, 2);
        assert_eq!(receipt.plan_receipts[0].attempted_target_count, 1);
        assert_eq!(receipt.plan_receipts[0].required_target_count, 1);
        assert_eq!(receipt.plan_receipts[0].satisfied_target_count, 0);
        assert_eq!(
            receipt.plan_receipts[0].satisfaction_state,
            RadrootsRuntimeDeliveryPlanSatisfactionState::Unsatisfied
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
        let worker = RadrootsRuntimeDeliveryWorker::new(
            &registry,
            RadrootsRuntimeDeliveryWorkerConfig {
                bounded_queue_capacity: 8,
            },
        );
        let receipt = worker
            .execute_job(RadrootsRuntimeDeliveryJob {
                outbox_event_id: 42,
                payload: opaque_payload(),
                plans: vec![RadrootsRuntimeDeliveryPlan {
                    delivery_plan_id: 7,
                    satisfaction_policy: RadrootsTransportSatisfactionPolicy::any_delivered(),
                    targets: vec![RadrootsRuntimeDeliveryTarget::ready(
                        1,
                        target(RadrootsTransportKind::Nostr, "wss://relay.example"),
                    )],
                }],
                now_ms: 1_000,
            })
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count, 1);
        assert!(!receipt.all_plans_satisfied);
        assert_eq!(receipt.plan_receipts[0].required_target_count, 1);
        assert_eq!(receipt.plan_receipts[0].satisfied_target_count, 0);
        assert_eq!(
            receipt.plan_receipts[0].satisfaction_state,
            RadrootsRuntimeDeliveryPlanSatisfactionState::Unsatisfied
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
        let worker = RadrootsRuntimeDeliveryWorker::new(
            &registry,
            RadrootsRuntimeDeliveryWorkerConfig {
                bounded_queue_capacity: 8,
            },
        );
        let receipt = worker
            .execute_job(RadrootsRuntimeDeliveryJob {
                outbox_event_id: 42,
                payload: opaque_payload(),
                plans: vec![RadrootsRuntimeDeliveryPlan {
                    delivery_plan_id: 7,
                    satisfaction_policy: RadrootsTransportSatisfactionPolicy::quorum_accepted(2),
                    targets: vec![
                        RadrootsRuntimeDeliveryTarget::ready(
                            1,
                            target(RadrootsTransportKind::Nostr, "wss://relay-a.example"),
                        ),
                        RadrootsRuntimeDeliveryTarget::ready(
                            2,
                            target(RadrootsTransportKind::Nostr, "wss://relay-b.example"),
                        ),
                    ],
                }],
                now_ms: 1_000,
            })
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count, 1);
        assert!(receipt.all_plans_satisfied);
        assert_eq!(receipt.plan_receipts[0].target_count, 2);
        assert_eq!(receipt.plan_receipts[0].attempted_target_count, 2);
        assert_eq!(receipt.plan_receipts[0].required_target_count, 2);
        assert_eq!(receipt.plan_receipts[0].satisfied_target_count, 2);
        assert_eq!(
            receipt.plan_receipts[0].satisfaction_state,
            RadrootsRuntimeDeliveryPlanSatisfactionState::Satisfied
        );
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn delivery_worker_reports_reticulum_preview_targets_unsatisfied_without_retry_failure() {
        let registry = RadrootsRuntimeTransportRegistry::new();
        let worker = RadrootsRuntimeDeliveryWorker::new(
            &registry,
            RadrootsRuntimeDeliveryWorkerConfig {
                bounded_queue_capacity: 8,
            },
        );
        let preview_target = RadrootsRuntimeDeliveryTarget {
            delivery_target_id: 1,
            target: target(
                RadrootsTransportKind::Reticulum,
                "reticulum:preview-unavailable",
            ),
            status: RadrootsTransportDeliveryTargetStatus::PreviewUnavailable,
        };
        let receipt = worker
            .execute_job(RadrootsRuntimeDeliveryJob {
                outbox_event_id: 42,
                payload: opaque_payload(),
                plans: vec![RadrootsRuntimeDeliveryPlan {
                    delivery_plan_id: 7,
                    satisfaction_policy: RadrootsTransportSatisfactionPolicy::any_accepted(),
                    targets: vec![preview_target],
                }],
                now_ms: 1_000,
            })
            .await
            .expect("worker receipt");

        assert_eq!(receipt.dispatch_count, 0);
        assert!(!receipt.all_plans_satisfied);
        assert_eq!(receipt.plan_receipts[0].target_count, 1);
        assert_eq!(receipt.plan_receipts[0].attempted_target_count, 0);
        assert_eq!(receipt.plan_receipts[0].required_target_count, 1);
        assert_eq!(receipt.plan_receipts[0].satisfied_target_count, 0);
        assert_eq!(
            receipt.plan_receipts[0].satisfaction_state,
            RadrootsRuntimeDeliveryPlanSatisfactionState::Unsatisfied
        );
        assert!(receipt.target_receipts.is_empty());
    }

    #[cfg(feature = "transport-workers")]
    #[test]
    fn lease_recovery_marks_expired_claims() {
        let mut leases = vec![
            RadrootsRuntimeLeaseRecord {
                record_id: 1,
                claimed: true,
                claim_expires_at_ms: 900,
                recovered: false,
            },
            RadrootsRuntimeLeaseRecord {
                record_id: 2,
                claimed: true,
                claim_expires_at_ms: 1_100,
                recovered: false,
            },
        ];
        assert_eq!(recover_expired_leases(&mut leases, 1_000), 1);
        assert!(!leases[0].claimed);
        assert!(leases[0].recovered);
        assert!(leases[1].claimed);
    }

    #[cfg(feature = "transport-workers")]
    #[tokio::test]
    async fn inbound_observation_sink_requires_verified_signed_events() {
        let event = signed_event();
        let sink = RecordingInboundSink {
            expected_event_id: event.id_str().to_owned(),
        };

        let observation = RadrootsRuntimeInboundObservation::verified_signed_event(
            &event,
            RadrootsTransportKind::Nostr,
            "wss://relay.example",
            1_000,
        );
        record_verified_inbound_observation(&sink, event.clone(), observation)
            .await
            .expect("record observation");

        let unverified = RadrootsRuntimeInboundObservation {
            event_id: event.id_str().to_owned(),
            verified: false,
            transport_kind: RadrootsTransportKind::Nostr,
            endpoint_uri: "wss://relay.example".to_owned(),
            observed_at_ms: 1_000,
        };
        assert!(matches!(
            record_verified_inbound_observation(&sink, event.clone(), unverified).await,
            Err(RadrootsRuntimeTransportError::InboundObservationUnverified { .. })
        ));

        let mismatched = RadrootsRuntimeInboundObservation {
            event_id: "a".repeat(64),
            verified: true,
            transport_kind: RadrootsTransportKind::Nostr,
            endpoint_uri: "wss://relay.example".to_owned(),
            observed_at_ms: 1_000,
        };
        assert!(matches!(
            record_verified_inbound_observation(&sink, event, mismatched).await,
            Err(RadrootsRuntimeTransportError::InboundObservationEventMismatch { .. })
        ));
    }
}
