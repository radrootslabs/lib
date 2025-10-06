use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    job::{
        JobPaymentRequest,
        feedback::models::{
            RadrootsJobFeedback, RadrootsJobFeedbackEventIndex, RadrootsJobFeedbackEventMetadata,
        },
    },
};

use crate::job::{
    error::JobParseError,
    util::{feedback_status_from_tag, parse_amount_tag_sat, parse_bool_encrypted},
};

pub fn job_feedback_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsJobFeedback, JobParseError> {
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

    let status_tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("status"))
        .ok_or(JobParseError::MissingTag("status"))?;

    let status = match status_tag.get(1).and_then(|s| feedback_status_from_tag(s)) {
        Some(s) => s,
        None => return Err(JobParseError::InvalidTag("status")),
    };

    let extra_info = status_tag.get(2).cloned();

    let payment = parse_amount_tag_sat(tags)?.map(|(sat, bolt11)| JobPaymentRequest {
        amount_sat: sat,
        bolt11,
    });

    let encrypted = parse_bool_encrypted(tags);

    let customer_pubkey = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("p"))
        .and_then(|t| t.get(1).cloned());

    Ok(RadrootsJobFeedback {
        kind: kind as u16,
        status,
        extra_info,
        request_event: RadrootsNostrEventPtr {
            id: req_id,
            relays: relay_hint,
        },
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

fn is_feedback_kind(kind: u32) -> bool {
    kind == 7000
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsJobFeedbackEventMetadata, JobParseError> {
    if !is_feedback_kind(kind) {
        return Err(JobParseError::InvalidTag("kind (expected 7000)"));
    }
    let job_feedback = job_feedback_from_tags(kind, &tags, &content)?;
    Ok(RadrootsJobFeedbackEventMetadata {
        id,
        author,
        published_at,
        kind,
        job_feedback,
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
) -> Result<RadrootsJobFeedbackEventIndex, JobParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsJobFeedbackEventIndex {
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
