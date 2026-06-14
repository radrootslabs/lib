#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RadrootsAuthorityError {
    #[error("invalid actor public key")]
    InvalidActorPubkey,

    #[error("invalid signer public key")]
    InvalidSignerPubkey,
}
