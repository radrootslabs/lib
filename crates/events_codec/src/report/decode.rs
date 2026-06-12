#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::KIND_REPORT,
    report::RadrootsReport,
    social::{RadrootsReportFileTarget, RadrootsReportType, RadrootsSocialTarget},
    tags::{TAG_A, TAG_E, TAG_MAGNET, TAG_P, TAG_SERVER, TAG_SHA256},
};

use crate::error::EventParseError;
use crate::field_helpers::{parse_address_tag, required_tag_value, validate_lowercase_hex_64_tag};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
use crate::social_helpers::first_tag_value;

pub fn report_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsReport, EventParseError> {
    if kind != KIND_REPORT {
        return Err(EventParseError::InvalidKind {
            expected: "1984",
            got: kind,
        });
    }
    let p_tag = find_tag(tags, TAG_P).ok_or(EventParseError::MissingTag(TAG_P))?;
    let reported_pubkey = required_tag_value(tags, TAG_P)?;
    let report_type = parse_report_type(
        p_tag
            .get(2)
            .map(|value| value.as_str())
            .ok_or(EventParseError::InvalidTag(TAG_P))?,
        TAG_P,
    )?;
    let event = parse_event_target(tags, &report_type)?;
    let file = parse_file_target(tags, &report_type)?;
    Ok(RadrootsReport {
        reported_pubkey,
        report_type,
        event,
        file,
        content: if content.is_empty() {
            None
        } else {
            Some(content.to_string())
        },
    })
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsReport>, EventParseError> {
    let report = report_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        report,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsReport>, EventParseError> {
    let data = data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        data,
    })
}

fn parse_event_target(
    tags: &[Vec<String>],
    report_type: &RadrootsReportType,
) -> Result<Option<RadrootsSocialTarget>, EventParseError> {
    if let Some(tag) = find_tag(tags, TAG_A) {
        validate_optional_target_report_type(tag, 2, report_type, TAG_A)?;
        let value = tag
            .get(1)
            .cloned()
            .ok_or(EventParseError::InvalidTag(TAG_A))?;
        let address = parse_address_tag(&value, TAG_A)?;
        return Ok(Some(RadrootsSocialTarget::Address {
            address: value,
            author: Some(address.pubkey),
            event_kind: Some(address.kind),
            relays: relays_from_tag(tag, 3),
        }));
    }
    if let Some(tag) = find_tag(tags, TAG_E) {
        validate_optional_target_report_type(tag, 2, report_type, TAG_E)?;
        let id = tag
            .get(1)
            .cloned()
            .ok_or(EventParseError::InvalidTag(TAG_E))?;
        validate_lowercase_hex_64_tag(&id, TAG_E)?;
        return Ok(Some(RadrootsSocialTarget::Event {
            id,
            author: first_tag_value(tags, TAG_P),
            event_kind: None,
            relays: relays_from_tag(tag, 3),
        }));
    }
    Ok(None)
}

fn parse_file_target(
    tags: &[Vec<String>],
    report_type: &RadrootsReportType,
) -> Result<Option<RadrootsReportFileTarget>, EventParseError> {
    let sha256 = if let Some(tag) = find_tag(tags, TAG_SHA256) {
        validate_optional_target_report_type(tag, 2, report_type, TAG_SHA256)?;
        let value = tag
            .get(1)
            .cloned()
            .ok_or(EventParseError::InvalidTag(TAG_SHA256))?;
        validate_lowercase_hex_64_tag(&value, TAG_SHA256)?;
        Some(value)
    } else {
        None
    };
    let url = first_tag_value(tags, TAG_SERVER);
    let magnet = first_tag_value(tags, TAG_MAGNET);
    if sha256.is_none() && url.is_none() && magnet.is_none() {
        Ok(None)
    } else {
        Ok(Some(RadrootsReportFileTarget {
            sha256,
            url,
            magnet,
        }))
    }
}

fn validate_optional_target_report_type(
    tag: &[String],
    index: usize,
    expected: &RadrootsReportType,
    tag_name: &'static str,
) -> Result<(), EventParseError> {
    let Some(value) = tag.get(index) else {
        return Ok(());
    };
    if parse_report_type(value, tag_name)? == *expected {
        Ok(())
    } else {
        Err(EventParseError::InvalidTag(tag_name))
    }
}

fn parse_report_type(
    value: &str,
    tag_name: &'static str,
) -> Result<RadrootsReportType, EventParseError> {
    match value {
        "nudity" => Ok(RadrootsReportType::Nudity),
        "malware" => Ok(RadrootsReportType::Malware),
        "profanity" => Ok(RadrootsReportType::Profanity),
        "illegal" => Ok(RadrootsReportType::Illegal),
        "spam" => Ok(RadrootsReportType::Spam),
        "impersonation" => Ok(RadrootsReportType::Impersonation),
        "other" => Ok(RadrootsReportType::Other),
        _ => Err(EventParseError::InvalidTag(tag_name)),
    }
}

fn find_tag<'a>(tags: &'a [Vec<String>], key: &'static str) -> Option<&'a Vec<String>> {
    tags.iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
}

fn relays_from_tag(tag: &[String], start: usize) -> Option<Vec<String>> {
    let relays = tag
        .iter()
        .skip(start)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    if relays.is_empty() {
        None
    } else {
        Some(relays)
    }
}
