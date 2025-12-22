extern crate alloc;
use alloc::{string::String, vec::Vec};

use nostr::event::Event;

use radroots_events_codec::job::{
    error::JobParseError, feedback::decode as fb_decode, request::decode as req_decode,
    result::decode as res_decode,
};
use crate::util::created_at_u32_saturating;

fn event_id(e: &Event) -> String {
    e.id.to_hex()
}

fn author(e: &Event) -> String {
    e.pubkey.to_hex()
}

fn published_at(e: &Event) -> u32 {
    created_at_u32_saturating(e.created_at)
}

fn kind_u32(e: &Event) -> u32 {
    e.kind.as_u16() as u32
}

fn content(e: &Event) -> String {
    e.content.clone()
}

fn tags_vec(e: &Event) -> Vec<Vec<String>> {
    e.tags.iter().map(|t| t.as_slice().to_vec()).collect()
}

fn sig_hex(e: &Event) -> String {
    e.sig.to_string()
}

pub fn to_job_request_metadata(
    e: &Event,
) -> Result<radroots_events::job::request::models::RadrootsJobRequestEventMetadata, JobParseError> {
    req_decode::metadata_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        tags_vec(e),
    )
}

pub fn to_job_result_metadata(
    e: &Event,
) -> Result<radroots_events::job::result::models::RadrootsJobResultEventMetadata, JobParseError> {
    res_decode::metadata_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        content(e),
        tags_vec(e),
    )
}

pub fn to_job_feedback_metadata(
    e: &Event,
) -> Result<radroots_events::job::feedback::models::RadrootsJobFeedbackEventMetadata, JobParseError>
{
    fb_decode::metadata_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        content(e),
        tags_vec(e),
    )
}

pub fn to_job_request_index(
    e: &Event,
) -> Result<radroots_events::job::request::models::RadrootsJobRequestEventIndex, JobParseError> {
    req_decode::index_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        content(e),
        tags_vec(e),
        sig_hex(e),
    )
}

pub fn to_job_result_index(
    e: &Event,
) -> Result<radroots_events::job::result::models::RadrootsJobResultEventIndex, JobParseError> {
    res_decode::index_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        content(e),
        tags_vec(e),
        sig_hex(e),
    )
}

pub fn to_job_feedback_index(
    e: &Event,
) -> Result<radroots_events::job::feedback::models::RadrootsJobFeedbackEventIndex, JobParseError> {
    fb_decode::index_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        content(e),
        tags_vec(e),
        sig_hex(e),
    )
}
