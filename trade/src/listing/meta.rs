use core::fmt;
use core::str::FromStr;

pub const MARKER_LISTING: &str = "listing";
pub const MARKER_PAYLOAD: &str = "payload";
pub const MARKER_PREVIOUS: &str = "previous";

pub const MARKER_ACCEPT_RESULT: &str = "accept_result";
pub const MARKER_INVOICE_RESULT: &str = "invoice_result";
pub const MARKER_FULFILLMENT_RESULT: &str = "fulfillment_result";
pub const MARKER_PROOF: &str = "proof";

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
pub enum TradeListingStage {
    Order,
    Accept,
    Conveyance,
    Invoice,
    Payment,
    Fulfillment,
    Receipt,
    Cancel,
    Refund,
}

impl TradeListingStage {
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            TradeListingStage::Order => "order",
            TradeListingStage::Accept => "accept",
            TradeListingStage::Conveyance => "conveyance",
            TradeListingStage::Invoice => "invoice",
            TradeListingStage::Payment => "payment",
            TradeListingStage::Fulfillment => "fulfillment",
            TradeListingStage::Receipt => "receipt",
            TradeListingStage::Cancel => "cancel",
            TradeListingStage::Refund => "refund",
        }
    }
}

impl fmt::Display for TradeListingStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeListingStageParseError {
    UnknownStage,
}

impl fmt::Display for TradeListingStageParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradeListingStageParseError::UnknownStage => {
                write!(f, "unknown trade listing stage")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TradeListingStageParseError {}

impl FromStr for TradeListingStage {
    type Err = TradeListingStageParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "order" => Ok(Self::Order),
            "accept" => Ok(Self::Accept),
            "conveyance" => Ok(Self::Conveyance),
            "invoice" => Ok(Self::Invoice),
            "payment" => Ok(Self::Payment),
            "fulfillment" => Ok(Self::Fulfillment),
            "receipt" => Ok(Self::Receipt),
            "cancel" => Ok(Self::Cancel),
            "refund" => Ok(Self::Refund),
            _ => Err(TradeListingStageParseError::UnknownStage),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TradeListingStage, TradeListingStageParseError};

    #[test]
    fn stage_roundtrip() {
        let cases = [
            (TradeListingStage::Order, "order"),
            (TradeListingStage::Accept, "accept"),
            (TradeListingStage::Conveyance, "conveyance"),
            (TradeListingStage::Invoice, "invoice"),
            (TradeListingStage::Payment, "payment"),
            (TradeListingStage::Fulfillment, "fulfillment"),
            (TradeListingStage::Receipt, "receipt"),
            (TradeListingStage::Cancel, "cancel"),
            (TradeListingStage::Refund, "refund"),
        ];

        for (stage, name) in cases {
            assert_eq!(stage.as_str(), name);
            assert_eq!(stage.to_string(), name);
            assert_eq!(name.parse::<TradeListingStage>().unwrap(), stage);
        }
    }

    #[test]
    fn stage_parse_rejects_unknown() {
        let err = "unknown".parse::<TradeListingStage>().unwrap_err();
        assert_eq!(err, TradeListingStageParseError::UnknownStage);
    }
}
