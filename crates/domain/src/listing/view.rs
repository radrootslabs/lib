//! Immutable public values. Construction authenticates no source or installation.
use super::{ListingCoordinate, ListingError, ListingId, ListingRevision, ListingTerms, text};
use crate::{EnvelopeEventId, PublicKey, UnixSeconds};
use alloc::string::String;
use core::fmt;

/// Bounded public presentation fields shared by authoring and retained views.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ListingText {
    title: String,
    summary: String,
    description_markdown: String,
}
impl ListingText {
    /// Validates before copying, preserving exact Unicode and whitespace.
    pub fn new(
        title: &str,
        summary: &str,
        description_markdown: &str,
    ) -> Result<Self, ListingError> {
        text(title, 160, false)?;
        text(summary, 280, false)?;
        text(description_markdown, 16_384, true)?;
        Ok(Self {
            title: title.into(),
            summary: summary.into(),
            description_markdown: description_markdown.into(),
        })
    }
    /// Original public title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Original public summary.
    pub fn summary(&self) -> &str {
        &self.summary
    }
    /// Original public Markdown, without rendering or sanitization effects.
    pub fn description_markdown(&self) -> &str {
        &self.description_markdown
    }
}
impl fmt::Debug for ListingText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ListingText(<redacted>)")
    }
}

/// One immutable public listing revision. This value proves neither authenticated
/// observation, local installation, relay freshness nor stock authority.
#[derive(Clone, Eq, PartialEq)]
pub struct Listing {
    id: ListingId,
    author: PublicKey,
    revision: EnvelopeEventId,
    created_at: UnixSeconds,
    published_at: UnixSeconds,
    text: ListingText,
    terms: ListingTerms,
}
impl Listing {
    /// Captures explicit original values. Concrete execution must separately
    /// authenticate the original command, signed revision and installation.
    pub fn new(
        id: ListingId,
        author: PublicKey,
        revision: EnvelopeEventId,
        created_at: UnixSeconds,
        published_at: UnixSeconds,
        text: ListingText,
        terms: ListingTerms,
    ) -> Result<Self, ListingError> {
        if published_at > created_at {
            return Err(ListingError::InvalidTime);
        }
        Ok(Self {
            id,
            author,
            revision,
            created_at,
            published_at,
            text,
            terms,
        })
    }
    /// Stable coordinate discriminator, separate from its signed revision.
    pub const fn id(&self) -> ListingId {
        self.id
    }
    /// Original public author.
    pub const fn author(&self) -> PublicKey {
        self.author
    }
    /// Exact signed event identity for this revision.
    pub const fn revision(&self) -> EnvelopeEventId {
        self.revision
    }
    /// Returns the exact structural source reference, without new source proof.
    pub const fn source(&self) -> ListingRevision {
        ListingRevision::new(ListingCoordinate::new(self.author, self.id), self.revision)
    }
    /// Original signed event time, not local installation or current time.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    /// Original declared publication time.
    pub const fn published_at(&self) -> UnixSeconds {
        self.published_at
    }
    /// Original bounded presentation values.
    pub const fn text(&self) -> &ListingText {
        &self.text
    }
    /// Exact original validated offer terms; these allocate no private stock.
    pub const fn terms(&self) -> &ListingTerms {
        &self.terms
    }
}
impl fmt::Debug for Listing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Listing(<redacted>)")
    }
}
