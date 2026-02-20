use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrAccountsError {
    #[error("not implemented")]
    NotImplemented,
}
