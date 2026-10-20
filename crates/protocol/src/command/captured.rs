//! Common immutable command data. Domain-specific payloads remain explicit;
//! no chat identity or recipient is fabricated for a public command.
use super::{CapturedChatCommand, ChatKind, CommandError, check_frame};
use alloc::string::String;
use core::fmt;
use radroots_domain::{ApprovalIntentDigest, CommandContextData, CommandDigest};
use serde::Deserialize;

/// Implemented known-kind command families, without execution permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CommandKind {
    /// Private inquiry.
    Inquire,
    /// Private reply requiring authenticated history.
    Reply,
    /// Public listing publication.
    PublishListing,
}
impl CommandKind {
    /// Exact closed command discriminator.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inquire => "inbox.inquire",
            Self::Reply => "inbox.reply",
            Self::PublishListing => "listing.publish",
        }
    }
}

/// Bounded captured data shared by authoritative admission. Parsing supplies
/// no grant, durable preparation, signer permission or installation evidence.
pub enum CapturedCommand {
    /// Original chat profile, retaining its exact established commitments.
    Chat(CapturedChatCommand),
    /// Supported quote-required public profile.
    #[cfg(feature = "listing")]
    Listing(crate::listing::CapturedListingCommand),
}
impl CapturedCommand {
    /// Selects an implemented family only after bounded JSON frame admission.
    /// Each selected parser independently validates its entire closed shape.
    pub fn parse(raw: &[u8]) -> Result<Self, CommandError> {
        check_frame(raw)?;
        #[derive(Deserialize)]
        struct Header {
            #[serde(rename = "type")]
            kind: String,
        }
        let header: Header = serde_json::from_slice(raw).map_err(|_| CommandError::InvalidInput)?;
        if header.kind.len() > 80 {
            return Err(CommandError::LimitExceeded);
        }
        match header.kind.as_str() {
            "inbox.inquire" | "inbox.reply" => CapturedChatCommand::parse(raw).map(Self::Chat),
            #[cfg(feature = "listing")]
            "listing.publish" => {
                crate::listing::CapturedListingCommand::parse(raw).map(Self::Listing)
            }
            _ => Err(CommandError::UnsupportedCommand),
        }
    }
    /// Original complete context data, requiring separate authentication.
    pub const fn context(&self) -> &CommandContextData {
        match self {
            Self::Chat(v) => v.context(),
            #[cfg(feature = "listing")]
            Self::Listing(v) => v.context(),
        }
    }
    /// Original semantic commitment, excluding diagnostic correlation.
    pub const fn digest(&self) -> CommandDigest {
        match self {
            Self::Chat(v) => v.digest(),
            #[cfg(feature = "listing")]
            Self::Listing(v) => v.digest(),
        }
    }
    /// Exact approval intent; its bytes authenticate no consent.
    pub const fn approval_intent_digest(&self) -> ApprovalIntentDigest {
        match self {
            Self::Chat(v) => v.approval_intent_digest(),
            #[cfg(feature = "listing")]
            Self::Listing(v) => v.approval_intent_digest(),
        }
    }
    /// Exact supported command family.
    pub const fn kind(&self) -> CommandKind {
        match self {
            Self::Chat(v) => match v.kind() {
                ChatKind::Inquire => CommandKind::Inquire,
                ChatKind::Reply => CommandKind::Reply,
            },
            #[cfg(feature = "listing")]
            Self::Listing(_) => CommandKind::PublishListing,
        }
    }
    /// Borrows retained canonical bytes for protected handling.
    pub fn protected_wire(&self) -> &[u8] {
        match self {
            Self::Chat(v) => v.protected_wire(),
            #[cfg(feature = "listing")]
            Self::Listing(v) => v.protected_wire(),
        }
    }
    /// Borrows actual chat data, never a synthesized peer or empty message.
    pub const fn chat(&self) -> Option<&CapturedChatCommand> {
        match self {
            Self::Chat(v) => Some(v),
            #[cfg(feature = "listing")]
            Self::Listing(_) => None,
        }
    }
    /// Borrows actual supported public listing data.
    #[cfg(feature = "listing")]
    pub const fn listing(&self) -> Option<&crate::listing::CapturedListingCommand> {
        match self {
            Self::Chat(_) => None,
            Self::Listing(v) => Some(v),
        }
    }
    /// Whether this capture names history the fresh outbox cannot invent.
    pub fn requires_history(&self) -> bool {
        self.context().expected().is_some() || self.chat().is_some_and(|v| v.parent().is_some())
    }
}
impl From<CapturedChatCommand> for CapturedCommand {
    fn from(v: CapturedChatCommand) -> Self {
        Self::Chat(v)
    }
}
#[cfg(feature = "listing")]
impl From<crate::listing::CapturedListingCommand> for CapturedCommand {
    fn from(v: crate::listing::CapturedListingCommand) -> Self {
        Self::Listing(v)
    }
}
impl fmt::Debug for CapturedCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedCommand(<redacted>)")
    }
}
