pub mod error;
pub mod spec;

pub use error::IdentityError;
pub use spec::{to_keys, load_or_generate, IdentitySpec, MinimalIdentity, ExtendedIdentity};

/// The canonical default identity file path.
pub const DEFAULT_IDENTITY_PATH: &str = "identity.json";
