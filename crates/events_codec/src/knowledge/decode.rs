#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::kinds::{
    KIND_CONTRIBUTION_ATTESTATION, KIND_EVIDENCE_BOUNTY, KIND_KNOWLEDGE_CHANGE_PROPOSAL,
    KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_FIELD_REPORT, KIND_KNOWLEDGE_RELATION,
    KIND_KNOWLEDGE_REVIEW, KIND_KNOWLEDGE_SOURCE, KIND_WIKI_ARTICLE, KIND_WIKI_MERGE_REQUEST,
    KIND_WIKI_REDIRECT,
};
use radroots_events::knowledge::{
    RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA, RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
    RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
    RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA, RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
    RADROOTS_KNOWLEDGE_REVIEW_SCHEMA, RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
    RADROOTS_KNOWLEDGE_SOURCE_SCHEMA, RadrootsAddressableRef, RadrootsContributionAttestation,
    RadrootsEvidenceBounty, RadrootsKnowledgeChangeProposal, RadrootsKnowledgeClaim,
    RadrootsKnowledgeFieldReport, RadrootsKnowledgeRelation, RadrootsKnowledgeReview,
    RadrootsKnowledgeSource, RadrootsWikiArticle, RadrootsWikiMergeRequest, RadrootsWikiRedirect,
    validate_wiki_d_tag,
};
use radroots_events::tags::{TAG_A, TAG_CONTRACT, TAG_D, TAG_E, TAG_SUMMARY, TAG_T};
use radroots_events::{RadrootsNostrEvent, RadrootsNostrEventRef};
use serde::de::DeserializeOwned;

use crate::error::EventParseError;
use crate::event_ref::parse_event_ref_tag;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const TAG_TITLE: &str = "title";
const TAG_SOURCE: &str = "source";
const TAG_REVIEW_TARGET: &str = "review_target";
const TAG_FORK: &str = "fork";
const TAG_DEFERRED_TO: &str = "deferred_to";
const E_MARKER_SOURCE: &str = "source";

fn ensure_kind(kind: u32, expected: u32, name: &'static str) -> Result<(), EventParseError> {
    if kind == expected {
        Ok(())
    } else {
        Err(EventParseError::InvalidKind {
            expected: name,
            got: kind,
        })
    }
}

fn matching_tags<'a>(tags: &'a [Vec<String>], name: &'static str) -> Vec<&'a Vec<String>> {
    tags.iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(name))
        .collect()
}

fn required_one_value(tags: &[Vec<String>], name: &'static str) -> Result<String, EventParseError> {
    let matches = matching_tags(tags, name);
    if matches.is_empty() {
        return Err(EventParseError::MissingTag(name));
    }
    if matches.len() > 1 {
        return Err(EventParseError::InvalidTag(name));
    }
    let value = matches[0].get(1).ok_or(EventParseError::InvalidTag(name))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(name));
    }
    Ok(value.clone())
}

fn optional_one_value(
    tags: &[Vec<String>],
    name: &'static str,
) -> Result<Option<String>, EventParseError> {
    let matches = matching_tags(tags, name);
    if matches.len() > 1 {
        return Err(EventParseError::InvalidTag(name));
    }
    Ok(matches
        .first()
        .and_then(|tag| tag.get(1))
        .filter(|value| !value.trim().is_empty())
        .cloned())
}

fn values(tags: &[Vec<String>], name: &'static str) -> Vec<String> {
    tags.iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(name))
        .filter_map(|tag| tag.get(1))
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .collect()
}

fn event_refs(
    tags: &[Vec<String>],
    name: &'static str,
) -> Result<Vec<RadrootsNostrEventRef>, EventParseError> {
    matching_tags(tags, name)
        .into_iter()
        .map(|tag| parse_event_ref_tag(tag, name))
        .collect()
}

fn first_event_ref(
    tags: &[Vec<String>],
    name: &'static str,
) -> Result<Option<RadrootsNostrEventRef>, EventParseError> {
    let matches = matching_tags(tags, name);
    if matches.len() > 1 {
        return Err(EventParseError::InvalidTag(name));
    }
    matches
        .first()
        .map(|tag| parse_event_ref_tag(tag, name))
        .transpose()
}

fn address_from_a_tag(
    tags: &[Vec<String>],
    name: &'static str,
) -> Result<RadrootsAddressableRef, EventParseError> {
    let tag = matching_tags(tags, name)
        .into_iter()
        .next()
        .ok_or(EventParseError::MissingTag(name))?;
    let value = tag.get(1).ok_or(EventParseError::InvalidTag(name))?;
    let mut parts = value.splitn(3, ':');
    let kind = parts
        .next()
        .ok_or(EventParseError::InvalidTag(name))?
        .parse()
        .map_err(|error| EventParseError::InvalidNumber(name, error))?;
    let pubkey = parts
        .next()
        .filter(|value| !value.trim().is_empty())
        .ok_or(EventParseError::InvalidTag(name))?;
    let d_tag = parts
        .next()
        .filter(|value| !value.trim().is_empty())
        .ok_or(EventParseError::InvalidTag(name))?;
    Ok(RadrootsAddressableRef {
        kind,
        pubkey: pubkey.to_string(),
        d_tag: d_tag.to_string(),
        relays: tag
            .get(2..)
            .map(|values| values.to_vec())
            .unwrap_or_default(),
    })
}

fn event_ref_from_a_tag(
    tags: &[Vec<String>],
    name: &'static str,
) -> Result<RadrootsNostrEventRef, EventParseError> {
    let address = address_from_a_tag(tags, name)?;
    Ok(RadrootsNostrEventRef {
        id: String::new(),
        author: address.pubkey,
        kind: address.kind,
        d_tag: Some(address.d_tag),
        relays: if address.relays.is_empty() {
            None
        } else {
            Some(address.relays)
        },
    })
}

fn json_content<T: DeserializeOwned>(content: &str) -> Result<T, EventParseError> {
    serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))
}

fn parsed<T>(event: RadrootsNostrEvent, data: T) -> RadrootsParsedEvent<T> {
    let parsed_data = RadrootsParsedData::new(
        event.id.clone(),
        event.author.clone(),
        event.created_at,
        event.kind,
        data,
    );
    RadrootsParsedEvent::new(event, parsed_data)
}

fn require_contract_tag(
    tags: &[Vec<String>],
    expected: &'static str,
) -> Result<(), EventParseError> {
    let actual = required_one_value(tags, TAG_CONTRACT)?;
    if actual == expected {
        Ok(())
    } else {
        Err(EventParseError::InvalidTag(TAG_CONTRACT))
    }
}

fn require_schema(
    schema: &str,
    schema_version: u16,
    expected: &'static str,
) -> Result<(), EventParseError> {
    if schema != expected {
        return Err(EventParseError::InvalidJson("schema"));
    }
    if schema_version != RADROOTS_KNOWLEDGE_SCHEMA_VERSION {
        return Err(EventParseError::InvalidJson("schema_version"));
    }
    Ok(())
}

fn require_source_marker(
    tags: &[Vec<String>],
    source_event_id: &str,
) -> Result<(), EventParseError> {
    let has_source = tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_E)
            && tag.get(1).map(|value| value.as_str()) == Some(source_event_id)
            && tag.iter().skip(2).any(|value| value == E_MARKER_SOURCE)
    });
    if has_source {
        Ok(())
    } else {
        Err(EventParseError::InvalidTag(TAG_E))
    }
}

fn reject_private_coordinate_keys(content: &str) -> Result<(), EventParseError> {
    let value: serde_json::Value =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;
    if contains_private_coordinate_key(&value) {
        Err(EventParseError::InvalidJson("content"))
    } else {
        Ok(())
    }
}

fn contains_private_coordinate_key(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
            let lower = key.as_str();
            matches!(
                lower,
                "latitude"
                    | "longitude"
                    | "lat"
                    | "lon"
                    | "lng"
                    | "coordinates"
                    | "exact_coordinates"
            ) || contains_private_coordinate_key(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(contains_private_coordinate_key),
        _ => false,
    }
}

pub fn wiki_article_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsWikiArticle>, EventParseError> {
    ensure_kind(event.kind, KIND_WIKI_ARTICLE, "wiki article")?;
    let d_tag = required_one_value(&event.tags, TAG_D)?;
    validate_wiki_d_tag(&d_tag).map_err(|_| EventParseError::InvalidTag(TAG_D))?;
    let title = required_one_value(&event.tags, TAG_TITLE)?;
    let summary = optional_one_value(&event.tags, TAG_SUMMARY)?;
    let topics = values(&event.tags, TAG_T);
    let references = event_refs(&event.tags, TAG_SOURCE)?;
    let forked_from = event_refs(&event.tags, TAG_FORK)?;
    let deferred_to = first_event_ref(&event.tags, TAG_DEFERRED_TO)?;
    let article = RadrootsWikiArticle {
        d_tag,
        title,
        content_djot: event.content.clone(),
        summary,
        topics,
        references,
        forked_from,
        deferred_to,
    };
    Ok(parsed(event, article))
}

pub fn wiki_redirect_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsWikiRedirect>, EventParseError> {
    ensure_kind(event.kind, KIND_WIKI_REDIRECT, "wiki redirect")?;
    if !event.content.is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }
    let d_tag = required_one_value(&event.tags, TAG_D)?;
    validate_wiki_d_tag(&d_tag).map_err(|_| EventParseError::InvalidTag(TAG_D))?;
    let target = event_ref_from_a_tag(&event.tags, TAG_A)?;
    Ok(parsed(event, RadrootsWikiRedirect { d_tag, target }))
}

pub fn wiki_merge_request_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsWikiMergeRequest>, EventParseError> {
    ensure_kind(event.kind, KIND_WIKI_MERGE_REQUEST, "wiki merge request")?;
    let request: RadrootsWikiMergeRequest = json_content(&event.content)?;
    address_from_a_tag(&event.tags, TAG_A)?;
    required_one_value(&event.tags, "p")?;
    require_source_marker(&event.tags, &request.source_version_event_id)?;
    Ok(parsed(event, request))
}

pub fn knowledge_source_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeSource>, EventParseError> {
    ensure_kind(event.kind, KIND_KNOWLEDGE_SOURCE, "knowledge source")?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_SOURCE_SCHEMA)?;
    let source: RadrootsKnowledgeSource = json_content(&event.content)?;
    require_schema(
        &source.schema,
        source.schema_version,
        RADROOTS_KNOWLEDGE_SOURCE_SCHEMA,
    )?;
    let d_tag = required_one_value(&event.tags, TAG_D)?;
    if d_tag != source.d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    Ok(parsed(event, source))
}

pub fn evidence_bounty_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsEvidenceBounty>, EventParseError> {
    ensure_kind(event.kind, KIND_EVIDENCE_BOUNTY, "evidence bounty")?;
    require_contract_tag(&event.tags, RADROOTS_EVIDENCE_BOUNTY_SCHEMA)?;
    let bounty: RadrootsEvidenceBounty = json_content(&event.content)?;
    require_schema(
        &bounty.schema,
        bounty.schema_version,
        RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
    )?;
    let d_tag = required_one_value(&event.tags, TAG_D)?;
    if d_tag != bounty.d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    Ok(parsed(event, bounty))
}

pub fn knowledge_claim_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeClaim>, EventParseError> {
    ensure_kind(event.kind, KIND_KNOWLEDGE_CLAIM, "knowledge claim")?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA)?;
    let claim: RadrootsKnowledgeClaim = json_content(&event.content)?;
    require_schema(
        &claim.schema,
        claim.schema_version,
        RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
    )?;
    Ok(parsed(event, claim))
}

pub fn knowledge_relation_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeRelation>, EventParseError> {
    ensure_kind(event.kind, KIND_KNOWLEDGE_RELATION, "knowledge relation")?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_RELATION_SCHEMA)?;
    let relation: RadrootsKnowledgeRelation = json_content(&event.content)?;
    require_schema(
        &relation.schema,
        relation.schema_version,
        RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
    )?;
    Ok(parsed(event, relation))
}

pub fn knowledge_review_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeReview>, EventParseError> {
    ensure_kind(event.kind, KIND_KNOWLEDGE_REVIEW, "knowledge review")?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_REVIEW_SCHEMA)?;
    required_one_value(&event.tags, TAG_REVIEW_TARGET)?;
    let review: RadrootsKnowledgeReview = json_content(&event.content)?;
    require_schema(
        &review.schema,
        review.schema_version,
        RADROOTS_KNOWLEDGE_REVIEW_SCHEMA,
    )?;
    Ok(parsed(event, review))
}

pub fn knowledge_field_report_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeFieldReport>, EventParseError> {
    ensure_kind(
        event.kind,
        KIND_KNOWLEDGE_FIELD_REPORT,
        "knowledge field report",
    )?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA)?;
    reject_private_coordinate_keys(&event.content)?;
    let report: RadrootsKnowledgeFieldReport = json_content(&event.content)?;
    require_schema(
        &report.schema,
        report.schema_version,
        RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
    )?;
    Ok(parsed(event, report))
}

pub fn knowledge_change_proposal_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeChangeProposal>, EventParseError> {
    ensure_kind(
        event.kind,
        KIND_KNOWLEDGE_CHANGE_PROPOSAL,
        "knowledge change proposal",
    )?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA)?;
    let proposal: RadrootsKnowledgeChangeProposal = json_content(&event.content)?;
    require_schema(
        &proposal.schema,
        proposal.schema_version,
        RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA,
    )?;
    Ok(parsed(event, proposal))
}

pub fn contribution_attestation_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsContributionAttestation>, EventParseError> {
    ensure_kind(
        event.kind,
        KIND_CONTRIBUTION_ATTESTATION,
        "contribution attestation",
    )?;
    require_contract_tag(&event.tags, RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA)?;
    let attestation: RadrootsContributionAttestation = json_content(&event.content)?;
    require_schema(
        &attestation.schema,
        attestation.schema_version,
        RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA,
    )?;
    Ok(parsed(event, attestation))
}
