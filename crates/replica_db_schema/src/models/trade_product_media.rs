use crate::media_image::MediaImageQueryBindValues;
use crate::trade_product::TradeProductQueryBindValues;
use radroots_types::types::IResultPass;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductMediaRelation {
    pub trade_product: TradeProductQueryBindValues,
    pub media_image: MediaImageQueryBindValues,
}

pub struct ITradeProductMediaResolveTs;
pub type ITradeProductMediaResolve = IResultPass;
