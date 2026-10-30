//! Exact `orders.install_terms` capture. Data capture supplies no source,
//! consent, current agreement/residual, stock effect or signing permission.
use super::{
    AllocationSelection, COMMAND_MAX_BYTES, CommandError, Context, check_frame, hash,
    order_acceptance::parse_selection, parse_object,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{
    ActorRole, ApprovalIntentDigest, CommandContextData, CommandDigest, RecordId, TermsDigest,
};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use serde_json::value::RawValue;
use zeroize::Zeroizing;

/// Complete proposal/authorization selectors and allocation claims. The
/// executor independently authenticates their terms, base and current state.
pub struct InstallTermsInput {
    proposal: RecordId,
    authorization: RecordId,
    terms: TermsDigest,
    allocation: AllocationSelection,
}
impl InstallTermsInput {
    /// Captures data only; both records and the actual digest remain unverified.
    pub const fn new(
        proposal: RecordId,
        authorization: RecordId,
        terms: TermsDigest,
        allocation: AllocationSelection,
    ) -> Self {
        Self {
            proposal,
            authorization,
            terms,
            allocation,
        }
    }
    /// Exact selected complete proposal.
    pub const fn proposal(&self) -> RecordId {
        self.proposal
    }
    /// Exact selected counterparty authorization.
    pub const fn authorization(&self) -> RecordId {
        self.authorization
    }
    /// Expected complete canonical terms digest, not a hash proof.
    pub const fn terms_digest(&self) -> TermsDigest {
        self.terms
    }
    /// Automatic resolution or explicit bounded allocation claims.
    pub const fn allocation(&self) -> &AllocationSelection {
        &self.allocation
    }
}
impl fmt::Debug for InstallTermsInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InstallTermsInput(<redacted>)")
    }
}

/// Closed normalized `orders.install_terms` data and separately domain-bound
/// command/approval digests. Construction grants no execution or stock right.
pub struct CapturedTermsInstallation {
    context: CommandContextData,
    input: InstallTermsInput,
    wire: Zeroizing<Box<[u8]>>,
    digest: CommandDigest,
    approval: ApprovalIntentDigest,
}
impl CapturedTermsInstallation {
    /// Requires seller and explicit order descriptors. Approval may be absent
    /// while obtaining consent; actual approval verification belongs to T0.
    pub fn new(
        context: CommandContextData,
        input: InstallTermsInput,
    ) -> Result<Self, CommandError> {
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
    /// Parses only the bounded exact command/context/input shape, without IO.
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
            proposal: RecordId,
            authorization: RecordId,
            terms_digest: TermsDigest,
            #[serde(borrow)]
            allocation: &'a RawValue,
        }
        let root: Root<'_> = parse_object(raw)?;
        if root.kind != "orders.install_terms" {
            return Err(CommandError::UnsupportedCommand);
        }
        let input: Input<'_> = parse_object(root.input.get().as_bytes())?;
        Self::new(
            root.context,
            InstallTermsInput::new(
                input.proposal,
                input.authorization,
                input.terms_digest,
                parse_selection(input.allocation.get().as_bytes())?,
            ),
        )
    }
    /// Captured context descriptors, not current native authority.
    pub const fn context(&self) -> &CommandContextData {
        &self.context
    }
    /// Exact selected source and allocation claims.
    pub const fn input(&self) -> &InstallTermsInput {
        &self.input
    }
    /// Bounded protected normalized command bytes, including correlation.
    pub fn protected_wire(&self) -> &[u8] {
        &self.wire
    }
    /// Command digest includes approval and excludes observer correlation.
    pub const fn digest(&self) -> CommandDigest {
        self.digest
    }
    /// Intent digest excludes approval and observer correlation.
    pub const fn approval_intent_digest(&self) -> ApprovalIntentDigest {
        self.approval
    }
}
impl fmt::Debug for CapturedTermsInstallation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedTermsInstallation(<redacted>)")
    }
}
struct Wire<'a> {
    context: &'a CommandContextData,
    input: &'a InstallTermsInput,
    approval: bool,
    correlation: bool,
}
impl Serialize for Wire<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("TermsInstallation", 3)?;
        out.serialize_field(
            "context",
            &Context(self.context, self.approval, self.correlation),
        )?;
        out.serialize_field("input", &InputWire(self.input))?;
        out.serialize_field("type", "orders.install_terms")?;
        out.end()
    }
}
struct InputWire<'a>(&'a InstallTermsInput);
impl Serialize for InputWire<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("InstallTermsInput", 4)?;
        out.serialize_field("allocation", &self.0.allocation)?;
        out.serialize_field("authorization", &self.0.authorization)?;
        out.serialize_field("proposal", &self.0.proposal)?;
        out.serialize_field("terms_digest", &self.0.terms)?;
        out.end()
    }
}
