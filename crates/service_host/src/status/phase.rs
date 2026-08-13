//! Service-neutral lifecycle phases and readiness state.

use core::fmt;

use serde::{Deserialize, Serialize};

use super::ReasonCodes;

/// Stable lifecycle phase shared by hardened services.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServicePhase {
    Starting,
    Ready,
    Degraded,
    Unready,
    Stopping,
    Failed,
}

impl ServicePhase {
    /// Returns whether moving from this phase to `next` is legal.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        if self as u8 == next as u8 {
            return true;
        }
        match self {
            Self::Starting => matches!(next, Self::Ready | Self::Degraded | Self::Failed),
            Self::Ready | Self::Degraded | Self::Unready => matches!(
                next,
                Self::Ready | Self::Degraded | Self::Unready | Self::Stopping | Self::Failed
            ),
            Self::Stopping => matches!(next, Self::Failed),
            Self::Failed => false,
        }
    }
}

/// Readiness is serialized as the exact boolean required by status contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Readiness(bool);

impl Readiness {
    pub const READY: Self = Self(true);
    pub const NOT_READY: Self = Self(false);

    #[must_use]
    pub const fn is_ready(self) -> bool {
        self.0
    }
}

/// One validated service-neutral operational observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceOperationalState {
    phase: ServicePhase,
    readiness: Readiness,
    reasons: ReasonCodes,
}

impl ServiceOperationalState {
    /// Constructs a phase/readiness pair, rejecting contradictory combinations.
    pub fn new(
        phase: ServicePhase,
        readiness: Readiness,
        reasons: ReasonCodes,
    ) -> Result<Self, StatusContractError> {
        validate_readiness(phase, readiness)?;
        Ok(Self {
            phase,
            readiness,
            reasons,
        })
    }

    #[must_use]
    pub const fn phase(&self) -> ServicePhase {
        self.phase
    }

    #[must_use]
    pub const fn readiness(&self) -> Readiness {
        self.readiness
    }

    #[must_use]
    pub const fn reasons(&self) -> &ReasonCodes {
        &self.reasons
    }

    /// Applies a legal transition while preserving the last valid state on failure.
    pub fn transition_to(
        &mut self,
        phase: ServicePhase,
        readiness: Readiness,
        reasons: ReasonCodes,
    ) -> Result<(), StatusContractError> {
        if !self.phase.can_transition_to(phase) {
            return Err(StatusContractError::IllegalTransition {
                from: self.phase,
                to: phase,
            });
        }
        validate_readiness(phase, readiness)?;
        self.phase = phase;
        self.readiness = readiness;
        self.reasons = reasons;
        Ok(())
    }
}

fn validate_readiness(
    phase: ServicePhase,
    readiness: Readiness,
) -> Result<(), StatusContractError> {
    let legal = match phase {
        ServicePhase::Ready => readiness.is_ready(),
        ServicePhase::Degraded => true,
        ServicePhase::Starting
        | ServicePhase::Unready
        | ServicePhase::Stopping
        | ServicePhase::Failed => !readiness.is_ready(),
    };
    if legal {
        Ok(())
    } else {
        Err(StatusContractError::InvalidReadiness { phase, readiness })
    }
}

/// Validation failure for common lifecycle and reason contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusContractError {
    InvalidReasonCode,
    TooManyReasonCodes {
        maximum: usize,
    },
    InvalidReadiness {
        phase: ServicePhase,
        readiness: Readiness,
    },
    IllegalTransition {
        from: ServicePhase,
        to: ServicePhase,
    },
}

impl fmt::Display for StatusContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidReasonCode => formatter.write_str("status reason code is invalid"),
            Self::TooManyReasonCodes { maximum } => {
                write!(formatter, "status exceeds its {maximum}-reason limit")
            }
            Self::InvalidReadiness { phase, readiness } => write!(
                formatter,
                "readiness {} is invalid for phase {phase:?}",
                readiness.is_ready()
            ),
            Self::IllegalTransition { from, to } => {
                write!(
                    formatter,
                    "service phase transition {from:?} -> {to:?} is illegal"
                )
            }
        }
    }
}

impl std::error::Error for StatusContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_and_illegal_phase_transitions_are_explicit() {
        let phases = [
            ServicePhase::Starting,
            ServicePhase::Ready,
            ServicePhase::Degraded,
            ServicePhase::Unready,
            ServicePhase::Stopping,
            ServicePhase::Failed,
        ];
        let expected = [
            [true, true, true, false, false, true],
            [false, true, true, true, true, true],
            [false, true, true, true, true, true],
            [false, true, true, true, true, true],
            [false, false, false, false, true, true],
            [false, false, false, false, false, true],
        ];
        for (from_index, from) in phases.into_iter().enumerate() {
            for (to_index, to) in phases.into_iter().enumerate() {
                assert_eq!(
                    from.can_transition_to(to),
                    expected[from_index][to_index],
                    "unexpected {from:?} -> {to:?} decision"
                );
            }
        }

        let mut state = ServiceOperationalState::new(
            ServicePhase::Starting,
            Readiness::NOT_READY,
            ReasonCodes::empty(),
        )
        .expect("starting state");
        state
            .transition_to(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty())
            .expect("ready transition");
        state
            .transition_to(
                ServicePhase::Stopping,
                Readiness::NOT_READY,
                ReasonCodes::empty(),
            )
            .expect("stopping transition");

        let before = state.clone();
        assert_eq!(
            state.transition_to(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty(),),
            Err(StatusContractError::IllegalTransition {
                from: ServicePhase::Stopping,
                to: ServicePhase::Ready,
            })
        );
        assert_eq!(state, before);
    }

    #[test]
    fn degraded_readiness_is_independent_but_other_phases_are_consistent() {
        for readiness in [Readiness::READY, Readiness::NOT_READY] {
            assert!(
                ServiceOperationalState::new(
                    ServicePhase::Degraded,
                    readiness,
                    ReasonCodes::empty()
                )
                .is_ok()
            );
        }
        assert!(
            ServiceOperationalState::new(
                ServicePhase::Ready,
                Readiness::NOT_READY,
                ReasonCodes::empty()
            )
            .is_err()
        );
        assert!(
            ServiceOperationalState::new(
                ServicePhase::Failed,
                Readiness::READY,
                ReasonCodes::empty()
            )
            .is_err()
        );
    }

    #[test]
    fn phase_and_readiness_serde_names_match_frozen_contracts() {
        let phases = [
            (ServicePhase::Starting, "\"starting\""),
            (ServicePhase::Ready, "\"ready\""),
            (ServicePhase::Degraded, "\"degraded\""),
            (ServicePhase::Unready, "\"unready\""),
            (ServicePhase::Stopping, "\"stopping\""),
            (ServicePhase::Failed, "\"failed\""),
        ];
        for (phase, json) in phases {
            assert_eq!(serde_json::to_string(&phase).expect("phase"), json);
            assert_eq!(
                serde_json::from_str::<ServicePhase>(json).expect("phase"),
                phase
            );
        }
        assert_eq!(serde_json::to_string(&Readiness::READY).unwrap(), "true");
        assert_eq!(
            serde_json::to_string(&Readiness::NOT_READY).unwrap(),
            "false"
        );
    }
}
