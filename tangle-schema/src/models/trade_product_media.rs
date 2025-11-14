use radroots_types::types::IResultPass;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;
use crate::trade_product::TradeProductQueryBindValues;
use crate::media_image::MediaImageQueryBindValues;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductMediaRelation {
    pub trade_product: TradeProductQueryBindValues,
    pub media_image: MediaImageQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ITradeProductMediaResolve",
        type = "IResultPass"
    )
)]
pub struct ITradeProductMediaResolveTs;
pub type ITradeProductMediaResolve = IResultPass;
