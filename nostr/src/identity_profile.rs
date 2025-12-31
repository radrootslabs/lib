use crate::error::RadrootsNostrError;
use crate::events::metadata::radroots_nostr_post_metadata_event;
use crate::types::{RadrootsNostrEventId, RadrootsNostrOutput};
use crate::client::RadrootsNostrClient;
use radroots_identity::RadrootsIdentity;

pub async fn radroots_nostr_publish_identity_profile(
    client: &RadrootsNostrClient,
    identity: &RadrootsIdentity,
) -> Result<Option<RadrootsNostrOutput<RadrootsNostrEventId>>, RadrootsNostrError> {
    let Some(profile) = identity.profile().and_then(|p| p.profile.as_ref()) else {
        return Ok(None);
    };
    let metadata = radroots_events_codec::profile::encode::to_metadata(profile)?;
    let out = radroots_nostr_post_metadata_event(client, &metadata).await?;
    Ok(Some(out))
}
