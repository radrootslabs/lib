//! Immutable complete known-kind signing data, without execution authority.
use super::{CapturedDmRoute, ChatRumor, ChatSeal};
use crate::{command::CapturedChatCommand, nip01::WireError};
use alloc::{boxed::Box, collections::BTreeSet, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{
    CommandDigest, EnvelopeEventId, MessageId, OperationId, PublicKey, UnixSeconds,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Complete protected plan bound, including original signed route sources.
pub const PLAN_MAX_BYTES: usize = 384 * 1024;
const MAGIC: &[u8] = b"radroots.chat.signing_plan.v1\0";

/// Captured provider declaration. Data never establishes a provider guarantee
/// or permits reissue after an uncertain effect; execution revalidates support.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatReplay {
    NonReplayable,
    LocalReplaySafe,
    ExactReplayByRequestId,
}

/// Complete sorted recipient set, exact seal preimages and retained route
/// observations. Construction verifies structural provenance only: encryption
/// authentication, signed route verification, network approval, provider
/// capability and protected T1 commit remain execution obligations.
pub struct ChatSigningPlan {
    operation_id: OperationId,
    command_digest: CommandDigest,
    message_id: MessageId,
    created_at: UnixSeconds,
    replay: ChatReplay,
    seals: Box<[ChatSeal]>,
    routes: Box<[CapturedDmRoute]>,
    wire: Zeroizing<Vec<u8>>,
    digest: [u8; 32],
}
impl ChatSigningPlan {
    /// Requires exactly one seal and observed route per sorted unique recipient.
    /// Combined selected targets cannot exceed the sixteen-relay session cap.
    pub fn new(
        command: &CapturedChatCommand,
        created_at: UnixSeconds,
        seals: Vec<ChatSeal>,
        routes: Vec<CapturedDmRoute>,
        replay: ChatReplay,
    ) -> Result<Self, WireError> {
        let rumor = ChatRumor::from_command(command, created_at)?;
        if seals.len() != rumor.recipients().len() || routes.len() != seals.len() {
            return Err(WireError::InvalidInput);
        }
        let mut targets = BTreeSet::new();
        for ((seal, route), recipient) in seals.iter().zip(&routes).zip(rumor.recipients()) {
            if seal.operation_id() != command.context().operation_id()
                || seal.command_digest() != command.digest()
                || seal.message_id() != rumor.message_id()
                || seal.recipient() != *recipient
                || route.participant() != *recipient
            {
                return Err(WireError::InvalidInput);
            }
            for target in route.selected_relays() {
                targets.insert(target.as_ref());
            }
        }
        if targets.len() > 16 {
            return Err(WireError::LimitExceeded);
        }
        // All variable fields are already bounded. Count the complete binary
        // representation before allocating its sole retained canonical buffer.
        let mut length = MAGIC.len() + 32 * 3 + 8 + 2;
        for (seal, route) in seals.iter().zip(&routes) {
            length += 32
                + 32
                + 4
                + 1
                + 1
                + 32
                + 8
                + 4
                + seal.fields().content.len()
                + 32
                + 4
                + route.raw_json().len()
                + 1;
            for target in route.selected_relays() {
                length += 4 + target.len();
            }
        }
        if length > PLAN_MAX_BYTES {
            return Err(WireError::LimitExceeded);
        }
        let mut wire = Zeroizing::new(Vec::with_capacity(length));
        wire.extend_from_slice(MAGIC);
        wire.extend_from_slice(command.context().operation_id().as_bytes());
        wire.extend_from_slice(command.digest().as_bytes());
        wire.extend_from_slice(rumor.message_id().as_bytes());
        wire.extend_from_slice(&created_at.get().to_be_bytes());
        wire.push(match replay {
            ChatReplay::NonReplayable => 0,
            ChatReplay::LocalReplaySafe => 1,
            ChatReplay::ExactReplayByRequestId => 2,
        });
        wire.push(seals.len() as u8);
        for (seal, route) in seals.iter().zip(&routes) {
            wire.extend_from_slice(seal.recipient().as_bytes());
            wire.extend_from_slice(seal.fields().author.as_bytes());
            wire.extend_from_slice(&13u32.to_be_bytes());
            wire.push(0); // Exact empty tag set.
            wire.push(1); // Exact BusinessSeal role.
            wire.extend_from_slice(&seal.event_id());
            wire.extend_from_slice(&seal.fields().created_at.to_be_bytes());
            put(&mut wire, seal.fields().content.as_bytes());
            wire.extend_from_slice(route.source_id().as_bytes());
            put(&mut wire, route.raw_json().as_bytes());
            wire.push(route.selected_relays().len() as u8);
            for target in route.selected_relays() {
                put(&mut wire, target.as_bytes());
            }
        }
        let digest = Sha256::digest(wire.as_slice()).into();
        Ok(Self {
            operation_id: command.context().operation_id(),
            command_digest: command.digest(),
            message_id: rumor.message_id(),
            created_at,
            replay,
            seals: seals.into_boxed_slice(),
            routes: routes.into_boxed_slice(),
            wire,
            digest,
        })
    }
    /// Decodes only the bounded canonical profile and reconstructs every seal
    /// from the original command. It supplies no cryptographic route proof.
    pub fn parse(command: &CapturedChatCommand, wire: &[u8]) -> Result<Self, WireError> {
        if wire.len() > PLAN_MAX_BYTES {
            return Err(WireError::LimitExceeded);
        }
        let mut input = Input(wire);
        if input.take(MAGIC.len())? != MAGIC
            || input.take(32)? != command.context().operation_id().as_bytes()
            || input.take(32)? != command.digest().as_bytes()
        {
            return Err(WireError::InvalidInput);
        }
        let message_id = MessageId::from_bytes(input.array()?);
        let time = UnixSeconds::new(u64::from_be_bytes(input.array()?))
            .map_err(|_| WireError::InvalidInput)?;
        let rumor = ChatRumor::from_command(command, time)?;
        if message_id != rumor.message_id() {
            return Err(WireError::InvalidInput);
        }
        let replay = match input.byte()? {
            0 => ChatReplay::NonReplayable,
            1 => ChatReplay::LocalReplaySafe,
            2 => ChatReplay::ExactReplayByRequestId,
            _ => return Err(WireError::InvalidInput),
        };
        if usize::from(input.byte()?) != rumor.recipients().len() {
            return Err(WireError::InvalidInput);
        }
        let mut seals = Vec::new();
        let mut routes = Vec::new();
        for recipient in rumor.recipients() {
            let described =
                PublicKey::from_bytes(input.array()?).map_err(|_| WireError::InvalidInput)?;
            if described != *recipient {
                return Err(WireError::InvalidInput);
            }
            let author: [u8; 32] = input.array()?;
            let kind = u32::from_be_bytes(input.array()?);
            let tags = input.byte()?;
            let purpose = input.byte()?;
            let expected_id: [u8; 32] = input.array()?;
            if author != *rumor.author().as_bytes() || kind != 13 || tags != 0 || purpose != 1 {
                return Err(WireError::InvalidInput);
            }
            let seal_time = UnixSeconds::new(u64::from_be_bytes(input.array()?))
                .map_err(|_| WireError::InvalidInput)?;
            let ciphertext = input.text(super::SIGNED_MAX_BYTES)?;
            let seal = ChatSeal::new(&rumor, *recipient, seal_time, ciphertext)?;
            if seal.event_id() != expected_id {
                return Err(WireError::InvalidInput);
            }
            seals.push(seal);
            let source_id = EnvelopeEventId::from_bytes(input.array()?);
            let raw_json = input.text(super::SIGNED_MAX_BYTES)?;
            let count = usize::from(input.byte()?);
            if count == 0 || count > 16 {
                return Err(WireError::InvalidInput);
            }
            let mut selected = Vec::with_capacity(count);
            for _ in 0..count {
                selected.push(String::from(input.text(2048)?));
            }
            routes.push(CapturedDmRoute::from_observed_descriptor(
                *recipient, source_id, raw_json, &selected,
            )?);
        }
        if !input.0.is_empty() {
            return Err(WireError::InvalidInput);
        }
        let plan = Self::new(command, time, seals, routes, replay)?;
        if plan.protected_wire() != wire {
            return Err(WireError::InvalidInput);
        }
        Ok(plan)
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub const fn command_digest(&self) -> CommandDigest {
        self.command_digest
    }
    pub const fn message_id(&self) -> MessageId {
        self.message_id
    }
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    pub const fn replay(&self) -> ChatReplay {
        self.replay
    }
    pub fn seals(&self) -> &[ChatSeal] {
        &self.seals
    }
    pub fn routes(&self) -> &[CapturedDmRoute] {
        &self.routes
    }
    /// Borrows exact versioned bytes requiring protected at-rest handling.
    pub fn protected_wire(&self) -> &[u8] {
        &self.wire
    }
    /// Commits every exact preimage, route source, target and replay descriptor.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}
impl fmt::Debug for ChatSigningPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatSigningPlan(<redacted>)")
    }
}
fn put(out: &mut Vec<u8>, value: &[u8]) {
    // Every caller has bounded this field far below u32::MAX.
    out.extend_from_slice(&(value.len() as u32).to_be_bytes());
    out.extend_from_slice(value);
}
struct Input<'a>(&'a [u8]);
impl<'a> Input<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], WireError> {
        let (value, rest) = self
            .0
            .split_at_checked(size)
            .ok_or(WireError::InvalidInput)?;
        self.0 = rest;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        self.take(N)?
            .try_into()
            .map_err(|_| WireError::InvalidInput)
    }
    fn byte(&mut self) -> Result<u8, WireError> {
        Ok(self.array::<1>()?[0])
    }
    fn text(&mut self, max: usize) -> Result<&'a str, WireError> {
        let size = u32::from_be_bytes(self.array()?) as usize;
        if size > max {
            return Err(WireError::LimitExceeded);
        }
        core::str::from_utf8(self.take(size)?).map_err(|_| WireError::InvalidInput)
    }
}
