//! Shared exact core for contract-specific sealed Nostr builders.

#![forbid(unsafe_code)]

use crate::{
    error::Error,
    types::{
        ExternalSigningRequest, RadrootsNostrEvent, RadrootsNostrKeys, RadrootsNostrPublicKey,
        RadrootsNostrTimestamp,
    },
};
use radroots_event_codec::authoring::{AuthoredEventBody, AuthoredEventPlan};

pub(crate) struct SealedBuilderCore {
    body: AuthoredEventBody,
    created_at: Option<RadrootsNostrTimestamp>,
}

impl SealedBuilderCore {
    pub(crate) const fn new(body: AuthoredEventBody) -> Self {
        Self {
            body,
            created_at: None,
        }
    }

    pub(crate) const fn at(body: AuthoredEventBody, created_at: RadrootsNostrTimestamp) -> Self {
        Self {
            body,
            created_at: Some(created_at),
        }
    }

    pub(crate) const fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }

    pub(crate) fn sign_with_keys(
        self,
        keys: &RadrootsNostrKeys,
    ) -> Result<RadrootsNostrEvent, Error> {
        self.into_external_signing_request(keys.public_key())?
            .sign_with_keys(keys)
    }

    pub(crate) fn into_external_signing_request(
        self,
        public_key: RadrootsNostrPublicKey,
    ) -> Result<ExternalSigningRequest, Error> {
        let created_at = self
            .created_at
            .unwrap_or_else(RadrootsNostrTimestamp::now)
            .as_secs();
        let plan = AuthoredEventPlan::bind(self.body, created_at, public_key.to_hex())?;
        ExternalSigningRequest::from_authored_plan(plan)
    }
}
