#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod agreement;
mod amount;
mod commerce;
mod context;
mod currency;
mod fulfillment;
mod identifier;
mod issuance;
/// Validated public-offer values; these establish no stock or command authority.
pub mod listing;
mod message;
mod money;
mod pricing;
mod quantity;
/// Exact physical stock values; these are neither authority nor a stock store.
pub mod stock;
mod time;
mod variable_price;

/// The identity-owned validated participant key; no second key type is defined.
pub use radroots_identity::PublicKey;

pub use agreement::{
    AgreedFixedPrice, AgreementLine, AgreementPrice, FulfillmentGroup, FuturePromise,
    SupplyCommitment, Terms, TermsError,
};
pub use amount::{AmountError, Decimal, PositiveDecimal};
pub use commerce::causal;
pub use commerce::{
    AgreementCommitment, CancellationProposal, CommerceContent, CommerceError, CommerceHeader,
    CommercePayload, CommerceType, FinalMeasurement, HandoverClaim, IssueCode, IssueReport,
    LineQuantity, ProposalAuthorization, ReadinessClaim, ReceiptObservation, RecordReason,
    ResidualRelease, Resolution, ResolutionCommitment, ResolutionProposal, TermsAuthorization,
    TermsProposal, TermsQuote, TermsRequest, WithdrawalAcknowledgement,
};
pub use context::{
    ActorRef, ActorRole, Approval, AuthorityRef, CheckpointRef, CommandContextData, ContextError,
    CorrelationId, Coverage, ExpectedOrder, Generation, Sequence, UnixSeconds,
};
pub use currency::{
    CURRENCY_REGISTRY_ID, CURRENCY_REGISTRY_SHA256, Currency, CurrencyError,
    SUPPORTED_CURRENCY_COUNT,
};
pub use fulfillment::{FulfillmentError, PrivateFulfillment};
pub use identifier::{
    AgreementLineId, ApprovalIntentDigest, CheckpointDigest, CommandDigest, EnvelopeEventId,
    ExecutorId, FulfillmentGroupId, GrantId, IdentifierError, ListingOptionId, MessageId,
    OperationId, OrderId, PoolId, RecordId, RouteDigest, RumorEventId, SigningAttemptId, SourceId,
    TermsDigest, ThreadId, TransactionId, WorkspaceId,
};
pub use issuance::{FIRST_ISSUANCE_MAX_SECONDS, IssuanceWindow};
pub use message::{CHAT_MAX_BYTES, ChatMessage, ChatRelation, MessageError};
pub use money::{MinorAmount, Money, MoneyError};
pub use pricing::{
    DiscountBps, FixedPrice, LinePrice, MAX_PRICED_LINES, PRICING_PROFILE, PriceTier, PricingError,
    checked_order_total,
};
pub use quantity::{ContentMeasure, Quantity, QuantityError};
pub use time::{TIME_ZONE_DATABASE_VERSION, TimeError, TimeZoneId, Window};
pub use variable_price::VariableWeight;
