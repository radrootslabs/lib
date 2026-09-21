//! Atomic installation of two opaque authored revisions, without effects.

use crate::{
    Error,
    authored_draft::{AuthoredDraft, AuthoredDraftRevision},
};

/// A fixed pair of distinct drafts belonging to one author.
///
/// Both expected heads use the existing single-draft compare-and-swap contract.
/// A backend must install both revisions or neither. Only an exact replay of
/// both existing revisions succeeds as replay; a partially existing pair is a
/// conflict. Historical replay does not establish current head ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredDraftPair {
    drafts: [AuthoredDraft; 2],
    expected_heads: [Option<AuthoredDraftRevision>; 2],
}

impl AuthoredDraftPair {
    pub fn new(
        first: AuthoredDraft,
        first_expected: Option<AuthoredDraftRevision>,
        second: AuthoredDraft,
        second_expected: Option<AuthoredDraftRevision>,
    ) -> Result<Self, Error> {
        first.validate()?;
        second.validate()?;
        if first.draft_id() == second.draft_id() || first.author() != second.author() {
            return Err(Error::InvalidAuthoredDraft);
        }
        Ok(Self {
            drafts: [first, second],
            expected_heads: [first_expected, second_expected],
        })
    }

    pub const fn drafts(&self) -> &[AuthoredDraft; 2] {
        &self.drafts
    }

    pub const fn expected_heads(&self) -> &[Option<AuthoredDraftRevision>; 2] {
        &self.expected_heads
    }
}
