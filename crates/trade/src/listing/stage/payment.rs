#![cfg_attr(not(feature = "serde"), allow(unused_attributes))]

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "kind", content = "amount")]
pub enum TradeListingPaymentProof {
    ZapEvent { id: String },
    Preimage { hex: String },
    Txid { id: String },
    ExternalRef { provider: String, ref_id: String },
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingPaymentProofRequest {
    pub invoice_result_event_id: String,
    pub proof: TradeListingPaymentProof,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingPaymentResult {
    pub verified: bool,
    pub message: Option<String>,
}
