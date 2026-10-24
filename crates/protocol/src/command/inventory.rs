//! Closed owner-private inventory command dispatch, without host authority.
use super::{
    CapturedPoolCorrection, CapturedPoolCreation, CommandError, check_frame, parse_object,
};
use alloc::string::String;
use radroots_domain::{
    ApprovalIntentDigest, CommandContextData, CommandDigest, stock::AvailableStockCorrection,
};
use serde::Deserialize;

/// Effect class claimed by an exact captured owner-private inventory command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryCommandKind {
    /// Creation of a new fixed-unit pool.
    CreatePool,
    /// Positive correction of introduced stock.
    AddStock,
    /// Audited removal of physical stock.
    RemoveStock,
    /// Audited physical spoilage.
    SpoilStock,
}
/// Original bounded captured inventory data. Its metadata establishes no grant,
/// physical stock, transaction completion or permission to change obligations.
pub enum CapturedInventoryCommand {
    /// Original exact pool-creation profile.
    CreatePool(CapturedPoolCreation),
    /// Exact adjustment or spoilage profile.
    CorrectPool(CapturedPoolCorrection),
}
impl CapturedInventoryCommand {
    pub fn parse(raw: &[u8]) -> Result<Self, CommandError> {
        check_frame(raw)?;
        #[derive(Deserialize)]
        struct Header {
            #[serde(rename = "type")]
            kind: String,
        }
        let header: Header = parse_object(raw)?;
        match header.kind.as_str() {
            "stock.create_pool" => CapturedPoolCreation::parse(raw).map(Self::CreatePool),
            "stock.adjust" | "stock.spoil" => {
                CapturedPoolCorrection::parse(raw).map(Self::CorrectPool)
            }
            _ => Err(CommandError::UnsupportedCommand),
        }
    }
    pub const fn context(&self) -> &CommandContextData {
        match self {
            Self::CreatePool(v) => v.context(),
            Self::CorrectPool(v) => v.context(),
        }
    }
    pub const fn digest(&self) -> CommandDigest {
        match self {
            Self::CreatePool(v) => v.digest(),
            Self::CorrectPool(v) => v.digest(),
        }
    }
    pub const fn approval_intent_digest(&self) -> ApprovalIntentDigest {
        match self {
            Self::CreatePool(v) => v.approval_intent_digest(),
            Self::CorrectPool(v) => v.approval_intent_digest(),
        }
    }
    pub fn protected_wire(&self) -> &[u8] {
        match self {
            Self::CreatePool(v) => v.protected_wire(),
            Self::CorrectPool(v) => v.protected_wire(),
        }
    }
    pub const fn kind(&self) -> InventoryCommandKind {
        match self {
            Self::CreatePool(_) => InventoryCommandKind::CreatePool,
            Self::CorrectPool(v) => match v.input().kind() {
                AvailableStockCorrection::Add => InventoryCommandKind::AddStock,
                AvailableStockCorrection::Remove => InventoryCommandKind::RemoveStock,
                AvailableStockCorrection::Spoil => InventoryCommandKind::SpoilStock,
            },
        }
    }
    /// Referenced private pool identity, without authenticating its existence.
    pub const fn pool_id(&self) -> radroots_domain::PoolId {
        match self {
            Self::CreatePool(v) => v.input().id(),
            Self::CorrectPool(v) => v.input().pool_id(),
        }
    }
    /// Borrows actual creation data; correction commands never fabricate it.
    pub const fn creation_input(&self) -> Option<&super::CreatePoolInput> {
        match self {
            Self::CreatePool(v) => Some(v.input()),
            Self::CorrectPool(_) => None,
        }
    }
    /// Borrows actual correction data; creation commands never fabricate it.
    pub const fn correction_input(&self) -> Option<&super::PoolCorrectionInput> {
        match self {
            Self::CorrectPool(v) => Some(v.input()),
            Self::CreatePool(_) => None,
        }
    }
}
impl From<CapturedPoolCreation> for CapturedInventoryCommand {
    fn from(value: CapturedPoolCreation) -> Self {
        Self::CreatePool(value)
    }
}
impl From<CapturedPoolCorrection> for CapturedInventoryCommand {
    fn from(value: CapturedPoolCorrection) -> Self {
        Self::CorrectPool(value)
    }
}
impl core::fmt::Debug for CapturedInventoryCommand {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("CapturedInventoryCommand(<redacted>)")
    }
}
