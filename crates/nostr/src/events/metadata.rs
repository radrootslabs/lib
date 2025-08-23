use crate::error::NostrUtilsError;
use core::time::Duration;
use nostr::{
    Kind, Metadata, event::Event, event::EventBuilder, event::EventId, filter::Filter,
    key::PublicKey,
};
use nostr_sdk::{Client, prelude::Output};

pub fn build_metadata_event(md: &Metadata) -> EventBuilder {
    EventBuilder::metadata(md)
}

pub async fn post_metadata_event(
    client: &Client,
    md: &Metadata,
) -> Result<Output<EventId>, NostrUtilsError> {
    let builder = build_metadata_event(md);
    Ok(client.send_event_builder(builder).await?)
}

pub async fn fetch_metadata_for_author(
    client: &Client,
    author: PublicKey,
    timeout: Duration,
) -> Result<Option<Event>, NostrUtilsError> {
    let filter = Filter::new().authors(vec![author]).kind(Kind::Metadata);
    let stored = client.database().query(filter.clone()).await?;
    let fetched = client.fetch_events(filter, timeout).await?;

    let mut latest: Option<Event> = None;
    for ev in stored.into_iter().chain(fetched.into_iter()) {
        if ev.kind != Kind::Metadata {
            continue;
        }
        match &latest {
            Some(cur) if ev.created_at <= cur.created_at => {}
            _ => latest = Some(ev),
        }
    }
    Ok(latest)
}
