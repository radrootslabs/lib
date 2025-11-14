use radroots_types::types::IResultPass;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;
use crate::trade_product::TradeProductQueryBindValues;
use crate::location_gcs::LocationGcsQueryBindValues;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct ITradeProductLocationRelation {
    pub trade_product: TradeProductQueryBindValues,
    pub location_gcs: LocationGcsQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "ITradeProductLocationResolve",
        type = "IResultPass"
    )
)]
pub struct ITradeProductLocationResolveTs;
pub type ITradeProductLocationResolve = IResultPass;
