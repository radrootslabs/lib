use super::{PublishInput, canonical};
use crate::command::{COMMAND_MAX_BYTES, CommandError, Context, check_frame, hash};
use alloc::{boxed::Box, string::String};
use core::fmt;
use radroots_domain::{ApprovalIntentDigest, CommandContextData, CommandDigest};
use serde::Deserialize;
use zeroize::Zeroizing;

/// Exact captured public command data. Construction performs no authentication,
/// durable admission, event signing, installation or network work.
pub struct CapturedListingCommand {
    context: CommandContextData,
    input: Box<PublishInput>,
    wire: Zeroizing<Box<[u8]>>,
    digest: CommandDigest,
    approval: ApprovalIntentDigest,
}
impl CapturedListingCommand {
    /// Captures validated input with the common context/hash projections.
    pub fn capture(context: CommandContextData, input: PublishInput) -> Result<Self, CommandError> {
        let encode = |approval, correlation| -> Result<Zeroizing<String>, CommandError> {
            let mut root = serde_json::Map::new();
            root.insert(
                "context".into(),
                serde_json::to_value(Context(&context, approval, correlation))
                    .map_err(|_| CommandError::InvalidInput)?,
            );
            root.insert(
                "input".into(),
                serde_json::to_value(&input).map_err(|_| CommandError::InvalidInput)?,
            );
            root.insert(
                "type".into(),
                serde_json::Value::String("listing.publish".into()),
            );
            let value = serde_json::Value::Object(root);
            let mut out = Zeroizing::new(String::new());
            canonical(&value, &mut out).map_err(|error| match error {
                crate::nip01::WireError::LimitExceeded => CommandError::LimitExceeded,
                _ => CommandError::InvalidInput,
            })?;
            if out.len() > COMMAND_MAX_BYTES {
                return Err(CommandError::LimitExceeded);
            }
            Ok(out)
        };
        let wire = Zeroizing::new(Box::from(encode(true, true)?.as_bytes()));
        let digest = CommandDigest::from_bytes(hash(
            b"radroots.command.v1\0",
            encode(true, false)?.as_bytes(),
        ));
        let approval = ApprovalIntentDigest::from_bytes(hash(
            b"radroots.approval.v1\0",
            encode(false, false)?.as_bytes(),
        ));
        Ok(Self {
            context,
            input: Box::new(input),
            wire,
            digest,
            approval,
        })
    }
    /// Strictly parses an implemented profile; unsupported modes never become
    /// a quote, a zero price, an empty attachment or a replacement command.
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
        let root: Root<'_> = serde_json::from_slice(raw).map_err(|_| CommandError::InvalidInput)?;
        if root.kind != "listing.publish" {
            return Err(CommandError::UnsupportedCommand);
        }
        Self::capture(
            root.context,
            PublishInput::parse(root.input.get().as_bytes())?,
        )
    }
    /// Original context descriptors, without workspace permission.
    pub const fn context(&self) -> &CommandContextData {
        &self.context
    }
    /// Exact immutable input, with bounded owned collections.
    pub const fn input(&self) -> &PublishInput {
        &self.input
    }
    /// Semantic commitment excluding correlation only.
    pub const fn digest(&self) -> CommandDigest {
        self.digest
    }
    /// Approval-intent commitment excluding approval and correlation.
    pub const fn approval_intent_digest(&self) -> ApprovalIntentDigest {
        self.approval
    }
    /// Original canonical bytes for protected retention.
    pub fn protected_wire(&self) -> &[u8] {
        &self.wire
    }
}
impl fmt::Debug for CapturedListingCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedListingCommand(<redacted>)")
    }
}
