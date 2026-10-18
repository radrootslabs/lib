//! Bounded logical private chat data, without authenticated installation facts.
use crate::{MessageId, PublicKey, SourceId, ThreadId, UnixSeconds};
use alloc::string::String;
use core::fmt;
use zeroize::Zeroizing;

/// Maximum plain UTF-8 chat bytes before JSON or encryption expansion.
pub const CHAT_MAX_BYTES: usize = 4096;

/// Input-free logical message failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageError {
    /// The plain text budget was exceeded.
    LimitExceeded,
}
impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("chat text limit exceeded")
    }
}
impl core::error::Error for MessageError {}

/// A logical relation. Selectors alone authenticate no source or parent history.
#[derive(Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum ChatRelation {
    /// An inquiry, optionally referring to the exact public source event.
    Inquire {
        /// Exact referenced public event, when supplied.
        source: Option<SourceId>,
    },
    /// A reply to an exact logical message, never an outer envelope ID.
    Reply {
        /// Original logical parent, whose history requires authentication.
        parent: MessageId,
    },
}
impl fmt::Debug for ChatRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatRelation(<redacted>)")
    }
}

/// Owned logical participant view. Construction checks bounds only; it supplies
/// no signature, authenticated history, durable admission or installation proof.
/// Private text is explicitly borrowed and is never included in Debug or an
/// automatic serialization implementation.
#[derive(Clone, Eq, PartialEq)]
pub struct ChatMessage {
    id: MessageId,
    thread: ThreadId,
    author: PublicKey,
    peer: PublicKey,
    created: UnixSeconds,
    relation: ChatRelation,
    text: Zeroizing<String>,
}
impl ChatMessage {
    /// Checks the UTF-8 budget before copying. Empty text and Unicode contents
    /// remain representable without trimming or normalization.
    pub fn new(
        id: MessageId,
        thread: ThreadId,
        author: PublicKey,
        peer: PublicKey,
        created: UnixSeconds,
        relation: ChatRelation,
        text: &str,
    ) -> Result<Self, MessageError> {
        if text.len() > CHAT_MAX_BYTES {
            return Err(MessageError::LimitExceeded);
        }
        Ok(Self {
            id,
            thread,
            author,
            peer,
            created,
            relation,
            text: Zeroizing::new(String::from(text)),
        })
    }
    /// Logical rumor identity, separate from seals and outer envelopes.
    pub const fn id(&self) -> MessageId {
        self.id
    }
    /// Exact application thread.
    pub const fn thread_id(&self) -> ThreadId {
        self.thread
    }
    /// Original participant author.
    pub const fn author(&self) -> PublicKey {
        self.author
    }
    /// Intended peer; a self copy does not replace the peer's outcome.
    pub const fn peer(&self) -> PublicKey {
        self.peer
    }
    /// Original logical creation time, not a later installation observation.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created
    }
    /// Original inquiry/source or logical reply relation.
    pub const fn relation(&self) -> ChatRelation {
        self.relation
    }
    /// Borrows private text for an authorized participant view.
    pub fn text(&self) -> &str {
        &self.text
    }
}
impl fmt::Debug for ChatMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatMessage(<redacted>)")
    }
}
