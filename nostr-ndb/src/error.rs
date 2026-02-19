use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrNdbError {
    #[error("not implemented")]
    NotImplemented,
}
