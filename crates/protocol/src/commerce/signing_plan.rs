//! Complete immutable private signing data. No store or provider fact is implied.
use super::{CommerceSeal, EncodedCommerceRumor};
use crate::{
    intent::SigningReplay,
    nip01::WireError,
    nip59::{CapturedDmRoute, SIGNED_MAX_BYTES},
};
use alloc::{collections::BTreeSet, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{CommandDigest, EnvelopeEventId, OperationId, UnixSeconds};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Includes both complete original route advertisements and exact seal fields.
pub const SIGNING_PLAN_MAX_BYTES: usize = 256 * 1024;
const MAGIC: &[u8] = b"radroots.commerce.signing_plan.v1\0";

/// Complete peer/self plan tied to one exact original record and rumor.
/// Construction supplies data only. The host must verify encryption, signed
/// route sources, selected targets, current consent and actual durable T1.
pub struct CommerceSigningPlan {
    operation: OperationId,
    command: CommandDigest,
    seals: [CommerceSeal; 2],
    routes: [CapturedDmRoute; 2],
    replay: SigningReplay,
    wire: Zeroizing<Vec<u8>>,
    digest: [u8; 32],
}
impl CommerceSigningPlan {
    /// Requires the complete sorted bilateral set and one original route for
    /// each recipient. Combined selected relays cannot exceed sixteen.
    pub fn new(
        operation: OperationId,
        command: CommandDigest,
        rumor: &EncodedCommerceRumor,
        seals: [CommerceSeal; 2],
        routes: [CapturedDmRoute; 2],
        replay: SigningReplay,
    ) -> Result<Self, WireError> {
        let mut targets = BTreeSet::new();
        let mut length = MAGIC.len() + 32 * 4 + 8 + 2;
        for ((seal, route), recipient) in seals.iter().zip(&routes).zip(rumor.recipients()) {
            if seal.operation_id() != operation
                || seal.command_digest() != command
                || seal.record_id() != rumor.record_id()
                || seal.rumor_id() != rumor.event_id()
                || seal.fields().author != rumor.author()
                || seal.recipient() != *recipient
                || route.participant() != *recipient
            {
                return Err(WireError::InvalidInput);
            }
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
                targets.insert(target.as_ref());
            }
        }
        if length > SIGNING_PLAN_MAX_BYTES || targets.len() > 16 {
            return Err(WireError::LimitExceeded);
        }
        let mut wire = Zeroizing::new(Vec::with_capacity(length));
        wire.extend_from_slice(MAGIC);
        wire.extend_from_slice(operation.as_bytes());
        wire.extend_from_slice(command.as_bytes());
        wire.extend_from_slice(rumor.record_id().as_bytes());
        wire.extend_from_slice(rumor.event_id().as_bytes());
        wire.extend_from_slice(&rumor.created_at().get().to_be_bytes());
        wire.push(replay.code());
        wire.push(2);
        for (seal, route) in seals.iter().zip(&routes) {
            wire.extend_from_slice(seal.recipient().as_bytes());
            wire.extend_from_slice(seal.fields().author.as_bytes());
            wire.extend_from_slice(&13u32.to_be_bytes());
            wire.push(0);
            wire.push(1);
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
            operation,
            command,
            seals,
            routes,
            replay,
            wire,
            digest,
        })
    }
    /// Reconstructs every intent against the independently supplied original
    /// T0 rumor. A stored ID alone cannot substitute for its exact contents.
    pub fn parse(
        operation: OperationId,
        command: CommandDigest,
        rumor: &EncodedCommerceRumor,
        wire: &[u8],
    ) -> Result<Self, WireError> {
        if wire.len() > SIGNING_PLAN_MAX_BYTES {
            return Err(WireError::LimitExceeded);
        }
        let mut input = Input(wire);
        if input.take(MAGIC.len())? != MAGIC
            || input.take(32)? != operation.as_bytes()
            || input.take(32)? != command.as_bytes()
            || input.take(32)? != rumor.record_id().as_bytes()
            || input.take(32)? != rumor.event_id().as_bytes()
            || u64::from_be_bytes(input.array()?) != rumor.created_at().get()
        {
            return Err(WireError::InvalidInput);
        }
        let replay = SigningReplay::from_code(input.byte()?)?;
        if input.byte()? != 2 {
            return Err(WireError::InvalidInput);
        }
        let mut seals = Vec::with_capacity(2);
        let mut routes = Vec::with_capacity(2);
        for recipient in rumor.recipients() {
            if input.take(32)? != recipient.as_bytes()
                || input.take(32)? != rumor.author().as_bytes()
                || u32::from_be_bytes(input.array()?) != 13
                || input.byte()? != 0
                || input.byte()? != 1
            {
                return Err(WireError::InvalidInput);
            }
            let id: [u8; 32] = input.array()?;
            let time = UnixSeconds::new(u64::from_be_bytes(input.array()?))
                .map_err(|_| WireError::InvalidInput)?;
            let ciphertext = input.text(SIGNED_MAX_BYTES)?;
            let seal = CommerceSeal::new(operation, command, rumor, *recipient, time, ciphertext)?;
            if seal.event_id() != id {
                return Err(WireError::InvalidInput);
            }
            seals.push(seal);
            let source = EnvelopeEventId::from_bytes(input.array()?);
            let raw = input.text(SIGNED_MAX_BYTES)?;
            let count = usize::from(input.byte()?);
            if !(1..=16).contains(&count) {
                return Err(WireError::InvalidInput);
            }
            let mut selected = Vec::with_capacity(count);
            for _ in 0..count {
                selected.push(String::from(input.text(2048)?));
            }
            routes.push(CapturedDmRoute::from_observed_descriptor(
                *recipient, source, raw, &selected,
            )?);
        }
        if !input.0.is_empty() {
            return Err(WireError::InvalidInput);
        }
        let plan = Self::new(
            operation,
            command,
            rumor,
            seals.try_into().map_err(|_| WireError::InvalidInput)?,
            routes.try_into().map_err(|_| WireError::InvalidInput)?,
            replay,
        )?;
        if plan.protected_wire() != wire {
            return Err(WireError::InvalidInput);
        }
        Ok(plan)
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation
    }
    pub const fn command_digest(&self) -> CommandDigest {
        self.command
    }
    pub const fn seals(&self) -> &[CommerceSeal; 2] {
        &self.seals
    }
    pub const fn routes(&self) -> &[CapturedDmRoute; 2] {
        &self.routes
    }
    pub const fn replay(&self) -> SigningReplay {
        self.replay
    }
    /// Original private data requiring protected at-rest handling.
    pub fn protected_wire(&self) -> &[u8] {
        &self.wire
    }
    /// Exact commitment of every original intent, route and replay descriptor.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}
impl fmt::Debug for CommerceSigningPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CommerceSigningPlan(<redacted>)")
    }
}
fn put(out: &mut Vec<u8>, value: &[u8]) {
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
