use crate::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum SatisfactionClass {
    Accepted,
    Delivered,
}
impl From<radroots::transport::SatisfactionClass> for SatisfactionClass {
    fn from(value: radroots::transport::SatisfactionClass) -> Self {
        match value {
            radroots::transport::SatisfactionClass::Accepted => Self::Accepted,
            radroots::transport::SatisfactionClass::Delivered => Self::Delivered,
        }
    }
}

// These are generated-language representations, not another state machine.
// A future source state refuses projection instead of gaining a default.
macro_rules! bridge {
    ($name:ident, $native:path, [$($variant:ident),+ $(,)?]) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
        pub enum $name { $($variant),+ }
        impl TryFrom<$native> for $name {
            type Error = Error;
            fn try_from(value: $native) -> Result<Self, Error> {
                use $native as Native;
                match value {
                    $(Native::$variant => Ok(Self::$variant),)+
                    _ => Err(Error::UnsupportedState { axis: stringify!($name).to_owned(), operation_id: None }),
                }
            }
        }
    };
}
bridge!(
    Admission,
    radroots::Admission,
    [NotAdmitted, Confirmed, Unknown]
);
bridge!(
    BusinessInstallation,
    radroots::BusinessInstallation,
    [NotApplicable, NotInstalled, Installed, Uncertain]
);
bridge!(
    Issuance,
    radroots::Issuance,
    [None, Pending, Obtained, Unknown, ProvablyUnissued]
);
bridge!(
    Delivery,
    radroots::Delivery,
    [NotStarted, Pending, Observed, Unknown]
);
bridge!(
    WaitTermination,
    radroots::WaitTermination,
    [Returned, Cancelled, DeadlineExceeded, ClientClosed]
);
bridge!(
    RecoveryRequirement,
    radroots::RecoveryRequirement,
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
    radroots::TargetObservation,
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
    radroots::signing::ArtifactRole,
    [PublicEvent, BusinessSeal, GiftWrap]
);
bridge!(OperationKind, radroots::OperationKind, [Chat, Listing]);

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum StartMode {
    LocalOnly,
    Online,
}
impl From<StartMode> for radroots::StartMode {
    fn from(value: StartMode) -> Self {
        match value {
            StartMode::LocalOnly => Self::LocalOnly,
            StartMode::Online => Self::Online,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum SessionState {
    Ready,
    Starting,
    Running,
    Closing,
    Closed,
}
impl From<radroots::SessionState> for SessionState {
    fn from(value: radroots::SessionState) -> Self {
        match value {
            radroots::SessionState::Ready => Self::Ready,
            radroots::SessionState::Starting => Self::Starting,
            radroots::SessionState::Running => Self::Running,
            radroots::SessionState::Closing => Self::Closing,
            radroots::SessionState::Closed => Self::Closed,
        }
    }
}
