#![forbid(unsafe_code)]

use crate::RadrootsAuthorityError;
use radroots_events::ids::RadrootsPublicKey;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActorContext {
    pub pubkey: RadrootsPublicKey,
}

impl RadrootsActorContext {
    pub fn new(pubkey: impl AsRef<str>) -> Result<Self, RadrootsAuthorityError> {
        let pubkey = RadrootsPublicKey::parse(pubkey.as_ref())
            .map_err(|_| RadrootsAuthorityError::InvalidActorPubkey)?;
        Ok(Self { pubkey })
    }

    pub fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }
}
