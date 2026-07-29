#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::envelope::kind::{
    KIND_CONTRIBUTION_ATTESTATION, KIND_EVIDENCE_BOUNTY, KIND_KNOWLEDGE_CHANGE_PROPOSAL,
    KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_FIELD_REPORT, KIND_KNOWLEDGE_RELATION,
    KIND_KNOWLEDGE_REVIEW, KIND_KNOWLEDGE_SOURCE, KIND_WIKI_ARTICLE, KIND_WIKI_MERGE_REQUEST,
    KIND_WIKI_REDIRECT,
};
use radroots_event::knowledge::{
    RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA, RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
    RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
    RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA, RADROOTS_KNOWLEDGE_RELATION_SCHEMA,
    RADROOTS_KNOWLEDGE_REVIEW_SCHEMA, RADROOTS_KNOWLEDGE_SOURCE_SCHEMA, RadrootsAddressableRef,
    RadrootsContributionAttestation, RadrootsEvidenceBounty, RadrootsKnowledgeChangeProposal,
    RadrootsKnowledgeClaim, RadrootsKnowledgeFieldReport, RadrootsKnowledgeRelation,
    RadrootsKnowledgeReview, RadrootsKnowledgeReviewTarget, RadrootsKnowledgeSource,
    RadrootsKnowledgeValidationError, RadrootsWikiArticle, RadrootsWikiArticleVersionRef,
    RadrootsWikiMergeRequest, RadrootsWikiRedirect, validate_contribution_attestation,
    validate_evidence_bounty, validate_knowledge_change_proposal, validate_knowledge_claim,
    validate_knowledge_field_report, validate_knowledge_relation, validate_knowledge_review,
    validate_knowledge_source, validate_wiki_article, validate_wiki_merge_request,
    validate_wiki_redirect,
};
use radroots_event::tag::RadrootsEventRef;
use radroots_event::tag::name::{
    TAG_A, TAG_CONTRACT, TAG_D, TAG_E, TAG_G, TAG_P, TAG_SUMMARY, TAG_T,
};
use serde::Serialize;

use crate::error::EventEncodeError;
use crate::event_ref::build_event_ref_tag;
use crate::wire::empty_content;
use radroots_event::wire::RadrootsNip01EventWireParts;
use radroots_identity::PublicKey;

const TAG_TITLE: &str = "title";
const TAG_SOURCE: &str = "source";
const TAG_CITATION: &str = "citation";
const TAG_REVIEW_TARGET: &str = "review_target";
const TAG_EVIDENCE: &str = "evidence";
const MARKER_FORK: &str = "fork";
const MARKER_DEFER: &str = "defer";
const E_MARKER_SOURCE: &str = "source";

fn push_value(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    if !value.trim().is_empty() {
        tags.push(vec![key.to_string(), value.to_string()]);
    }
}

fn push_optional_value(tags: &mut Vec<Vec<String>>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        push_value(tags, key, value);
    }
}

fn push_topics(tags: &mut Vec<Vec<String>>, topics: &[String]) {
    for topic in topics {
        push_value(tags, TAG_T, topic);
    }
}

fn push_event_refs(tags: &mut Vec<Vec<String>>, tag_name: &str, refs: &[RadrootsEventRef]) {
    for event_ref in refs {
        tags.push(build_event_ref_tag(tag_name, event_ref));
    }
}

fn address_coordinate(address: &RadrootsAddressableRef) -> String {
    format!("{}:{}:{}", address.kind, address.pubkey, address.d_tag)
}

fn address_tag(tag_name: &str, address: &RadrootsAddressableRef) -> Vec<String> {
    let mut tag = Vec::with_capacity(2 + address.relays.len());
    tag.push(tag_name.to_string());
    tag.push(address_coordinate(address));
    tag.extend(address.relays.iter().cloned());
    tag
}

fn marker_address_tag(address: &RadrootsAddressableRef, marker: &'static str) -> Vec<String> {
    let mut tag = Vec::with_capacity(4 + address.relays.len());
    tag.push(TAG_A.to_string());
    tag.push(address_coordinate(address));
    if address.relays.is_empty() {
        tag.push(String::new());
    } else {
        tag.extend(address.relays.iter().cloned());
    }
    tag.push(marker.to_string());
    tag
}

fn marker_event_tag(
    version_ref: &RadrootsWikiArticleVersionRef,
    marker: &'static str,
) -> Vec<String> {
    let mut tag = Vec::with_capacity(4 + version_ref.address_ref.relays.len());
    tag.push(TAG_E.to_string());
    tag.push(version_ref.event_id.clone());
    if version_ref.address_ref.relays.is_empty() {
        tag.push(String::new());
    } else {
        tag.extend(version_ref.address_ref.relays.iter().cloned());
    }
    tag.push(marker.to_string());
    tag
}

fn push_wiki_version_ref_tags(
    tags: &mut Vec<Vec<String>>,
    version_ref: &RadrootsWikiArticleVersionRef,
    marker: &'static str,
) {
    tags.push(marker_address_tag(&version_ref.address_ref, marker));
    tags.push(marker_event_tag(version_ref, marker));
}

fn review_target_ref(
    target: &RadrootsKnowledgeReviewTarget,
) -> Result<RadrootsEventRef, EventEncodeError> {
    Ok(RadrootsEventRef {
        id: target.event_id.clone(),
        author: PublicKey::from_hex(&target.author_pubkey)
            .map_err(|_| EventEncodeError::InvalidField("review_target"))?,
        kind: target.kind,
        d_tag: target
            .address
            .as_deref()
            .and_then(|address| address.rsplit_once(':').map(|(_, d_tag)| d_tag.to_string())),
        relays: if target.relays.is_empty() {
            None
        } else {
            Some(target.relays.clone())
        },
    })
}

fn json_content<T: Serialize>(value: &T) -> Result<String, EventEncodeError> {
    serde_json::to_string(value).map_err(|_| EventEncodeError::Json)
}

fn encode_validation_error(error: RadrootsKnowledgeValidationError) -> EventEncodeError {
    match error {
        RadrootsKnowledgeValidationError::EmptyField(field) => {
            EventEncodeError::EmptyRequiredField(field)
        }
        RadrootsKnowledgeValidationError::InvalidField(field) => {
            EventEncodeError::InvalidField(field)
        }
    }
}

fn custom_tags(contract_id: &'static str) -> Vec<Vec<String>> {
    vec![vec![TAG_CONTRACT.to_string(), contract_id.to_string()]]
}

pub fn wiki_article_build_tags(
    article: &RadrootsWikiArticle,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_wiki_article(article).map_err(encode_validation_error)?;
    let mut tags = Vec::new();
    tags.push(vec![TAG_D.to_string(), article.d_tag.clone()]);
    push_optional_value(&mut tags, TAG_TITLE, article.title.as_deref());
    push_optional_value(&mut tags, TAG_SUMMARY, article.summary.as_deref());
    push_topics(&mut tags, &article.topics);
    push_event_refs(&mut tags, TAG_SOURCE, &article.references);
    for forked_from in &article.forked_from {
        push_wiki_version_ref_tags(&mut tags, forked_from, MARKER_FORK);
    }
    if let Some(deferred_to) = &article.deferred_to {
        push_wiki_version_ref_tags(&mut tags, deferred_to, MARKER_DEFER);
    }
    Ok(tags)
}

pub fn wiki_article_to_wire_parts(
    article: &RadrootsWikiArticle,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_WIKI_ARTICLE,
        content: article.content_djot.clone(),
        tags: wiki_article_build_tags(article)?,
    })
}

pub fn wiki_redirect_build_tags(
    redirect: &RadrootsWikiRedirect,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_wiki_redirect(redirect).map_err(encode_validation_error)?;
    let tags = vec![
        vec![TAG_D.to_string(), redirect.d_tag.clone()],
        address_tag(TAG_A, &redirect.target),
    ];
    Ok(tags)
}

pub fn wiki_redirect_to_wire_parts(
    redirect: &RadrootsWikiRedirect,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_WIKI_REDIRECT,
        content: empty_content(),
        tags: wiki_redirect_build_tags(redirect)?,
    })
}

pub fn wiki_merge_request_build_tags(
    request: &RadrootsWikiMergeRequest,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_wiki_merge_request(request).map_err(encode_validation_error)?;
    let mut tags = Vec::new();
    tags.push(address_tag(TAG_A, &request.target_article));
    tags.push(vec![TAG_P.to_string(), request.destination_pubkey.clone()]);
    if let Some(base) = request
        .base_version_event_id
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        tags.push(vec![TAG_E.to_string(), base.clone(), String::new()]);
    }
    tags.push(vec![
        TAG_E.to_string(),
        request.source_version_event_id.clone(),
        String::new(),
        E_MARKER_SOURCE.to_string(),
    ]);
    Ok(tags)
}

pub fn wiki_merge_request_to_wire_parts(
    request: &RadrootsWikiMergeRequest,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_WIKI_MERGE_REQUEST,
        content: request.explanation.clone().unwrap_or_default(),
        tags: wiki_merge_request_build_tags(request)?,
    })
}

pub fn knowledge_source_build_tags(
    source: &RadrootsKnowledgeSource,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_knowledge_source(source).map_err(encode_validation_error)?;
    let mut tags = custom_tags(RADROOTS_KNOWLEDGE_SOURCE_SCHEMA);
    tags.push(vec![TAG_D.to_string(), source.d_tag.clone()]);
    push_topics(&mut tags, &source.topics);
    push_event_refs(&mut tags, TAG_SOURCE, &source.artifact_refs);
    Ok(tags)
}

pub fn knowledge_source_to_wire_parts(
    source: &RadrootsKnowledgeSource,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_KNOWLEDGE_SOURCE,
        content: json_content(source)?,
        tags: knowledge_source_build_tags(source)?,
    })
}

pub fn evidence_bounty_build_tags(
    bounty: &RadrootsEvidenceBounty,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_evidence_bounty(bounty).map_err(encode_validation_error)?;
    let mut tags = custom_tags(RADROOTS_EVIDENCE_BOUNTY_SCHEMA);
    tags.push(vec![TAG_D.to_string(), bounty.d_tag.clone()]);
    push_topics(&mut tags, &bounty.topics);
    push_event_refs(&mut tags, TAG_EVIDENCE, &bounty.target_refs);
    Ok(tags)
}

pub fn evidence_bounty_to_wire_parts(
    bounty: &RadrootsEvidenceBounty,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_EVIDENCE_BOUNTY,
        content: json_content(bounty)?,
        tags: evidence_bounty_build_tags(bounty)?,
    })
}

pub fn knowledge_claim_build_tags(
    claim: &RadrootsKnowledgeClaim,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_knowledge_claim(claim).map_err(encode_validation_error)?;
    let mut tags = custom_tags(RADROOTS_KNOWLEDGE_CLAIM_SCHEMA);
    push_topics(&mut tags, &claim.topics);
    for citation in &claim.citation_spans {
        tags.push(build_event_ref_tag(TAG_SOURCE, &citation.source_ref));
        if let Some(quote_hash) = citation.quote_hash.as_deref() {
            push_value(&mut tags, TAG_CITATION, quote_hash);
        }
    }
    Ok(tags)
}

pub fn knowledge_claim_to_wire_parts(
    claim: &RadrootsKnowledgeClaim,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_KNOWLEDGE_CLAIM,
        content: json_content(claim)?,
        tags: knowledge_claim_build_tags(claim)?,
    })
}

pub fn knowledge_relation_build_tags(
    relation: &RadrootsKnowledgeRelation,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_knowledge_relation(relation).map_err(encode_validation_error)?;
    let mut tags = custom_tags(RADROOTS_KNOWLEDGE_RELATION_SCHEMA);
    push_event_refs(&mut tags, TAG_SOURCE, &relation.support_refs);
    Ok(tags)
}

pub fn knowledge_relation_to_wire_parts(
    relation: &RadrootsKnowledgeRelation,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_KNOWLEDGE_RELATION,
        content: json_content(relation)?,
        tags: knowledge_relation_build_tags(relation)?,
    })
}

pub fn knowledge_review_build_tags(
    review: &RadrootsKnowledgeReview,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_knowledge_review(review).map_err(encode_validation_error)?;
    let mut tags = custom_tags(RADROOTS_KNOWLEDGE_REVIEW_SCHEMA);
    tags.push(build_event_ref_tag(
        TAG_REVIEW_TARGET,
        &review_target_ref(&review.target)?,
    ));
    push_event_refs(&mut tags, TAG_EVIDENCE, &review.evidence_refs);
    Ok(tags)
}

pub fn knowledge_review_to_wire_parts(
    review: &RadrootsKnowledgeReview,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_KNOWLEDGE_REVIEW,
        content: json_content(review)?,
        tags: knowledge_review_build_tags(review)?,
    })
}

pub fn knowledge_field_report_build_tags(
    report: &RadrootsKnowledgeFieldReport,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_knowledge_field_report(report).map_err(encode_validation_error)?;
    let mut tags = custom_tags(RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA);
    push_topics(&mut tags, &report.context.topics);
    if let Some(location) = &report.context.public_location
        && let Some(geohash) = location.geohash.as_deref()
    {
        push_value(&mut tags, TAG_G, geohash);
    }
    push_event_refs(&mut tags, TAG_EVIDENCE, &report.artifact_refs);
    push_event_refs(&mut tags, TAG_EVIDENCE, &report.related_refs);
    Ok(tags)
}

pub fn knowledge_field_report_to_wire_parts(
    report: &RadrootsKnowledgeFieldReport,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_KNOWLEDGE_FIELD_REPORT,
        content: json_content(report)?,
        tags: knowledge_field_report_build_tags(report)?,
    })
}

pub fn knowledge_change_proposal_build_tags(
    proposal: &RadrootsKnowledgeChangeProposal,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_knowledge_change_proposal(proposal).map_err(encode_validation_error)?;
    let mut tags = custom_tags(RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA);
    push_event_refs(&mut tags, TAG_EVIDENCE, &proposal.evidence_refs);
    Ok(tags)
}

pub fn knowledge_change_proposal_to_wire_parts(
    proposal: &RadrootsKnowledgeChangeProposal,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_KNOWLEDGE_CHANGE_PROPOSAL,
        content: json_content(proposal)?,
        tags: knowledge_change_proposal_build_tags(proposal)?,
    })
}

pub fn contribution_attestation_build_tags(
    attestation: &RadrootsContributionAttestation,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_contribution_attestation(attestation).map_err(encode_validation_error)?;
    let mut tags = custom_tags(RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA);
    push_event_refs(&mut tags, TAG_EVIDENCE, &attestation.evidence_refs);
    Ok(tags)
}

pub fn contribution_attestation_to_wire_parts(
    attestation: &RadrootsContributionAttestation,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_CONTRIBUTION_ATTESTATION,
        content: json_content(attestation)?,
        tags: contribution_attestation_build_tags(attestation)?,
    })
}
