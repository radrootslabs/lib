//! Common captured commands and the original closed chat-command profile.
//! Parsing and hashing are pure data operations.
//! They never authorize a principal, confirm a transaction or request signing.
//!
//! Canonical encoding follows the RFC 8785 subset specified for this profile:
//! every schema key is ASCII, the only JSON number is fixed kind 30402, and
//! exact integers are strings. Field order is explicit and independent of map
//! features. Strings preserve their Unicode contents without normalization.

use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{
    ActorRef, ActorRole, Approval, ApprovalIntentDigest, AuthorityRef, CheckpointRef,
    CommandContextData, CommandDigest, ExpectedOrder, MessageId, PublicKey, SourceId, ThreadId,
};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Maximum raw or canonical command bytes accepted by this closed boundary.
pub const COMMAND_MAX_BYTES: usize = 65_536;
/// Shared plain chat text budget, before JSON/encryption expansion.
pub use radroots_domain::CHAT_MAX_BYTES;
const JSON_MAX_DEPTH: usize = 16;

/// Fixed input-free command data failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CommandError {
    /// The closed input shape or a value is invalid.
    InvalidInput,
    /// This command type is not implemented by the selected profile.
    UnsupportedCommand,
    /// A known command requires a profile feature not implemented here.
    UnsupportedProfile,
    /// A byte/depth bound was exceeded.
    LimitExceeded,
}
impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid command input",
            Self::UnsupportedCommand => "unsupported command",
            Self::UnsupportedProfile => "unsupported command profile",
            Self::LimitExceeded => "command limit exceeded",
        })
    }
}
impl core::error::Error for CommandError {}

mod captured;
pub use captured::{CapturedCommand, CommandKind};

/// A captured public listing reference. This descriptor verifies neither its
/// source event nor its availability, terms or stock backing.
#[derive(Clone, Eq, PartialEq)]
pub struct ListingRevisionRef {
    author: PublicKey,
    d: Box<str>,
    event_id: SourceId,
}
impl ListingRevisionRef {
    /// Validates the fixed 16-byte lowercase-hex listing coordinate component.
    pub fn new(author: PublicKey, d: &str, event_id: SourceId) -> Result<Self, CommandError> {
        if d.len() != 32
            || !d
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self {
            author,
            d: Box::from(d),
            event_id,
        })
    }
    /// Returns the referenced listing author.
    pub const fn author(&self) -> PublicKey {
        self.author
    }
    /// Borrows the canonical listing discriminator.
    pub fn d(&self) -> &str {
        &self.d
    }
    /// Returns the referenced public event digest.
    pub const fn event_id(&self) -> SourceId {
        self.event_id
    }
}
impl fmt::Debug for ListingRevisionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ListingRevisionRef(<descriptor>)")
    }
}

/// Validated inquiry data, without command or participant authority.
pub struct InquireInput {
    thread_id: ThreadId,
    peer: PublicKey,
    source: Option<ListingRevisionRef>,
    text: Zeroizing<String>,
}
impl InquireInput {
    /// Checks the plain UTF-8 budget before copying private text. Empty text is
    /// representable; nested rumor/encryption budgets remain separately checked.
    pub fn new(
        thread_id: ThreadId,
        peer: PublicKey,
        source: Option<ListingRevisionRef>,
        text: &str,
    ) -> Result<Self, CommandError> {
        if text.len() > CHAT_MAX_BYTES {
            return Err(CommandError::LimitExceeded);
        }
        Ok(Self {
            thread_id,
            peer,
            source,
            text: Zeroizing::new(String::from(text)),
        })
    }
    /// Returns the exact application thread.
    pub const fn thread_id(&self) -> ThreadId {
        self.thread_id
    }
    /// Returns the intended peer.
    pub const fn peer(&self) -> PublicKey {
        self.peer
    }
    /// Borrows the optional untrusted public-source descriptor.
    pub fn source(&self) -> Option<&ListingRevisionRef> {
        self.source.as_ref()
    }
    /// Exposes private text for its authorized encoder or participant view.
    pub fn text(&self) -> &str {
        &self.text
    }
}
impl fmt::Debug for InquireInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InquireInput(<redacted>)")
    }
}

/// Validated reply data. A parent selector alone does not authenticate history.
pub struct ReplyInput {
    thread_id: ThreadId,
    peer: PublicKey,
    parent: MessageId,
    text: Zeroizing<String>,
}
impl ReplyInput {
    /// Captures an exact logical parent and checks private text before copying.
    pub fn new(
        thread_id: ThreadId,
        peer: PublicKey,
        parent: MessageId,
        text: &str,
    ) -> Result<Self, CommandError> {
        if text.len() > CHAT_MAX_BYTES {
            return Err(CommandError::LimitExceeded);
        }
        Ok(Self {
            thread_id,
            peer,
            parent,
            text: Zeroizing::new(String::from(text)),
        })
    }
    /// Returns the captured thread.
    pub const fn thread_id(&self) -> ThreadId {
        self.thread_id
    }
    /// Returns the intended peer.
    pub const fn peer(&self) -> PublicKey {
        self.peer
    }
    /// Returns the logical rumor parent, independently of any outer envelope.
    pub const fn parent(&self) -> MessageId {
        self.parent
    }
    /// Exposes private text to an authorized consumer.
    pub fn text(&self) -> &str {
        &self.text
    }
}
impl fmt::Debug for ReplyInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReplyInput(<redacted>)")
    }
}

/// The two assigned known-kind private-chat jobs; no commerce kind is inferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatKind {
    /// A private question, independent of an order.
    Inquire,
    /// A reply to authenticated thread history.
    Reply,
}
impl ChatKind {
    /// Returns the exact command-schema discriminator.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inquire => "inbox.inquire",
            Self::Reply => "inbox.reply",
        }
    }
}
enum Input {
    Inquire(InquireInput),
    Reply(ReplyInput),
}

/// Immutable, bounded command *data*, not an authenticated command context.
/// The store must authenticate its separate protected proof and enforce every
/// current precondition in the actual admission transaction. This type has no
/// automatic private diagnostics, cloning or serialization.
pub struct CapturedChatCommand {
    context: CommandContextData,
    input: Input,
    wire: Zeroizing<Box<[u8]>>,
    digest: CommandDigest,
    approval_intent: ApprovalIntentDigest,
}
impl CapturedChatCommand {
    /// Captures an inquiry and computes exact profile digests without effects.
    pub fn inquire(context: CommandContextData, input: InquireInput) -> Result<Self, CommandError> {
        Self::new(context, Input::Inquire(input))
    }
    /// Captures a reply without trusting its parent selector as evidence.
    pub fn reply(context: CommandContextData, input: ReplyInput) -> Result<Self, CommandError> {
        Self::new(context, Input::Reply(input))
    }
    fn new(context: CommandContextData, input: Input) -> Result<Self, CommandError> {
        let encode = |approval, correlation| -> Result<Zeroizing<Vec<u8>>, CommandError> {
            let bytes = Zeroizing::new(
                serde_json::to_vec(&CanonicalCommand {
                    context: &context,
                    input: &input,
                    approval,
                    correlation,
                })
                .map_err(|_| CommandError::InvalidInput)?,
            );
            if bytes.len() > COMMAND_MAX_BYTES {
                return Err(CommandError::LimitExceeded);
            }
            Ok(bytes)
        };
        let wire = Zeroizing::new(Box::from(encode(true, true)?.as_slice()));
        let digest = hash(b"radroots.command.v1\0", &encode(true, false)?);
        let approval_intent = hash(b"radroots.approval.v1\0", &encode(false, false)?);
        Ok(Self {
            context,
            input,
            wire,
            digest: CommandDigest::from_bytes(digest),
            approval_intent: ApprovalIntentDigest::from_bytes(approval_intent),
        })
    }
    /// Parses a bounded closed profile. JSON syntax supplies no grant or consent.
    pub fn parse(raw: &[u8]) -> Result<Self, CommandError> {
        check_frame(raw)?;
        let root: RawRoot<'_> = parse_object(raw)?;
        if root.kind.len() > 80 {
            return Err(CommandError::LimitExceeded);
        }
        match root.kind.as_str() {
            "inbox.inquire" => {
                let raw: RawInquiry = parse_object(root.input.get().as_bytes())?;
                if raw.text.len() > CHAT_MAX_BYTES {
                    return Err(CommandError::LimitExceeded);
                }
                let source = raw.source.map(|source| source.0.validate()).transpose()?;
                Self::new(
                    root.context,
                    Input::Inquire(InquireInput {
                        thread_id: raw.thread_id,
                        peer: raw.peer,
                        source,
                        text: raw.text,
                    }),
                )
            }
            "inbox.reply" => {
                let raw: RawReply = parse_object(root.input.get().as_bytes())?;
                if raw.text.len() > CHAT_MAX_BYTES {
                    return Err(CommandError::LimitExceeded);
                }
                Self::new(
                    root.context,
                    Input::Reply(ReplyInput {
                        thread_id: raw.thread_id,
                        peer: raw.peer,
                        parent: raw.parent,
                        text: raw.text,
                    }),
                )
            }
            _ => Err(CommandError::UnsupportedCommand),
        }
    }
    /// Borrows untrusted captured context data for authoritative comparison.
    pub const fn context(&self) -> &CommandContextData {
        &self.context
    }
    /// Returns the command digest, with correlation removed entirely.
    pub const fn digest(&self) -> CommandDigest {
        self.digest
    }
    /// Returns the approval-intent digest with approval and correlation removed.
    /// The digest alone authenticates no consent origin.
    pub const fn approval_intent_digest(&self) -> ApprovalIntentDigest {
        self.approval_intent
    }
    /// Returns the known command discriminator.
    pub const fn kind(&self) -> ChatKind {
        match self.input {
            Input::Inquire(_) => ChatKind::Inquire,
            Input::Reply(_) => ChatKind::Reply,
        }
    }
    /// Returns the thread selected by this immutable command.
    pub const fn thread_id(&self) -> ThreadId {
        match &self.input {
            Input::Inquire(v) => v.thread_id,
            Input::Reply(v) => v.thread_id,
        }
    }
    /// Returns its intended peer.
    pub const fn peer(&self) -> PublicKey {
        match &self.input {
            Input::Inquire(v) => v.peer,
            Input::Reply(v) => v.peer,
        }
    }
    /// Borrows its exact logical parent where applicable.
    pub const fn parent(&self) -> Option<MessageId> {
        match &self.input {
            Input::Inquire(_) => None,
            Input::Reply(v) => Some(v.parent),
        }
    }
    /// Borrows an optional inquiry source descriptor.
    pub fn source(&self) -> Option<&ListingRevisionRef> {
        match &self.input {
            Input::Inquire(v) => v.source(),
            Input::Reply(_) => None,
        }
    }
    /// Exposes private text to its authorized store or wire encoder.
    pub fn text(&self) -> &str {
        match &self.input {
            Input::Inquire(v) => v.text(),
            Input::Reply(v) => v.text(),
        }
    }
    /// Exposes complete canonical private command bytes for protected handling.
    /// These include the original diagnostic correlation and are not the hash input.
    pub fn protected_wire(&self) -> &[u8] {
        &self.wire
    }
}
impl fmt::Debug for CapturedChatCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedChatCommand(<redacted>)")
    }
}

pub(crate) fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    hash.finalize().into()
}
pub(crate) fn check_frame(raw: &[u8]) -> Result<(), CommandError> {
    if raw.len() > COMMAND_MAX_BYTES {
        return Err(CommandError::LimitExceeded);
    }
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for byte in raw {
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > JSON_MAX_DEPTH {
                        return Err(CommandError::LimitExceeded);
                    }
                }
                b'}' | b']' => depth = depth.checked_sub(1).ok_or(CommandError::InvalidInput)?,
                _ => {}
            }
        }
    }
    if quoted || depth != 0 {
        return Err(CommandError::InvalidInput);
    }
    Ok(())
}

// Require maps without materializing a second JSON tree. The typed decoder
// still owns duplicate, required-field and unknown-field checks, and borrowed
// RawValue fields continue to refer to the bounded original frame.
pub(crate) fn object<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<T, D::Error> {
    struct ObjectVisitor<T>(core::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for ObjectVisitor<T> {
        type Value = T;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an object")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<T, M::Error> {
            T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }
    d.deserialize_map(ObjectVisitor::<T>(core::marker::PhantomData))
}
struct MapOnly<T>(T);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for MapOnly<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        object(d).map(Self)
    }
}
pub(crate) fn parse_object<'de, T: Deserialize<'de>>(raw: &'de [u8]) -> Result<T, CommandError> {
    serde_json::from_slice::<MapOnly<T>>(raw)
        .map(|value| value.0)
        .map_err(|_| CommandError::InvalidInput)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoot<'a> {
    #[serde(rename = "type")]
    kind: String,
    context: CommandContextData,
    #[serde(borrow)]
    input: &'a serde_json::value::RawValue,
}
pub(crate) fn required_nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
fn private_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Zeroizing<String>, D::Error> {
    String::deserialize(d).map(Zeroizing::new)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInquiry {
    thread_id: ThreadId,
    peer: PublicKey,
    #[serde(deserialize_with = "required_nullable")]
    source: Option<MapOnly<RawSource>>,
    #[serde(deserialize_with = "private_text")]
    text: Zeroizing<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReply {
    thread_id: ThreadId,
    peer: PublicKey,
    parent: MessageId,
    #[serde(deserialize_with = "private_text")]
    text: Zeroizing<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    #[serde(deserialize_with = "object")]
    coordinate: RawCoordinate,
    event_id: SourceId,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCoordinate {
    kind: u32,
    author: PublicKey,
    d: String,
}
impl RawSource {
    fn validate(self) -> Result<ListingRevisionRef, CommandError> {
        if self.coordinate.kind != 30402 {
            return Err(CommandError::InvalidInput);
        }
        ListingRevisionRef::new(self.coordinate.author, &self.coordinate.d, self.event_id)
    }
}

struct CanonicalCommand<'a> {
    context: &'a CommandContextData,
    input: &'a Input,
    approval: bool,
    correlation: bool,
}
impl Serialize for CanonicalCommand<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut o = s.serialize_struct("ChatCommand", 3)?;
        o.serialize_field(
            "context",
            &Context(self.context, self.approval, self.correlation),
        )?;
        o.serialize_field("input", &CanonicalInput(self.input))?;
        o.serialize_field(
            "type",
            match self.input {
                Input::Inquire(_) => "inbox.inquire",
                Input::Reply(_) => "inbox.reply",
            },
        )?;
        o.end()
    }
}
pub(crate) struct Context<'a>(
    pub(crate) &'a CommandContextData,
    pub(crate) bool,
    pub(crate) bool,
);
impl Serialize for Context<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        let mut o = s.serialize_struct("Context", 7 + usize::from(self.1) + usize::from(self.2))?;
        o.serialize_field("actor", &Actor(v.actor()))?;
        if self.1 {
            o.serialize_field("approval", &v.approval().map(CanonicalApproval))?;
        }
        o.serialize_field("authority", &Authority(v.authority()))?;
        if self.2 {
            o.serialize_field("correlation_id", &v.correlation_id().map(|c| c.as_str()))?;
        }
        o.serialize_field("expected", &v.expected().map(Expected))?;
        o.serialize_field("not_after", &v.not_after())?;
        o.serialize_field("operation_id", &v.operation_id())?;
        o.serialize_field("route_digest", &v.route_digest())?;
        o.serialize_field("workspace_id", &v.workspace_id())?;
        o.end()
    }
}
struct Actor(ActorRef);
impl Serialize for Actor {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let role = match self.0.role() {
            ActorRole::Buyer => "buyer",
            ActorRole::Seller => "seller",
            ActorRole::Coordinator => "coordinator",
            _ => return Err(serde::ser::Error::custom("unsupported actor role")),
        };
        let mut o = s.serialize_struct("Actor", 4)?;
        o.serialize_field("grant_id", &self.0.grant_id())?;
        o.serialize_field("organization", &self.0.organization())?;
        o.serialize_field("participant", &self.0.participant())?;
        o.serialize_field("role", role)?;
        o.end()
    }
}
struct Authority(AuthorityRef);
impl Serialize for Authority {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut o = s.serialize_struct("Authority", 2)?;
        o.serialize_field("executor_id", &self.0.executor_id())?;
        o.serialize_field("generation", &self.0.generation())?;
        o.end()
    }
}
struct CanonicalApproval<'a>(&'a Approval);
impl Serialize for CanonicalApproval<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut o = s.serialize_struct("Approval", 3)?;
        o.serialize_field("expires_at", &self.0.expires_at())?;
        o.serialize_field("intent_digest", &self.0.intent_digest())?;
        o.serialize_field("scope", self.0.scope())?;
        o.end()
    }
}
struct Expected<'a>(&'a ExpectedOrder);
impl Serialize for Expected<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        let mut o = s.serialize_struct("Expected", 5)?;
        o.serialize_field("agreement", &v.agreement())?;
        o.serialize_field("buyer_head", &v.buyer_head())?;
        o.serialize_field("checkpoint", &Checkpoint(v.checkpoint()))?;
        o.serialize_field("order_id", &v.order_id())?;
        o.serialize_field("seller_sequence", &v.seller_sequence())?;
        o.end()
    }
}
struct Checkpoint<'a>(&'a CheckpointRef);
impl Serialize for Checkpoint<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        let mut o = s.serialize_struct("Checkpoint", 4)?;
        o.serialize_field("coverage", &v.coverage())?;
        o.serialize_field("digest", &v.digest())?;
        o.serialize_field("missing", v.missing())?;
        o.serialize_field("sequence", &v.sequence())?;
        o.end()
    }
}
struct CanonicalInput<'a>(&'a Input);
impl Serialize for CanonicalInput<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut o = s.serialize_struct("ChatInput", 4)?;
        match self.0 {
            Input::Inquire(v) => {
                o.serialize_field("peer", &v.peer)?;
                o.serialize_field("source", &v.source)?;
                o.serialize_field("text", v.text())?;
                o.serialize_field("thread_id", &v.thread_id)?;
            }
            Input::Reply(v) => {
                o.serialize_field("parent", &v.parent)?;
                o.serialize_field("peer", &v.peer)?;
                o.serialize_field("text", v.text())?;
                o.serialize_field("thread_id", &v.thread_id)?;
            }
        }
        o.end()
    }
}
impl Serialize for ListingRevisionRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut o = s.serialize_struct("ListingRevision", 2)?;
        o.serialize_field("coordinate", &Coordinate(self))?;
        o.serialize_field("event_id", &self.event_id)?;
        o.end()
    }
}
struct Coordinate<'a>(&'a ListingRevisionRef);
impl Serialize for Coordinate<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut o = s.serialize_struct("ListingCoordinate", 3)?;
        o.serialize_field("author", &self.0.author)?;
        o.serialize_field("d", &self.0.d)?;
        o.serialize_field("kind", &30402u32)?;
        o.end()
    }
}
