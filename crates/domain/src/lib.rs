#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod amount;
mod context;
mod identifier;
mod issuance;

/// The identity-owned validated participant key; no second key type is defined.
pub use radroots_identity::PublicKey;

pub use amount::{AmountError, Decimal, PositiveDecimal};
pub use context::{
    ActorRef, ActorRole, Approval, AuthorityRef, CheckpointRef, CommandContextData, ContextError,
    CorrelationId, Coverage, ExpectedOrder, Generation, Sequence, UnixSeconds,
};
pub use identifier::{
    ApprovalIntentDigest, CheckpointDigest, CommandDigest, EnvelopeEventId, ExecutorId, GrantId,
    IdentifierError, MessageId, OperationId, OrderId, RecordId, RouteDigest, SourceId, ThreadId,
    TransactionId, WorkspaceId,
};
pub use issuance::{FIRST_ISSUANCE_MAX_SECONDS, IssuanceWindow};
