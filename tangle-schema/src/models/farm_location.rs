use radroots_types::types::IResultPass;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;
use crate::farm::FarmQueryBindValues;
use crate::location_gcs::LocationGcsQueryBindValues;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct IFarmLocationRelation {
    pub farm: FarmQueryBindValues,
    pub location_gcs: LocationGcsQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "IFarmLocationResolve",
        type = "IResultPass"
    )
)]
pub struct IFarmLocationResolveTs;
pub type IFarmLocationResolve = IResultPass;
