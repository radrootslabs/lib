//! Policy-authorized event admission and visibility typestates.

pub use crate::verification::{
    ContractValidatedEvent, Error, IdVerifiedEvent, RawEvent, SignatureVerifiedEvent,
};

use crate::envelope::RadrootsEventEnvelope;

/// Host policy that authorizes a contract-validated event for admission.
pub trait AdmissionPolicy {
    type Error;

    /// Stable identifier for the policy whose decision produced the state.
    fn policy_id(&self) -> &'static str;

    /// Returns success only when this policy admits the supplied event.
    fn admit(&self, event: &ContractValidatedEvent) -> Result<(), Self::Error>;
}

/// Host policy that authorizes an admitted event to become visible.
pub trait VisibilityPolicy {
    type Error;

    /// Stable identifier for the policy whose decision produced the state.
    fn policy_id(&self) -> &'static str;

    /// Returns success only when this policy permits the event to be visible.
    fn make_visible(&self, event: &AdmittedEvent) -> Result<(), Self::Error>;
}

/// A contract-validated event accepted by an explicit admission policy.
///
/// ```compile_fail
/// use radroots_event::admission::{AdmittedEvent, ContractValidatedEvent};
///
/// fn bypass_policy(event: ContractValidatedEvent) -> AdmittedEvent {
///     AdmittedEvent::new(event, "allow-all")
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmittedEvent {
    event: ContractValidatedEvent,
    policy_id: &'static str,
}

impl ContractValidatedEvent {
    /// Runs an admission policy and advances only when it succeeds.
    pub fn admit_with<P>(self, policy: &P) -> Result<AdmittedEvent, P::Error>
    where
        P: AdmissionPolicy + ?Sized,
    {
        policy.admit(&self)?;
        Ok(AdmittedEvent {
            event: self,
            policy_id: policy.policy_id(),
        })
    }
}

impl AdmittedEvent {
    #[must_use]
    pub const fn validated_event(&self) -> &ContractValidatedEvent {
        &self.event
    }

    #[must_use]
    pub const fn event(&self) -> &RadrootsEventEnvelope {
        self.event.event()
    }

    #[must_use]
    pub const fn policy_id(&self) -> &'static str {
        self.policy_id
    }

    #[must_use]
    pub fn into_validated_event(self) -> ContractValidatedEvent {
        self.event
    }

    /// Runs a visibility policy and advances only when it succeeds.
    pub fn make_visible_with<P>(self, policy: &P) -> Result<VisibleEvent, P::Error>
    where
        P: VisibilityPolicy + ?Sized,
    {
        policy.make_visible(&self)?;
        Ok(VisibleEvent {
            event: self,
            policy_id: policy.policy_id(),
        })
    }
}

/// An admitted event accepted by an explicit visibility policy.
///
/// ```compile_fail
/// use radroots_event::admission::{AdmittedEvent, VisibleEvent};
///
/// fn bypass_visibility(event: AdmittedEvent) -> VisibleEvent {
///     VisibleEvent::from(event)
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleEvent {
    event: AdmittedEvent,
    policy_id: &'static str,
}

impl VisibleEvent {
    #[must_use]
    pub const fn admitted_event(&self) -> &AdmittedEvent {
        &self.event
    }

    #[must_use]
    pub const fn event(&self) -> &RadrootsEventEnvelope {
        self.event.event()
    }

    #[must_use]
    pub const fn policy_id(&self) -> &'static str {
        self.policy_id
    }

    #[must_use]
    pub fn into_admitted_event(self) -> AdmittedEvent {
        self.event
    }
}

#[cfg(all(test, feature = "signature"))]
mod tests {
    use super::*;
    use crate::envelope::{RadrootsEventEnvelope, RadrootsEventEnvelopeParts};

    struct Allow;

    impl AdmissionPolicy for Allow {
        type Error = core::convert::Infallible;

        fn policy_id(&self) -> &'static str {
            "test.admission.allow.v1"
        }

        fn admit(&self, _event: &ContractValidatedEvent) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl VisibilityPolicy for Allow {
        type Error = core::convert::Infallible;

        fn policy_id(&self) -> &'static str {
            "test.visibility.allow.v1"
        }

        fn make_visible(&self, _event: &AdmittedEvent) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn positive_vector_traverses_the_complete_transition_graph() {
        let envelope = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: "762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0"
                .to_owned(),
            author: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df"
                .to_owned(),
            created_at: 1_800_000_100,
            kind: 0,
            tags: vec![],
            content: "{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}".to_owned(),
            sig: "4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109".to_owned(),
        })
        .expect("valid profile event");

        let visible = RawEvent::new(envelope)
            .verify_id()
            .expect("verified id")
            .verify_signature()
            .expect("verified signature")
            .validate_contract()
            .expect("validated contract")
            .admit_with(&Allow)
            .expect("admitted")
            .make_visible_with(&Allow)
            .expect("visible");

        assert_eq!(
            visible.admitted_event().policy_id(),
            "test.admission.allow.v1"
        );
        assert_eq!(visible.policy_id(), "test.visibility.allow.v1");
        assert_eq!(
            visible.event().id().to_hex(),
            "762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0"
        );
    }
}
