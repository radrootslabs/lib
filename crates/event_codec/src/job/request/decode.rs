use radroots_event::{
    envelope::kind::is_request_kind,
    social::job_request::{RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest},
};

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::job::{
    error::JobParseError,
    util::{parse_bid_tag_sat, parse_bool_encrypted, parse_i_tags, parse_params},
};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

pub fn job_request_from_tags(
    kind: u32,
    tags: &[Vec<String>],
) -> Result<RadrootsJobRequest, JobParseError> {
    let kind = u16::try_from(kind).map_err(|_| JobParseError::KindOutOfRange(kind))?;
    let inputs: Vec<RadrootsJobInput> = parse_i_tags(tags);

    let output = tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("output"))
        .and_then(|t| t.get(1).cloned());

    let params: Vec<RadrootsJobParam> = parse_params(tags);

    let bid_sat = parse_bid_tag_sat(tags)?;

    let relays = tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("relays"))
        .filter_map(|t| t.get(1).cloned())
        .collect::<Vec<_>>();

    let providers = tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("p"))
        .filter_map(|t| t.get(1).cloned())
        .collect::<Vec<_>>();

    let topics = tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("t"))
        .filter_map(|t| t.get(1).cloned())
        .collect::<Vec<_>>();

    let encrypted = parse_bool_encrypted(tags);

    if encrypted && providers.is_empty() {
        return Err(JobParseError::MissingTag("p"));
    }

    Ok(RadrootsJobRequest {
        kind,
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

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsJobRequest>, JobParseError> {
    if !is_request_kind(kind) {
        return Err(JobParseError::InvalidTag("kind (expected 5000-5999)"));
    }
    let job_request = job_request_from_tags(kind, &tags)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        job_request,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsJobRequest>, JobParseError> {
    let data = data_from_event(id.clone(), author.clone(), published_at, kind, tags.clone())?;
    RadrootsParsedEvent::from_event_parts(id, author, published_at, kind, content, tags, sig, data)
        .map_err(|_| JobParseError::InvalidTag("event_envelope"))
}
