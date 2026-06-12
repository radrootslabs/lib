#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString, vec, vec::Vec};

use radroots_events::{
    kinds::KIND_REPORT,
    report::RadrootsReport,
    social::{RadrootsReportFileTarget, RadrootsReportType, RadrootsSocialTarget},
    tags::{TAG_A, TAG_E, TAG_MAGNET, TAG_P, TAG_SERVER, TAG_SHA256},
};

use crate::error::EventEncodeError;
use crate::field_helpers::{
    parse_address_tag, push_tag, validate_lowercase_hex_64, validate_non_empty_field,
};
use crate::social_helpers::validate_http_url;
use crate::wire::WireEventParts;

pub fn report_build_tags(report: &RadrootsReport) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_report(report)?;
    let report_type = report_type_as_str(&report.report_type);
    let mut tags = Vec::new();
    tags.push(vec![
        TAG_P.to_string(),
        report.reported_pubkey.clone(),
        report_type.to_string(),
    ]);
    if let Some(event) = report.event.as_ref() {
        push_report_event_target(&mut tags, event, report_type)?;
    }
    if let Some(file) = report.file.as_ref() {
        push_report_file_target(&mut tags, file, report_type)?;
    }
    Ok(tags)
}

pub fn to_wire_parts(report: &RadrootsReport) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(report, KIND_REPORT)
}

pub fn to_wire_parts_with_kind(
    report: &RadrootsReport,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_REPORT {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(WireEventParts {
        kind,
        content: report.content.clone().unwrap_or_default(),
        tags: report_build_tags(report)?,
    })
}

fn validate_report(report: &RadrootsReport) -> Result<(), EventEncodeError> {
    validate_non_empty_field(&report.reported_pubkey, "reported_pubkey")?;
    if let Some(file) = report.file.as_ref() {
        validate_file_target(file)?;
    }
    Ok(())
}

fn push_report_event_target(
    tags: &mut Vec<Vec<String>>,
    target: &RadrootsSocialTarget,
    report_type: &'static str,
) -> Result<(), EventEncodeError> {
    match target {
        RadrootsSocialTarget::Event { id, relays, .. } => {
            validate_lowercase_hex_64(id, "event.id")?;
            let mut tag = vec![TAG_E.to_string(), id.clone(), report_type.to_string()];
            if let Some(relays) = relays.as_ref() {
                tag.extend(
                    relays
                        .iter()
                        .filter(|relay| !relay.trim().is_empty())
                        .cloned(),
                );
            }
            tags.push(tag);
            Ok(())
        }
        RadrootsSocialTarget::Address {
            address, relays, ..
        } => {
            let address = parse_address_tag(address, "event.address")
                .map_err(|_| EventEncodeError::InvalidField("event.address"))?;
            let mut tag = vec![
                TAG_A.to_string(),
                format!("{}:{}:{}", address.kind, address.pubkey, address.d_tag),
                report_type.to_string(),
            ];
            if let Some(relays) = relays.as_ref() {
                tag.extend(
                    relays
                        .iter()
                        .filter(|relay| !relay.trim().is_empty())
                        .cloned(),
                );
            }
            tags.push(tag);
            Ok(())
        }
        RadrootsSocialTarget::External { .. } => Err(EventEncodeError::InvalidField("event")),
    }
}

fn push_report_file_target(
    tags: &mut Vec<Vec<String>>,
    file: &RadrootsReportFileTarget,
    report_type: &'static str,
) -> Result<(), EventEncodeError> {
    if let Some(hash) = file.sha256.as_deref() {
        tags.push(vec![
            TAG_SHA256.to_string(),
            hash.to_string(),
            report_type.to_string(),
        ]);
    }
    if let Some(url) = file.url.as_deref() {
        push_tag(tags, TAG_SERVER, url);
    }
    if let Some(magnet) = file.magnet.as_deref() {
        push_tag(tags, TAG_MAGNET, magnet);
    }
    Ok(())
}

fn validate_file_target(file: &RadrootsReportFileTarget) -> Result<(), EventEncodeError> {
    if file.sha256.is_none() && file.url.is_none() && file.magnet.is_none() {
        return Err(EventEncodeError::EmptyRequiredField("file"));
    }
    if let Some(hash) = file.sha256.as_deref() {
        validate_lowercase_hex_64(hash, "file.sha256")?;
    }
    if let Some(url) = file.url.as_deref() {
        validate_http_url(url, "file.url")?;
    }
    if let Some(magnet) = file.magnet.as_deref() {
        validate_non_empty_field(magnet, "file.magnet")?;
    }
    Ok(())
}

fn report_type_as_str(report_type: &RadrootsReportType) -> &'static str {
    match report_type {
        RadrootsReportType::Nudity => "nudity",
        RadrootsReportType::Malware => "malware",
        RadrootsReportType::Profanity => "profanity",
        RadrootsReportType::Illegal => "illegal",
        RadrootsReportType::Spam => "spam",
        RadrootsReportType::Impersonation => "impersonation",
        RadrootsReportType::Other => "other",
    }
}
