//! Runtime operation descriptor contract generation 1.
//!
//! This module owns passive, serialized operation identities and policy
//! descriptors. Native SDK operation implementations and host execution state
//! deliberately remain outside the wire-contract package.

use alloc::{
    collections::BTreeSet,
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;

use crate::{
    capability::v1::{Maturity, TransportKind},
    schema::{Descriptor as SchemaDescriptor, ModuleVersion, Registry},
};

/// Schema generation shared by every runtime operation request and receipt.
pub const OPERATION_SCHEMA_VERSION: u16 = 1;

/// Typed readiness of one optional synchronization capability.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncCapabilityState {
    Unsupported,
    Compiled,
    Configured,
    Available,
    Degraded,
}

/// Aggregate synchronization health for the passive status operation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncHealth {
    Healthy,
    Degraded,
    Unavailable,
}

/// Host-action classification for one durable outbox record.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncRetryDecision {
    Ready,
    DeferredUntil { unix_ms: u64 },
    InFlightUntil { unix_ms: u64 },
    Satisfied,
    Exhausted,
    Expired,
}

/// Passive durable outbox cardinalities for `sync.status` generation 1.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncOutboxStatus {
    pub pending: u64,
    pub leased: u64,
    pub retryable: u64,
    pub satisfied: u64,
    pub exhausted: u64,
}

/// Passive projection cardinalities for `sync.status` generation 1.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncProjectionStatus {
    pub ready: u32,
    pub invalidated: u32,
    pub rebuilding: u32,
    pub failed: u32,
    pub untracked: u32,
}

/// Versioned passive receipt for the `sync.status` operation.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncStatusReceipt {
    pub schema_version: u16,
    pub health: SyncHealth,
    pub storage: SyncCapabilityState,
    pub source: SyncCapabilityState,
    pub sink: SyncCapabilityState,
    pub signer: SyncCapabilityState,
    pub outbox: SyncOutboxStatus,
    pub projections: SyncProjectionStatus,
}

impl SyncStatusReceipt {
    /// Rejects status receipts from an unsupported operation generation.
    pub const fn validate(&self) -> Result<(), Error> {
        if self.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(Error::UnsupportedOperationSchemaVersion {
                operation_id: OperationId::SyncStatus,
                version: self.schema_version,
            });
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SyncStatusReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u16,
            health: SyncHealth,
            storage: SyncCapabilityState,
            source: SyncCapabilityState,
            sink: SyncCapabilityState,
            signer: SyncCapabilityState,
            outbox: SyncOutboxStatus,
            projections: SyncProjectionStatus,
        }
        let wire = Wire::deserialize(deserializer)?;
        let receipt = Self {
            schema_version: wire.schema_version,
            health: wire.health,
            storage: wire.storage,
            source: wire.source,
            sink: wire.sink,
            signer: wire.signer,
            outbox: wire.outbox,
            projections: wire.projections,
        };
        receipt.validate().map_err(serde::de::Error::custom)?;
        Ok(receipt)
    }
}

macro_rules! operation_ids {
    ($( $variant:ident => $value:literal ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum OperationId {
            $( $variant, )+
        }

        impl OperationId {
            pub fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $value, )+
                }
            }

            pub fn parse(value: &str) -> Result<Self, Error> {
                match value {
                    $( $value => Ok(Self::$variant), )+
                    _ => Err(Error::UnknownOperationId {
                        operation_id: value.to_string(),
                    }),
                }
            }

            pub fn request_schema_id(self) -> &'static str {
                match self {
                    $( Self::$variant => concat!("radroots.runtime.", $value, ".request.v1"), )+
                }
            }

            pub fn receipt_schema_id(self) -> &'static str {
                match self {
                    $( Self::$variant => concat!("radroots.runtime.", $value, ".receipt.v1"), )+
                }
            }
        }

        #[cfg(feature = "serde")]
        impl serde::Serialize for OperationId {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for OperationId {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                Self::parse(value.as_str()).map_err(serde::de::Error::custom)
            }
        }
    };
}

operation_ids! {
    ProfileInspect => "profile.inspect",
    ProfileReset => "profile.reset",
    AccountCreate => "account.create",
    AccountImport => "account.import",
    AccountSelect => "account.select",
    AccountList => "account.list",
    AccountRemove => "account.remove",
    SignerStatus => "signer.status",
    StoreInspect => "store.inspect",
    StoreBackup => "store.backup",
    StoreRestore => "store.restore",
    FarmCreate => "farm.create",
    FarmUpdate => "farm.update",
    FarmPublish => "farm.publish",
    FarmGet => "farm.get",
    FarmList => "farm.list",
    ListingCreate => "listing.create",
    ListingUpdate => "listing.update",
    ListingPublish => "listing.publish",
    ListingPause => "listing.pause",
    ListingWithdraw => "listing.withdraw",
    ListingGet => "listing.get",
    ListingList => "listing.list",
    MarketPull => "market.pull",
    MarketSearch => "market.search",
    MarketGet => "market.get",
    BasketCreate => "basket.create",
    BasketGet => "basket.get",
    BasketList => "basket.list",
    BasketItemAdd => "basket.item.add",
    BasketItemUpdate => "basket.item.update",
    BasketItemRemove => "basket.item.remove",
    BasketQuote => "basket.quote",
    TradeProposalSubmit => "trade.proposal.submit",
    TradeRevisionPropose => "trade.revision.propose",
    TradeCandidateDecide => "trade.candidate.decide",
    TradeCancellationSubmit => "trade.cancellation.submit",
    TradeOperationResume => "trade.operation.resume",
    TradeGet => "trade.get",
    TradeList => "trade.list",
    TradeEvidenceRefresh => "trade.evidence.refresh",
    TradeEvidenceInspect => "trade.evidence.inspect",
    TradePrivateArtifactSeal => "trade.private_artifact.seal",
    TradePrivateArtifactOpen => "trade.private_artifact.open",
    TradePrivateArtifactDelete => "trade.private_artifact.delete",
    ValidationStatus => "validation.status",
    SyncStatus => "sync.status",
    SyncPull => "sync.pull",
    SyncPush => "sync.push",
    HealthInspect => "health.inspect",
    TransportCapabilityList => "transport.capability.list",
    TransportConfigInspect => "transport.config.inspect",
    TransportConfigUpdate => "transport.config.update",
    TransportStatusInspect => "transport.status.inspect",
    TransportDeliveryInspect => "transport.delivery.inspect",
    TransportDeliveryRetry => "transport.delivery.retry",
    DiagnosticsInspect => "diagnostics.inspect",
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mutability {
    Read,
    Mutation,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Risk {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ApprovalRequirement {
    None,
    ConditionalOrRequiredByMode,
    Required,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignerRequirement {
    None,
    Required,
    ConditionalRelayAuth,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdempotencyPolicy {
    Forbidden,
    RequiredUuidV7,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DryRunSupport {
    NotApplicable,
    PureLocalPlan,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeadlinePolicy {
    DefaultBounded,
    OperationDeclared,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrivacyEffect {
    None,
    PublicEvent,
    PrivateCoordination,
    PrivateStore,
    BackupRestore,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectionEffect {
    None,
    ReadsProjection,
    WritesProjection,
    MayUpdateProjection,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransportRoute {
    pub local: bool,
    pub nostr: bool,
    pub reticulum: bool,
    pub deliver: bool,
    pub fetch: bool,
    pub synchronize: bool,
    pub diagnostics: bool,
}

impl TransportRoute {
    pub const fn none() -> Self {
        Self {
            local: false,
            nostr: false,
            reticulum: false,
            deliver: false,
            fetch: false,
            synchronize: false,
            diagnostics: false,
        }
    }

    pub const fn local() -> Self {
        Self {
            local: true,
            nostr: false,
            reticulum: false,
            deliver: false,
            fetch: false,
            synchronize: false,
            diagnostics: false,
        }
    }

    pub const fn delivery() -> Self {
        Self {
            local: false,
            nostr: true,
            reticulum: true,
            deliver: true,
            fetch: false,
            synchronize: false,
            diagnostics: false,
        }
    }

    pub const fn fetch() -> Self {
        Self {
            local: false,
            nostr: true,
            reticulum: true,
            deliver: false,
            fetch: true,
            synchronize: true,
            diagnostics: false,
        }
    }

    pub const fn diagnostics() -> Self {
        Self {
            local: true,
            nostr: true,
            reticulum: true,
            deliver: false,
            fetch: false,
            synchronize: false,
            diagnostics: true,
        }
    }

    pub fn includes_transport(self, kind: TransportKind) -> bool {
        match kind {
            TransportKind::LOCAL => self.local,
            TransportKind::NOSTR => self.nostr,
            TransportKind::RETICULUM => self.reticulum,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationDescriptor {
    pub operation_id: OperationId,
    pub schema_version: u16,
    pub mutability: Mutability,
    pub risk: Risk,
    pub approval: ApprovalRequirement,
    pub signer: SignerRequirement,
    pub transport_capability: TransportRoute,
    pub idempotency: IdempotencyPolicy,
    pub dry_run: DryRunSupport,
    pub deadline: DeadlinePolicy,
    pub privacy: PrivacyEffect,
    pub projection: ProjectionEffect,
    pub maturity: Maturity,
}

impl OperationDescriptor {
    pub fn request_schema_id(self) -> &'static str {
        self.operation_id.request_schema_id()
    }

    pub fn receipt_schema_id(self) -> &'static str {
        self.operation_id.receipt_schema_id()
    }
}

struct DescriptorSpec {
    operation_id: OperationId,
    mutability: Mutability,
    risk: Risk,
    approval: ApprovalRequirement,
    signer: SignerRequirement,
    transport_capability: TransportRoute,
    idempotency: IdempotencyPolicy,
    dry_run: DryRunSupport,
    privacy: PrivacyEffect,
    projection: ProjectionEffect,
}

const fn read(
    operation_id: OperationId,
    risk: Risk,
    transport_capability: TransportRoute,
    privacy: PrivacyEffect,
    projection: ProjectionEffect,
) -> OperationDescriptor {
    descriptor(DescriptorSpec {
        operation_id,
        mutability: Mutability::Read,
        risk,
        approval: ApprovalRequirement::None,
        signer: SignerRequirement::None,
        transport_capability,
        idempotency: IdempotencyPolicy::Forbidden,
        dry_run: DryRunSupport::NotApplicable,
        privacy,
        projection,
    })
}

const fn mutation(
    operation_id: OperationId,
    risk: Risk,
    approval: ApprovalRequirement,
    signer: SignerRequirement,
    transport_capability: TransportRoute,
    privacy: PrivacyEffect,
    projection: ProjectionEffect,
) -> OperationDescriptor {
    descriptor(DescriptorSpec {
        operation_id,
        mutability: Mutability::Mutation,
        risk,
        approval,
        signer,
        transport_capability,
        idempotency: IdempotencyPolicy::RequiredUuidV7,
        dry_run: DryRunSupport::PureLocalPlan,
        privacy,
        projection,
    })
}

const fn descriptor(spec: DescriptorSpec) -> OperationDescriptor {
    OperationDescriptor {
        operation_id: spec.operation_id,
        schema_version: OPERATION_SCHEMA_VERSION,
        mutability: spec.mutability,
        risk: spec.risk,
        approval: spec.approval,
        signer: spec.signer,
        transport_capability: spec.transport_capability,
        idempotency: spec.idempotency,
        dry_run: spec.dry_run,
        deadline: DeadlinePolicy::DefaultBounded,
        privacy: spec.privacy,
        projection: spec.projection,
        maturity: Maturity::Stable,
    }
}

pub const CATALOG: &[OperationDescriptor] = &[
    read(
        OperationId::ProfileInspect,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::ProfileReset,
        Risk::Critical,
        ApprovalRequirement::Required,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::AccountCreate,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::AccountImport,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::AccountSelect,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    read(
        OperationId::AccountList,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::AccountRemove,
        Risk::Critical,
        ApprovalRequirement::Required,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    read(
        OperationId::SignerStatus,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::None,
        ProjectionEffect::None,
    ),
    read(
        OperationId::StoreInspect,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::StoreBackup,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::BackupRestore,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::StoreRestore,
        Risk::Critical,
        ApprovalRequirement::Required,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::BackupRestore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::FarmCreate,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::FarmUpdate,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::FarmPublish,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::Required,
        TransportRoute::delivery(),
        PrivacyEffect::PublicEvent,
        ProjectionEffect::MayUpdateProjection,
    ),
    read(
        OperationId::FarmGet,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    read(
        OperationId::FarmList,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::ListingCreate,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::ListingUpdate,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::ListingPublish,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::Required,
        TransportRoute::delivery(),
        PrivacyEffect::PublicEvent,
        ProjectionEffect::MayUpdateProjection,
    ),
    mutation(
        OperationId::ListingPause,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::Required,
        TransportRoute::delivery(),
        PrivacyEffect::PublicEvent,
        ProjectionEffect::MayUpdateProjection,
    ),
    mutation(
        OperationId::ListingWithdraw,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::Required,
        TransportRoute::delivery(),
        PrivacyEffect::PublicEvent,
        ProjectionEffect::MayUpdateProjection,
    ),
    read(
        OperationId::ListingGet,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    read(
        OperationId::ListingList,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::MarketPull,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::ConditionalRelayAuth,
        TransportRoute::fetch(),
        PrivacyEffect::None,
        ProjectionEffect::MayUpdateProjection,
    ),
    read(
        OperationId::MarketSearch,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    read(
        OperationId::MarketGet,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::BasketCreate,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    read(
        OperationId::BasketGet,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::ReadsProjection,
    ),
    read(
        OperationId::BasketList,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::BasketItemAdd,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::BasketItemUpdate,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::BasketItemRemove,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::BasketQuote,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    mutation(
        OperationId::TradeProposalSubmit,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::Required,
        TransportRoute::delivery(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::MayUpdateProjection,
    ),
    mutation(
        OperationId::TradeRevisionPropose,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::Required,
        TransportRoute::delivery(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::MayUpdateProjection,
    ),
    mutation(
        OperationId::TradeCandidateDecide,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::Required,
        TransportRoute::delivery(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::MayUpdateProjection,
    ),
    mutation(
        OperationId::TradeCancellationSubmit,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::Required,
        TransportRoute::delivery(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::MayUpdateProjection,
    ),
    mutation(
        OperationId::TradeOperationResume,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::Required,
        TransportRoute::delivery(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::MayUpdateProjection,
    ),
    read(
        OperationId::TradeGet,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::ReadsProjection,
    ),
    read(
        OperationId::TradeList,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::TradeEvidenceRefresh,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::WritesProjection,
    ),
    read(
        OperationId::TradeEvidenceInspect,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::TradePrivateArtifactSeal,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::WritesProjection,
    ),
    read(
        OperationId::TradePrivateArtifactOpen,
        Risk::High,
        TransportRoute::local(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::TradePrivateArtifactDelete,
        Risk::Critical,
        ApprovalRequirement::Required,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateCoordination,
        ProjectionEffect::WritesProjection,
    ),
    read(
        OperationId::ValidationStatus,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    read(
        OperationId::SyncStatus,
        Risk::Low,
        TransportRoute::diagnostics(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::SyncPull,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::ConditionalRelayAuth,
        TransportRoute::fetch(),
        PrivacyEffect::None,
        ProjectionEffect::MayUpdateProjection,
    ),
    mutation(
        OperationId::SyncPush,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::ConditionalRelayAuth,
        TransportRoute::delivery(),
        PrivacyEffect::PublicEvent,
        ProjectionEffect::MayUpdateProjection,
    ),
    read(
        OperationId::HealthInspect,
        Risk::Low,
        TransportRoute::diagnostics(),
        PrivacyEffect::None,
        ProjectionEffect::None,
    ),
    read(
        OperationId::TransportCapabilityList,
        Risk::Low,
        TransportRoute::diagnostics(),
        PrivacyEffect::None,
        ProjectionEffect::None,
    ),
    read(
        OperationId::TransportConfigInspect,
        Risk::Low,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::TransportConfigUpdate,
        Risk::High,
        ApprovalRequirement::ConditionalOrRequiredByMode,
        SignerRequirement::None,
        TransportRoute::local(),
        PrivacyEffect::PrivateStore,
        ProjectionEffect::WritesProjection,
    ),
    read(
        OperationId::TransportStatusInspect,
        Risk::Low,
        TransportRoute::diagnostics(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    read(
        OperationId::TransportDeliveryInspect,
        Risk::Low,
        TransportRoute::diagnostics(),
        PrivacyEffect::None,
        ProjectionEffect::ReadsProjection,
    ),
    mutation(
        OperationId::TransportDeliveryRetry,
        Risk::Medium,
        ApprovalRequirement::None,
        SignerRequirement::ConditionalRelayAuth,
        TransportRoute::delivery(),
        PrivacyEffect::PublicEvent,
        ProjectionEffect::MayUpdateProjection,
    ),
    read(
        OperationId::DiagnosticsInspect,
        Risk::Low,
        TransportRoute::diagnostics(),
        PrivacyEffect::None,
        ProjectionEffect::None,
    ),
];

/// Returns the descriptor for an exact operation identity.
pub fn operation_descriptor(operation_id: OperationId) -> Result<OperationDescriptor, Error> {
    CATALOG
        .iter()
        .copied()
        .find(|descriptor| descriptor.operation_id == operation_id)
        .ok_or_else(|| Error::UnknownOperationId {
            operation_id: operation_id.as_str().to_string(),
        })
}

/// Validates uniqueness, schema generation, and descriptor policy invariants.
pub fn validate_catalog(descriptors: &[OperationDescriptor]) -> Result<(), Error> {
    let mut operation_ids = BTreeSet::new();
    for descriptor in descriptors {
        if !operation_ids.insert(descriptor.operation_id) {
            return Err(Error::DuplicateOperationId {
                operation_id: descriptor.operation_id,
            });
        }
        match (descriptor.mutability, descriptor.idempotency) {
            (Mutability::Read, IdempotencyPolicy::Forbidden)
            | (Mutability::Mutation, IdempotencyPolicy::RequiredUuidV7) => {}
            _ => {
                return Err(Error::CatalogInvalid {
                    message: format!(
                        "operation {} has invalid idempotency policy",
                        descriptor.operation_id.as_str()
                    ),
                });
            }
        }
        if descriptor.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(Error::UnsupportedOperationSchemaVersion {
                operation_id: descriptor.operation_id,
                version: descriptor.schema_version,
            });
        }
    }

    for required in [
        OperationId::TransportCapabilityList,
        OperationId::TransportConfigInspect,
        OperationId::TransportConfigUpdate,
        OperationId::TransportStatusInspect,
        OperationId::TransportDeliveryInspect,
        OperationId::TransportDeliveryRetry,
        OperationId::SyncStatus,
        OperationId::SyncPull,
        OperationId::SyncPush,
        OperationId::DiagnosticsInspect,
    ] {
        if !operation_ids.contains(&required) {
            return Err(Error::MissingRequiredOperation {
                operation_id: required,
            });
        }
    }

    for delivery in [
        OperationId::FarmPublish,
        OperationId::ListingPublish,
        OperationId::ListingPause,
        OperationId::ListingWithdraw,
        OperationId::TradeProposalSubmit,
        OperationId::TradeRevisionPropose,
        OperationId::TradeCandidateDecide,
        OperationId::TradeCancellationSubmit,
        OperationId::TradeOperationResume,
        OperationId::SyncPush,
        OperationId::TransportDeliveryRetry,
    ] {
        let descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.operation_id == delivery)
            .ok_or(Error::MissingRequiredOperation {
                operation_id: delivery,
            })?;
        if !descriptor.transport_capability.deliver
            || !descriptor
                .transport_capability
                .includes_transport(TransportKind::NOSTR)
            || !descriptor
                .transport_capability
                .includes_transport(TransportKind::RETICULUM)
        {
            return Err(Error::CatalogInvalid {
                message: format!(
                    "operation {} must use Nostr and Reticulum delivery capability",
                    descriptor.operation_id.as_str()
                ),
            });
        }
    }
    Ok(())
}

/// Builds the registry for every exact request and receipt schema identity.
pub fn schema_registry() -> Result<Registry, crate::schema::Error> {
    let mut schemas = Vec::with_capacity(CATALOG.len() * 2);
    for descriptor in CATALOG {
        schemas.push(SchemaDescriptor::try_new(
            descriptor.request_schema_id(),
            ModuleVersion::RuntimeV1,
        )?);
        schemas.push(SchemaDescriptor::try_new(
            descriptor.receipt_schema_id(),
            ModuleVersion::RuntimeV1,
        )?);
    }
    Registry::try_new(schemas)
}

/// Runtime operation catalog validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// An operation identity appears more than once.
    DuplicateOperationId {
        /// Duplicated operation identity.
        operation_id: OperationId,
    },
    /// A required operation is missing from a candidate catalog.
    MissingRequiredOperation {
        /// Missing operation identity.
        operation_id: OperationId,
    },
    /// An operation identity is unknown.
    UnknownOperationId {
        /// Rejected identity.
        operation_id: String,
    },
    /// An operation declares a non-V1 schema generation.
    UnsupportedOperationSchemaVersion {
        /// Operation with the incompatible version.
        operation_id: OperationId,
        /// Rejected version.
        version: u16,
    },
    /// A descriptor invariant is violated.
    CatalogInvalid {
        /// Secret-safe validation diagnostic.
        message: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateOperationId { operation_id } => {
                write!(
                    formatter,
                    "duplicate operation id {}",
                    operation_id.as_str()
                )
            }
            Self::MissingRequiredOperation { operation_id } => {
                write!(
                    formatter,
                    "missing required operation {}",
                    operation_id.as_str()
                )
            }
            Self::UnknownOperationId { operation_id } => {
                write!(formatter, "unknown operation id {operation_id}")
            }
            Self::UnsupportedOperationSchemaVersion {
                operation_id,
                version,
            } => write!(
                formatter,
                "unsupported operation schema version {version} for {}",
                operation_id.as_str()
            ),
            Self::CatalogInvalid { message } => formatter.write_str(message),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_unique_and_preserves_every_v1_operation() {
        validate_catalog(CATALOG).expect("runtime operation catalog");
        assert_eq!(CATALOG.len(), 57);

        let identities = CATALOG
            .iter()
            .map(|descriptor| descriptor.operation_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(identities.len(), CATALOG.len());
        for descriptor in CATALOG {
            assert_eq!(
                OperationId::parse(descriptor.operation_id.as_str()),
                Ok(descriptor.operation_id)
            );
            assert!(
                descriptor
                    .request_schema_id()
                    .starts_with("radroots.runtime.")
            );
            assert!(descriptor.request_schema_id().ends_with(".request.v1"));
            assert!(
                descriptor
                    .receipt_schema_id()
                    .starts_with("radroots.runtime.")
            );
            assert!(descriptor.receipt_schema_id().ends_with(".receipt.v1"));
            assert_eq!(
                operation_descriptor(descriptor.operation_id),
                Ok(*descriptor)
            );
        }
    }

    #[test]
    fn parser_is_exact_and_rejects_retired_preview_names() {
        assert_eq!(
            OperationId::parse("trade.proposal.submit"),
            Ok(OperationId::TradeProposalSubmit)
        );
        assert_eq!(
            OperationId::parse("runtime.unknown")
                .expect_err("unknown")
                .to_string(),
            "unknown operation id runtime.unknown"
        );
        for value in [
            ["sync.try_reticulum", "_preview_now"].concat(),
            ["transport.reticulum", "_preview.status"].concat(),
            ["transport.", "hybrid", ".publish"].concat(),
            ["radrootsd.", "proxy", ".publish"].concat(),
        ] {
            assert!(OperationId::parse(value.as_str()).is_err());
        }
    }

    #[test]
    fn schema_ids_and_registry_preserve_v1_vectors() {
        let profile = operation_descriptor(OperationId::ProfileInspect).expect("profile");
        assert_eq!(
            profile.request_schema_id(),
            "radroots.runtime.profile.inspect.request.v1"
        );
        assert_eq!(
            profile.receipt_schema_id(),
            "radroots.runtime.profile.inspect.receipt.v1"
        );

        let registry = schema_registry().expect("runtime schema registry");
        assert_eq!(registry.len(), CATALOG.len() * 2);
        assert!(
            registry
                .descriptors()
                .iter()
                .all(|descriptor| descriptor.module() == ModuleVersion::RuntimeV1)
        );
    }

    #[test]
    fn validation_rejects_duplicates_and_policy_drift() {
        assert_eq!(
            validate_catalog(&[CATALOG[0], CATALOG[0]]),
            Err(Error::DuplicateOperationId {
                operation_id: OperationId::ProfileInspect,
            })
        );

        let mut invalid = CATALOG.to_vec();
        invalid[0].idempotency = IdempotencyPolicy::RequiredUuidV7;
        assert_eq!(
            validate_catalog(invalid.as_slice()),
            Err(Error::CatalogInvalid {
                message: "operation profile.inspect has invalid idempotency policy".into(),
            })
        );

        let mut invalid = CATALOG.to_vec();
        invalid[1].idempotency = IdempotencyPolicy::Forbidden;
        assert!(matches!(
            validate_catalog(&invalid),
            Err(Error::CatalogInvalid { .. })
        ));

        let mut invalid = CATALOG.to_vec();
        invalid[0].schema_version = 2;
        assert_eq!(
            validate_catalog(&invalid),
            Err(Error::UnsupportedOperationSchemaVersion {
                operation_id: OperationId::ProfileInspect,
                version: 2,
            })
        );

        for required in [
            OperationId::TransportCapabilityList,
            OperationId::TransportConfigInspect,
            OperationId::TransportConfigUpdate,
            OperationId::TransportStatusInspect,
            OperationId::TransportDeliveryInspect,
            OperationId::TransportDeliveryRetry,
            OperationId::SyncStatus,
            OperationId::SyncPull,
            OperationId::SyncPush,
            OperationId::DiagnosticsInspect,
        ] {
            let missing = CATALOG
                .iter()
                .copied()
                .filter(|descriptor| descriptor.operation_id != required)
                .collect::<Vec<_>>();
            assert_eq!(
                validate_catalog(&missing),
                Err(Error::MissingRequiredOperation {
                    operation_id: required,
                })
            );
        }

        for delivery in [
            OperationId::FarmPublish,
            OperationId::ListingPublish,
            OperationId::ListingPause,
            OperationId::ListingWithdraw,
            OperationId::TradeProposalSubmit,
            OperationId::TradeRevisionPropose,
            OperationId::TradeCandidateDecide,
            OperationId::TradeCancellationSubmit,
            OperationId::TradeOperationResume,
        ] {
            let missing = CATALOG
                .iter()
                .copied()
                .filter(|descriptor| descriptor.operation_id != delivery)
                .collect::<Vec<_>>();
            assert_eq!(
                validate_catalog(&missing),
                Err(Error::MissingRequiredOperation {
                    operation_id: delivery,
                })
            );
        }

        let mut invalid = CATALOG.to_vec();
        let delivery = invalid
            .iter_mut()
            .find(|descriptor| descriptor.operation_id == OperationId::FarmPublish)
            .expect("delivery descriptor");
        delivery.transport_capability.deliver = false;
        assert!(matches!(
            validate_catalog(&invalid),
            Err(Error::CatalogInvalid { .. })
        ));
    }

    #[test]
    fn route_constructors_and_errors_cover_all_variants() {
        let none = TransportRoute::none();
        assert!(!none.includes_transport(TransportKind::LOCAL));
        assert!(!none.includes_transport(TransportKind::NOSTR));
        assert!(!none.includes_transport(TransportKind::RETICULUM));
        assert!(!none.includes_transport(TransportKind::parse("future").expect("custom")));
        assert!(TransportRoute::local().includes_transport(TransportKind::LOCAL));
        assert!(TransportRoute::delivery().includes_transport(TransportKind::NOSTR));
        assert!(TransportRoute::delivery().includes_transport(TransportKind::RETICULUM));
        assert!(TransportRoute::fetch().fetch);
        assert!(TransportRoute::diagnostics().diagnostics);

        let errors = [
            Error::DuplicateOperationId {
                operation_id: OperationId::ProfileInspect,
            },
            Error::MissingRequiredOperation {
                operation_id: OperationId::SyncStatus,
            },
            Error::UnknownOperationId {
                operation_id: "unknown".to_owned(),
            },
            Error::UnsupportedOperationSchemaVersion {
                operation_id: OperationId::SyncStatus,
                version: 2,
            },
            Error::CatalogInvalid {
                message: "invalid catalog".to_owned(),
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }

        let invalid = SyncStatusReceipt {
            schema_version: 2,
            health: SyncHealth::Unavailable,
            storage: SyncCapabilityState::Unsupported,
            source: SyncCapabilityState::Compiled,
            sink: SyncCapabilityState::Configured,
            signer: SyncCapabilityState::Degraded,
            outbox: SyncOutboxStatus {
                pending: 0,
                leased: 0,
                retryable: 0,
                satisfied: 0,
                exhausted: 0,
            },
            projections: SyncProjectionStatus {
                ready: 0,
                invalidated: 0,
                rebuilding: 0,
                failed: 0,
                untracked: 0,
            },
        };
        assert_eq!(
            invalid.validate(),
            Err(Error::UnsupportedOperationSchemaVersion {
                operation_id: OperationId::SyncStatus,
                version: 2,
            })
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serialized_descriptor_vector_is_exact() {
        let descriptor = operation_descriptor(OperationId::ProfileInspect).expect("profile");
        assert_eq!(
            serde_json::to_value(descriptor).expect("descriptor JSON"),
            serde_json::json!({
                "operation_id": "profile.inspect",
                "schema_version": 1,
                "mutability": "read",
                "risk": "low",
                "approval": "none",
                "signer": "none",
                "transport_capability": {
                    "local": true,
                    "nostr": false,
                    "reticulum": false,
                    "deliver": false,
                    "fetch": false,
                    "synchronize": false,
                    "diagnostics": false
                },
                "idempotency": "forbidden",
                "dry_run": "not_applicable",
                "deadline": "default_bounded",
                "privacy": "private_store",
                "projection": "reads_projection",
                "maturity": "stable"
            })
        );
        assert_eq!(
            serde_json::to_string(&OperationId::ProfileInspect).expect("operation JSON"),
            "\"profile.inspect\""
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn sync_status_receipt_is_typed_and_rejects_wire_drift() {
        let receipt = SyncStatusReceipt {
            schema_version: OPERATION_SCHEMA_VERSION,
            health: SyncHealth::Degraded,
            storage: SyncCapabilityState::Available,
            source: SyncCapabilityState::Available,
            sink: SyncCapabilityState::Degraded,
            signer: SyncCapabilityState::Configured,
            outbox: SyncOutboxStatus {
                pending: 1,
                leased: 2,
                retryable: 3,
                satisfied: 4,
                exhausted: 5,
            },
            projections: SyncProjectionStatus {
                ready: 6,
                invalidated: 7,
                rebuilding: 8,
                failed: 9,
                untracked: 10,
            },
        };
        receipt.validate().expect("status receipt");
        let value = serde_json::to_value(receipt).expect("status JSON");
        assert_eq!(value["source"], "available");
        assert_eq!(value["outbox"]["retryable"], 3);
        assert_eq!(value["projections"]["untracked"], 10);
        assert_eq!(
            serde_json::from_value::<SyncStatusReceipt>(value.clone()).expect("status decode"),
            receipt
        );
        let mut invalid_version = value.clone();
        invalid_version["schema_version"] = 2.into();
        assert!(serde_json::from_value::<SyncStatusReceipt>(invalid_version).is_err());
        let mut unknown = value;
        unknown["unknown"] = true.into();
        assert!(serde_json::from_value::<SyncStatusReceipt>(unknown).is_err());
    }
}
