//! Exact captured-context descriptors. These values authenticate no principal
//! and grant no command permission, approval or stock authority.

use crate::{
    ApprovalIntentDigest, CheckpointDigest, ExecutorId, GrantId, OperationId, OrderId, RecordId,
    RouteDigest, SourceId, WorkspaceId,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::{fmt, str::FromStr};
use radroots_identity::PublicKey;

/// Maximum sequence or authority generation supported by every governed host.
const MAX_SEQUENCE: u64 = i64::MAX as u64;
const MAX_UNIX_SECONDS: u64 = 253_402_300_799;
const MAX_MISSING: usize = 128;

/// A fixed descriptor failure with no retained caller input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContextError {
    /// An integer has a noncanonical decimal representation.
    InvalidInteger,
    /// A value exceeds the shared bound or requires a positive value.
    OutOfRange,
    /// Bounded diagnostic or scope text is invalid.
    InvalidText,
    /// A checkpoint has more missing references than supported.
    TooManyReferences,
    /// Satisfied coverage cannot retain missing references.
    InconsistentCoverage,
}
impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInteger => "context integer is not canonical",
            Self::OutOfRange => "context value is out of range",
            Self::InvalidText => "context text is invalid",
            Self::TooManyReferences => "checkpoint reference limit exceeded",
            Self::InconsistentCoverage => "checkpoint coverage is inconsistent",
        })
    }
}
impl core::error::Error for ContextError {}

fn integer(value: &str, max: u64, positive: bool) -> Result<u64, ContextError> {
    if value.is_empty()
        || value.len() > 20
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(ContextError::InvalidInteger);
    }
    let number = value.parse::<u64>().map_err(|_| ContextError::OutOfRange)?;
    if number > max || (positive && number == 0) {
        return Err(ContextError::OutOfRange);
    }
    Ok(number)
}
macro_rules! exact_integer {
    ($name:ident,$doc:literal,$max:expr,$positive:expr) => {
        #[doc=$doc]
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(u64);
        impl $name {
            /// Checks the cross-host range without coercing or rounding.
            pub const fn new(value: u64) -> Result<Self, ContextError> {
                if value > $max || ($positive && value == 0) {
                    Err(ContextError::OutOfRange)
                } else {
                    Ok(Self(value))
                }
            }
            /// Returns the exact integer for checked host operations.
            pub const fn get(self) -> u64 {
                self.0
            }
        }
        impl FromStr for $name {
            type Err = ContextError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                integer(value, $max, $positive).map(Self)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        #[cfg(feature = "serde")]
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }
        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct Visitor;
                impl serde::de::Visitor<'_> for Visitor {
                    type Value = $name;
                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("a bounded canonical decimal string")
                    }
                    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<$name, E> {
                        value.parse().map_err(E::custom)
                    }
                }
                d.deserialize_str(Visitor)
            }
        }
    };
}
exact_integer!(
    Sequence,
    "A canonical sequence, including an explicit genesis zero.",
    MAX_SEQUENCE,
    false
);
exact_integer!(
    Generation,
    "A positive authority generation, independent of cache cursors.",
    MAX_SEQUENCE,
    true
);
exact_integer!(
    UnixSeconds,
    "Exact bounded Unix seconds serialized as a decimal string.",
    MAX_UNIX_SECONDS,
    false
);

/// A participant's requested business role; this is not permission evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[non_exhaustive]
pub enum ActorRole {
    /// The seller participant.
    Seller,
    /// The buyer participant.
    Buyer,
    /// A separately granted logistics coordinator.
    Coordinator,
}
/// An untrusted actor descriptor. Grant origin and scope require authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ActorRef {
    participant: PublicKey,
    role: ActorRole,
    organization: Option<PublicKey>,
    grant_id: Option<GrantId>,
}
impl ActorRef {
    /// Captures validated values without authenticating their relationship.
    pub const fn new(
        participant: PublicKey,
        role: ActorRole,
        organization: Option<PublicKey>,
        grant_id: Option<GrantId>,
    ) -> Self {
        Self {
            participant,
            role,
            organization,
            grant_id,
        }
    }
    /// Returns the claimed participant.
    pub const fn participant(self) -> PublicKey {
        self.participant
    }
    /// Returns the requested role.
    pub const fn role(self) -> ActorRole {
        self.role
    }
    /// Returns the represented organization, if supplied.
    pub const fn organization(self) -> Option<PublicKey> {
        self.organization
    }
    /// Returns a grant selector, never its credential.
    pub const fn grant_id(self) -> Option<GrantId> {
        self.grant_id
    }
}
/// Exact selected executor and authority epoch, without an authority certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AuthorityRef {
    executor_id: ExecutorId,
    generation: Generation,
}
impl AuthorityRef {
    /// Captures one exact selected executor/generation pair.
    pub const fn new(executor_id: ExecutorId, generation: Generation) -> Self {
        Self {
            executor_id,
            generation,
        }
    }
    /// Returns the selected executor.
    pub const fn executor_id(self) -> ExecutorId {
        self.executor_id
    }
    /// Returns the authority generation.
    pub const fn generation(self) -> Generation {
        self.generation
    }
}
/// Described checkpoint coverage, independently verified by the execution owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[non_exhaustive]
pub enum Coverage {
    /// Coverage is incomplete or unknown within its scope.
    Partial,
    /// The named scope has no reported missing references.
    ScopeSatisfied,
}
/// A bounded checkpoint descriptor; it is neither a stock authority nor a proof.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CheckpointRef {
    digest: CheckpointDigest,
    sequence: Sequence,
    coverage: Coverage,
    missing: Box<[SourceId]>,
}
impl CheckpointRef {
    /// Captures bounded, internally consistent coverage claims.
    pub fn new(
        digest: CheckpointDigest,
        sequence: Sequence,
        coverage: Coverage,
        missing: Vec<SourceId>,
    ) -> Result<Self, ContextError> {
        if missing.len() > MAX_MISSING {
            return Err(ContextError::TooManyReferences);
        }
        if coverage == Coverage::ScopeSatisfied && !missing.is_empty() {
            return Err(ContextError::InconsistentCoverage);
        }
        Ok(Self {
            digest,
            sequence,
            coverage,
            missing: missing.into_boxed_slice(),
        })
    }
    /// Returns the claimed checkpoint digest.
    pub const fn digest(&self) -> CheckpointDigest {
        self.digest
    }
    /// Returns its exact sequence, including explicit genesis.
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }
    /// Returns the described coverage.
    pub const fn coverage(&self) -> Coverage {
        self.coverage
    }
    /// Borrows the bounded missing-source list.
    pub fn missing(&self) -> &[SourceId] {
        &self.missing
    }
}
/// A bounded diagnostic reference excluded from semantic idempotency.
#[derive(Clone, Eq, PartialEq)]
pub struct CorrelationId(Box<str>);
impl CorrelationId {
    /// Retains at most 128 UTF-8 bytes, with no terminal/control characters.
    pub fn new(value: impl Into<String>) -> Result<Self, ContextError> {
        let value = value.into();
        if value.len() > 128 || value.chars().any(char::is_control) {
            return Err(ContextError::InvalidText);
        }
        Ok(Self(value.into_boxed_str()))
    }
    /// Borrows the explicit diagnostic value for an authorized presentation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CorrelationId(<redacted>)")
    }
}
/// Untrusted approval data. The descriptor alone proves no consent or grant.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Approval {
    intent_digest: ApprovalIntentDigest,
    scope: Box<str>,
    expires_at: UnixSeconds,
}
impl Approval {
    /// Checks representation only; the host must authenticate origin and scope.
    pub fn new(
        intent_digest: ApprovalIntentDigest,
        scope: impl Into<String>,
        expires_at: UnixSeconds,
    ) -> Result<Self, ContextError> {
        let scope = scope.into();
        if scope.is_empty() || scope.len() > 160 || scope.chars().any(char::is_control) {
            return Err(ContextError::InvalidText);
        }
        if expires_at.get() == 0 {
            return Err(ContextError::OutOfRange);
        }
        Ok(Self {
            intent_digest,
            scope: scope.into_boxed_str(),
            expires_at,
        })
    }
    /// Returns the exact displayed-intent commitment.
    pub const fn intent_digest(&self) -> ApprovalIntentDigest {
        self.intent_digest
    }
    /// Borrows the bounded requested consent scope.
    pub fn scope(&self) -> &str {
        &self.scope
    }
    /// Returns the captured consent expiry.
    pub const fn expires_at(&self) -> UnixSeconds {
        self.expires_at
    }
}
impl fmt::Debug for Approval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Approval(<untrusted descriptor>)")
    }
}
/// Captured order preconditions, independent of whether the history is present.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ExpectedOrder {
    order_id: OrderId,
    agreement: Option<RecordId>,
    seller_sequence: Sequence,
    buyer_head: Option<RecordId>,
    checkpoint: CheckpointRef,
}
impl ExpectedOrder {
    /// Captures exact expected values without querying or installing an order.
    pub const fn new(
        order_id: OrderId,
        agreement: Option<RecordId>,
        seller_sequence: Sequence,
        buyer_head: Option<RecordId>,
        checkpoint: CheckpointRef,
    ) -> Self {
        Self {
            order_id,
            agreement,
            seller_sequence,
            buyer_head,
            checkpoint,
        }
    }
    /// Returns the order identity.
    pub const fn order_id(&self) -> OrderId {
        self.order_id
    }
    /// Returns the expected installed agreement.
    pub const fn agreement(&self) -> Option<RecordId> {
        self.agreement
    }
    /// Returns the exact expected seller sequence.
    pub const fn seller_sequence(&self) -> Sequence {
        self.seller_sequence
    }
    /// Returns the expected buyer head.
    pub const fn buyer_head(&self) -> Option<RecordId> {
        self.buyer_head
    }
    /// Borrows the expected evidence checkpoint.
    pub const fn checkpoint(&self) -> &CheckpointRef {
        &self.checkpoint
    }
}
/// Complete wire-shaped captured-context data. Deserializing it never constructs
/// an authenticated execution context or grants any permission.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CommandContextData {
    operation_id: OperationId,
    actor: ActorRef,
    workspace_id: WorkspaceId,
    authority: AuthorityRef,
    route_digest: RouteDigest,
    approval: Option<Approval>,
    expected: Option<ExpectedOrder>,
    not_after: UnixSeconds,
    correlation_id: Option<CorrelationId>,
}
impl CommandContextData {
    /// Captures every explicit descriptor field, with no ambient effects.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation_id: OperationId,
        actor: ActorRef,
        workspace_id: WorkspaceId,
        authority: AuthorityRef,
        route_digest: RouteDigest,
        approval: Option<Approval>,
        expected: Option<ExpectedOrder>,
        not_after: UnixSeconds,
        correlation_id: Option<CorrelationId>,
    ) -> Result<Self, ContextError> {
        if not_after.get() == 0 {
            return Err(ContextError::OutOfRange);
        }
        Ok(Self {
            operation_id,
            actor,
            workspace_id,
            authority,
            route_digest,
            approval,
            expected,
            not_after,
            correlation_id,
        })
    }
    /// Returns the full caller-retained operation identity.
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    /// Returns claimed actor/grant data, without authenticating it.
    pub const fn actor(&self) -> ActorRef {
        self.actor
    }
    /// Returns the selected workspace.
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    /// Returns the exact executor and epoch.
    pub const fn authority(&self) -> AuthorityRef {
        self.authority
    }
    /// Returns the immutable selected route commitment.
    pub const fn route_digest(&self) -> RouteDigest {
        self.route_digest
    }
    /// Borrows untrusted approval data.
    pub const fn approval(&self) -> Option<&Approval> {
        self.approval.as_ref()
    }
    /// Borrows captured expected order state.
    pub const fn expected(&self) -> Option<&ExpectedOrder> {
        self.expected.as_ref()
    }
    /// Returns the original admission deadline.
    pub const fn not_after(&self) -> UnixSeconds {
        self.not_after
    }
    /// Borrows the diagnostic reference, which is not idempotency.
    pub const fn correlation_id(&self) -> Option<&CorrelationId> {
        self.correlation_id.as_ref()
    }
}
impl fmt::Debug for CommandContextData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CommandContextData(<untrusted descriptor>)")
    }
}

#[cfg(feature = "serde")]
mod serialization {
    use super::*;
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    fn canonical_key<'de, D: Deserializer<'de>>(d: D) -> Result<PublicKey, D::Error> {
        struct KeyVisitor;
        impl serde::de::Visitor<'_> for KeyVisitor {
            type Value = PublicKey;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a canonical public key")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<PublicKey, E> {
                let key =
                    PublicKey::from_hex(value).map_err(|_| E::custom("invalid public key"))?;
                if key.to_hex() != value {
                    return Err(E::custom("noncanonical public key"));
                }
                Ok(key)
            }
        }
        d.deserialize_str(KeyVisitor)
    }
    fn nullable_key<'de, D: Deserializer<'de>>(d: D) -> Result<Option<PublicKey>, D::Error> {
        #[derive(Deserialize)]
        struct Key(#[serde(deserialize_with = "canonical_key")] PublicKey);
        Option::<Key>::deserialize(d).map(|value| value.map(|key| key.0))
    }

    fn object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<T, D::Error> {
        struct ObjectVisitor<T>(core::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for ObjectVisitor<T> {
            type Value = T;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<T, M::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            }
        }
        d.deserialize_map(ObjectVisitor::<T>(core::marker::PhantomData))
    }

    fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
        d: D,
    ) -> Result<Option<T>, D::Error> {
        Option::<T>::deserialize(d)
    }
    impl serde::Serialize for CorrelationId {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_str(&self.0)
        }
    }
    impl<'de> Deserialize<'de> for CorrelationId {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            Self::new(String::deserialize(d)?).map_err(D::Error::custom)
        }
    }
    impl<'de> Deserialize<'de> for AuthorityRef {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Raw {
                executor_id: ExecutorId,
                generation: Generation,
            }
            let raw: Raw = object(d)?;
            Ok(Self::new(raw.executor_id, raw.generation))
        }
    }
    impl<'de> Deserialize<'de> for ActorRef {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Raw {
                #[serde(deserialize_with = "canonical_key")]
                participant: PublicKey,
                role: ActorRole,
                #[serde(deserialize_with = "nullable_key")]
                organization: Option<PublicKey>,
                #[serde(deserialize_with = "nullable")]
                grant_id: Option<GrantId>,
            }
            let raw: Raw = object(d)?;
            Ok(Self::new(
                raw.participant,
                raw.role,
                raw.organization,
                raw.grant_id,
            ))
        }
    }
    impl<'de> Deserialize<'de> for CheckpointRef {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Raw {
                digest: CheckpointDigest,
                sequence: Sequence,
                coverage: Coverage,
                missing: Vec<SourceId>,
            }
            let raw: Raw = object(d)?;
            Self::new(raw.digest, raw.sequence, raw.coverage, raw.missing).map_err(D::Error::custom)
        }
    }
    impl<'de> Deserialize<'de> for Approval {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Raw {
                intent_digest: ApprovalIntentDigest,
                scope: String,
                expires_at: UnixSeconds,
            }
            let raw: Raw = object(d)?;
            Self::new(raw.intent_digest, raw.scope, raw.expires_at).map_err(D::Error::custom)
        }
    }
    impl<'de> Deserialize<'de> for ExpectedOrder {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Raw {
                order_id: OrderId,
                #[serde(deserialize_with = "nullable")]
                agreement: Option<RecordId>,
                seller_sequence: Sequence,
                #[serde(deserialize_with = "nullable")]
                buyer_head: Option<RecordId>,
                checkpoint: CheckpointRef,
            }
            let raw: Raw = object(d)?;
            Ok(Self::new(
                raw.order_id,
                raw.agreement,
                raw.seller_sequence,
                raw.buyer_head,
                raw.checkpoint,
            ))
        }
    }
    impl<'de> Deserialize<'de> for CommandContextData {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Raw {
                operation_id: OperationId,
                actor: ActorRef,
                workspace_id: WorkspaceId,
                authority: AuthorityRef,
                route_digest: RouteDigest,
                #[serde(deserialize_with = "nullable")]
                approval: Option<Approval>,
                #[serde(deserialize_with = "nullable")]
                expected: Option<ExpectedOrder>,
                not_after: UnixSeconds,
                #[serde(deserialize_with = "nullable")]
                correlation_id: Option<CorrelationId>,
            }
            let raw: Raw = object(d)?;
            Self::new(
                raw.operation_id,
                raw.actor,
                raw.workspace_id,
                raw.authority,
                raw.route_digest,
                raw.approval,
                raw.expected,
                raw.not_after,
                raw.correlation_id,
            )
            .map_err(D::Error::custom)
        }
    }
}
