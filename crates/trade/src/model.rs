//! Native trade-domain projections and validated business identifiers.
//!
//! Protocol trade identity remains owned by [`radroots_event::trade`]. This
//! module owns projection state plus the semantically distinct [`OrderId`].

pub use crate::trade_contract_v1::{
    RadrootsTradeAgreementClaimV1, RadrootsTradeAgreementStateV1, RadrootsTradeAttestationStateV1,
    RadrootsTradeConflictStateV1, RadrootsTradeFulfillmentStateV1, RadrootsTradeNegotiationStateV1,
    RadrootsTradePaymentStateV1, RadrootsTradePrivateTermsStateV1, RadrootsTradeProjectionV1,
};

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use std::string::String;

use core::{fmt, str::FromStr};

/// Maximum encoded length of a human or business order identifier.
pub const ORDER_ID_MAX_LEN: usize = 128;

/// A human or business-workflow order identifier.
///
/// This identifier is deliberately distinct from the canonical protocol
/// [`radroots_event::trade::TradeId`]. No conversion exists between them.
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrderId(String);

impl OrderId {
    /// Parses a non-empty, whitespace-free business identifier.
    pub fn parse(value: impl AsRef<str>) -> Result<Self, OrderIdError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(OrderIdError::Empty);
        }
        if value.len() > ORDER_ID_MAX_LEN {
            return Err(OrderIdError::TooLong {
                max: ORDER_ID_MAX_LEN,
                actual: value.len(),
            });
        }
        if value.trim() != value
            || value
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
        {
            return Err(OrderIdError::InvalidCharacter);
        }
        Ok(Self(value.to_string()))
    }

    /// Returns the validated business identifier.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Consumes the identifier and returns its owned representation.
    #[inline]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for OrderId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<OrderId> for String {
    fn from(value: OrderId) -> Self {
        value.into_string()
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for OrderId {
    type Err = OrderIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for OrderId {
    type Error = OrderIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for OrderId {
    type Error = OrderIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for OrderId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for OrderId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Business order identifier validation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderIdError {
    /// The identifier was empty.
    Empty,
    /// The identifier exceeded its bounded encoded length.
    TooLong { max: usize, actual: usize },
    /// The identifier contained whitespace or a control character.
    InvalidCharacter,
}

impl fmt::Display for OrderIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("order identifier is empty"),
            Self::TooLong { max, actual } => write!(
                formatter,
                "order identifier length {actual} exceeds maximum length {max}"
            ),
            Self::InvalidCharacter => {
                formatter.write_str("order identifier contains an invalid character")
            }
        }
    }
}

impl core::error::Error for OrderIdError {}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::{ORDER_ID_MAX_LEN, OrderId, OrderIdError};

    #[test]
    fn order_id_parses_business_values_without_becoming_a_protocol_trade_id() {
        let order_id = OrderId::parse("order-1").expect("business order id");

        assert_eq!(order_id.as_str(), "order-1");
        assert_eq!(order_id.as_ref(), "order-1");
        assert_eq!(order_id.to_string(), "order-1");
        assert_eq!(OrderId::try_from("order-1").unwrap(), order_id);
        assert_eq!(OrderId::from_str("order-1").unwrap(), order_id);
        assert_eq!(String::from(order_id.clone()), order_id.into_string());
        assert!(radroots_event::trade::TradeId::parse("order-1").is_err());
    }

    #[test]
    fn order_id_rejects_invalid_business_values() {
        assert_eq!(OrderId::parse("").unwrap_err(), OrderIdError::Empty);
        assert_eq!(
            OrderId::parse("x".repeat(ORDER_ID_MAX_LEN + 1)).unwrap_err(),
            OrderIdError::TooLong {
                max: ORDER_ID_MAX_LEN,
                actual: ORDER_ID_MAX_LEN + 1,
            }
        );
        for value in [" order-1", "order-1 ", "order 1", "order\n1"] {
            assert_eq!(
                OrderId::parse(value).unwrap_err(),
                OrderIdError::InvalidCharacter
            );
        }
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn order_id_serde_round_trip_preserves_validation() {
        let order_id = OrderId::parse("order-1").expect("business order id");
        let encoded = serde_json::to_string(&order_id).expect("serialize order id");

        assert_eq!(encoded, "\"order-1\"");
        assert_eq!(
            serde_json::from_str::<OrderId>(&encoded).expect("deserialize order id"),
            order_id
        );
        assert!(serde_json::from_str::<OrderId>("\"bad order\"").is_err());
    }
}
