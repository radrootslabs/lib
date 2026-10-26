use super::Error;
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[wasm_bindgen]
pub enum SatisfactionClass {
    Accepted,
    Delivered,
}
impl From<radroots_transport::policy::SatisfactionClass> for SatisfactionClass {
    fn from(value: radroots_transport::policy::SatisfactionClass) -> Self {
        match value {
            radroots_transport::policy::SatisfactionClass::Accepted => Self::Accepted,
            radroots_transport::policy::SatisfactionClass::Delivered => Self::Delivered,
        }
    }
}

// These are generated-language representations, not another state machine.
// A future source state refuses projection instead of gaining a default.
macro_rules! bridge {
    ($name:ident, $native:path, [$($variant:ident),+ $(,)?]) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[wasm_bindgen]
        pub enum $name { $($variant),+ }
        impl TryFrom<$native> for $name {
            type Error = Error;
            fn try_from(value: $native) -> Result<Self, Error> {
                use $native as Native;
                match value {
                    $(Native::$variant => Ok(Self::$variant),)+
                    _ => Err(Error::UnsupportedState { axis: stringify!($name).to_owned(), operation_id: None, signer_rejection: None }),
                }
            }
        }
    };
}
bridge!(
    Admission,
    radroots_execution::Admission,
    [NotAdmitted, Confirmed, Unknown]
);
bridge!(
    BusinessInstallation,
    radroots_execution::BusinessInstallation,
    [NotApplicable, NotInstalled, Installed, Uncertain]
);
bridge!(
    Issuance,
    radroots_execution::Issuance,
    [None, Pending, Obtained, Unknown, ProvablyUnissued]
);
bridge!(
    Delivery,
    radroots_execution::Delivery,
    [NotStarted, Pending, Observed, Unknown]
);
bridge!(
    WaitTermination,
    radroots_execution::WaitTermination,
    [Returned, Cancelled, DeadlineExceeded, ClientClosed]
);
bridge!(
    RecoveryRequirement,
    radroots_execution::RecoveryRequirement,
    [
        ContinueOriginalOperation,
        ReconcileIssuance,
        InstallObtainedEvidence,
        ResumeDelivery,
        ManualReconciliation
    ]
);
bridge!(
    TargetObservation,
    radroots_execution::TargetObservation,
    [
        Accepted,
        Rejected,
        TimedOut,
        Disconnected,
        Unknown,
        NotAttempted
    ]
);
bridge!(
    ArtifactRole,
    radroots_protocol::intent::ArtifactRole,
    [PublicEvent, BusinessSeal, GiftWrap]
);
bridge!(
    OperationKind,
    radroots_execution::OperationKind,
    [Chat, Listing]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[wasm_bindgen]
pub enum StartMode {
    LocalOnly,
    Online,
}
impl From<StartMode> for crate::StartMode {
    fn from(value: StartMode) -> Self {
        match value {
            StartMode::LocalOnly => Self::LocalOnly,
            StartMode::Online => Self::Online,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[wasm_bindgen]
pub enum SessionState {
    Ready,
    Starting,
    Running,
    Closing,
    Closed,
}
impl From<crate::SessionState> for SessionState {
    fn from(value: crate::SessionState) -> Self {
        match value {
            crate::SessionState::Ready => Self::Ready,
            crate::SessionState::Starting => Self::Starting,
            crate::SessionState::Running => Self::Running,
            crate::SessionState::Closing => Self::Closing,
            crate::SessionState::Closed => Self::Closed,
        }
    }
}
