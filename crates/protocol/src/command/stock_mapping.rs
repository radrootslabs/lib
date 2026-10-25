//! Exact owner-private option mapping capture, without ownership or stock effects.
use super::{COMMAND_MAX_BYTES, CommandError, Context, check_frame, hash, parse_object};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{
    ApprovalIntentDigest, CommandContextData, CommandDigest, ListingOptionId, PoolId, Quantity,
    Sequence, listing::ListingCoordinate, stock::OptionMappingKey,
};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use zeroize::Zeroizing;

/// An explicit private draw claim. Version0 means the mapping must not exist;
/// positive versions compare an existing mapping before an actual update.
pub struct MapOptionInput {
    key: OptionMappingKey,
    pool: PoolId,
    per_sale_unit: Quantity,
    expected: Sequence,
}
impl MapOptionInput {
    /// Captures a positive declared draw. Pool existence, unit conversion,
    /// ownership and freshness are verified by the authoritative executor.
    pub fn new(
        key: OptionMappingKey,
        pool: PoolId,
        per_sale_unit: Quantity,
        expected: Sequence,
    ) -> Result<Self, CommandError> {
        if per_sale_unit.amount().is_zero() {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self {
            key,
            pool,
            per_sale_unit,
            expected,
        })
    }
    /// Returns the complete listing/option identity.
    pub const fn key(&self) -> OptionMappingKey {
        self.key
    }
    /// Returns the referenced private pool without asserting it exists.
    pub const fn pool_id(&self) -> PoolId {
        self.pool
    }
    /// Returns the exact original per-sale-unit quantity.
    pub const fn per_sale_unit(&self) -> Quantity {
        self.per_sale_unit
    }
    /// Returns the expected mapping version, including absence at0.
    pub const fn expected_mapping_version(&self) -> Sequence {
        self.expected
    }
}
impl fmt::Debug for MapOptionInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MapOptionInput(<redacted>)")
    }
}

/// Closed bounded `stock.map_option` data; capturing it reserves no physical stock.
pub struct CapturedOptionMapping {
    context: CommandContextData,
    input: MapOptionInput,
    wire: Zeroizing<Box<[u8]>>,
    digest: CommandDigest,
    approval: ApprovalIntentDigest,
}
impl CapturedOptionMapping {
    /// Retains the complete command and separate canonical command/approval hashes.
    pub fn new(context: CommandContextData, input: MapOptionInput) -> Result<Self, CommandError> {
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
    /// Validates a duplicate-safe bounded closed command and canonical values.
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
            listing: ListingCoordinate,
            option_id: ListingOptionId,
            pool_id: PoolId,
            per_sale_unit: Quantity,
            expected_mapping_version: Sequence,
        }
        let root: Root<'_> = parse_object(raw)?;
        if root.kind != "stock.map_option" {
            return Err(CommandError::UnsupportedCommand);
        }
        let input: Input = parse_object(root.input.get().as_bytes())?;
        Self::new(
            root.context,
            MapOptionInput::new(
                OptionMappingKey::new(input.listing, input.option_id),
                input.pool_id,
                input.per_sale_unit,
                input.expected_mapping_version,
            )?,
        )
    }
    /// Borrows exact captured context; no field is refreshed during replay.
    pub const fn context(&self) -> &CommandContextData {
        &self.context
    }
    /// Borrows the private mapping input.
    pub const fn input(&self) -> &MapOptionInput {
        &self.input
    }
    /// Borrows bounded canonical protected bytes.
    pub fn protected_wire(&self) -> &[u8] {
        &self.wire
    }
    /// Returns the exact command digest, excluding diagnostic correlation only.
    pub const fn digest(&self) -> CommandDigest {
        self.digest
    }
    /// Returns the non-circular intent digest without approval or correlation.
    pub const fn approval_intent_digest(&self) -> ApprovalIntentDigest {
        self.approval
    }
}
impl fmt::Debug for CapturedOptionMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedOptionMapping(<redacted>)")
    }
}
struct Wire<'a> {
    context: &'a CommandContextData,
    input: &'a MapOptionInput,
    approval: bool,
    correlation: bool,
}
impl Serialize for Wire<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut value = serializer.serialize_struct("OptionMapping", 3)?;
        value.serialize_field(
            "context",
            &Context(self.context, self.approval, self.correlation),
        )?;
        value.serialize_field("input", &InputWire(self.input))?;
        value.serialize_field("type", "stock.map_option")?;
        value.end()
    }
}
struct InputWire<'a>(&'a MapOptionInput);
impl Serialize for InputWire<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        let mut value = serializer.serialize_struct("MapOptionInput", 5)?;
        value.serialize_field("expected_mapping_version", &v.expected)?;
        value.serialize_field("listing", &Coordinate(v.key.listing()))?;
        value.serialize_field("option_id", &v.key.option_id())?;
        value.serialize_field("per_sale_unit", &v.per_sale_unit)?;
        value.serialize_field("pool_id", &v.pool)?;
        value.end()
    }
}
struct Coordinate(ListingCoordinate);
impl Serialize for Coordinate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut value = serializer.serialize_struct("ListingCoordinate", 3)?;
        value.serialize_field("author", &self.0.author())?;
        value.serialize_field("d", &self.0.d())?;
        value.serialize_field("kind", &self.0.kind())?;
        value.end()
    }
}
