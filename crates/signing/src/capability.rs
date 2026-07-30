//! Signer capability declarations.

/// How a signer implementation obtains signatures.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerKind {
    /// A local adapter performs signing without publishing a remote request.
    Local,
    /// A remote service or device performs signing.
    Remote,
    /// A composing host mediates signing through an explicit user interaction.
    HostMediated,
}

/// Cancellation behavior advertised by a signer.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationSupport {
    /// Cancellation is observed only before a remote request is published.
    BeforePublication,
    /// Cancellation remains observable after publication, without implying
    /// rollback of the already-published request.
    BeforeAndAfterPublication,
}

/// Portable signer behavior advertised to a composing host.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignerCapability {
    kind: SignerKind,
    cancellation: CancellationSupport,
    reports_progress: bool,
    may_require_authentication: bool,
}

impl SignerCapability {
    /// Creates an explicit capability declaration.
    #[must_use]
    pub const fn new(
        kind: SignerKind,
        cancellation: CancellationSupport,
        reports_progress: bool,
        may_require_authentication: bool,
    ) -> Self {
        Self {
            kind,
            cancellation,
            reports_progress,
            may_require_authentication,
        }
    }

    /// Returns the implementation kind.
    #[must_use]
    pub const fn kind(self) -> SignerKind {
        self.kind
    }

    /// Returns the advertised cancellation contract.
    #[must_use]
    pub const fn cancellation(self) -> CancellationSupport {
        self.cancellation
    }

    /// Reports whether the signer emits progress updates.
    #[must_use]
    pub const fn reports_progress(self) -> bool {
        self.reports_progress
    }

    /// Reports whether the signer may emit an authentication challenge.
    #[must_use]
    pub const fn may_require_authentication(self) -> bool {
        self.may_require_authentication
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn capability_round_trips_with_stable_wire_labels() {
        let capability = SignerCapability::new(
            SignerKind::Remote,
            CancellationSupport::BeforeAndAfterPublication,
            true,
            true,
        );
        let encoded = serde_json::to_string(&capability).expect("serialize capability");
        let decoded: SignerCapability =
            serde_json::from_str(&encoded).expect("deserialize capability");

        assert_eq!(decoded, capability);
        assert!(encoded.contains("remote"));
        assert!(encoded.contains("before_and_after_publication"));
    }
}
