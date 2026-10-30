//! Exact T0 command data. Capture establishes no source, approval, grant or hold.
use super::{COMMAND_MAX_BYTES, CommandError, Context, check_frame, hash, parse_object};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{
    ActorRole, AgreementLineId, ApprovalIntentDigest, CommandContextData, CommandDigest, PoolId,
    Quantity, RecordId, Sequence, TermsDigest,
};
use serde::{
    Deserialize, Serialize,
    ser::{SerializeSeq, SerializeStruct},
};
use serde_json::value::RawValue;
use zeroize::Zeroizing;

/// One advanced seller allocation claim, before live validation and acquisition.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AllocationInput {
    line: AgreementLineId,
    pool: PoolId,
    quantity: Quantity,
    expected: Sequence,
}
impl AllocationInput {
    /// Requires positive quantity and an existing pool version. The executor
    /// separately checks line coverage, compatible units and the actual pool.
    pub fn new(
        line: AgreementLineId,
        pool: PoolId,
        quantity: Quantity,
        expected: Sequence,
    ) -> Result<Self, CommandError> {
        if quantity.amount().is_zero() || expected.get() == 0 {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self {
            line,
            pool,
            quantity,
            expected,
        })
    }
    pub const fn line_id(self) -> AgreementLineId {
        self.line
    }
    pub const fn pool_id(self) -> PoolId {
        self.pool
    }
    pub const fn quantity(self) -> Quantity {
        self.quantity
    }
    pub const fn expected_pool_version(self) -> Sequence {
        self.expected
    }
}
impl fmt::Debug for AllocationInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AllocationInput(<redacted>)")
    }
}
/// Automatic mapping resolution or an explicit, nonempty bounded seller choice.
/// Neither choice authorizes stock-free acceptance or proves physical backing.
pub struct AllocationSelection(Choice);
enum Choice {
    Automatic,
    Explicit(Box<[AllocationInput]>),
}
impl AllocationSelection {
    pub const MAX_ALLOCATIONS: usize = 64;
    pub const fn automatic() -> Self {
        Self(Choice::Automatic)
    }
    /// Refuses empty/oversized lists and ambiguous duplicate line/pool pairs.
    /// Capture preserves supplied order; acquisition order belongs to the plan.
    pub fn explicit(allocations: Vec<AllocationInput>) -> Result<Self, CommandError> {
        if allocations.is_empty() || allocations.len() > Self::MAX_ALLOCATIONS {
            return Err(CommandError::InvalidInput);
        }
        for (index, value) in allocations.iter().enumerate() {
            if allocations[..index]
                .iter()
                .any(|prior| prior.line == value.line && prior.pool == value.pool)
            {
                return Err(CommandError::InvalidInput);
            }
        }
        Ok(Self(Choice::Explicit(allocations.into_boxed_slice())))
    }
    pub const fn is_automatic(&self) -> bool {
        matches!(self.0, Choice::Automatic)
    }
    pub fn explicit_allocations(&self) -> Option<&[AllocationInput]> {
        match &self.0 {
            Choice::Automatic => None,
            Choice::Explicit(values) => Some(values),
        }
    }
}
impl fmt::Debug for AllocationSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AllocationSelection(<redacted>)")
    }
}
/// Exact request and Terms identities; source lookup and authorization are IO.
pub struct AcceptOrderInput {
    request: RecordId,
    terms: TermsDigest,
    allocation: AllocationSelection,
}
impl AcceptOrderInput {
    pub const fn new(
        request: RecordId,
        terms: TermsDigest,
        allocation: AllocationSelection,
    ) -> Self {
        Self {
            request,
            terms,
            allocation,
        }
    }
    pub const fn request(&self) -> RecordId {
        self.request
    }
    pub const fn terms_digest(&self) -> TermsDigest {
        self.terms
    }
    pub const fn allocation(&self) -> &AllocationSelection {
        &self.allocation
    }
}
impl fmt::Debug for AcceptOrderInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AcceptOrderInput(<redacted>)")
    }
}
/// Closed `orders.accept` capture. A concrete authority must revalidate source,
/// approval, current access, order/mapping/pool state and commit actual holds.
pub struct CapturedOrderAcceptance {
    context: CommandContextData,
    input: AcceptOrderInput,
    wire: Zeroizing<Box<[u8]>>,
    digest: CommandDigest,
    approval: ApprovalIntentDigest,
}
impl CapturedOrderAcceptance {
    /// Requires seller/order descriptors as data. Approval may be absent while
    /// computing an intent for consent; capture never verifies that consent.
    pub fn new(context: CommandContextData, input: AcceptOrderInput) -> Result<Self, CommandError> {
        if context.actor().role() != ActorRole::Seller || context.expected().is_none() {
            return Err(CommandError::InvalidInput);
        }
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
        let wire = Zeroizing::new(encode(true, true)?.to_vec().into_boxed_slice());
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
            input: &'a RawValue,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input<'a> {
            request: RecordId,
            terms_digest: TermsDigest,
            #[serde(borrow)]
            allocation: &'a RawValue,
        }
        let root: Root<'_> = parse_object(raw)?;
        if root.kind != "orders.accept" {
            return Err(CommandError::UnsupportedCommand);
        }
        let input: Input<'_> = parse_object(root.input.get().as_bytes())?;
        Self::new(
            root.context,
            AcceptOrderInput::new(
                input.request,
                input.terms_digest,
                parse_selection(input.allocation.get().as_bytes())?,
            ),
        )
    }
    pub const fn context(&self) -> &CommandContextData {
        &self.context
    }
    pub const fn input(&self) -> &AcceptOrderInput {
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
impl fmt::Debug for CapturedOrderAcceptance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedOrderAcceptance(<redacted>)")
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AllocationWire {
    line_id: AgreementLineId,
    pool_id: PoolId,
    quantity: Quantity,
    expected_pool_version: Sequence,
}
pub(super) fn parse_selection(raw: &[u8]) -> Result<AllocationSelection, CommandError> {
    #[derive(Deserialize)]
    struct Probe {
        #[serde(rename = "type")]
        kind: String,
    }
    let probe: Probe = parse_object(raw)?;
    match probe.kind.as_str() {
        "automatic" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Automatic {
                #[serde(rename = "type")]
                _kind: String,
            }
            let _: Automatic = parse_object(raw)?;
            Ok(AllocationSelection::automatic())
        }
        "explicit" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Explicit<'a> {
                #[serde(rename = "type")]
                _kind: String,
                #[serde(borrow)]
                allocations: &'a RawValue,
            }
            let value: Explicit<'_> = parse_object(raw)?;
            let mut decoder = serde_json::Deserializer::from_str(value.allocations.get());
            struct Allocations;
            impl<'de> serde::de::Visitor<'de> for Allocations {
                type Value = Vec<AllocationInput>;
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("one through64 closed allocation objects")
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut seq: A,
                ) -> Result<Self::Value, A::Error> {
                    let mut values = Vec::new();
                    while let Some(raw) = seq.next_element::<&RawValue>()? {
                        if values.len() == AllocationSelection::MAX_ALLOCATIONS {
                            return Err(serde::de::Error::custom("allocation count limit"));
                        }
                        let value: AllocationWire =
                            parse_object(raw.get().as_bytes()).map_err(serde::de::Error::custom)?;
                        values.push(
                            AllocationInput::new(
                                value.line_id,
                                value.pool_id,
                                value.quantity,
                                value.expected_pool_version,
                            )
                            .map_err(serde::de::Error::custom)?,
                        );
                    }
                    Ok(values)
                }
            }
            let values = serde::Deserializer::deserialize_seq(&mut decoder, Allocations)
                .map_err(|_| CommandError::InvalidInput)?;
            decoder.end().map_err(|_| CommandError::InvalidInput)?;
            AllocationSelection::explicit(values)
        }
        _ => Err(CommandError::InvalidInput),
    }
}
struct Wire<'a> {
    context: &'a CommandContextData,
    input: &'a AcceptOrderInput,
    approval: bool,
    correlation: bool,
}
impl Serialize for Wire<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_struct("OrderAcceptance", 3)?;
        out.serialize_field(
            "context",
            &Context(self.context, self.approval, self.correlation),
        )?;
        out.serialize_field("input", &InputWire(self.input))?;
        out.serialize_field("type", "orders.accept")?;
        out.end()
    }
}
struct InputWire<'a>(&'a AcceptOrderInput);
impl Serialize for InputWire<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_struct("AcceptOrderInput", 3)?;
        out.serialize_field("allocation", &self.0.allocation)?;
        out.serialize_field("request", &self.0.request)?;
        out.serialize_field("terms_digest", &self.0.terms)?;
        out.end()
    }
}
impl Serialize for AllocationSelection {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_struct(
            "AllocationSelection",
            if self.is_automatic() { 1 } else { 2 },
        )?;
        if let Some(values) = self.explicit_allocations() {
            out.serialize_field("allocations", &AllocationsWire(values))?;
        }
        out.serialize_field(
            "type",
            if self.is_automatic() {
                "automatic"
            } else {
                "explicit"
            },
        )?;
        out.end()
    }
}
struct AllocationsWire<'a>(&'a [AllocationInput]);
impl Serialize for AllocationsWire<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_seq(Some(self.0.len()))?;
        for value in self.0 {
            out.serialize_element(&AllocationWrite(value))?;
        }
        out.end()
    }
}
struct AllocationWrite<'a>(&'a AllocationInput);
impl Serialize for AllocationWrite<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_struct("AllocationInput", 4)?;
        out.serialize_field("expected_pool_version", &self.0.expected)?;
        out.serialize_field("line_id", &self.0.line)?;
        out.serialize_field("pool_id", &self.0.pool)?;
        out.serialize_field("quantity", &self.0.quantity)?;
        out.end()
    }
}
