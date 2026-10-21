use super::Admission;
use wasm_bindgen::prelude::*;

/// Pure command-parser failures retained across the native boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputFailure {
    UnsupportedCommand,
    UnsupportedProfile,
    LimitExceeded,
}

/// Input-free failures; native details never become dependency/debug prose.
#[derive(Clone, Debug)]
pub enum Error {
    InvalidInput {
        field: String,
    },
    CommandInput {
        field: String,
        kind: InputFailure,
    },
    Operation {
        code: String,
        retry: String,
        operation_id: Option<String>,
        admission: Admission,
    },
    UnsupportedState {
        axis: String,
        operation_id: Option<String>,
    },
}
impl Error {
    pub(crate) fn command(
        field: &'static str,
        error: radroots_protocol::command::CommandError,
    ) -> Self {
        let kind = match error {
            radroots_protocol::command::CommandError::InvalidInput => return Self::input(field),
            radroots_protocol::command::CommandError::UnsupportedCommand => {
                InputFailure::UnsupportedCommand
            }
            radroots_protocol::command::CommandError::UnsupportedProfile => {
                InputFailure::UnsupportedProfile
            }
            radroots_protocol::command::CommandError::LimitExceeded => InputFailure::LimitExceeded,
            _ => {
                return Self::UnsupportedState {
                    axis: "command_input".to_owned(),
                    operation_id: None,
                };
            }
        };
        Self::CommandInput {
            field: field.to_owned(),
            kind,
        }
    }
    pub(crate) fn input(field: &'static str) -> Self {
        Self::InvalidInput {
            field: field.to_owned(),
        }
    }
    pub(crate) fn with_operation(self, id: String) -> Self {
        match self {
            Self::UnsupportedState { axis, .. } => Self::UnsupportedState {
                axis,
                operation_id: Some(id),
            },
            value => value,
        }
    }
}
impl From<radroots_execution::Error> for Error {
    fn from(value: radroots_execution::Error) -> Self {
        let operation_id = value.context().operation_id().map(|v| v.to_string());
        match Admission::try_from(value.context().admission()) {
            Ok(admission) => Self::Operation {
                code: value.code().as_str().to_owned(),
                retry: value.retry().as_str().to_owned(),
                operation_id,
                admission,
            },
            Err(Self::UnsupportedState { axis, .. }) => {
                Self::UnsupportedState { axis, operation_id }
            }
            Err(other) => other,
        }
    }
}

/// Input-free exception value. No private input, provider cause or path crosses.
#[derive(Clone)]
#[wasm_bindgen]
pub struct Failure {
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub code: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub retry: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub operation_id: Option<String>,
    #[wasm_bindgen(readonly)]
    pub admission: Option<Admission>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub field: Option<String>,
}
impl From<Error> for Failure {
    fn from(value: Error) -> Self {
        match value {
            Error::InvalidInput { field } => Self {
                code: "InvalidInput".into(),
                retry: None,
                operation_id: None,
                admission: None,
                field: Some(field),
            },
            Error::CommandInput { field, kind } => Self {
                code: match kind {
                    InputFailure::UnsupportedCommand => "UnsupportedCommand",
                    InputFailure::UnsupportedProfile => "UnsupportedProfile",
                    InputFailure::LimitExceeded => "LimitExceeded",
                }
                .into(),
                retry: None,
                operation_id: None,
                admission: None,
                field: Some(field),
            },
            Error::Operation {
                code,
                retry,
                operation_id,
                admission,
            } => Self {
                code,
                retry: Some(retry),
                operation_id,
                admission: Some(admission),
                field: None,
            },
            Error::UnsupportedState { axis, operation_id } => Self {
                code: "UnsupportedState".into(),
                retry: None,
                operation_id,
                admission: None,
                field: Some(axis),
            },
        }
    }
}
impl From<Error> for JsValue {
    fn from(value: Error) -> Self {
        Failure::from(value).into()
    }
}
