//! Exact public T1 data. No signature, route approval or durable fact is minted.
use super::{CapturedListingCommand, LISTING_EVENT_MAX_BYTES, ListingEvent};
use crate::{
    intent::SigningReplay,
    nip01::{self, WireError},
};
use alloc::{boxed::Box, collections::BTreeSet, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{CommandDigest, OperationId, UnixSeconds};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;
const MAGIC: &[u8] = b"radroots.listing.signing_plan.v1\0";
const MAX_BYTES: usize = 128 * 1024;

/// Complete original public artifact and bounded write-target descriptors.
/// Actual URI syntax, current destination approval and signer guarantees stay
/// executor-owned. Public data has no private peer, seal or message identity.
pub struct PublicSigningPlan {
    operation: OperationId,
    command: CommandDigest,
    created_at: UnixSeconds,
    replay: SigningReplay,
    event: ListingEvent,
    targets: Box<[Box<str>]>,
    wire: Zeroizing<Vec<u8>>,
    digest: [u8; 32],
}
impl PublicSigningPlan {
    /// Captures the exact known preimage and explicit selected write targets.
    pub fn capture(
        command: &CapturedListingCommand,
        created_at: UnixSeconds,
        targets: &[String],
        replay: SigningReplay,
    ) -> Result<Self, WireError> {
        if targets.len() > 16 {
            return Err(WireError::LimitExceeded);
        }
        if targets.is_empty() {
            return Err(WireError::InvalidInput);
        }
        let mut seen = BTreeSet::new();
        for target in targets {
            if target.len() > 2048 {
                return Err(WireError::LimitExceeded);
            }
            if !(target.starts_with("wss://") || target.starts_with("ws://"))
                || target.trim() != target
                || target.chars().any(char::is_control)
                || !seen.insert(target)
            {
                return Err(WireError::InvalidInput);
            }
        }
        let event = ListingEvent::new(
            command.input(),
            command.context().actor().participant(),
            created_at,
        )?;
        let preimage = nip01::bounded_preimage(&event.fields(), LISTING_EVENT_MAX_BYTES)?;
        let length = MAGIC.len()
            + 32 * 3
            + 8
            + 4
            + 4
            + preimage.len()
            + targets.iter().map(|s| 4 + s.len()).sum::<usize>();
        if length > MAX_BYTES {
            return Err(WireError::LimitExceeded);
        }
        let mut wire = Zeroizing::new(Vec::with_capacity(length));
        wire.extend_from_slice(MAGIC);
        wire.extend_from_slice(command.context().operation_id().as_bytes());
        wire.extend_from_slice(command.digest().as_bytes());
        wire.extend_from_slice(&created_at.get().to_be_bytes());
        wire.push(replay.code());
        wire.push(0); // PublicEvent artifact role.
        wire.push(0); // No recipient, including no invented self recipient.
        wire.extend_from_slice(event.event_id());
        put(&mut wire, preimage.as_bytes());
        wire.push(targets.len() as u8);
        for target in targets {
            put(&mut wire, target.as_bytes());
        }
        let digest = Sha256::digest(wire.as_slice()).into();
        Ok(Self {
            operation: command.context().operation_id(),
            command: command.digest(),
            created_at,
            replay,
            event,
            targets: targets.iter().map(|s| Box::from(s.as_str())).collect(),
            wire,
            digest,
        })
    }
    /// Reconstructs the full original listing from the retained command and
    /// compares every encoded preimage field, role, target and framing byte.
    pub fn parse(command: &CapturedListingCommand, wire: &[u8]) -> Result<Self, WireError> {
        if wire.len() > MAX_BYTES {
            return Err(WireError::LimitExceeded);
        }
        let mut input = Input(wire);
        if input.take(MAGIC.len())? != MAGIC
            || input.take(32)? != command.context().operation_id().as_bytes()
            || input.take(32)? != command.digest().as_bytes()
        {
            return Err(WireError::InvalidInput);
        }
        let time = UnixSeconds::new(u64::from_be_bytes(input.array()?))
            .map_err(|_| WireError::InvalidInput)?;
        let replay = SigningReplay::from_code(input.array::<1>()?[0])?;
        if input.array::<2>()? != [0, 0] {
            return Err(WireError::InvalidInput);
        }
        let event_id: [u8; 32] = input.array()?;
        let preimage = input.text(LISTING_EVENT_MAX_BYTES)?;
        let count = usize::from(input.array::<1>()?[0]);
        if count == 0 || count > 16 {
            return Err(WireError::InvalidInput);
        }
        let mut targets = Vec::with_capacity(count);
        for _ in 0..count {
            targets.push(String::from(input.text(2048)?));
        }
        if !input.0.is_empty() {
            return Err(WireError::InvalidInput);
        }
        let plan = Self::capture(command, time, &targets, replay)?;
        let expected = nip01::bounded_preimage(&plan.event.fields(), LISTING_EVENT_MAX_BYTES)?;
        if plan.event.event_id() != &event_id
            || expected.as_str() != preimage
            || plan.protected_wire() != wire
        {
            return Err(WireError::InvalidInput);
        }
        Ok(plan)
    }
    /// Exact original operation identity.
    pub const fn operation_id(&self) -> OperationId {
        self.operation
    }
    /// Exact original semantic command commitment.
    pub const fn command_digest(&self) -> CommandDigest {
        self.command
    }
    /// Exact retained public timestamp.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    /// Captured declaration, without a provider guarantee.
    pub const fn replay(&self) -> SigningReplay {
        self.replay
    }
    /// Exact unsigned public event, with no signature or installation claim.
    pub const fn event(&self) -> &ListingEvent {
        &self.event
    }
    /// Original bounded write-target descriptors in retained order.
    pub fn selected_targets(&self) -> &[Box<str>] {
        &self.targets
    }
    /// Complete protected representation, distinct from chat plan bytes.
    pub fn protected_wire(&self) -> &[u8] {
        &self.wire
    }
    /// Commitment to the entire retained representation.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}
impl fmt::Debug for PublicSigningPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicSigningPlan(<redacted>)")
    }
}
fn put(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u32).to_be_bytes());
    out.extend_from_slice(value);
}
struct Input<'a>(&'a [u8]);
impl<'a> Input<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], WireError> {
        let (a, b) = self
            .0
            .split_at_checked(size)
            .ok_or(WireError::InvalidInput)?;
        self.0 = b;
        Ok(a)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        self.take(N)?
            .try_into()
            .map_err(|_| WireError::InvalidInput)
    }
    fn text(&mut self, max: usize) -> Result<&'a str, WireError> {
        let size = u32::from_be_bytes(self.array()?) as usize;
        if size > max {
            return Err(WireError::LimitExceeded);
        }
        core::str::from_utf8(self.take(size)?).map_err(|_| WireError::InvalidInput)
    }
}
