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
    RADROOTS_KNOWLEDGE_REVIEW_SCHEMA, RADROOTS_KNOWLEDGE_SOURCE_SCHEMA, RadrootsAddressableRef,
    RadrootsContributionAttestation, RadrootsEvidenceBounty, RadrootsKnowledgeChangeProposal,
    RadrootsKnowledgeClaim, RadrootsKnowledgeFieldReport, RadrootsKnowledgeRelation,
    RadrootsKnowledgeReview, RadrootsKnowledgeSource, RadrootsKnowledgeValidationError,
    RadrootsWikiArticle, RadrootsWikiArticleVersionRef, RadrootsWikiMergeRequest,
    RadrootsWikiRedirect, validate_contribution_attestation, validate_evidence_bounty,
    validate_knowledge_change_proposal, validate_knowledge_claim, validate_knowledge_field_report,
    validate_knowledge_relation, validate_knowledge_review, validate_knowledge_source,
    validate_wiki_article, validate_wiki_d_tag, validate_wiki_merge_request,
    validate_wiki_redirect,
};
use radroots_events::tags::{TAG_A, TAG_CONTRACT, TAG_D, TAG_E, TAG_P, TAG_SUMMARY, TAG_T};
use radroots_events::{RadrootsNostrEvent, RadrootsNostrEventRef};
use serde::de::DeserializeOwned;

use crate::error::EventParseError;
use crate::event_ref::parse_event_ref_tag;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const TAG_TITLE: &str = "title";
const TAG_SOURCE: &str = "source";
const TAG_REVIEW_TARGET: &str = "review_target";
const MARKER_FORK: &str = "fork";
const MARKER_DEFER: &str = "defer";
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

fn marker(tag: &[String]) -> Option<&str> {
    tag.last()
        .map(|value| value.as_str())
        .filter(|value| matches!(*value, MARKER_FORK | MARKER_DEFER | E_MARKER_SOURCE))
}

fn unmarked_tags<'a>(tags: &'a [Vec<String>], name: &'static str) -> Vec<&'a Vec<String>> {
    matching_tags(tags, name)
        .into_iter()
        .filter(|tag| marker(tag).is_none())
        .collect()
}

fn address_from_tag(
    tag: &[String],
    name: &'static str,
) -> Result<RadrootsAddressableRef, EventParseError> {
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
    let relays_end = if marker(tag).is_some() {
        tag.len().saturating_sub(1)
    } else {
        tag.len()
    };
    Ok(RadrootsAddressableRef {
        kind,
        pubkey: pubkey.to_string(),
        d_tag: d_tag.to_string(),
        relays: tag
            .get(2..relays_end)
            .map(|values| {
                values
                    .iter()
                    .filter(|value| !value.is_empty())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn address_from_a_tag(
    tags: &[Vec<String>],
    name: &'static str,
) -> Result<RadrootsAddressableRef, EventParseError> {
    let matches = unmarked_tags(tags, name);
    if matches.is_empty() {
        return Err(EventParseError::MissingTag(name));
    }
    if matches.len() > 1 {
        return Err(EventParseError::InvalidTag(name));
    }
    address_from_tag(matches[0], name)
}

fn wiki_version_refs(
    tags: &[Vec<String>],
    marker_name: &'static str,
) -> Result<Vec<RadrootsWikiArticleVersionRef>, EventParseError> {
    let address_tags = matching_tags(tags, TAG_A)
        .into_iter()
        .filter(|tag| marker(tag) == Some(marker_name))
        .collect::<Vec<_>>();
    let event_tags = matching_tags(tags, TAG_E)
        .into_iter()
        .filter(|tag| marker(tag) == Some(marker_name))
        .collect::<Vec<_>>();
    if address_tags.len() != event_tags.len() {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    address_tags
        .into_iter()
        .zip(event_tags)
        .map(|(address_tag, event_tag)| {
            let address_ref = address_from_tag(address_tag, TAG_A)?;
            if address_ref.kind != KIND_WIKI_ARTICLE {
                return Err(EventParseError::InvalidTag(TAG_A));
            }
            let event_id = event_tag
                .get(1)
                .filter(|value| !value.trim().is_empty())
                .ok_or(EventParseError::InvalidTag(TAG_E))?
                .clone();
            Ok(RadrootsWikiArticleVersionRef {
                event_id,
                address_ref,
            })
        })
        .collect()
}

fn wiki_merge_source_event_id(tags: &[Vec<String>]) -> Result<String, EventParseError> {
    let source_tags = matching_tags(tags, TAG_E)
        .into_iter()
        .filter(|tag| marker(tag) == Some(E_MARKER_SOURCE))
        .collect::<Vec<_>>();
    if source_tags.len() != 1 {
        return Err(EventParseError::InvalidTag(TAG_E));
    }
    source_tags[0]
        .get(1)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or(EventParseError::InvalidTag(TAG_E))
}

fn wiki_merge_base_event_id(tags: &[Vec<String>]) -> Result<Option<String>, EventParseError> {
    let base_tags = unmarked_tags(tags, TAG_E);
    if base_tags.len() > 1 {
        return Err(EventParseError::InvalidTag(TAG_E));
    }
    base_tags
        .first()
        .map(|tag| {
            tag.get(1)
                .filter(|value| !value.trim().is_empty())
                .cloned()
                .ok_or(EventParseError::InvalidTag(TAG_E))
        })
        .transpose()
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

fn parse_validation_error(error: RadrootsKnowledgeValidationError) -> EventParseError {
    match error.field() {
        "d_tag" => EventParseError::InvalidTag(TAG_D),
        "references" => EventParseError::InvalidTag(TAG_SOURCE),
        "forked_from" | "deferred_to" | "wiki_redirect.target" | "target_article" => {
            EventParseError::InvalidTag(TAG_A)
        }
        "destination_pubkey" => EventParseError::InvalidTag(TAG_P),
        "base_version_event_id" | "source_version_event_id" => EventParseError::InvalidTag(TAG_E),
        field => EventParseError::InvalidJson(field),
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
    let title = optional_one_value(&event.tags, TAG_TITLE)?;
    let summary = optional_one_value(&event.tags, TAG_SUMMARY)?;
    let topics = values(&event.tags, TAG_T);
    let references = event_refs(&event.tags, TAG_SOURCE)?;
    let forked_from = wiki_version_refs(&event.tags, MARKER_FORK)?;
    let mut deferred_refs = wiki_version_refs(&event.tags, MARKER_DEFER)?;
    if deferred_refs.len() > 1 {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    let deferred_to = deferred_refs.pop();
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
    validate_wiki_article(&article).map_err(parse_validation_error)?;
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
    let target = address_from_a_tag(&event.tags, TAG_A)?;
    if target.kind != KIND_WIKI_ARTICLE {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    let redirect = RadrootsWikiRedirect { d_tag, target };
    validate_wiki_redirect(&redirect).map_err(parse_validation_error)?;
    Ok(parsed(event, redirect))
}

pub fn wiki_merge_request_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsWikiMergeRequest>, EventParseError> {
    ensure_kind(event.kind, KIND_WIKI_MERGE_REQUEST, "wiki merge request")?;
    let target_article = address_from_a_tag(&event.tags, TAG_A)?;
    if target_article.kind != KIND_WIKI_ARTICLE {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    let destination_pubkey = required_one_value(&event.tags, TAG_P)?;
    let base_version_event_id = wiki_merge_base_event_id(&event.tags)?;
    let source_version_event_id = wiki_merge_source_event_id(&event.tags)?;
    let explanation = if event.content.is_empty() {
        None
    } else {
        Some(event.content.clone())
    };
    let request = RadrootsWikiMergeRequest {
        target_article,
        destination_pubkey,
        base_version_event_id,
        source_version_event_id,
        explanation,
    };
    validate_wiki_merge_request(&request).map_err(parse_validation_error)?;
    Ok(parsed(event, request))
}

pub fn knowledge_source_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeSource>, EventParseError> {
    ensure_kind(event.kind, KIND_KNOWLEDGE_SOURCE, "knowledge source")?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_SOURCE_SCHEMA)?;
    let source: RadrootsKnowledgeSource = json_content(&event.content)?;
    let d_tag = required_one_value(&event.tags, TAG_D)?;
    if d_tag != source.d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    validate_knowledge_source(&source).map_err(parse_validation_error)?;
    Ok(parsed(event, source))
}

pub fn evidence_bounty_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsEvidenceBounty>, EventParseError> {
    ensure_kind(event.kind, KIND_EVIDENCE_BOUNTY, "evidence bounty")?;
    require_contract_tag(&event.tags, RADROOTS_EVIDENCE_BOUNTY_SCHEMA)?;
    let bounty: RadrootsEvidenceBounty = json_content(&event.content)?;
    let d_tag = required_one_value(&event.tags, TAG_D)?;
    if d_tag != bounty.d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    validate_evidence_bounty(&bounty).map_err(parse_validation_error)?;
    Ok(parsed(event, bounty))
}

pub fn knowledge_claim_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeClaim>, EventParseError> {
    ensure_kind(event.kind, KIND_KNOWLEDGE_CLAIM, "knowledge claim")?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA)?;
    let claim: RadrootsKnowledgeClaim = json_content(&event.content)?;
    validate_knowledge_claim(&claim).map_err(parse_validation_error)?;
    Ok(parsed(event, claim))
}

pub fn knowledge_relation_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeRelation>, EventParseError> {
    ensure_kind(event.kind, KIND_KNOWLEDGE_RELATION, "knowledge relation")?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_RELATION_SCHEMA)?;
    let relation: RadrootsKnowledgeRelation = json_content(&event.content)?;
    validate_knowledge_relation(&relation).map_err(parse_validation_error)?;
    Ok(parsed(event, relation))
}

pub fn knowledge_review_from_event(
    event: RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsKnowledgeReview>, EventParseError> {
    ensure_kind(event.kind, KIND_KNOWLEDGE_REVIEW, "knowledge review")?;
    require_contract_tag(&event.tags, RADROOTS_KNOWLEDGE_REVIEW_SCHEMA)?;
    required_one_value(&event.tags, TAG_REVIEW_TARGET)?;
    let review: RadrootsKnowledgeReview = json_content(&event.content)?;
    validate_knowledge_review(&review).map_err(parse_validation_error)?;
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
    validate_knowledge_field_report(&report).map_err(parse_validation_error)?;
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
    validate_knowledge_change_proposal(&proposal).map_err(parse_validation_error)?;
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
    validate_contribution_attestation(&attestation).map_err(parse_validation_error)?;
    Ok(parsed(event, attestation))
}
