//! Exact shared values and independently typed identities.
//!
//! Construction validates representation. These values do not authenticate a
//! participant, select an executor or prove durable command admission.

pub use radroots_domain::{
    AmountError, Decimal, ExecutorId, IdentifierError, OperationId, OrderId, PositiveDecimal,
    ThreadId, WorkspaceId,
};
