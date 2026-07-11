use crate::gcs_location::GcsLocationQueryBindValues;
use crate::models::ReplicaSchemaResultPass;
use crate::trade_product::TradeProductQueryBindValues;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductLocationRelation {
    pub trade_product: TradeProductQueryBindValues,
    pub gcs_location: GcsLocationQueryBindValues,
}

pub struct ITradeProductLocationResolveTs;
pub type ITradeProductLocationResolve = ReplicaSchemaResultPass;
