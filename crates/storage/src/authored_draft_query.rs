//! Bounded, independently scoped pages of current opaque draft revisions.
use crate::{
    Error,
    authored_draft::{
        AUTHORED_DRAFT_QUERY_LIMIT_MAX, AUTHORED_DRAFT_SCHEMA_MAX_BYTES, AuthoredDraft,
        AuthoredDraftId, AuthoredDraftRevision,
    },
};

/// Maximum decoded payload bytes retained by one query page.
pub const AUTHORED_DRAFT_PAGE_PAYLOAD_MAX_BYTES: usize = 4 * 1024 * 1024;
/// Maximum serialized snapshot bytes read by one native query page.
pub const AUTHORED_DRAFT_PAGE_SNAPSHOT_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const AUTHORED_DRAFT_CURSOR_SCHEMA_VERSION: u16 = 1;

/// An opaque, stable application-selected scope digest; never a credential.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "[u8; 32]", into = "[u8; 32]"))]
pub struct AuthoredDraftScope([u8; 32]);

impl AuthoredDraftScope {
    pub fn new(bytes: [u8; 32]) -> Result<Self, Error> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(Error::InvalidAuthoredDraft);
        }
        Ok(Self(bytes))
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl TryFrom<[u8; 32]> for AuthoredDraftScope {
    type Error = Error;
    fn try_from(value: [u8; 32]) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<AuthoredDraftScope> for [u8; 32] {
    fn from(value: AuthoredDraftScope) -> Self {
        value.0
    }
}

/// Scope is selected independently on every call, including continuations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredDraftQuery {
    author: [u8; 32],
    payload_schema: String,
    scope: Option<AuthoredDraftScope>,
    limit: u16,
    after: Option<[u8; 16]>,
}
impl AuthoredDraftQuery {
    pub fn new(
        author: [u8; 32],
        payload_schema: impl AsRef<str>,
        scope: Option<AuthoredDraftScope>,
        limit: u16,
    ) -> Result<Self, Error> {
        let schema = payload_schema.as_ref();
        if author.iter().all(|byte| *byte == 0)
            || schema.is_empty()
            || schema.len() > AUTHORED_DRAFT_SCHEMA_MAX_BYTES
            || schema != schema.trim()
            || schema.chars().any(char::is_control)
            || limit == 0
            || limit > AUTHORED_DRAFT_QUERY_LIMIT_MAX
        {
            return Err(Error::InvalidAuthoredDraft);
        }
        Ok(Self {
            author,
            payload_schema: schema.to_owned(),
            scope,
            limit,
            after: None,
        })
    }
    pub fn with_cursor(mut self, cursor: &AuthoredDraftCursor) -> Result<Self, Error> {
        if cursor.schema_version != AUTHORED_DRAFT_CURSOR_SCHEMA_VERSION
            || cursor.author != self.author
            || cursor.payload_schema != self.payload_schema
            || cursor.scope != self.scope
        {
            return Err(Error::InvalidAuthoredDraft);
        }
        self.after = Some(cursor.after_id);
        Ok(self)
    }
    pub const fn author(&self) -> &[u8; 32] {
        &self.author
    }
    pub fn payload_schema(&self) -> &str {
        &self.payload_schema
    }
    pub const fn scope(&self) -> Option<AuthoredDraftScope> {
        self.scope
    }
    pub const fn limit(&self) -> u16 {
        self.limit
    }
    pub const fn after(&self) -> Option<[u8; 16]> {
        self.after
    }
    pub fn matches(&self, draft: &AuthoredDraft) -> bool {
        draft.author() == &self.author
            && draft.payload_schema() == self.payload_schema
            && draft.scope() == self.scope
    }
    pub fn cursor_after(&self, after_id: [u8; 16]) -> AuthoredDraftCursor {
        AuthoredDraftCursor {
            schema_version: AUTHORED_DRAFT_CURSOR_SCHEMA_VERSION,
            author: self.author,
            payload_schema: self.payload_schema.clone(),
            scope: self.scope,
            after_id,
        }
    }
}

/// Stable ID ordering does not move when a draft receives another revision.
/// This is a bounded scan, not an immutable cross-page database snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "CursorWire", into = "CursorWire"))]
pub struct AuthoredDraftCursor {
    schema_version: u16,
    author: [u8; 32],
    payload_schema: String,
    scope: Option<AuthoredDraftScope>,
    after_id: [u8; 16],
}
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CursorWire {
    schema_version: u16,
    author: [u8; 32],
    payload_schema: String,
    scope: Option<AuthoredDraftScope>,
    after_id: [u8; 16],
}
#[cfg(feature = "serde")]
impl TryFrom<CursorWire> for AuthoredDraftCursor {
    type Error = Error;
    fn try_from(value: CursorWire) -> Result<Self, Error> {
        if value.schema_version != AUTHORED_DRAFT_CURSOR_SCHEMA_VERSION {
            return Err(Error::InvalidAuthoredDraft);
        }
        Ok(
            AuthoredDraftQuery::new(value.author, value.payload_schema, value.scope, 1)?
                .cursor_after(value.after_id),
        )
    }
}
#[cfg(feature = "serde")]
impl From<AuthoredDraftCursor> for CursorWire {
    fn from(value: AuthoredDraftCursor) -> Self {
        Self {
            schema_version: value.schema_version,
            author: value.author,
            payload_schema: value.payload_schema,
            scope: value.scope,
            after_id: value.after_id,
        }
    }
}

/// Corrupt rows expose only their local position, never untrusted payloads.
/// A raw position can identify a malformed all-zero draft ID for repair.
#[derive(Clone, Eq, PartialEq)]
pub enum AuthoredDraftQueryRecord {
    Draft(AuthoredDraft),
    Corrupt {
        draft_key: [u8; 16],
        revision: AuthoredDraftRevision,
    },
}
impl core::fmt::Debug for AuthoredDraftQueryRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct(match self {
            Self::Draft(_) => "Draft",
            Self::Corrupt { .. } => "Corrupt",
        })
        .field("draft_key", &self.draft_key())
        .field("revision", &self.revision())
        .finish_non_exhaustive()
    }
}

impl AuthoredDraftQueryRecord {
    pub fn draft_key(&self) -> [u8; 16] {
        match self {
            Self::Draft(draft) => *draft.draft_id().as_bytes(),
            Self::Corrupt { draft_key, .. } => *draft_key,
        }
    }
    pub const fn revision(&self) -> AuthoredDraftRevision {
        match self {
            Self::Draft(draft) => draft.revision(),
            Self::Corrupt { revision, .. } => *revision,
        }
    }
    pub fn draft_id(&self) -> Result<AuthoredDraftId, Error> {
        AuthoredDraftId::new(self.draft_key())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredDraftPage {
    records: Vec<AuthoredDraftQueryRecord>,
    next_cursor: Option<AuthoredDraftCursor>,
}
impl AuthoredDraftPage {
    /// Backends return an ordered, bounded page after applying independent scope.
    pub fn new(
        query: &AuthoredDraftQuery,
        records: Vec<AuthoredDraftQueryRecord>,
        has_more: bool,
    ) -> Result<Self, Error> {
        if records.len() > usize::from(query.limit) || (has_more && records.is_empty()) {
            return Err(Error::InvalidAuthoredDraft);
        }
        let mut previous = query.after;
        let mut payload_bytes = 0usize;
        for record in &records {
            let key = record.draft_key();
            if previous.is_some_and(|previous| key <= previous) {
                return Err(Error::InvalidAuthoredDraft);
            }
            previous = Some(key);
            if let AuthoredDraftQueryRecord::Draft(draft) = record {
                if !query.matches(draft) {
                    return Err(Error::InvalidAuthoredDraft);
                }
                draft.validate()?;
                payload_bytes = payload_bytes
                    .checked_add(draft.payload().len())
                    .ok_or(Error::InvalidAuthoredDraft)?;
                if payload_bytes > AUTHORED_DRAFT_PAGE_PAYLOAD_MAX_BYTES {
                    return Err(Error::InvalidAuthoredDraft);
                }
            }
        }
        let next_cursor = if has_more {
            previous.map(|key| query.cursor_after(key))
        } else {
            None
        };
        Ok(Self {
            records,
            next_cursor,
        })
    }
    pub fn records(&self) -> &[AuthoredDraftQueryRecord] {
        &self.records
    }
    pub const fn next_cursor(&self) -> Option<&AuthoredDraftCursor> {
        self.next_cursor.as_ref()
    }
    pub fn into_records(self) -> Vec<AuthoredDraftQueryRecord> {
        self.records
    }
}
