use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    job::{
        JobPaymentRequest,
        request::models::RadrootsJobInput,
        result::models::{
            RadrootsJobResult, RadrootsJobResultEventIndex, RadrootsJobResultEventMetadata,
        },
    },
};

use crate::job::{
    error::JobParseError,
    util::{parse_amount_tag_sat, parse_bool_encrypted, parse_i_tags},
};

pub fn job_result_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsJobResult, JobParseError> {
    let etag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("e"))
        .or_else(|| {
            tags.iter()
                .find(|t| t.get(0).map(|s| s.as_str()) == Some("e_ref"))
        })
        .ok_or(JobParseError::MissingTag("e"))?;

    let req_id = etag.get(1).ok_or(JobParseError::InvalidTag("e"))?.clone();
    let relay_hint = etag.get(2).cloned();

    let request_json = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("request"))
        .and_then(|t| t.get(1).cloned());

    let inputs: Vec<RadrootsJobInput> = parse_i_tags(tags);

    let payment = parse_amount_tag_sat(tags)?.map(|(sat, bolt11)| JobPaymentRequest {
        amount_sat: sat,
        bolt11,
    });

    let encrypted = parse_bool_encrypted(tags);

    let customer_pubkey = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("p"))
        .and_then(|t| t.get(1).cloned());

    Ok(RadrootsJobResult {
        kind: kind as u16,
        request_event: RadrootsNostrEventPtr {
            id: req_id,
            relays: relay_hint,
        },
        request_json,
        inputs,
        customer_pubkey,
        payment,
        content: if content.is_empty() {
            None
        } else {
            Some(content.to_string())
        },
        encrypted,
    })
}

fn is_result_kind(kind: u32) -> bool {
    (6000..=6999).contains(&kind)
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsJobResultEventMetadata, JobParseError> {
    if !is_result_kind(kind) {
        return Err(JobParseError::InvalidTag("kind (expected 6000-6999)"));
    }
    let job_result = job_result_from_tags(kind, &tags, &content)?;
    Ok(RadrootsJobResultEventMetadata {
        id,
        author,
        published_at,
        kind,
        job_result,
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
) -> Result<RadrootsJobResultEventIndex, JobParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsJobResultEventIndex {
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
