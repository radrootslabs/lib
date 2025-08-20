use core::fmt;
use core::str::FromStr;

pub const MARKER_LISTING: &str = "listing";
pub const MARKER_PAYLOAD: &str = "payload";
pub const MARKER_PREVIOUS: &str = "previous";

pub const MARKER_ACCEPT_RESULT: &str = "accept_result";
pub const MARKER_INVOICE_RESULT: &str = "invoice_result";
pub const MARKER_FULFILLMENT_RESULT: &str = "fulfillment_result";
pub const MARKER_PROOF: &str = "proof";

#[typeshare::typeshare]
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

impl fmt::Display for TradeListingStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TradeListingStage::Order => "order",
            TradeListingStage::Accept => "accept",
            TradeListingStage::Conveyance => "conveyance",
            TradeListingStage::Invoice => "invoice",
            TradeListingStage::Payment => "payment",
            TradeListingStage::Fulfillment => "fulfillment",
            TradeListingStage::Receipt => "receipt",
            TradeListingStage::Cancel => "cancel",
            TradeListingStage::Refund => "refund",
        })
    }
}

impl FromStr for TradeListingStage {
    type Err = ();
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
            _ => Err(()),
        }
    }
}
