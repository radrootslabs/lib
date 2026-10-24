//! Exact owner-private pool creation data, without durable stock permission.
use super::{COMMAND_MAX_BYTES, CommandError, Context, check_frame, hash, parse_object};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{
    ApprovalIntentDigest, CommandContextData, CommandDigest, Decimal, PoolId, Quantity,
    listing::MeasurementUnit,
    stock::{PoolUnit, StockTicks},
};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use zeroize::Zeroizing;

/// Fixed unit/precision and initial quantity. This is a claim, not inventory.
pub struct CreatePoolInput {
    id: PoolId,
    label: Zeroizing<String>,
    unit: PoolUnit,
    quantity: Quantity,
    ticks: StockTicks,
}
impl CreatePoolInput {
    pub fn new(
        id: PoolId,
        label: &str,
        unit: PoolUnit,
        quantity: Quantity,
    ) -> Result<Self, CommandError> {
        if label.len() > 160 {
            return Err(CommandError::LimitExceeded);
        }
        if label.chars().any(char::is_control) {
            return Err(CommandError::InvalidInput);
        }
        let ticks = unit
            .ticks_for(quantity)
            .map_err(|_| CommandError::InvalidInput)?;
        Ok(Self {
            id,
            label: Zeroizing::new(String::from(label)),
            unit,
            quantity,
            ticks,
        })
    }
    pub const fn id(&self) -> PoolId {
        self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub const fn unit(&self) -> PoolUnit {
        self.unit
    }
    pub const fn quantity(&self) -> Quantity {
        self.quantity
    }
    pub const fn initial_ticks(&self) -> StockTicks {
        self.ticks
    }
}
impl fmt::Debug for CreatePoolInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CreatePoolInput(<redacted>)")
    }
}
/// Complete bounded command commitment. No host or signer is acquired here.
pub struct CapturedPoolCreation {
    context: CommandContextData,
    input: CreatePoolInput,
    wire: Zeroizing<Box<[u8]>>,
    digest: CommandDigest,
    approval: ApprovalIntentDigest,
}
impl CapturedPoolCreation {
    pub fn new(context: CommandContextData, input: CreatePoolInput) -> Result<Self, CommandError> {
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
        struct Input {
            id: PoolId,
            label: String,
            unit: MeasurementUnit,
            tick_scale: String,
            initial_quantity: Decimal,
        }
        let root: Root<'_> = parse_object(raw)?;
        if root.kind != "stock.create_pool" {
            return Err(CommandError::UnsupportedCommand);
        }
        let v: Input = parse_object(root.input.get().as_bytes())?;
        let scale = match v.tick_scale.as_bytes() {
            [byte @ b'0'..=b'6'] => *byte - b'0',
            _ => return Err(CommandError::InvalidInput),
        };
        let unit = PoolUnit::new(v.unit, scale).map_err(|_| CommandError::InvalidInput)?;
        let quantity =
            Quantity::new(v.initial_quantity, v.unit).map_err(|_| CommandError::InvalidInput)?;
        Self::new(
            root.context,
            CreatePoolInput::new(v.id, &v.label, unit, quantity)?,
        )
    }
    pub const fn context(&self) -> &CommandContextData {
        &self.context
    }
    pub const fn input(&self) -> &CreatePoolInput {
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
impl fmt::Debug for CapturedPoolCreation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedPoolCreation(<redacted>)")
    }
}
struct Wire<'a> {
    context: &'a CommandContextData,
    input: &'a CreatePoolInput,
    approval: bool,
    correlation: bool,
}
impl Serialize for Wire<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut o = s.serialize_struct("PoolCreation", 3)?;
        o.serialize_field(
            "context",
            &Context(self.context, self.approval, self.correlation),
        )?;
        o.serialize_field("input", &InputWire(self.input))?;
        o.serialize_field("type", "stock.create_pool")?;
        o.end()
    }
}
struct Scale(u8);
impl Serialize for Scale {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}
struct InputWire<'a>(&'a CreatePoolInput);
impl Serialize for InputWire<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        let mut o = s.serialize_struct("CreatePoolInput", 5)?;
        o.serialize_field("id", &v.id)?;
        o.serialize_field("initial_quantity", &v.quantity.amount())?;
        o.serialize_field("label", v.label())?;
        o.serialize_field("tick_scale", &Scale(v.unit.scale()))?;
        o.serialize_field("unit", &v.unit.unit())?;
        o.end()
    }
}
