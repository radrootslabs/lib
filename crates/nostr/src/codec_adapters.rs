extern crate alloc;
use alloc::{string::String, vec::Vec};

use crate::types::RadrootsNostrEvent;

use radroots_event::{
    social::job_feedback::JobFeedback, social::job_request::JobRequest,
    social::job_result::JobResult,
};
use radroots_event_codec::job::{
    error::JobParseError, feedback::decode as fb_decode, request::decode as req_decode,
    result::decode as res_decode,
};
use radroots_event_codec::parsed::{RadrootsParsedData, RadrootsParsedEvent};

fn event_id(e: &RadrootsNostrEvent) -> String {
    e.id.to_hex()
}

fn author(e: &RadrootsNostrEvent) -> String {
    e.pubkey.to_hex()
}

fn published_at(e: &RadrootsNostrEvent) -> u64 {
    e.created_at.as_secs()
}

fn kind_u32(e: &RadrootsNostrEvent) -> u32 {
    e.kind.as_u16() as u32
}

fn content(e: &RadrootsNostrEvent) -> String {
    e.content.clone()
}

fn tags_vec(e: &RadrootsNostrEvent) -> Vec<Vec<String>> {
    e.tags.iter().map(|t| t.as_slice().to_vec()).collect()
}

fn sig_hex(e: &RadrootsNostrEvent) -> String {
    e.sig.to_string()
}

pub fn to_job_request_metadata(
    e: &RadrootsNostrEvent,
) -> Result<RadrootsParsedData<JobRequest>, JobParseError> {
    req_decode::data_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        tags_vec(e),
    )
}

pub fn to_job_result_metadata(
    e: &RadrootsNostrEvent,
) -> Result<RadrootsParsedData<JobResult>, JobParseError> {
    res_decode::data_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        content(e),
        tags_vec(e),
    )
}

pub fn to_job_feedback_metadata(
    e: &RadrootsNostrEvent,
) -> Result<RadrootsParsedData<JobFeedback>, JobParseError> {
    fb_decode::data_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        content(e),
        tags_vec(e),
    )
}

pub fn to_job_request_index(
    e: &RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<JobRequest>, JobParseError> {
    req_decode::parsed_from_event(
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
    e: &RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<JobResult>, JobParseError> {
    res_decode::parsed_from_event(
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
    e: &RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<JobFeedback>, JobParseError> {
    fb_decode::parsed_from_event(
        event_id(e),
        author(e),
        published_at(e),
        kind_u32(e),
        content(e),
        tags_vec(e),
        sig_hex(e),
    )
}
