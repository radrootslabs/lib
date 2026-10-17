//! Scoped workspace authentication primitives. These are storage operations,
//! not an application-owned command, stock or recovery engine.

use core::fmt;
use radroots_domain::{
    ActorRef, AuthorityRef, CheckpointRef, GrantId, PublicKey, RouteDigest, Sequence, UnixSeconds,
    WorkspaceId,
};
use radroots_transport::{BoxFuture, host::HostBound};
use zeroize::Zeroizing;

/// A fixed workspace boundary error; no path, credential, SQL or caller prose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkspaceError {
    /// Descriptor representation is invalid.
    InvalidInput,
    /// The selected protected provider or host is unavailable.
    Unavailable,
    /// The explicit private namespace cannot be safely opened.
    NamespaceUnavailable,
    /// Another writer retains this exact namespace.
    WriterActive,
    /// Namespace identity does not match the selected workspace.
    WrongNamespace,
    /// Protected state or creation evidence requires explicit recovery.
    RecoveryRequired,
    /// The presented protected credential does not authenticate its grant.
    Unauthorized,
    /// The current grant has expired.
    GrantExpired,
    /// The current grant has been revoked.
    GrantRevoked,
    /// The selected authority no longer matches.
    StaleAuthority,
    /// The required checkpoint does not match retained state.
    StaleCheckpoint,
    /// Admission is permanently closed.
    Closed,
    /// Storage reports a known failure before confirmed completion.
    StorageFailure,
    /// Transaction completion is uncertain and must be reconciled.
    CommitUnknown,
}
impl WorkspaceError {
    /// Returns the stable source-redacted diagnostic code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::Unavailable => "workspace_unavailable",
            Self::NamespaceUnavailable => "namespace_unavailable",
            Self::WriterActive => "writer_active",
            Self::WrongNamespace => "wrong_workspace",
            Self::RecoveryRequired => "recovery_required",
            Self::Unauthorized => "unauthorized",
            Self::GrantExpired => "grant_expired",
            Self::GrantRevoked => "grant_revoked",
            Self::StaleAuthority => "stale_authority",
            Self::StaleCheckpoint => "stale_checkpoint",
            Self::Closed => "closed",
            Self::StorageFailure => "storage_failure",
            Self::CommitUnknown => "commit_unknown",
        }
    }
}
impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl core::error::Error for WorkspaceError {}

/// An opaque protected bearer credential, independent of signer keys and grant
/// selectors. No automatic serialization, cloning or secret diagnostics exist.
pub struct AttachmentProof(Zeroizing<[u8; 32]>);
impl AttachmentProof {
    /// Copies one qualified protected-channel value into a zeroizing owner.
    /// This checks only width; the persisted grant must authenticate it.
    pub fn from_protected_input(value: &[u8]) -> Result<Self, WorkspaceError> {
        let bytes = <[u8; 32]>::try_from(value).map_err(|_| WorkspaceError::InvalidInput)?;
        Ok(Self(Zeroizing::new(bytes)))
    }
    /// Exposes the fixed secret only for explicit backend authentication.
    pub fn with_secret<T>(&self, use_secret: impl FnOnce(&[u8; 32]) -> T) -> T {
        use_secret(&self.0)
    }
}
impl fmt::Debug for AttachmentProof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AttachmentProof(<redacted>)")
    }
}

/// Explicit selected endpoint, participant, grant and checkpoint data. This is
/// never itself proof that the actor or endpoint is trusted.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AttachmentInput {
    workspace_id: WorkspaceId,
    authority: AuthorityRef,
    participant: PublicKey,
    endpoint: Box<str>,
    endpoint_identity: PublicKey,
    grant_id: GrantId,
    checkpoint: CheckpointRef,
    route_digest: RouteDigest,
}
impl AttachmentInput {
    /// Captures explicit selected input; it performs no authentication or I/O.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        workspace_id: WorkspaceId,
        authority: AuthorityRef,
        participant: PublicKey,
        endpoint: impl Into<String>,
        endpoint_identity: PublicKey,
        grant_id: GrantId,
        checkpoint: CheckpointRef,
        route_digest: RouteDigest,
    ) -> Result<Self, WorkspaceError> {
        let endpoint = endpoint.into();
        if endpoint.is_empty() || endpoint.len() > 2048 || endpoint.chars().any(char::is_control) {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(Self {
            workspace_id,
            authority,
            participant,
            endpoint: endpoint.into_boxed_str(),
            endpoint_identity,
            grant_id,
            checkpoint,
            route_digest,
        })
    }
    /// Returns the selected workspace, independent of a cache generation.
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    /// Returns the selected executor/epoch.
    pub const fn authority(&self) -> AuthorityRef {
        self.authority
    }
    /// Returns the claimed participant.
    pub const fn participant(&self) -> PublicKey {
        self.participant
    }
    /// Borrows the explicit selected endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    /// Returns its pinned application identity.
    pub const fn endpoint_identity(&self) -> PublicKey {
        self.endpoint_identity
    }
    /// Returns the requested grant selector, never its credential.
    pub const fn grant_id(&self) -> GrantId {
        self.grant_id
    }
    /// Borrows the required checkpoint.
    pub const fn checkpoint(&self) -> &CheckpointRef {
        &self.checkpoint
    }
    /// Returns the exact selected route commitment.
    pub const fn route_digest(&self) -> RouteDigest {
        self.route_digest
    }
}
impl fmt::Debug for AttachmentInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AttachmentInput(<untrusted descriptor>)")
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for AttachmentInput {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            workspace_id: WorkspaceId,
            authority: AuthorityRef,
            participant: PublicKey,
            endpoint: String,
            endpoint_identity: PublicKey,
            grant_id: GrantId,
            checkpoint: CheckpointRef,
            route_digest: RouteDigest,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::new(
            raw.workspace_id,
            raw.authority,
            raw.participant,
            raw.endpoint,
            raw.endpoint_identity,
            raw.grant_id,
            raw.checkpoint,
            raw.route_digest,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// A native/remote adapter's authenticated grant observation. It does not
/// replace transaction-time checks and contains no bearer credential.
#[derive(Clone, Eq, PartialEq)]
pub struct WorkspaceBinding {
    input: AttachmentInput,
    actor: ActorRef,
    principal_id: Box<str>,
    grant_revision: Sequence,
    expires_at: UnixSeconds,
    observed_at: UnixSeconds,
}
impl WorkspaceBinding {
    /// Checks the internal binding of a trusted storage adapter's observation.
    /// Host implementations require actual authentication qualification; this
    /// constructor does not turn public metadata into an authenticated context.
    pub fn from_authenticated_host(
        input: AttachmentInput,
        actor: ActorRef,
        principal_id: impl Into<String>,
        grant_revision: Sequence,
        expires_at: UnixSeconds,
        observed_at: UnixSeconds,
    ) -> Result<Self, WorkspaceError> {
        let principal_id = principal_id.into();
        if actor.participant() != input.participant()
            || actor.grant_id() != Some(input.grant_id())
            || principal_id.is_empty()
            || principal_id.len() > 128
            || principal_id.chars().any(char::is_control)
            || grant_revision.get() == 0
            || expires_at.get() == 0
            || observed_at.get() == 0
            || expires_at <= observed_at
        {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(Self {
            input,
            actor,
            principal_id: principal_id.into_boxed_str(),
            grant_revision,
            expires_at,
            observed_at,
        })
    }
    /// Borrows the exact authenticated selection.
    pub const fn input(&self) -> &AttachmentInput {
        &self.input
    }
    /// Returns actor data from the persisted authenticated grant.
    pub const fn actor(&self) -> ActorRef {
        self.actor
    }
    /// Borrows the authenticated local device/principal audit identity.
    pub fn principal_id(&self) -> &str {
        &self.principal_id
    }
    /// Returns the observed revocation-sensitive grant revision.
    pub const fn grant_revision(&self) -> Sequence {
        self.grant_revision
    }
    /// Returns the observed grant expiry, without reading an ambient clock.
    pub const fn expires_at(&self) -> UnixSeconds {
        self.expires_at
    }
    /// Returns the actual adapter clock observation used to validate expiry.
    pub const fn observed_at(&self) -> UnixSeconds {
        self.observed_at
    }
}
impl fmt::Debug for WorkspaceBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorkspaceBinding(<redacted>)")
    }
}

/// Backend-neutral current workspace authentication. Applications consume the
/// concrete shared implementation; they do not supply command/stock semantics.
pub trait WorkspaceStore: HostBound {
    /// Authenticates protected input against current persisted scope. The
    /// returned observation is rechecked at each authoritative transaction.
    /// The adapter owns its request credential through its safe completion;
    /// observers must not retain an attachment's credential root across await.
    fn authenticate(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<WorkspaceBinding, WorkspaceError>>;
    /// Stops storage admission and observes actual backend closure.
    fn close_workspace(&self) -> BoxFuture<'_, Result<(), WorkspaceError>>;
}

/// The backend's explicit time source. Failure cannot become a valid timestamp.
pub trait WorkspaceClock: HostBound {
    /// Returns a checked, positive current wall time or an unavailable result.
    fn now(&self) -> Result<UnixSeconds, WorkspaceError>;
}
