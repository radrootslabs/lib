//! Exact owner-private correction claims, without inventory permission or effects.
use super::{COMMAND_MAX_BYTES, CommandError, Context, check_frame, hash, parse_object};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{
    ApprovalIntentDigest, CommandContextData, CommandDigest, PoolId, PositiveDecimal, Sequence,
    stock::AvailableStockCorrection,
};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use zeroize::Zeroizing;

/// Positive correction quantity in the referenced pool's fixed unit. Resolving
/// that unit, current version and affected obligations belongs to the executor.
pub struct PoolCorrectionInput {
    pool: PoolId,
    expected: Sequence,
    kind: AvailableStockCorrection,
    amount: PositiveDecimal,
    reason: Zeroizing<String>,
}
impl PoolCorrectionInput {
    pub fn new(
        pool: PoolId,
        expected: Sequence,
        kind: AvailableStockCorrection,
        amount: PositiveDecimal,
        reason: &str,
    ) -> Result<Self, CommandError> {
        if expected.get() == 0 {
            return Err(CommandError::InvalidInput);
        }
        if reason.len() > 4096 || reason.chars().count() > 1024 {
            return Err(CommandError::LimitExceeded);
        }
        Ok(Self {
            pool,
            expected,
            kind,
            amount,
            reason: Zeroizing::new(String::from(reason)),
        })
    }
    pub const fn pool_id(&self) -> PoolId {
        self.pool
    }
    pub const fn expected_version(&self) -> Sequence {
        self.expected
    }
    pub const fn kind(&self) -> AvailableStockCorrection {
        self.kind
    }
    pub const fn amount(&self) -> PositiveDecimal {
        self.amount
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
}
impl fmt::Debug for PoolCorrectionInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PoolCorrectionInput(<redacted>)")
    }
}

/// Complete bounded `stock.adjust` or `stock.spoil` commitment. Construction
/// acquires no host and does not authorize changes to physical backing.
pub struct CapturedPoolCorrection {
    context: CommandContextData,
    input: PoolCorrectionInput,
    wire: Zeroizing<Box<[u8]>>,
    digest: CommandDigest,
    approval: ApprovalIntentDigest,
}
impl CapturedPoolCorrection {
    pub fn new(
        context: CommandContextData,
        input: PoolCorrectionInput,
    ) -> Result<Self, CommandError> {
        let encode = |approval, correlation| -> Result<Zeroizing<Vec<u8>>, CommandError> {
            let bytes = Zeroizing::new(
                serde_json::to_vec(&Wire {
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
        let digest =
            CommandDigest::from_bytes(hash(b"radroots.command.v1\0", &encode(true, false)?));
        let approval = ApprovalIntentDigest::from_bytes(hash(
            b"radroots.approval.v1\0",
            &encode(false, false)?,
        ));
        Ok(Self {
            context,
            input,
            wire,
            digest,
            approval,
        })
    }
    pub fn parse(raw: &[u8]) -> Result<Self, CommandError> {
        check_frame(raw)?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Root<'a> {
            #[serde(rename = "type")]
            kind: String,
            context: CommandContextData,
            #[serde(borrow)]
            input: &'a serde_json::value::RawValue,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Adjust {
            amount: PositiveDecimal,
            direction: String,
            expected_version: Sequence,
            pool_id: PoolId,
            reason: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Spoil {
            amount: PositiveDecimal,
            expected_version: Sequence,
            pool_id: PoolId,
            reason: String,
        }
        let root: Root<'_> = parse_object(raw)?;
        let input = match root.kind.as_str() {
            "stock.adjust" => {
                let v: Adjust = parse_object(root.input.get().as_bytes())?;
                let kind = match v.direction.as_str() {
                    "add" => AvailableStockCorrection::Add,
                    "remove" => AvailableStockCorrection::Remove,
                    _ => return Err(CommandError::InvalidInput),
                };
                PoolCorrectionInput::new(v.pool_id, v.expected_version, kind, v.amount, &v.reason)?
            }
            "stock.spoil" => {
                let v: Spoil = parse_object(root.input.get().as_bytes())?;
                PoolCorrectionInput::new(
                    v.pool_id,
                    v.expected_version,
                    AvailableStockCorrection::Spoil,
                    v.amount,
                    &v.reason,
                )?
            }
            _ => return Err(CommandError::UnsupportedCommand),
        };
        Self::new(root.context, input)
    }
    pub const fn context(&self) -> &CommandContextData {
        &self.context
    }
    pub const fn input(&self) -> &PoolCorrectionInput {
        &self.input
    }
    pub fn protected_wire(&self) -> &[u8] {
        &self.wire
    }
    pub const fn digest(&self) -> CommandDigest {
        self.digest
    }
    pub const fn approval_intent_digest(&self) -> ApprovalIntentDigest {
        self.approval
    }
}
impl fmt::Debug for CapturedPoolCorrection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedPoolCorrection(<redacted>)")
    }
}
struct Wire<'a> {
    context: &'a CommandContextData,
    input: &'a PoolCorrectionInput,
    approval: bool,
    correlation: bool,
}
impl Serialize for Wire<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut object = s.serialize_struct("PoolCorrection", 3)?;
        object.serialize_field(
            "context",
            &Context(self.context, self.approval, self.correlation),
        )?;
        object.serialize_field("input", &InputWire(self.input))?;
        object.serialize_field(
            "type",
            if self.input.kind == AvailableStockCorrection::Spoil {
                "stock.spoil"
            } else {
                "stock.adjust"
            },
        )?;
        object.end()
    }
}
struct InputWire<'a>(&'a PoolCorrectionInput);
impl Serialize for InputWire<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        let direction = match v.kind {
            AvailableStockCorrection::Add => Some("add"),
            AvailableStockCorrection::Remove => Some("remove"),
            AvailableStockCorrection::Spoil => None,
        };
        let mut object = s.serialize_struct(
            "PoolCorrectionInput",
            if direction.is_some() { 5 } else { 4 },
        )?;
        object.serialize_field("amount", &v.amount)?;
        if let Some(direction) = direction {
            object.serialize_field("direction", direction)?;
        }
        object.serialize_field("expected_version", &v.expected)?;
        object.serialize_field("pool_id", &v.pool)?;
        object.serialize_field("reason", v.reason())?;
        object.end()
    }
}
