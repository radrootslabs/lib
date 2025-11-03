use nostr::{
    event::{Event, EventBuilder, Tag},
    nips::nip90::{DataVendingMachineStatus, JobFeedbackData},
};

use crate::error::NostrUtilsError;

pub fn nostr_build_event_job_result(
    job_request: &Event,
    payload: impl Into<String>,
    millisats: u64,
    bolt11: Option<String>,
    tags: Option<Vec<Tag>>,
) -> Result<EventBuilder, NostrUtilsError> {
    let builder = EventBuilder::job_result(job_request.clone(), payload, millisats, bolt11)?
        .tags(tags.unwrap_or_default());
    Ok(builder)
}

pub fn nostr_build_event_job_feedback(
    job_request: &Event,
    status: &str,
    extra_info: Option<String>,
    tags: Option<Vec<Tag>>,
) -> Result<EventBuilder, NostrUtilsError> {
    let status = status
        .parse::<DataVendingMachineStatus>()
        .unwrap_or(DataVendingMachineStatus::Error);
    let feedback_data = JobFeedbackData::new(&job_request.clone(), status)
        .extra_info(extra_info.unwrap_or_default());
    let builder = EventBuilder::job_feedback(feedback_data).tags(tags.unwrap_or_default());
    Ok(builder)
}
