use crate::client::RadrootsNostrClient;
use crate::error::RadrootsNostrError;
#[cfg(feature = "events")]
use crate::events::application_handler::{
    RadrootsNostrApplicationHandlerSpec, radroots_nostr_metadata_has_fields,
    radroots_nostr_publish_application_handler,
};
use crate::events::metadata::radroots_nostr_build_metadata_event;
#[cfg(feature = "events")]
use crate::types::RadrootsNostrMetadata;
use crate::types::{
    RadrootsNostrEventId, RadrootsNostrOutput, RadrootsNostrTag, RadrootsNostrTagKind,
};
#[cfg(feature = "events")]
use core::time::Duration;
use radroots_events::profile::RadrootsProfileType;
use radroots_events_codec::profile::encode::profile_build_tags;
use radroots_identity::RadrootsIdentity;

pub async fn radroots_nostr_publish_identity_profile(
    client: &RadrootsNostrClient,
    identity: &RadrootsIdentity,
) -> Result<Option<RadrootsNostrOutput<RadrootsNostrEventId>>, RadrootsNostrError> {
    radroots_nostr_publish_identity_profile_with_type(client, identity, None).await
}

pub async fn radroots_nostr_publish_identity_profile_with_type(
    client: &RadrootsNostrClient,
    identity: &RadrootsIdentity,
    profile_type: Option<RadrootsProfileType>,
) -> Result<Option<RadrootsNostrOutput<RadrootsNostrEventId>>, RadrootsNostrError> {
    let Some(profile) = identity.profile().and_then(|p| p.profile.as_ref()) else {
        return Ok(None);
    };
    let metadata = radroots_events_codec::profile::encode::to_metadata(profile)?;
    let tags = profile_build_tags(profile_type);
    let mut tag_list: Vec<RadrootsNostrTag> = Vec::new();
    for mut tag in tags {
        if tag.is_empty() {
            continue;
        }
        let key = tag.remove(0);
        tag_list.push(RadrootsNostrTag::custom(
            RadrootsNostrTagKind::Custom(key.into()),
            tag,
        ));
    }
    let builder = radroots_nostr_build_metadata_event(&metadata).tags(tag_list);
    let out = client.send_event_builder(builder).await?;
    Ok(Some(out))
}

#[cfg(feature = "events")]
pub async fn radroots_nostr_bootstrap_service_presence(
    client: &RadrootsNostrClient,
    identity: &RadrootsIdentity,
    profile_type: Option<RadrootsProfileType>,
    metadata: &RadrootsNostrMetadata,
    handler_spec: &RadrootsNostrApplicationHandlerSpec,
    connect_timeout: Duration,
) -> Result<(), RadrootsNostrError> {
    client.connect().await;
    client.wait_for_connection(connect_timeout).await;

    let profile_published =
        match radroots_nostr_publish_identity_profile_with_type(client, identity, profile_type)
            .await?
        {
            Some(_) => true,
            None => false,
        };

    if radroots_nostr_metadata_has_fields(metadata) && !profile_published {
        let builder =
            radroots_nostr_build_metadata_event(metadata).tags(profile_type_tags(profile_type));
        client.send_event_builder(builder).await?;
    }

    radroots_nostr_publish_application_handler(client, handler_spec).await?;
    Ok(())
}

fn profile_type_tags(profile_type: Option<RadrootsProfileType>) -> Vec<RadrootsNostrTag> {
    let mut tag_list: Vec<RadrootsNostrTag> = Vec::new();
    for mut tag in profile_build_tags(profile_type) {
        if tag.is_empty() {
            continue;
        }
        let key = tag.remove(0);
        tag_list.push(RadrootsNostrTag::custom(
            RadrootsNostrTagKind::Custom(key.into()),
            tag,
        ));
    }
    tag_list
}
