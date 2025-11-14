use serde::Serialize;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize)]
pub struct IError<T> {
    pub err: T,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize)]
pub struct IResult<T> {
    pub result: T,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize)]
pub struct IResultList<T> {
    pub results: Vec<T>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize)]
pub struct IResultPass {
    pub pass: bool,
}

impl<T> From<T> for IError<T> {
    fn from(err: T) -> Self {
        Self { err }
    }
}
