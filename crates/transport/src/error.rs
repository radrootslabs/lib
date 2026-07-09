use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTransportError {
    EmptyTransportKind,
    InvalidTransportKind,
    EmptyTargetUri,
    InvalidTargetUri,
    EmptyTargetScope,
    InvalidTargetScope,
    EmptyTargetLabel,
    InvalidTargetLabel,
    EmptyTargetSet,
    DuplicateTargetFingerprint,
    InvalidTargetFingerprint,
    InvalidSatisfactionPolicy,
}

impl fmt::Display for RadrootsTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTransportKind => f.write_str("transport kind is empty"),
            Self::InvalidTransportKind => f.write_str("transport kind is invalid"),
            Self::EmptyTargetUri => f.write_str("transport target URI is empty"),
            Self::InvalidTargetUri => f.write_str("transport target URI is invalid"),
            Self::EmptyTargetScope => f.write_str("transport target scope is empty"),
            Self::InvalidTargetScope => f.write_str("transport target scope is invalid"),
            Self::EmptyTargetLabel => f.write_str("transport target label is empty"),
            Self::InvalidTargetLabel => f.write_str("transport target label is invalid"),
            Self::EmptyTargetSet => f.write_str("transport target set is empty"),
            Self::DuplicateTargetFingerprint => {
                f.write_str("transport target set contains duplicate fingerprints")
            }
            Self::InvalidTargetFingerprint => {
                f.write_str("transport target fingerprint is invalid")
            }
            Self::InvalidSatisfactionPolicy => {
                f.write_str("transport satisfaction policy is invalid")
            }
        }
    }
}
