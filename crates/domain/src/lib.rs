#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod amount;
mod context;
mod currency;
mod identifier;
mod issuance;
/// Validated public-offer values; these establish no stock or command authority.
pub mod listing;
mod message;
mod money;
mod pricing;

/// The identity-owned validated participant key; no second key type is defined.
pub use radroots_identity::PublicKey;

pub use amount::{AmountError, Decimal, PositiveDecimal};
pub use context::{
    ActorRef, ActorRole, Approval, AuthorityRef, CheckpointRef, CommandContextData, ContextError,
    CorrelationId, Coverage, ExpectedOrder, Generation, Sequence, UnixSeconds,
};
pub use currency::{
    CURRENCY_REGISTRY_ID, CURRENCY_REGISTRY_SHA256, Currency, CurrencyError,
    SUPPORTED_CURRENCY_COUNT,
};
pub use identifier::{
    ApprovalIntentDigest, CheckpointDigest, CommandDigest, EnvelopeEventId, ExecutorId, GrantId,
    IdentifierError, ListingOptionId, MessageId, OperationId, OrderId, RecordId, RouteDigest,
    SigningAttemptId, SourceId, ThreadId, TransactionId, WorkspaceId,
};
pub use issuance::{FIRST_ISSUANCE_MAX_SECONDS, IssuanceWindow};
pub use message::{CHAT_MAX_BYTES, ChatMessage, ChatRelation, MessageError};
pub use money::{MinorAmount, Money, MoneyError};
pub use pricing::{
    DiscountBps, FixedPrice, LinePrice, MAX_PRICED_LINES, PRICING_PROFILE, PriceTier, PricingError,
    checked_order_total,
};
