use nostr::nips::nip90::{DataVendingMachineStatus, JobFeedbackData};

use crate::error::RadrootsNostrError;
use crate::types::{RadrootsNostrEvent, RadrootsNostrEventBuilder, RadrootsNostrTag};

pub fn radroots_nostr_build_event_job_result(
    job_request: &RadrootsNostrEvent,
    payload: impl Into<String>,
    millisats: u64,
    bolt11: Option<String>,
    tags: Option<Vec<RadrootsNostrTag>>,
) -> Result<RadrootsNostrEventBuilder, RadrootsNostrError> {
    let builder =
        RadrootsNostrEventBuilder::job_result(job_request.clone(), payload, millisats, bolt11)?
            .tags(tags.unwrap_or_default())
            .allow_self_tagging();
    Ok(builder)
}

pub fn radroots_nostr_build_event_job_feedback(
    job_request: &RadrootsNostrEvent,
    status: &str,
    extra_info: Option<String>,
    tags: Option<Vec<RadrootsNostrTag>>,
) -> Result<RadrootsNostrEventBuilder, RadrootsNostrError> {
    let status = status
        .parse::<DataVendingMachineStatus>()
        .unwrap_or(DataVendingMachineStatus::Error);
    let feedback_data = JobFeedbackData::new(&job_request.clone(), status)
        .extra_info(extra_info.unwrap_or_default());
    let builder = RadrootsNostrEventBuilder::job_feedback(feedback_data)
        .tags(tags.unwrap_or_default())
        .allow_self_tagging();
    Ok(builder)
}
