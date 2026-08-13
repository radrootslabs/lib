//! Process-signal sequencing over a binary-owned, injected signal source.

use core::{fmt, future::Future, pin::Pin};
use std::error::Error;

/// One normalized process signal supplied by a consuming binary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessSignal {
    /// Interactive interrupt, conventionally produced by Ctrl-C.
    Interrupt,
    /// Unix termination request, conventionally produced by SIGTERM.
    #[cfg(unix)]
    Terminate,
}

impl ProcessSignal {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Interrupt => "interrupt",
            #[cfg(unix)]
            Self::Terminate => "terminate",
        }
    }
}

impl fmt::Display for ProcessSignal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Binary-owned asynchronous source of normalized process signals.
///
/// Implementations install any operating-system handlers at the binary
/// boundary. The service-host library only consumes the resulting events.
pub type ProcessSignalFuture<'a> = Pin<Box<dyn Future<Output = Option<ProcessSignal>> + Send + 'a>>;

pub trait ProcessSignalSource: Send {
    fn next_signal(&mut self) -> ProcessSignalFuture<'_>;
}

/// Action assigned to an observed signal by the shared sequencing contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessSignalAction {
    BeginGracefulShutdown { signal: ProcessSignal },
    ForceTermination { signal: ProcessSignal },
}

impl ProcessSignalAction {
    #[must_use]
    pub const fn signal(self) -> ProcessSignal {
        match self {
            Self::BeginGracefulShutdown { signal } | Self::ForceTermination { signal } => signal,
        }
    }

    #[must_use]
    pub const fn forces_termination(self) -> bool {
        matches!(self, Self::ForceTermination { .. })
    }
}

/// Which event the adapter was awaiting when its source closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessSignalStage {
    FirstCancellation,
    ForceTermination,
}

/// Typed closure of an injected signal source before the expected event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessSignalSourceClosed {
    stage: ProcessSignalStage,
}

impl ProcessSignalSourceClosed {
    #[must_use]
    pub const fn stage(self) -> ProcessSignalStage {
        self.stage
    }
}

impl fmt::Display for ProcessSignalSourceClosed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("process signal source closed before the expected event")
    }
}

impl Error for ProcessSignalSourceClosed {}

/// Maps the first injected signal to graceful cancellation and later signals to force.
pub struct ProcessSignalAdapter<S> {
    source: S,
    first_observed: bool,
}

impl<S> ProcessSignalAdapter<S>
where
    S: ProcessSignalSource,
{
    #[must_use]
    pub const fn new(source: S) -> Self {
        Self {
            source,
            first_observed: false,
        }
    }

    #[must_use]
    pub const fn stage(&self) -> ProcessSignalStage {
        if self.first_observed {
            ProcessSignalStage::ForceTermination
        } else {
            ProcessSignalStage::FirstCancellation
        }
    }

    /// Waits for and classifies the next binary-supplied process signal.
    ///
    /// Cancelling this future does not advance adapter state before the source
    /// returns an event. Source-specific event cancellation semantics remain
    /// the responsibility of the binary-owned source implementation.
    pub async fn next_action(&mut self) -> Result<ProcessSignalAction, ProcessSignalSourceClosed> {
        let stage = self.stage();
        let signal = self
            .source
            .next_signal()
            .await
            .ok_or(ProcessSignalSourceClosed { stage })?;
        if self.first_observed {
            Ok(ProcessSignalAction::ForceTermination { signal })
        } else {
            self.first_observed = true;
            Ok(ProcessSignalAction::BeginGracefulShutdown { signal })
        }
    }

    #[must_use]
    pub fn into_inner(self) -> S {
        self.source
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    struct InjectedSignals {
        events: VecDeque<ProcessSignal>,
    }

    impl InjectedSignals {
        fn new(events: impl IntoIterator<Item = ProcessSignal>) -> Self {
            Self {
                events: events.into_iter().collect(),
            }
        }
    }

    impl ProcessSignalSource for InjectedSignals {
        fn next_signal(&mut self) -> ProcessSignalFuture<'_> {
            Box::pin(async move { self.events.pop_front() })
        }
    }

    #[tokio::test]
    async fn first_event_cancels_and_every_later_event_forces() {
        #[cfg(unix)]
        let events = [
            ProcessSignal::Interrupt,
            ProcessSignal::Terminate,
            ProcessSignal::Interrupt,
        ];
        #[cfg(not(unix))]
        let events = [
            ProcessSignal::Interrupt,
            ProcessSignal::Interrupt,
            ProcessSignal::Interrupt,
        ];
        let mut adapter = ProcessSignalAdapter::new(InjectedSignals::new(events));

        assert_eq!(adapter.stage(), ProcessSignalStage::FirstCancellation);
        assert_eq!(
            adapter.next_action().await.unwrap(),
            ProcessSignalAction::BeginGracefulShutdown {
                signal: ProcessSignal::Interrupt,
            }
        );
        assert_eq!(adapter.stage(), ProcessSignalStage::ForceTermination);

        let second = adapter.next_action().await.unwrap();
        assert!(second.forces_termination());
        #[cfg(unix)]
        assert_eq!(second.signal(), ProcessSignal::Terminate);
        #[cfg(not(unix))]
        assert_eq!(second.signal(), ProcessSignal::Interrupt);

        assert!(adapter.next_action().await.unwrap().forces_termination());
    }

    #[tokio::test]
    async fn source_closure_is_typed_and_does_not_advance_the_stage() {
        let mut before_first = ProcessSignalAdapter::new(InjectedSignals::new([]));
        let error = before_first.next_action().await.unwrap_err();
        assert_eq!(error.stage(), ProcessSignalStage::FirstCancellation);
        assert_eq!(before_first.stage(), ProcessSignalStage::FirstCancellation);

        let mut before_force =
            ProcessSignalAdapter::new(InjectedSignals::new([ProcessSignal::Interrupt]));
        assert!(
            !before_force
                .next_action()
                .await
                .unwrap()
                .forces_termination()
        );
        let error = before_force.next_action().await.unwrap_err();
        assert_eq!(error.stage(), ProcessSignalStage::ForceTermination);
        assert_eq!(before_force.stage(), ProcessSignalStage::ForceTermination);
    }

    #[test]
    fn normalized_names_are_stable_and_platform_bounded() {
        assert_eq!(ProcessSignal::Interrupt.as_str(), "interrupt");
        #[cfg(unix)]
        assert_eq!(ProcessSignal::Terminate.as_str(), "terminate");

        let adapter = ProcessSignalAdapter::new(InjectedSignals::new([ProcessSignal::Interrupt]));
        assert_eq!(adapter.into_inner().events.len(), 1);
    }
}
