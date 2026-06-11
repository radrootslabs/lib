use crate::gcs_location::GcsLocationQueryBindValues;
use crate::trade_product::TradeProductQueryBindValues;
use radroots_types::types::IResultPass;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductLocationRelation {
    pub trade_product: TradeProductQueryBindValues,
    pub gcs_location: GcsLocationQueryBindValues,
}

pub struct ITradeProductLocationResolveTs;
pub type ITradeProductLocationResolve = IResultPass;
