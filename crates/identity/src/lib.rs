pub mod error;
pub mod spec;

pub use error::IdentityError;
pub use spec::{ExtendedIdentity, IdentitySpec, MinimalIdentity, load_or_generate, to_keys};

pub const DEFAULT_IDENTITY_PATH: &str = "identity.json";
