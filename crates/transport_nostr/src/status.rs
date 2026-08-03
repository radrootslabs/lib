//! Stable Nostr relay status and outcome normalization.

use radroots_transport::{
    SinkStatus, SourceStatus,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
    outcome::{DeliveryOutcome, DeliveryOutcomeKind, FetchTargetState, Retryability},
};
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

#[derive(Clone, Debug)]
struct Snapshot {
    source: Availability,
    sink: Availability,
    source_diagnostic: Option<RedactedDiagnostic>,
    sink_diagnostic: Option<RedactedDiagnostic>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            source: Availability::Available,
            sink: Availability::Available,
            source_diagnostic: None,
            sink_diagnostic: None,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct StatusTracker {
    snapshot: Mutex<Snapshot>,
}

impl StatusTracker {
    pub(crate) fn record_sink(&self, accepted: usize, failed: usize, diagnostic: Option<&str>) {
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.sink = availability(accepted, failed);
            snapshot.sink_diagnostic = diagnostic.map(classify);
        }
    }

    pub(crate) fn record_source(&self, succeeded: usize, failed: usize, diagnostic: Option<&str>) {
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.source = availability(succeeded, failed);
            snapshot.source_diagnostic = diagnostic.map(classify);
        }
    }

    fn source_availability(&self) -> Availability {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.source)
            .unwrap_or(Availability::Unavailable)
    }

    fn sink_availability(&self) -> Availability {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.sink)
            .unwrap_or(Availability::Unavailable)
    }
}

pub(crate) fn source_status(tracker: &StatusTracker, configured: bool) -> SourceStatus {
    SourceStatus::new(
        radroots_transport::TransportId::NOSTR,
        configured,
        Maturity::Preview,
        if configured {
            tracker.source_availability()
        } else {
            Availability::Unavailable
        },
        SourceCapabilities::FETCH,
        if configured {
            "bounded Nostr event source configured"
        } else {
            "Nostr event source is not configured"
        },
    )
}

pub(crate) fn sink_status(tracker: &StatusTracker, configured: bool) -> SinkStatus {
    SinkStatus::new(
        radroots_transport::TransportId::NOSTR,
        configured,
        Maturity::Preview,
        if configured {
            tracker.sink_availability()
        } else {
            Availability::Unavailable
        },
        SinkCapabilities::DELIVER,
        if configured {
            "bounded Nostr event delivery configured"
        } else {
            "Nostr event sink is not configured"
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

fn availability(succeeded: usize, failed: usize) -> Availability {
    match (succeeded, failed) {
        (0, 0) | (_, 0) => Availability::Available,
        (0, _) => Availability::Unavailable,
        (_, _) => Availability::Degraded,
    }
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
    fn status_tracks_available_degraded_and_unavailable_without_io() {
        let tracker = StatusTracker::default();
        assert_eq!(
            sink_status(&tracker, true).availability(),
            Availability::Available
        );
        tracker.record_sink(1, 1, Some("token=secret timeout"));
        assert_eq!(
            sink_status(&tracker, true).availability(),
            Availability::Degraded
        );
        tracker.record_source(0, 2, Some("token=secret offline"));
        assert_eq!(
            source_status(&tracker, true).availability(),
            Availability::Unavailable
        );
        assert!(!format!("{tracker:?}").contains("token=secret"));
    }
}
