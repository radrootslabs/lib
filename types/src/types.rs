use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[ts(export, export_to = "types.ts")]
pub struct IResult<T> {
    pub result: T,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "types.ts")]
pub struct IResultList<T> {
    pub results: Vec<T>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "types.ts")]
pub struct IResultPass {
    pub pass: bool,
}
