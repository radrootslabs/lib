//! Stable per-relay evidence, aggregate status, and outcome normalization.

use crate::{Config, ReconnectBackoff, RelayEndpoint, RelayProfileKind, RelayUrl};
use radroots_transport::{
    SinkStatus, SourceStatus,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
    outcome::{DeliveryOutcome, DeliveryOutcomeKind, FetchTargetState, Retryability},
};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureClass {
    Duplicate,
    Rejected,
    AuthRequired,
    RateLimited,
    Timeout,
    Connection,
    Malformed,
    Unknown,
}

impl FailureClass {
    const fn code(self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate",
            Self::Rejected => "rejected",
            Self::AuthRequired => "auth_required",
            Self::RateLimited => "rate_limited",
            Self::Timeout => "timeout",
            Self::Connection => "connection_failed",
            Self::Malformed => "malformed_event",
            Self::Unknown => "relay_failure",
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::Duplicate => "relay already has the event",
            Self::Rejected => "relay rejected the event",
            Self::AuthRequired => "relay authentication is required",
            Self::RateLimited => "relay rate limit was reached",
            Self::Timeout => "relay operation timed out",
            Self::Connection => "relay connection failed",
            Self::Malformed => "relay returned a malformed event",
            Self::Unknown => "relay operation failed",
        }
    }
}

#[derive(Clone)]
struct RedactedDiagnostic {
    class: FailureClass,
}

impl fmt::Debug for RedactedDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedactedDiagnostic")
            .field("class", &self.class)
            .field("upstream", &"[redacted]")
            .finish()
    }
}

/// Evidence state for one relay capability direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RelayEvidenceState {
    /// The profile does not authorize this capability direction.
    Unsupported,
    /// The capability is configured but no successful or failed attempt exists.
    Unobserved,
    /// An authorized operation is currently awaiting relay evidence.
    Connecting,
    /// The latest accepted observation proved the capability usable.
    Available,
    /// The latest accepted observation failed to prove the capability usable.
    Unavailable,
}

/// Immutable evidence for one relay capability direction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayCapabilityEvidence {
    state: RelayEvidenceState,
    last_attempt_unix_ms: Option<u64>,
    last_success_unix_ms: Option<u64>,
    consecutive_failures: u32,
    next_attempt_unix_ms: Option<u64>,
    last_failure_retryable: Option<bool>,
}

impl RelayCapabilityEvidence {
    /// Returns whether this direction is unsupported, unobserved, or observed.
    #[must_use]
    pub const fn state(&self) -> RelayEvidenceState {
        self.state
    }

    /// Returns the latest monotonic accepted attempt timestamp.
    #[must_use]
    pub const fn last_attempt_unix_ms(&self) -> Option<u64> {
        self.last_attempt_unix_ms
    }

    /// Returns the latest successful observation timestamp.
    #[must_use]
    pub const fn last_success_unix_ms(&self) -> Option<u64> {
        self.last_success_unix_ms
    }

    /// Returns consecutive failures since the latest success.
    #[must_use]
    pub const fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Returns the earliest adapter-permitted reconnect time after failure.
    #[must_use]
    pub const fn next_attempt_unix_ms(&self) -> Option<u64> {
        self.next_attempt_unix_ms
    }

    /// Returns the retry class of the latest failure, when one exists.
    #[must_use]
    pub const fn last_failure_retryable(&self) -> Option<bool> {
        self.last_failure_retryable
    }
}

#[derive(Clone, Debug)]
struct MutableEvidence {
    public: RelayCapabilityEvidence,
}

impl MutableEvidence {
    const fn unobserved() -> Self {
        Self {
            public: RelayCapabilityEvidence {
                state: RelayEvidenceState::Unobserved,
                last_attempt_unix_ms: None,
                last_success_unix_ms: None,
                consecutive_failures: 0,
                next_attempt_unix_ms: None,
                last_failure_retryable: None,
            },
        }
    }

    const fn unsupported() -> Self {
        Self {
            public: RelayCapabilityEvidence {
                state: RelayEvidenceState::Unsupported,
                last_attempt_unix_ms: None,
                last_success_unix_ms: None,
                consecutive_failures: 0,
                next_attempt_unix_ms: None,
                last_failure_retryable: None,
            },
        }
    }

    fn begin(&mut self, observed_at_unix_ms: u64) {
        if matches!(self.public.state, RelayEvidenceState::Unsupported)
            || self
                .public
                .last_attempt_unix_ms
                .is_some_and(|current| observed_at_unix_ms < current)
        {
            return;
        }
        self.public.state = RelayEvidenceState::Connecting;
        self.public.last_attempt_unix_ms = Some(observed_at_unix_ms);
    }

    fn record(
        &mut self,
        succeeded: bool,
        retryable: bool,
        observed_at_unix_ms: u64,
        backoff: ReconnectBackoff,
    ) {
        if matches!(self.public.state, RelayEvidenceState::Unsupported)
            || self
                .public
                .last_attempt_unix_ms
                .is_some_and(|current| observed_at_unix_ms < current)
        {
            return;
        }
        self.public.last_attempt_unix_ms = Some(observed_at_unix_ms);
        if succeeded {
            self.public.state = RelayEvidenceState::Available;
            self.public.last_success_unix_ms = Some(observed_at_unix_ms);
            self.public.consecutive_failures = 0;
            self.public.next_attempt_unix_ms = None;
            self.public.last_failure_retryable = None;
        } else {
            self.public.state = RelayEvidenceState::Unavailable;
            self.public.consecutive_failures = self.public.consecutive_failures.saturating_add(1);
            self.public.last_failure_retryable = Some(retryable);
            self.public.next_attempt_unix_ms = retryable.then(|| {
                observed_at_unix_ms
                    .saturating_add(backoff.delay_ms(self.public.consecutive_failures))
            });
        }
    }

    fn may_attempt(&self, now_unix_ms: u64) -> bool {
        !matches!(self.public.state, RelayEvidenceState::Unsupported)
            && self.public.last_failure_retryable != Some(false)
            && self
                .public
                .next_attempt_unix_ms
                .is_none_or(|retry_at| now_unix_ms >= retry_at)
    }
}

#[derive(Clone, Debug)]
struct MutableRelayStatus {
    endpoint: RelayEndpoint,
    read: MutableEvidence,
    write: MutableEvidence,
}

/// Passive typed status for one configured relay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayStatus {
    endpoint: RelayEndpoint,
    read: RelayCapabilityEvidence,
    write: RelayCapabilityEvidence,
}

impl RelayStatus {
    /// Returns the canonical endpoint and its declared authority.
    #[must_use]
    pub const fn endpoint(&self) -> &RelayEndpoint {
        &self.endpoint
    }

    /// Returns independent read evidence.
    #[must_use]
    pub const fn read(&self) -> &RelayCapabilityEvidence {
        &self.read
    }

    /// Returns independent write evidence.
    #[must_use]
    pub const fn write(&self) -> &RelayCapabilityEvidence {
        &self.write
    }
}

/// Passive per-relay and aggregate status for one configured profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayStatusReport {
    profile_kind: RelayProfileKind,
    state: RelayAggregateState,
    relays: Vec<RelayStatus>,
    read_availability: Availability,
    write_availability: Availability,
}

/// Aggregate lifecycle derived from current per-relay evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RelayAggregateState {
    /// A profile is installed but no relay operation has started.
    Configured,
    /// At least one authorized relay operation is in flight.
    Connecting,
    /// Current evidence proves reads but not publication.
    ReadOnly,
    /// Current evidence proves both reads and publication.
    Writable,
    /// Some, but not all, authorized relay capabilities have current success.
    Degraded,
    /// Every attempted capability is temporarily unavailable and retry-bounded.
    Offline,
    /// Every attempted capability failed terminally for the unchanged request.
    Failed,
}

impl RelayStatusReport {
    /// Returns the host profile whose policies govern this report.
    #[must_use]
    pub const fn profile_kind(&self) -> RelayProfileKind {
        self.profile_kind
    }

    /// Returns the aggregate lifecycle derived only from current evidence.
    #[must_use]
    pub const fn state(&self) -> RelayAggregateState {
        self.state
    }

    /// Returns one status per configured relay in profile order.
    #[must_use]
    pub fn relays(&self) -> &[RelayStatus] {
        self.relays.as_slice()
    }

    /// Returns aggregate read availability derived only from observations.
    #[must_use]
    pub const fn read_availability(&self) -> Availability {
        self.read_availability
    }

    /// Returns aggregate write availability derived only from writable relays.
    #[must_use]
    pub const fn write_availability(&self) -> Availability {
        self.write_availability
    }
}

#[derive(Clone, Debug)]
struct Snapshot {
    order: Vec<RelayUrl>,
    relays: BTreeMap<RelayUrl, MutableRelayStatus>,
}

impl Snapshot {
    fn new(config: &Config) -> Self {
        Self {
            order: config.relays().to_vec(),
            relays: config
                .endpoints()
                .iter()
                .map(|endpoint| {
                    (
                        endpoint.url().clone(),
                        MutableRelayStatus {
                            endpoint: endpoint.clone(),
                            read: MutableEvidence::unobserved(),
                            write: if endpoint.access().can_write() {
                                MutableEvidence::unobserved()
                            } else {
                                MutableEvidence::unsupported()
                            },
                        },
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct StatusTracker {
    initial: Snapshot,
    snapshot: Mutex<Snapshot>,
    profile_kind: RelayProfileKind,
    backoff: ReconnectBackoff,
}

impl StatusTracker {
    pub(crate) fn new(config: &Config) -> Self {
        let initial = Snapshot::new(config);
        Self {
            snapshot: Mutex::new(initial.clone()),
            initial,
            profile_kind: config.profile_kind(),
            backoff: config.reconnect_backoff(),
        }
    }

    pub(crate) fn begin_read(&self, relay: &RelayUrl, observed_at_unix_ms: u64) {
        if let Ok(mut snapshot) = self.snapshot.lock()
            && let Some(status) = snapshot.relays.get_mut(relay)
        {
            status.read.begin(observed_at_unix_ms);
        }
    }

    pub(crate) fn begin_write(&self, relay: &RelayUrl, observed_at_unix_ms: u64) {
        if let Ok(mut snapshot) = self.snapshot.lock()
            && let Some(status) = snapshot.relays.get_mut(relay)
        {
            status.write.begin(observed_at_unix_ms);
        }
    }

    pub(crate) fn record_read(
        &self,
        relay: &RelayUrl,
        succeeded: bool,
        retryable: bool,
        observed_at_unix_ms: u64,
    ) {
        if let Ok(mut snapshot) = self.snapshot.lock()
            && let Some(status) = snapshot.relays.get_mut(relay)
        {
            status
                .read
                .record(succeeded, retryable, observed_at_unix_ms, self.backoff);
        }
    }

    pub(crate) fn record_write(
        &self,
        relay: &RelayUrl,
        succeeded: bool,
        retryable: bool,
        observed_at_unix_ms: u64,
    ) {
        if let Ok(mut snapshot) = self.snapshot.lock()
            && let Some(status) = snapshot.relays.get_mut(relay)
        {
            status
                .write
                .record(succeeded, retryable, observed_at_unix_ms, self.backoff);
        }
    }

    pub(crate) fn may_read(&self, relay: &RelayUrl, now_unix_ms: u64) -> bool {
        self.snapshot
            .lock()
            .ok()
            .and_then(|snapshot| {
                snapshot
                    .relays
                    .get(relay)
                    .map(|status| status.read.may_attempt(now_unix_ms))
            })
            .unwrap_or(false)
    }

    pub(crate) fn may_write(&self, relay: &RelayUrl, now_unix_ms: u64) -> bool {
        self.snapshot
            .lock()
            .ok()
            .and_then(|snapshot| {
                snapshot
                    .relays
                    .get(relay)
                    .map(|status| status.write.may_attempt(now_unix_ms))
            })
            .unwrap_or(false)
    }

    pub(crate) fn report(&self) -> RelayStatusReport {
        let snapshot = self
            .snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_else(|_| self.initial.clone());
        let relays = self
            .initial
            .order
            .iter()
            .filter_map(|relay| snapshot.relays.get(relay))
            .map(|status| RelayStatus {
                endpoint: status.endpoint.clone(),
                read: status.read.public.clone(),
                write: status.write.public.clone(),
            })
            .collect::<Vec<_>>();
        RelayStatusReport {
            profile_kind: self.profile_kind,
            state: aggregate_state(relays.as_slice()),
            read_availability: aggregate(relays.iter().map(RelayStatus::read)),
            write_availability: aggregate(relays.iter().map(RelayStatus::write)),
            relays,
        }
    }
}

fn aggregate_state(relays: &[RelayStatus]) -> RelayAggregateState {
    let evidence = relays
        .iter()
        .flat_map(|relay| [relay.read(), relay.write()])
        .filter(|evidence| !matches!(evidence.state(), RelayEvidenceState::Unsupported))
        .collect::<Vec<_>>();
    if evidence
        .iter()
        .any(|evidence| matches!(evidence.state(), RelayEvidenceState::Connecting))
    {
        return RelayAggregateState::Connecting;
    }
    if evidence
        .iter()
        .all(|evidence| matches!(evidence.state(), RelayEvidenceState::Unobserved))
    {
        return RelayAggregateState::Configured;
    }
    let read = relays.iter().map(RelayStatus::read).collect::<Vec<_>>();
    let write = relays
        .iter()
        .map(RelayStatus::write)
        .filter(|evidence| !matches!(evidence.state(), RelayEvidenceState::Unsupported))
        .collect::<Vec<_>>();
    let read_available = read
        .iter()
        .filter(|evidence| matches!(evidence.state(), RelayEvidenceState::Available))
        .count();
    let write_available = write
        .iter()
        .filter(|evidence| matches!(evidence.state(), RelayEvidenceState::Available))
        .count();
    if read_available == read.len() && !write.is_empty() && write_available == write.len() {
        RelayAggregateState::Writable
    } else if read_available == read.len() && write_available == 0 {
        RelayAggregateState::ReadOnly
    } else if read_available + write_available > 0 {
        RelayAggregateState::Degraded
    } else if evidence
        .iter()
        .any(|evidence| evidence.last_failure_retryable() == Some(true))
    {
        RelayAggregateState::Offline
    } else if evidence
        .iter()
        .any(|evidence| evidence.last_failure_retryable() == Some(false))
    {
        RelayAggregateState::Failed
    } else {
        RelayAggregateState::Configured
    }
}

fn aggregate<'a>(evidence: impl Iterator<Item = &'a RelayCapabilityEvidence>) -> Availability {
    let mut supported = 0usize;
    let mut available = 0usize;
    for evidence in evidence {
        if !matches!(evidence.state, RelayEvidenceState::Unsupported) {
            supported += 1;
            if matches!(evidence.state, RelayEvidenceState::Available) {
                available += 1;
            }
        }
    }
    match (supported, available) {
        (0, _) | (_, 0) => Availability::Unavailable,
        (supported, available) if supported == available => Availability::Available,
        _ => Availability::Degraded,
    }
}

pub(crate) fn source_status(tracker: &StatusTracker) -> SourceStatus {
    let report = tracker.report();
    SourceStatus::new(
        radroots_transport::TransportId::NOSTR,
        !report.relays().is_empty(),
        Maturity::Preview,
        report.read_availability(),
        SourceCapabilities::FETCH,
        match report.read_availability() {
            Availability::Available => "Nostr read capability has current successful evidence",
            Availability::Degraded => "Nostr read capability has partial successful evidence",
            Availability::Unavailable => "Nostr read capability has no current successful evidence",
        },
    )
}

pub(crate) fn sink_status(tracker: &StatusTracker) -> SinkStatus {
    let report = tracker.report();
    let configured = report
        .relays()
        .iter()
        .any(|status| status.endpoint().access().can_write());
    SinkStatus::new(
        radroots_transport::TransportId::NOSTR,
        configured,
        Maturity::Preview,
        report.write_availability(),
        SinkCapabilities::DELIVER,
        match report.write_availability() {
            Availability::Available => "Nostr write capability has current successful evidence",
            Availability::Degraded => "Nostr write capability has partial successful evidence",
            Availability::Unavailable => {
                "Nostr write capability has no current successful evidence"
            }
        },
    )
}

pub(crate) fn delivery_failure(upstream: &str) -> DeliveryOutcome {
    let class = classify(upstream).class;
    let outcome = match class {
        FailureClass::Duplicate => DeliveryOutcome::accepted(),
        FailureClass::Rejected | FailureClass::Malformed => DeliveryOutcome::rejected(),
        FailureClass::AuthRequired => DeliveryOutcome::failed(Retryability::Retryable)
            .expect("retryable authentication outcome"),
        FailureClass::RateLimited
        | FailureClass::Timeout
        | FailureClass::Connection
        | FailureClass::Unknown => DeliveryOutcome::unavailable(),
    };
    outcome
        .with_detail(class.code(), class.message())
        .expect("static normalized relay outcome")
}

pub(crate) fn fetch_failure(upstream: &str) -> (FetchTargetState, &'static str) {
    let class = classify(upstream).class;
    let state = match class {
        FailureClass::Rejected | FailureClass::Malformed => FetchTargetState::FailedTerminal,
        _ => FetchTargetState::FailedRetryable,
    };
    (state, class.message())
}

fn classify(upstream: &str) -> RedactedDiagnostic {
    let message = upstream.to_ascii_lowercase();
    let class = if message.contains("duplicate") || message.contains("already have") {
        FailureClass::Duplicate
    } else if message.contains("auth") {
        FailureClass::AuthRequired
    } else if message.contains("blocked")
        || message.contains("restricted")
        || message.contains("invalid")
        || message.contains("reject")
    {
        FailureClass::Rejected
    } else if message.contains("rate") {
        FailureClass::RateLimited
    } else if message.contains("timeout") || message.contains("timed out") {
        FailureClass::Timeout
    } else if message.contains("connect") || message.contains("offline") {
        FailureClass::Connection
    } else if message.contains("malformed") || message.contains("decode") {
        FailureClass::Malformed
    } else {
        FailureClass::Unknown
    };
    RedactedDiagnostic { class }
}

pub(crate) fn delivery_succeeded(outcome: &DeliveryOutcome) -> bool {
    matches!(
        outcome.kind(),
        DeliveryOutcomeKind::Accepted | DeliveryOutcomeKind::Delivered
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReconnectBackoff, RelayProfile};

    fn tracker() -> (StatusTracker, RelayUrl, RelayUrl) {
        let config =
            Config::from_profile(RelayProfile::public(["wss://write.example"]).expect("profile"))
                .with_reconnect_backoff(ReconnectBackoff::new(10, 40).expect("backoff"));
        let read_only = config.relays()[0].clone();
        let writable = config.relays()[1].clone();
        (StatusTracker::new(&config), read_only, writable)
    }

    #[test]
    fn every_upstream_class_maps_to_stable_secret_safe_output() {
        let secret = "token=very-secret-value";
        let cases = [
            ("duplicate: already have", "duplicate"),
            ("blocked by policy", "rejected"),
            ("auth required", "auth_required"),
            ("rate limited", "rate_limited"),
            ("connection timeout", "timeout"),
            ("connection offline", "connection_failed"),
            ("unknown failure", "relay_failure"),
        ];
        for (message, code) in cases {
            let outcome = delivery_failure(format!("{message} {secret}").as_str());
            assert_eq!(outcome.code(), Some(code));
            assert!(!outcome.message().expect("message").contains(secret));
            let diagnostic = classify(format!("{message} {secret}").as_str());
            assert!(!format!("{diagnostic:?}").contains(secret));
        }
    }

    #[test]
    fn status_requires_directional_evidence_and_backoff_is_monotonic() {
        let (tracker, read_only, writable) = tracker();
        let initial = tracker.report();
        assert_eq!(initial.read_availability(), Availability::Unavailable);
        assert_eq!(initial.write_availability(), Availability::Unavailable);
        assert_eq!(initial.state(), RelayAggregateState::Configured);
        assert_eq!(
            initial.relays()[0].write().state(),
            RelayEvidenceState::Unsupported
        );
        assert!(!tracker.may_write(&read_only, 100));

        tracker.begin_read(&read_only, 100);
        assert_eq!(tracker.report().state(), RelayAggregateState::Connecting);
        tracker.record_read(&read_only, true, false, 100);
        tracker.record_read(&writable, false, true, 100);
        tracker.record_write(&writable, false, true, 100);
        let partial = tracker.report();
        assert_eq!(partial.state(), RelayAggregateState::Degraded);
        assert_eq!(partial.read_availability(), Availability::Degraded);
        assert_eq!(partial.write_availability(), Availability::Unavailable);
        assert!(!tracker.may_write(&writable, 109));
        assert!(tracker.may_write(&writable, 110));

        tracker.record_write(&writable, false, true, 110);
        assert_eq!(
            tracker.report().relays()[1].write().next_attempt_unix_ms(),
            Some(130)
        );
        tracker.record_write(&writable, true, false, 109);
        assert_eq!(
            tracker.report().relays()[1].write().state(),
            RelayEvidenceState::Unavailable
        );
        tracker.record_write(&writable, true, false, 130);
        let available = tracker.report();
        assert_eq!(available.write_availability(), Availability::Available);
        assert_eq!(available.relays()[1].write().consecutive_failures(), 0);
        assert_eq!(
            available.relays()[1].write().last_success_unix_ms(),
            Some(130)
        );
        assert!(tracker.may_write(&writable, 130));
        assert_eq!(
            source_status(&tracker).availability(),
            Availability::Degraded
        );
        assert_eq!(
            sink_status(&tracker).availability(),
            Availability::Available
        );

        tracker.record_write(&writable, false, false, 140);
        assert!(!tracker.may_write(&writable, u64::MAX));
        assert_eq!(
            tracker.report().relays()[1]
                .write()
                .last_failure_retryable(),
            Some(false)
        );
        assert_eq!(
            sink_status(&tracker).availability(),
            Availability::Unavailable
        );
    }

    #[test]
    fn normalized_failures_and_success_helpers_cover_every_state() {
        for message in [
            "invalid event",
            "restricted",
            "rejected",
            "malformed event",
            "decode failed",
        ] {
            assert_eq!(fetch_failure(message).0, FetchTargetState::FailedTerminal);
        }
        assert_eq!(
            fetch_failure("offline").0,
            FetchTargetState::FailedRetryable
        );
        assert_eq!(
            delivery_failure("malformed event").kind(),
            DeliveryOutcomeKind::Rejected
        );
        assert!(delivery_succeeded(&DeliveryOutcome::accepted()));
        assert!(delivery_succeeded(&DeliveryOutcome::delivered()));
        assert!(!delivery_succeeded(&DeliveryOutcome::rejected()));
    }

    #[test]
    fn all_success_is_available_and_no_writable_relay_is_honestly_unavailable() {
        let config =
            Config::from_profile(RelayProfile::public(Vec::<String>::new()).expect("profile"));
        let tracker = StatusTracker::new(&config);
        let relay = config.relays()[0].clone();
        tracker.record_read(&relay, true, false, 1);
        assert_eq!(
            source_status(&tracker).availability(),
            Availability::Available
        );
        let sink = sink_status(&tracker);
        assert!(!sink.is_configured());
        assert_eq!(sink.availability(), Availability::Unavailable);
        assert_eq!(tracker.report().state(), RelayAggregateState::ReadOnly);
    }

    #[test]
    fn aggregate_states_and_unconfigured_targets_cover_fail_closed_edges() {
        let (live, read_only, writable) = tracker();
        let unknown =
            RelayUrl::parse("wss://unknown.example", crate::RelayUrlPolicy::Public).expect("relay");

        live.begin_read(&unknown, 1);
        live.begin_write(&unknown, 1);
        live.record_read(&unknown, true, false, 1);
        live.record_write(&unknown, true, false, 1);
        live.begin_write(&read_only, 1);
        live.record_write(&read_only, true, false, 1);
        assert_eq!(
            live.report().relays()[0].write().state(),
            RelayEvidenceState::Unsupported
        );

        live.begin_read(&read_only, 2);
        live.begin_read(&read_only, 1);
        live.record_read(&read_only, true, false, 2);
        live.record_read(&writable, true, false, 2);
        live.record_write(&writable, true, false, 2);
        let writable_report = live.report();
        assert_eq!(writable_report.state(), RelayAggregateState::Writable);
        assert_eq!(writable_report.read_availability(), Availability::Available);
        assert_eq!(
            writable_report.write_availability(),
            Availability::Available
        );

        let (offline, read_only, writable) = tracker();
        offline.record_read(&read_only, false, true, 10);
        offline.record_read(&writable, false, true, 10);
        offline.record_write(&writable, false, true, 10);
        assert_eq!(offline.report().state(), RelayAggregateState::Offline);

        let (failed, read_only, writable) = tracker();
        failed.record_read(&read_only, false, false, 10);
        failed.record_read(&writable, false, false, 10);
        failed.record_write(&writable, false, false, 10);
        assert_eq!(failed.report().state(), RelayAggregateState::Failed);

        assert_eq!(delivery_failure("already have").code(), Some("duplicate"));
        assert_eq!(
            fetch_failure("timed out").0,
            FetchTargetState::FailedRetryable
        );
    }
}
