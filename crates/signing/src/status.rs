//! Signer progress and status models.

use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use crate::{Error, capability::SignerCapability, error::Kind};

const MAX_AUTH_URI_BYTES: usize = 2_048;

/// A remote authentication interaction required to continue signing.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, PartialEq, Eq)]
pub struct AuthChallenge {
    uri: String,
    required_at_unix: u64,
    expires_at_unix: Option<u64>,
}

impl AuthChallenge {
    /// Creates a bounded HTTPS authentication challenge.
    pub fn new(
        uri: impl Into<String>,
        required_at_unix: u64,
        expires_at_unix: Option<u64>,
    ) -> Result<Self, Error> {
        let uri = uri.into();
        if uri.len() > MAX_AUTH_URI_BYTES
            || uri.trim() != uri
            || !uri.starts_with("https://")
            || uri.chars().any(char::is_control)
        {
            return Err(Error::new(Kind::InvalidArgument));
        }
        if let Some(expires_at_unix) = expires_at_unix
            && expires_at_unix < required_at_unix
        {
            return Err(Error::new(Kind::InvalidArgument));
        }
        Ok(Self {
            uri,
            required_at_unix,
            expires_at_unix,
        })
    }

    /// Borrows the host-displayable authentication URI.
    #[must_use]
    pub fn uri(&self) -> &str {
        self.uri.as_str()
    }

    /// Returns when the challenge became required.
    #[must_use]
    pub const fn required_at_unix(&self) -> u64 {
        self.required_at_unix
    }

    /// Returns the optional absolute challenge expiry.
    #[must_use]
    pub const fn expires_at_unix(&self) -> Option<u64> {
        self.expires_at_unix
    }
}

impl fmt::Debug for AuthChallenge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthChallenge")
            .field("uri", &"[redacted]")
            .field("required_at_unix", &self.required_at_unix)
            .field("expires_at_unix", &self.expires_at_unix)
            .finish()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for AuthChallenge {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            uri: String,
            required_at_unix: u64,
            expires_at_unix: Option<u64>,
        }

        let value = Repr::deserialize(deserializer)?;
        Self::new(value.uri, value.required_at_unix, value.expires_at_unix)
            .map_err(serde::de::Error::custom)
    }
}

/// Stable signing progress stages.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignProgressStage {
    Queued,
    Validating,
    AwaitingAuthentication,
    RequestPublished,
    AwaitingSignature,
    VerifyingOutput,
    Complete,
}

/// One immutable signer progress update.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignProgress {
    stage: SignProgressStage,
    challenge: Option<AuthChallenge>,
}

impl SignProgress {
    /// Creates a progress update without an authentication challenge.
    pub const fn stage(stage: SignProgressStage) -> Result<Self, Error> {
        if matches!(stage, SignProgressStage::AwaitingAuthentication) {
            return Err(Error::new(Kind::InvalidArgument));
        }
        Ok(Self {
            stage,
            challenge: None,
        })
    }

    /// Creates an explicit authentication-challenge update.
    #[must_use]
    pub const fn authentication(challenge: AuthChallenge) -> Self {
        Self {
            stage: SignProgressStage::AwaitingAuthentication,
            challenge: Some(challenge),
        }
    }

    /// Returns the stable progress stage.
    #[must_use]
    pub const fn stage_value(&self) -> SignProgressStage {
        self.stage
    }

    /// Borrows the authentication challenge, when present.
    #[must_use]
    pub const fn challenge(&self) -> Option<&AuthChallenge> {
        self.challenge.as_ref()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SignProgress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            stage: SignProgressStage,
            challenge: Option<AuthChallenge>,
        }

        let value = Repr::deserialize(deserializer)?;
        match (value.stage, value.challenge) {
            (SignProgressStage::AwaitingAuthentication, Some(challenge)) => {
                Ok(Self::authentication(challenge))
            }
            (SignProgressStage::AwaitingAuthentication, None) => {
                Err(serde::de::Error::custom(Error::new(Kind::InvalidArgument)))
            }
            (_, Some(_)) => Err(serde::de::Error::custom(Error::new(Kind::InvalidArgument))),
            (stage, None) => Self::stage(stage).map_err(serde::de::Error::custom),
        }
    }
}

/// Current signer availability.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerAvailability {
    Ready,
    Busy,
    AwaitingAuthentication,
    Unavailable,
}

/// Current signer availability, capabilities, and optional progress.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignerStatus {
    availability: SignerAvailability,
    capabilities: Vec<SignerCapability>,
    progress: Option<SignProgress>,
}

impl SignerStatus {
    /// Creates an explicit status snapshot.
    #[must_use]
    pub fn new(
        availability: SignerAvailability,
        capabilities: Vec<SignerCapability>,
        progress: Option<SignProgress>,
    ) -> Self {
        Self {
            availability,
            capabilities,
            progress,
        }
    }

    /// Creates an unavailable status without claiming capabilities.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            availability: SignerAvailability::Unavailable,
            capabilities: Vec::new(),
            progress: None,
        }
    }

    #[must_use]
    pub const fn availability(&self) -> SignerAvailability {
        self.availability
    }

    #[must_use]
    pub fn capabilities(&self) -> &[SignerCapability] {
        &self.capabilities
    }

    #[must_use]
    pub const fn progress(&self) -> Option<&SignProgress> {
        self.progress.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "serde")]
    use crate::capability::{CancellationSupport, SignerKind};

    #[cfg(not(feature = "std"))]
    use alloc::format;
    #[cfg(all(not(feature = "std"), feature = "serde"))]
    use alloc::vec;

    #[test]
    fn challenge_validation_and_debug_redaction_are_explicit() {
        let challenge =
            AuthChallenge::new("https://auth.example/approve?token=sensitive", 10, Some(20))
                .expect("challenge");

        assert_eq!(challenge.required_at_unix(), 10);
        assert_eq!(
            challenge.uri(),
            "https://auth.example/approve?token=sensitive"
        );
        assert_eq!(challenge.expires_at_unix(), Some(20));
        assert!(!format!("{challenge:?}").contains("sensitive"));
        assert_eq!(
            AuthChallenge::new("http://auth.example", 10, None)
                .expect_err("HTTP challenge must fail")
                .kind(),
            Kind::InvalidArgument
        );
        assert_eq!(
            AuthChallenge::new("https://auth.example", 20, Some(10))
                .expect_err("invalid expiry must fail")
                .kind(),
            Kind::InvalidArgument
        );
        for invalid in [
            " https://auth.example",
            "https://auth.example ",
            "https://auth.example/line\nbreak",
        ] {
            assert_eq!(
                AuthChallenge::new(invalid, 10, None).unwrap_err().kind(),
                Kind::InvalidArgument
            );
        }
        assert_eq!(
            AuthChallenge::new(
                format!("https://auth.example/{}", "x".repeat(MAX_AUTH_URI_BYTES)),
                10,
                None
            )
            .unwrap_err()
            .kind(),
            Kind::InvalidArgument
        );
    }

    #[test]
    fn progress_requires_challenges_only_at_the_authentication_stage() {
        assert_eq!(
            SignProgress::stage(SignProgressStage::AwaitingAuthentication)
                .expect_err("missing challenge must fail")
                .kind(),
            Kind::InvalidArgument
        );
        let challenge =
            AuthChallenge::new("https://auth.example/approve", 10, None).expect("challenge");
        let progress = SignProgress::authentication(challenge);
        assert_eq!(
            progress.stage_value(),
            SignProgressStage::AwaitingAuthentication
        );
        assert!(progress.challenge().is_some());
        let queued = SignProgress::stage(SignProgressStage::Queued).unwrap();
        assert_eq!(queued.stage_value(), SignProgressStage::Queued);
        assert_eq!(queued.challenge(), None);
        let unavailable = SignerStatus::unavailable();
        assert_eq!(unavailable.availability(), SignerAvailability::Unavailable);
        assert!(unavailable.capabilities().is_empty());
        assert_eq!(unavailable.progress(), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn status_round_trips_and_invalid_progress_fails_closed() {
        let capability = SignerCapability::new(
            SignerKind::Remote,
            CancellationSupport::BeforePublication,
            true,
            true,
        );
        let challenge =
            AuthChallenge::new("https://auth.example/approve", 10, Some(20)).expect("challenge");
        let status = SignerStatus::new(
            SignerAvailability::AwaitingAuthentication,
            vec![capability],
            Some(SignProgress::authentication(challenge)),
        );
        let encoded = serde_json::to_string(&status).expect("serialize status");
        let decoded: SignerStatus = serde_json::from_str(&encoded).expect("deserialize status");

        assert_eq!(decoded, status);
        assert!(
            serde_json::from_str::<SignProgress>(
                r#"{"stage":"awaiting_authentication","challenge":null}"#
            )
            .is_err()
        );
        assert!(serde_json::from_str::<SignProgress>(
            r#"{"stage":"queued","challenge":{"uri":"https://auth.example","required_at_unix":1,"expires_at_unix":null}}"#
        )
        .is_err());
        assert!(
            serde_json::from_str::<AuthChallenge>(
                r#"{"uri":"http://auth.example","required_at_unix":1,"expires_at_unix":null}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<SignProgress>(r#"{"stage":"queued","challenge":null}"#).is_ok()
        );
    }
}
