//! Source progress metadata, independent of physical inventory progress.
use super::{StockAuthorizationRef, WorkspaceError};
use radroots_domain::{AuthorityRef, CheckpointDigest, Sequence};
/// A caller-retained acknowledged source minimum. It establishes neither
/// protected provenance nor source completeness, actionability or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockSourceCheckpointRef {
    authority: AuthorityRef,
    revision: Sequence,
    head: Option<CheckpointDigest>,
    authorization: Option<StockAuthorizationRef>,
}
impl StockSourceCheckpointRef {
    pub fn from_host_metadata(
        authority: AuthorityRef,
        revision: Sequence,
        head: Option<CheckpointDigest>,
    ) -> Result<Self, WorkspaceError> {
        if (revision.get() == 0) != head.is_none() {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(Self {
            authority,
            revision,
            head,
            authorization: None,
        })
    }
    pub const fn authority(self) -> AuthorityRef {
        self.authority
    }
    pub const fn revision(self) -> Sequence {
        self.revision
    }
    pub const fn head(self) -> Option<CheckpointDigest> {
        self.head
    }
    pub const fn authorization(self) -> Option<StockAuthorizationRef> {
        self.authorization
    }
    pub const fn with_authorization(mut self, state: StockAuthorizationRef) -> Self {
        self.authorization = Some(state);
        self
    }
    pub const fn with_optional_authorization(
        mut self,
        state: Option<StockAuthorizationRef>,
    ) -> Self {
        self.authorization = state;
        self
    }
}

/// Independently retained public-listing observation progress. This metadata
/// proves neither a global current head nor permission to request or accept.
/// Its distinct type prevents accidentally passing private-record progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockListingCheckpointRef(StockSourceCheckpointRef);
impl StockListingCheckpointRef {
    pub fn from_host_metadata(
        authority: AuthorityRef,
        revision: Sequence,
        head: Option<CheckpointDigest>,
    ) -> Result<Self, WorkspaceError> {
        StockSourceCheckpointRef::from_host_metadata(authority, revision, head).map(Self)
    }
    pub const fn authority(self) -> AuthorityRef {
        self.0.authority()
    }
    pub const fn revision(self) -> Sequence {
        self.0.revision()
    }
    pub const fn head(self) -> Option<CheckpointDigest> {
        self.0.head()
    }
    pub const fn authorization(self) -> Option<StockAuthorizationRef> {
        self.0.authorization()
    }
    pub const fn with_authorization(self, state: StockAuthorizationRef) -> Self {
        Self(self.0.with_authorization(state))
    }
    pub const fn with_optional_authorization(self, state: Option<StockAuthorizationRef>) -> Self {
        Self(self.0.with_optional_authorization(state))
    }
}
