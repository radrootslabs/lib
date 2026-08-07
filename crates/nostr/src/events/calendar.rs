//! Sealed authoring for strict NIP-52 Event profiles.

use crate::{
    error::Error,
    events::sealed::SealedBuilderCore,
    types::{
        ExternalSigningRequest, RadrootsNostrEvent, RadrootsNostrKeys, RadrootsNostrPublicKey,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::calendar::{AuthoredCalendarDateEvent, AuthoredCalendarTimeEvent};
use radroots_event_codec::authoring::AuthoredEventBody;

/// A sealed builder for one validated NIP-52 date or time Event.
///
/// Callers cannot change the event kind, coordinate, content, or tags after
/// the typed calendar model has been converted into the authored body.
#[must_use = "calendar event builders must be signed or published"]
pub struct CalendarEventBuilder {
    inner: SealedBuilderCore,
}

impl CalendarEventBuilder {
    /// Sets the event timestamp without changing the validated NIP-52 profile.
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    /// Signs the exact authored Event directly with local keys.
    pub fn sign_with_keys(self, keys: &RadrootsNostrKeys) -> Result<RadrootsNostrEvent, Error> {
        self.inner.sign_with_keys(keys)
    }

    /// Finalizes the exact authored plan for an opaque host signer.
    pub fn into_external_signing_request(
        self,
        public_key: RadrootsNostrPublicKey,
    ) -> Result<ExternalSigningRequest, Error> {
        self.inner.into_external_signing_request(public_key)
    }
}

/// Builds a sealed kind-31922 all-day Event.
pub fn build_calendar_date_event(
    event: &AuthoredCalendarDateEvent,
) -> Result<CalendarEventBuilder, Error> {
    Ok(CalendarEventBuilder {
        inner: SealedBuilderCore::new(AuthoredEventBody::from_calendar_date_event(event)?),
    })
}

/// Builds a sealed kind-31923 timed Event.
pub fn build_calendar_time_event(
    event: &AuthoredCalendarTimeEvent,
) -> Result<CalendarEventBuilder, Error> {
    Ok(CalendarEventBuilder {
        inner: SealedBuilderCore::new(AuthoredEventBody::from_calendar_time_event(event)?),
    })
}
