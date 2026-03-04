extern crate alloc;
use alloc::{string::String, vec::Vec};

use crate::types::RadrootsNostrEvent;

use crate::util::created_at_u32_saturating;
use radroots_events::{
    job_feedback::RadrootsJobFeedback, job_request::RadrootsJobRequest,
    job_result::RadrootsJobResult,
};
use radroots_events_codec::job::{
    error::JobParseError, feedback::decode as fb_decode, request::decode as req_decode,
    result::decode as res_decode,
};
use radroots_events_codec::parsed::{RadrootsParsedData, RadrootsParsedEvent};

fn event_id(e: &RadrootsNostrEvent) -> String {
    e.id.to_hex()
}

fn author(e: &RadrootsNostrEvent) -> String {
    e.pubkey.to_hex()
}

fn published_at(e: &RadrootsNostrEvent) -> u32 {
    created_at_u32_saturating(e.created_at)
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
) -> Result<RadrootsParsedData<RadrootsJobRequest>, JobParseError> {
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
) -> Result<RadrootsParsedData<RadrootsJobResult>, JobParseError> {
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
) -> Result<RadrootsParsedData<RadrootsJobFeedback>, JobParseError> {
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
) -> Result<RadrootsParsedEvent<RadrootsJobRequest>, JobParseError> {
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
) -> Result<RadrootsParsedEvent<RadrootsJobResult>, JobParseError> {
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
) -> Result<RadrootsParsedEvent<RadrootsJobFeedback>, JobParseError> {
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
