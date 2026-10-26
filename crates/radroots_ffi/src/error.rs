use crate::{Admission, SignerRejection};

/// Pure command-parser failures retained across the native boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum InputFailure {
    UnsupportedCommand,
    UnsupportedProfile,
    LimitExceeded,
}

/// Input-free failures; native details never become dependency/debug prose.
#[derive(Clone, Debug, thiserror::Error, uniffi::Error)]
pub enum Error {
    #[error("invalid {field}")]
    InvalidInput { field: String },
    #[error("unsupported or bounded {field}: {kind:?}")]
    CommandInput { field: String, kind: InputFailure },
    #[error("operation {code}")]
    Operation {
        code: String,
        retry: String,
        operation_id: Option<String>,
        admission: Admission,
        signer_rejection: Option<Box<SignerRejection>>,
    },
    #[error("unsupported boundary state {axis}")]
    UnsupportedState {
        axis: String,
        operation_id: Option<String>,
        signer_rejection: Option<Box<SignerRejection>>,
    },
}
const _: () = assert!(core::mem::size_of::<Error>() <= 128);
impl Error {
    pub(crate) fn command(field: &'static str, error: radroots::CommandError) -> Self {
        let kind = match error {
            radroots::CommandError::InvalidInput => return Self::input(field),
            radroots::CommandError::UnsupportedCommand => InputFailure::UnsupportedCommand,
            radroots::CommandError::UnsupportedProfile => InputFailure::UnsupportedProfile,
            radroots::CommandError::LimitExceeded => InputFailure::LimitExceeded,
            _ => {
                return Self::UnsupportedState {
                    axis: "command_input".to_owned(),
                    operation_id: None,
                    signer_rejection: None,
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
            Self::UnsupportedState {
                axis,
                signer_rejection,
                ..
            } => Self::UnsupportedState {
                axis,
                operation_id: Some(id),
                signer_rejection,
            },
            value => value,
        }
    }
    fn operation(value: radroots::Error, signer_rejection: Option<Box<SignerRejection>>) -> Self {
        let operation_id = value.context().operation_id().map(|v| v.to_string());
        match Admission::try_from(value.context().admission()) {
            Ok(admission) => Self::Operation {
                code: value.code().as_str().to_owned(),
                retry: value.retry().as_str().to_owned(),
                operation_id,
                admission,
                signer_rejection,
            },
            Err(Self::UnsupportedState { axis, .. }) => Self::UnsupportedState {
                axis,
                operation_id,
                signer_rejection,
            },
            Err(_) => Self::UnsupportedState {
                axis: "admission".to_owned(),
                operation_id,
                signer_rejection,
            },
        }
    }
}
impl From<radroots::Error> for Error {
    fn from(value: radroots::Error) -> Self {
        Self::operation(value, None)
    }
}
impl From<radroots::ExecutionFailure> for Error {
    fn from(value: radroots::ExecutionFailure) -> Self {
        Self::operation(
            value.error(),
            value.signer_rejection().map(|v| Box::new(v.into())),
        )
    }
}
