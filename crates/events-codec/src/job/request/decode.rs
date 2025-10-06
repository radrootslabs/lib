use radroots_events::{
    RadrootsNostrEvent,
    job::request::models::{
        RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest, RadrootsJobRequestEventIndex,
        RadrootsJobRequestEventMetadata,
    },
};

use crate::job::{
    error::JobParseError,
    util::{parse_bid_tag_sat, parse_bool_encrypted, parse_i_tags, parse_params},
};

pub fn job_request_from_tags(
    kind: u32,
    tags: &[Vec<String>],
) -> Result<RadrootsJobRequest, JobParseError> {
    let inputs: Vec<RadrootsJobInput> = parse_i_tags(tags);

    let output = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("output"))
        .and_then(|t| t.get(1).cloned());

    let params: Vec<RadrootsJobParam> = parse_params(tags);

    let bid_sat = parse_bid_tag_sat(tags)?;

    let relays = tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some("relays"))
        .filter_map(|t| t.get(1).cloned())
        .collect::<Vec<_>>();

    let providers = tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some("p"))
        .filter_map(|t| t.get(1).cloned())
        .collect::<Vec<_>>();

    let topics = tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some("t"))
        .filter_map(|t| t.get(1).cloned())
        .collect::<Vec<_>>();

    let encrypted = parse_bool_encrypted(tags);

    if encrypted && providers.is_empty() {
        return Err(JobParseError::MissingTag("p"));
    }

    Ok(RadrootsJobRequest {
        kind: kind as u16,
        inputs,
        output,
        params,
        bid_sat,
        relays,
        providers,
        topics,
        encrypted,
    })
}

fn is_request_kind(kind: u32) -> bool {
    (5000..=5999).contains(&kind)
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsJobRequestEventMetadata, JobParseError> {
    if !is_request_kind(kind) {
        return Err(JobParseError::InvalidTag("kind (expected 5000-5999)"));
    }
    let job_request = job_request_from_tags(kind, &tags)?;
    Ok(RadrootsJobRequestEventMetadata {
        id,
        author,
        published_at,
        kind,
        job_request,
    })
}

pub fn index_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsJobRequestEventIndex, JobParseError> {
    let metadata =
        metadata_from_event(id.clone(), author.clone(), published_at, kind, tags.clone())?;
    Ok(RadrootsJobRequestEventIndex {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        metadata,
    })
}
