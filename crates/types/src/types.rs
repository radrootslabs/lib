use serde::Serialize;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IError<T> {
    pub err: T,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IResult<T> {
    pub result: T,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IResultList<T> {
    pub results: Vec<T>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IResultPass {
    pub pass: bool,
}

impl<T> From<T> for IError<T> {
    fn from(err: T) -> Self {
        Self { err }
    }
}

impl<T> IError<T> {
    pub fn new(err: T) -> Self {
        Self { err }
    }
}

impl<T> IResult<T> {
    pub fn new(result: T) -> Self {
        Self { result }
    }
}

impl<T> IResultList<T> {
    pub fn new(results: Vec<T>) -> Self {
        Self { results }
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

impl IResultPass {
    pub fn new(pass: bool) -> Self {
        Self { pass }
    }

    pub fn status_label(&self) -> &'static str {
        if self.pass {
            "pass"
        } else {
            "fail"
        }
    }
}
