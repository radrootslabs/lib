use core::fmt;

#[derive(Debug)]
pub enum EventParseError {
    MissingTag(&'static str),
    InvalidTag(&'static str),
    DuplicateTag(&'static str),
    InvalidEnvelope,
    InvalidKind { expected: &'static str, got: u32 },
    InvalidNumber(&'static str, core::num::ParseIntError),
    InvalidJson(&'static str),
}

impl EventParseError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingTag(_) => "missing_tag",
            Self::InvalidTag(_) => "invalid_tag",
            Self::DuplicateTag(_) => "duplicate_tag",
            Self::InvalidEnvelope => "invalid_envelope",
            Self::InvalidKind { .. } => "invalid_kind",
            Self::InvalidNumber(_, _) => "invalid_number",
            Self::InvalidJson(_) => "invalid_json",
        }
    }
}

impl fmt::Display for EventParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventParseError::MissingTag(t) => write!(f, "missing tag: {}", t),
            EventParseError::InvalidTag(t) => write!(f, "invalid tag structure for '{}'", t),
            EventParseError::DuplicateTag(t) => write!(f, "duplicate tag: {}", t),
            EventParseError::InvalidEnvelope => write!(f, "invalid event envelope"),
            EventParseError::InvalidKind { expected, got } => {
                write!(f, "invalid kind {} (expected {})", got, expected)
            }
            EventParseError::InvalidNumber(t, e) => write!(f, "invalid number in '{}': {}", t, e),
            EventParseError::InvalidJson(ctx) => write!(f, "invalid JSON in '{}'", ctx),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EventParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EventParseError::InvalidNumber(_, e) => Some(e),
            _ => None,
        }
    }
}

impl From<radroots_event::RadrootsEventEnvelopeError> for EventParseError {
    fn from(_: radroots_event::RadrootsEventEnvelopeError) -> Self {
        Self::InvalidEnvelope
    }
}

#[derive(Debug)]
pub enum EventEncodeError {
    InvalidKind(u32),
    EmptyRequiredField(&'static str),
    InvalidField(&'static str),
    Json,
}

pub type RadrootsEncodeError = EventEncodeError;

impl EventEncodeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidKind(_) => "invalid_kind",
            Self::EmptyRequiredField(_) => "empty_required_field",
            Self::InvalidField(_) => "invalid_field",
            Self::Json => "json",
        }
    }
}

impl fmt::Display for EventEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventEncodeError::InvalidKind(kind) => write!(f, "invalid event kind: {}", kind),
            EventEncodeError::EmptyRequiredField(field) => {
                write!(f, "empty required field: {}", field)
            }
            EventEncodeError::InvalidField(field) => write!(f, "invalid field: {}", field),
            EventEncodeError::Json => write!(f, "failed to serialize JSON"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EventEncodeError {}
