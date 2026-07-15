#![forbid(unsafe_code)]

use radroots_event::article::RadrootsArticle;
use radroots_event::calendar::{
    RadrootsCalendar, RadrootsCalendarDateEvent, RadrootsCalendarEventRsvp,
    RadrootsCalendarTimeEvent,
};
use radroots_event::comment::RadrootsComment;
use radroots_event::coop::RadrootsCoop;
use radroots_event::document::RadrootsDocument;
use radroots_event::farm::RadrootsFarm;
use radroots_event::farm_crdt::RadrootsFarmCrdtChange;
use radroots_event::farm_file::RadrootsFarmFileMetadata;
use radroots_event::farm_workspace::RadrootsFarmWorkspaceManifest;
use radroots_event::file_metadata::RadrootsFileMetadata;
use radroots_event::follow::RadrootsFollow;
use radroots_event::gift_wrap::RadrootsGiftWrap;
use radroots_event::group::{
    RadrootsGroupAdmins, RadrootsGroupCreateGroup, RadrootsGroupCreateInvite,
    RadrootsGroupDeleteEvent, RadrootsGroupDeleteGroup, RadrootsGroupEditMetadata,
    RadrootsGroupJoinRequest, RadrootsGroupLeaveRequest, RadrootsGroupMembers,
    RadrootsGroupMetadata, RadrootsGroupPutUser, RadrootsGroupRemoveUser, RadrootsGroupRoles,
};
use radroots_event::http_auth::RadrootsHttpAuth;
use radroots_event::job_feedback::RadrootsJobFeedback;
use radroots_event::job_request::RadrootsJobRequest;
use radroots_event::job_result::RadrootsJobResult;
use radroots_event::knowledge::{
    RadrootsKnowledgeClaim, RadrootsKnowledgeFieldReport, RadrootsKnowledgeRelation,
    RadrootsKnowledgeReview, RadrootsKnowledgeSource, RadrootsWikiArticle,
    RadrootsWikiMergeRequest, RadrootsWikiRedirect,
};
use radroots_event::list::RadrootsList;
use radroots_event::list_set::RadrootsListSet;
use radroots_event::listing::RadrootsListing;
use radroots_event::message::RadrootsMessage;
use radroots_event::message_file::RadrootsMessageFile;
use radroots_event::plot::RadrootsPlot;
use radroots_event::post::RadrootsPost;
use radroots_event::reaction::RadrootsReaction;
use radroots_event::relay_auth::RadrootsRelayAuth;
use radroots_event::report::RadrootsReport;
use radroots_event::repost::{RadrootsGenericRepost, RadrootsRepost};
use radroots_event::seal::RadrootsSeal;
use radroots_event_codec::article::encode::article_build_tags;
use radroots_event_codec::calendar::encode::{
    calendar_collection_build_tags, calendar_date_event_build_tags, calendar_time_event_build_tags,
    rsvp_build_tags,
};
use radroots_event_codec::comment::encode::comment_build_tags;
use radroots_event_codec::coop::encode::coop_build_tags;
use radroots_event_codec::document::encode::document_build_tags;
use radroots_event_codec::farm::encode::farm_build_tags;
use radroots_event_codec::farm_crdt::encode::farm_crdt_change_build_tags_with_author;
use radroots_event_codec::farm_file::encode::farm_file_metadata_build_tags;
use radroots_event_codec::farm_workspace::encode::farm_workspace_build_tags;
use radroots_event_codec::file_metadata::encode::file_metadata_build_tags;
use radroots_event_codec::follow::encode::follow_build_tags;
use radroots_event_codec::gift_wrap::encode::gift_wrap_build_tags;
use radroots_event_codec::group::encode::{
    group_admins_build_tags, group_create_group_build_tags, group_create_invite_build_tags,
    group_delete_event_build_tags, group_delete_group_build_tags, group_edit_metadata_build_tags,
    group_join_request_build_tags, group_leave_request_build_tags, group_members_build_tags,
    group_metadata_build_tags, group_put_user_build_tags, group_remove_user_build_tags,
    group_roles_build_tags,
};
use radroots_event_codec::http_auth::encode::http_auth_build_tags;
use radroots_event_codec::job::feedback::encode::job_feedback_build_tags;
use radroots_event_codec::job::request::encode::job_request_build_tags;
use radroots_event_codec::job::result::encode::job_result_build_tags;
use radroots_event_codec::knowledge::{
    knowledge_claim_build_tags, knowledge_field_report_build_tags, knowledge_relation_build_tags,
    knowledge_review_build_tags, knowledge_source_build_tags, wiki_article_build_tags,
    wiki_merge_request_build_tags, wiki_redirect_build_tags,
};
use radroots_event_codec::list::encode::list_build_tags;
use radroots_event_codec::list_set::encode::list_set_build_tags;
use radroots_event_codec::listing::tags::{
    listing_tags as listing_tags_impl, listing_tags_full as listing_tags_full_impl,
};
use radroots_event_codec::message::encode::message_build_tags;
use radroots_event_codec::message_file::encode::message_file_build_tags;
use radroots_event_codec::plot::encode::plot_build_tags;
use radroots_event_codec::post::encode::post_build_tags;
use radroots_event_codec::reaction::encode::reaction_build_tags;
use radroots_event_codec::relay_auth::encode::relay_auth_build_tags;
use radroots_event_codec::report::encode::report_build_tags;
use radroots_event_codec::repost::encode::{generic_repost_build_tags, repost_build_tags};
use radroots_event_codec::seal::encode::seal_build_tags;
use radroots_event_codec::verification::{RadrootsDecodeError, RadrootsDecodedEvent};
use serde::Serialize;
use serde::de::DeserializeOwned;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
type RadrootsJsValue = JsValue;

#[cfg(not(target_arch = "wasm32"))]
type RadrootsJsValue = String;

fn err_js<E: ToString>(err: E) -> RadrootsJsValue {
    #[cfg(target_arch = "wasm32")]
    {
        JsValue::from_str(&err.to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        err.to_string()
    }
}

fn error_json(code: &str, inner_code: Option<&str>) -> RadrootsJsValue {
    let value = serde_json::json!({
        "code": code,
        "inner_code": inner_code,
    });
    err_js(value)
}

fn normalized_payload(input: &str) -> &str {
    if input.is_empty() { "{}" } else { input }
}

fn parse_json<T: DeserializeOwned>(input: &str) -> Result<T, RadrootsJsValue> {
    serde_json::from_str(normalized_payload(input)).map_err(err_js)
}

fn tags_to_json(tags: Vec<Vec<String>>) -> Result<String, RadrootsJsValue> {
    serde_json::to_string(&tags).map_err(err_js)
}

fn parse_event_json(input: &str) -> Result<radroots_event::RadrootsEventEnvelope, RadrootsJsValue> {
    serde_json::from_str(input).map_err(|_| error_json("invalid_json", Some("event_json")))
}

fn build_tags_json<T, E, F>(input: &str, build: F) -> Result<String, RadrootsJsValue>
where
    T: DeserializeOwned,
    E: ToString,
    F: FnOnce(&T) -> Result<Vec<Vec<String>>, E>,
{
    let value = parse_json::<T>(input)?;
    let tags = build(&value).map_err(err_js)?;
    tags_to_json(tags)
}

fn build_tags_json_infallible<T, F>(input: &str, build: F) -> Result<String, RadrootsJsValue>
where
    T: DeserializeOwned,
    F: FnOnce(&T) -> Vec<Vec<String>>,
{
    let value = parse_json::<T>(input)?;
    let tags = build(&value);
    tags_to_json(tags)
}

#[derive(Serialize)]
struct DecodedEventJson<'a, T>
where
    T: Serialize,
{
    event_type: &'static str,
    contract_id: &'static str,
    event: &'a radroots_event::RadrootsEventEnvelope,
    payload: &'a T,
}

fn decoded_event_to_json(decoded: RadrootsDecodedEvent) -> Result<String, RadrootsJsValue> {
    match decoded {
        RadrootsDecodedEvent::WikiArticle(parsed) => {
            decoded_payload_to_json("wiki_article", "radroots.wiki.article.v1", &parsed)
        }
        RadrootsDecodedEvent::WikiRedirect(parsed) => {
            decoded_payload_to_json("wiki_redirect", "radroots.wiki.redirect.v1", &parsed)
        }
        RadrootsDecodedEvent::WikiMergeRequest(parsed) => decoded_payload_to_json(
            "wiki_merge_request",
            "radroots.wiki.merge_request.v1",
            &parsed,
        ),
        RadrootsDecodedEvent::KnowledgeSource(parsed) => {
            decoded_payload_to_json("knowledge_source", "radroots.knowledge.source.v1", &parsed)
        }
        RadrootsDecodedEvent::KnowledgeClaim(parsed) => {
            decoded_payload_to_json("knowledge_claim", "radroots.knowledge.claim.v1", &parsed)
        }
        RadrootsDecodedEvent::KnowledgeRelation(parsed) => decoded_payload_to_json(
            "knowledge_relation",
            "radroots.knowledge.relation.v1",
            &parsed,
        ),
        RadrootsDecodedEvent::KnowledgeReview(parsed) => {
            decoded_payload_to_json("knowledge_review", "radroots.knowledge.review.v1", &parsed)
        }
        RadrootsDecodedEvent::KnowledgeFieldReport(parsed) => decoded_payload_to_json(
            "knowledge_field_report",
            "radroots.knowledge.field_report.v1",
            &parsed,
        ),
        RadrootsDecodedEvent::EvidenceBounty(parsed) => decoded_payload_to_json(
            "evidence_bounty",
            "radroots.knowledge.evidence_bounty.v1",
            &parsed,
        ),
        RadrootsDecodedEvent::KnowledgeChangeProposal(parsed) => decoded_payload_to_json(
            "knowledge_change_proposal",
            "radroots.knowledge.change_proposal.v1",
            &parsed,
        ),
        RadrootsDecodedEvent::ContributionAttestation(parsed) => decoded_payload_to_json(
            "contribution_attestation",
            "radroots.knowledge.contribution_attestation.v1",
            &parsed,
        ),
    }
}

fn decoded_payload_to_json<T>(
    event_type: &'static str,
    contract_id: &'static str,
    parsed: &radroots_event_codec::parsed::RadrootsParsedEvent<T>,
) -> Result<String, RadrootsJsValue>
where
    T: Serialize,
{
    serde_json::to_string(&DecodedEventJson {
        event_type,
        contract_id,
        event: &parsed.event,
        payload: &parsed.data.data,
    })
    .map_err(err_js)
}

fn decode_error_json(error: RadrootsDecodeError) -> RadrootsJsValue {
    let inner = match &error {
        RadrootsDecodeError::Nip01Verification(error) => Some(error.code()),
        RadrootsDecodeError::ContractValidation(error) => Some(error.code()),
        RadrootsDecodeError::EventParse(error) => Some(error.code()),
        RadrootsDecodeError::UnsupportedContract { .. } => None,
    };
    error_json(error.code(), inner)
}

#[derive(serde::Deserialize)]
struct FarmCrdtTagsInput {
    change: RadrootsFarmCrdtChange,
    author_pubkey: String,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = listing_tags))]
pub fn listing_tags(listing_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsListing, _, _>(listing_json, listing_tags_impl)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = listing_tags_full))]
pub fn listing_tags_full(listing_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsListing, _, _>(listing_json, listing_tags_full_impl)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = post_tags))]
pub fn post_tags(post_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsPost, _, _>(post_json, post_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = comment_tags))]
pub fn comment_tags(comment_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsComment, _, _>(comment_json, comment_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = article_tags))]
pub fn article_tags(article_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsArticle, _, _>(article_json, article_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = wiki_article_tags))]
pub fn wiki_article_tags(article_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsWikiArticle, _, _>(article_json, wiki_article_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = wiki_redirect_tags))]
pub fn wiki_redirect_tags(redirect_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsWikiRedirect, _, _>(redirect_json, wiki_redirect_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = wiki_merge_request_tags))]
pub fn wiki_merge_request_tags(request_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsWikiMergeRequest, _, _>(request_json, wiki_merge_request_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = knowledge_source_tags))]
pub fn knowledge_source_tags(source_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsKnowledgeSource, _, _>(source_json, knowledge_source_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = knowledge_claim_tags))]
pub fn knowledge_claim_tags(claim_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsKnowledgeClaim, _, _>(claim_json, knowledge_claim_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = knowledge_relation_tags))]
pub fn knowledge_relation_tags(relation_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsKnowledgeRelation, _, _>(relation_json, knowledge_relation_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = knowledge_review_tags))]
pub fn knowledge_review_tags(review_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsKnowledgeReview, _, _>(review_json, knowledge_review_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = knowledge_field_report_tags))]
pub fn knowledge_field_report_tags(report_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsKnowledgeFieldReport, _, _>(
        report_json,
        knowledge_field_report_build_tags,
    )
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = verify_and_decode_event_json))]
pub fn verify_and_decode_event_json(event_json: &str) -> Result<String, RadrootsJsValue> {
    let event = parse_event_json(event_json)?;
    let decoded =
        radroots_event_codec::verify_and_decode_radroots_event(event).map_err(decode_error_json)?;
    decoded_event_to_json(decoded)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = contract_manifest_json))]
pub fn contract_manifest_json() -> Result<String, RadrootsJsValue> {
    radroots_event_codec::contract_manifest_json().map_err(err_js)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = file_metadata_tags))]
pub fn file_metadata_tags(metadata_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsFileMetadata, _, _>(metadata_json, file_metadata_build_tags)
}

#[cfg_attr(
    target_arch = "wasm32",
    wasm_bindgen(js_name = calendar_date_event_tags)
)]
pub fn calendar_date_event_tags(event_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsCalendarDateEvent, _, _>(event_json, calendar_date_event_build_tags)
}

#[cfg_attr(
    target_arch = "wasm32",
    wasm_bindgen(js_name = calendar_time_event_tags)
)]
pub fn calendar_time_event_tags(event_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsCalendarTimeEvent, _, _>(event_json, calendar_time_event_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = calendar_tags))]
pub fn calendar_tags(calendar_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsCalendar, _, _>(calendar_json, calendar_collection_build_tags)
}

#[cfg_attr(
    target_arch = "wasm32",
    wasm_bindgen(js_name = calendar_event_rsvp_tags)
)]
pub fn calendar_event_rsvp_tags(rsvp_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsCalendarEventRsvp, _, _>(rsvp_json, rsvp_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = repost_tags))]
pub fn repost_tags(repost_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsRepost, _, _>(repost_json, repost_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = generic_repost_tags))]
pub fn generic_repost_tags(repost_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGenericRepost, _, _>(repost_json, generic_repost_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = report_tags))]
pub fn report_tags(report_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsReport, _, _>(report_json, report_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = follow_tags))]
pub fn follow_tags(follow_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsFollow, _, _>(follow_json, follow_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = document_tags))]
pub fn document_tags(document_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsDocument, _, _>(document_json, document_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = coop_tags))]
pub fn coop_tags(coop_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsCoop, _, _>(coop_json, coop_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = farm_tags))]
pub fn farm_tags(farm_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsFarm, _, _>(farm_json, farm_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = list_tags))]
pub fn list_tags(list_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsList, _, _>(list_json, list_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = list_set_tags))]
pub fn list_set_tags(list_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsListSet, _, _>(list_json, list_set_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = plot_tags))]
pub fn plot_tags(plot_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsPlot, _, _>(plot_json, plot_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = job_request_tags))]
pub fn job_request_tags(job_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json_infallible::<RadrootsJobRequest, _>(job_json, job_request_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = job_result_tags))]
pub fn job_result_tags(job_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json_infallible::<RadrootsJobResult, _>(job_json, job_result_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = job_feedback_tags))]
pub fn job_feedback_tags(job_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json_infallible::<RadrootsJobFeedback, _>(job_json, job_feedback_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = reaction_tags))]
pub fn reaction_tags(reaction_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsReaction, _, _>(reaction_json, reaction_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = message_tags))]
pub fn message_tags(message_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsMessage, _, _>(message_json, message_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = message_file_tags))]
pub fn message_file_tags(message_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsMessageFile, _, _>(message_json, message_file_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = seal_tags))]
pub fn seal_tags(seal_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsSeal, _, _>(seal_json, seal_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = gift_wrap_tags))]
pub fn gift_wrap_tags(gift_wrap_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGiftWrap, _, _>(gift_wrap_json, gift_wrap_build_tags)
}

#[cfg_attr(
    target_arch = "wasm32",
    wasm_bindgen(js_name = farm_workspace_manifest_tags)
)]
pub fn farm_workspace_manifest_tags(workspace_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsFarmWorkspaceManifest, _, _>(
        workspace_json,
        farm_workspace_build_tags,
    )
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = farm_crdt_change_tags))]
pub fn farm_crdt_change_tags(input_json: &str) -> Result<String, RadrootsJsValue> {
    let input = parse_json::<FarmCrdtTagsInput>(input_json)?;
    let tags = farm_crdt_change_build_tags_with_author(&input.change, Some(&input.author_pubkey))
        .map_err(err_js)?;
    tags_to_json(tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = farm_file_metadata_tags))]
pub fn farm_file_metadata_tags(file_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsFarmFileMetadata, _, _>(file_json, farm_file_metadata_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = relay_auth_tags))]
pub fn relay_auth_tags(auth_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsRelayAuth, _, _>(auth_json, relay_auth_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = http_auth_tags))]
pub fn http_auth_tags(auth_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsHttpAuth, _, _>(auth_json, http_auth_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_put_user_tags))]
pub fn group_put_user_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupPutUser, _, _>(group_json, group_put_user_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_remove_user_tags))]
pub fn group_remove_user_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupRemoveUser, _, _>(group_json, group_remove_user_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_create_group_tags))]
pub fn group_create_group_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupCreateGroup, _, _>(group_json, group_create_group_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_edit_metadata_tags))]
pub fn group_edit_metadata_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupEditMetadata, _, _>(group_json, group_edit_metadata_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_delete_group_tags))]
pub fn group_delete_group_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupDeleteGroup, _, _>(group_json, group_delete_group_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_delete_event_tags))]
pub fn group_delete_event_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupDeleteEvent, _, _>(group_json, group_delete_event_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_create_invite_tags))]
pub fn group_create_invite_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupCreateInvite, _, _>(group_json, group_create_invite_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_join_request_tags))]
pub fn group_join_request_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupJoinRequest, _, _>(group_json, group_join_request_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_leave_request_tags))]
pub fn group_leave_request_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupLeaveRequest, _, _>(group_json, group_leave_request_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_metadata_tags))]
pub fn group_metadata_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupMetadata, _, _>(group_json, group_metadata_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_admins_tags))]
pub fn group_admins_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupAdmins, _, _>(group_json, group_admins_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_members_tags))]
pub fn group_members_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupMembers, _, _>(group_json, group_members_build_tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = group_roles_tags))]
pub fn group_roles_tags(group_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsGroupRoles, _, _>(group_json, group_roles_build_tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_event::farm::RadrootsFarmRef;
    use radroots_event::farm_crdt::{
        RADROOTS_FARM_CRDT_CHANGE_SCHEMA, RadrootsCrdtBackend, RadrootsFarmCrdtDocumentKind,
        RadrootsFarmSemanticKind,
    };
    use radroots_event::farm_file::{
        RadrootsFarmFileDimensions, RadrootsFarmFileMetadata, RadrootsFarmFileSource,
    };
    use radroots_event::farm_workspace::{
        RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION, RADROOTS_FARM_WORKSPACE_SCHEMA,
        RadrootsFarmWorkspaceManifest, RadrootsFarmWorkspaceMediaServer, RadrootsFarmWorkspaceRef,
        RadrootsFarmWorkspaceRelay, RadrootsFarmWorkspaceRelayMode,
    };
    use radroots_event::group::{
        RadrootsGroupAdmins, RadrootsGroupCreateGroup, RadrootsGroupCreateInvite,
        RadrootsGroupDeleteEvent, RadrootsGroupDeleteGroup, RadrootsGroupEditMetadata,
        RadrootsGroupEditableMetadata, RadrootsGroupJoinRequest, RadrootsGroupLeaveRequest,
        RadrootsGroupMembers, RadrootsGroupMetadata, RadrootsGroupPutUser, RadrootsGroupRemoveUser,
        RadrootsGroupRole, RadrootsGroupRoles, RadrootsGroupUserRef,
    };
    use radroots_event::http_auth::RadrootsHttpAuth;
    use radroots_event::job::JobInputType;
    use radroots_event::job_request::{RadrootsJobInput, RadrootsJobParam};
    use radroots_event::kinds::{
        KIND_FARM_FILE_METADATA, KIND_FILE_METADATA, KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_SOURCE,
        KIND_WIKI_ARTICLE,
    };
    use radroots_event::knowledge::{
        RADROOTS_KNOWLEDGE_CLAIM_SCHEMA, RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
        RADROOTS_KNOWLEDGE_RELATION_SCHEMA, RADROOTS_KNOWLEDGE_REVIEW_SCHEMA,
        RADROOTS_KNOWLEDGE_SCHEMA_VERSION, RADROOTS_KNOWLEDGE_SOURCE_SCHEMA,
        RadrootsAddressableRef, RadrootsKnowledgeCitationSpan, RadrootsKnowledgeFieldContext,
        RadrootsKnowledgeLocation, RadrootsKnowledgeLocationPrecision, RadrootsKnowledgeNodeRef,
        RadrootsKnowledgeObservation, RadrootsKnowledgeObservationValue,
        RadrootsKnowledgeReviewScope, RadrootsKnowledgeReviewScore, RadrootsKnowledgeReviewTarget,
        RadrootsWikiArticleVersionRef,
    };
    use radroots_event::listing::{RadrootsListingBin, RadrootsListingProduct};
    use radroots_event::relay_auth::RadrootsRelayAuth;
    use radroots_event::social::{
        RadrootsCalendarDateValue, RadrootsCalendarEventFreeBusy, RadrootsCalendarEventRsvpStatus,
        RadrootsCalendarParticipant, RadrootsReportFileTarget, RadrootsReportType,
        RadrootsSocialFarmAnchor, RadrootsSocialLocation, RadrootsSocialMediaDimensions,
        RadrootsSocialMediaMetadata, RadrootsSocialTarget,
    };
    use radroots_event::{RadrootsEventEnvelope, RadrootsEventEnvelopeParts};

    fn sample_listing() -> RadrootsListing {
        let quantity =
            RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
        let price = RadrootsCoreQuantityPrice::new(
            RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
            quantity.clone(),
        );

        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".parse().expect("listing d tag"),
            published_at: None,
            farm: RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            product: RadrootsListingProduct {
                key: "sku".to_string(),
                title: "widget".to_string(),
                category: "tools".to_string(),
                summary: None,
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: "bin-1".parse().expect("primary bin id"),
            bins: vec![RadrootsListingBin {
                bin_id: "bin-1".parse().expect("bin id"),
                quantity,
                price_per_canonical_unit: price,
                display_amount: None,
                display_unit: None,
                display_label: None,
                display_price: None,
                display_price_unit: None,
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: None,
            availability: None,
            delivery_method: None,
            location: None,
            images: None,
        }
    }

    fn synthetic_pubkey(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn synthetic_event_id(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn social_farm_anchor() -> RadrootsSocialFarmAnchor {
        RadrootsSocialFarmAnchor {
            farm: RadrootsFarmRef {
                pubkey: synthetic_pubkey('a'),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            relays: Some(vec!["wss://relay.example.test".to_string()]),
        }
    }

    fn event_target(kind: u32, seed: char) -> RadrootsSocialTarget {
        RadrootsSocialTarget::Event {
            id: synthetic_event_id(seed),
            author: Some(synthetic_pubkey('b')),
            event_kind: Some(kind),
            relays: Some(vec!["wss://relay.example.test".to_string()]),
        }
    }

    fn address_target(kind: u32, d_tag: &str) -> RadrootsSocialTarget {
        let author = synthetic_pubkey('c');
        RadrootsSocialTarget::Address {
            address: format!("{kind}:{author}:{d_tag}"),
            author: Some(author),
            event_kind: Some(kind),
            relays: Some(vec!["wss://relay2.example.test".to_string()]),
        }
    }

    fn knowledge_event_ref(seed: char, kind: u32) -> radroots_event::RadrootsEventRef {
        radroots_event::RadrootsEventRef {
            id: synthetic_event_id(seed),
            author: synthetic_pubkey('a'),
            kind,
            d_tag: None,
            relays: Some(vec!["wss://relay.example.test".to_string()]),
        }
    }

    fn knowledge_address_ref() -> RadrootsAddressableRef {
        RadrootsAddressableRef {
            kind: KIND_WIKI_ARTICLE,
            pubkey: synthetic_pubkey('a'),
            d_tag: "soil-health".to_string(),
            relays: vec!["wss://relay.example.test".to_string()],
        }
    }

    fn sample_wiki_article() -> RadrootsWikiArticle {
        RadrootsWikiArticle {
            d_tag: "soil-health".to_string(),
            title: Some("Soil health".to_string()),
            content_djot: "# Soil health".to_string(),
            summary: Some("Living soil basics".to_string()),
            topics: vec!["soil".to_string()],
            references: vec![knowledge_event_ref('1', KIND_KNOWLEDGE_SOURCE)],
            forked_from: vec![RadrootsWikiArticleVersionRef {
                event_id: synthetic_event_id('b'),
                address_ref: knowledge_address_ref(),
            }],
            deferred_to: None,
        }
    }

    fn sample_wiki_redirect() -> RadrootsWikiRedirect {
        RadrootsWikiRedirect {
            d_tag: "soil".to_string(),
            target: knowledge_address_ref(),
        }
    }

    fn sample_wiki_merge_request() -> RadrootsWikiMergeRequest {
        RadrootsWikiMergeRequest {
            target_article: knowledge_address_ref(),
            destination_pubkey: synthetic_pubkey('a'),
            base_version_event_id: Some(synthetic_event_id('e')),
            source_version_event_id: synthetic_event_id('f'),
            explanation: Some("Merge synthetic soil article updates".to_string()),
        }
    }

    fn sample_knowledge_source() -> RadrootsKnowledgeSource {
        RadrootsKnowledgeSource {
            schema: RADROOTS_KNOWLEDGE_SOURCE_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            d_tag: "soil-source".to_string(),
            title: "Soil Source".to_string(),
            source_type: "book".to_string(),
            authors: vec!["A. Example".to_string()],
            publisher: Some("Radroots Synthetic Press".to_string()),
            publication_year: Some(2026),
            edition: None,
            canonical_url: Some("https://source.example.test/soil-source".to_string()),
            artifact_refs: vec![knowledge_event_ref('3', KIND_FILE_METADATA)],
            author_asserted_rights: None,
            topics: vec!["soil".to_string()],
            summary: Some("Synthetic source for wasm coverage".to_string()),
        }
    }

    fn sample_knowledge_claim() -> RadrootsKnowledgeClaim {
        RadrootsKnowledgeClaim {
            schema: RADROOTS_KNOWLEDGE_CLAIM_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            claim_type: "practice_effect".to_string(),
            text: "Cover crops improve soil structure.".to_string(),
            citation_spans: vec![RadrootsKnowledgeCitationSpan {
                source_ref: knowledge_event_ref('4', KIND_KNOWLEDGE_SOURCE),
                artifact_ref: None,
                page_start: Some(12),
                page_end: Some(13),
                section_path: vec!["chapter-1".to_string()],
                quote_hash: Some(synthetic_event_id('5')),
                chunk_id: Some("chunk-1".to_string()),
            }],
            topics: vec!["cover-crops".to_string()],
            applies_to: vec!["local-food".to_string()],
            author_asserted_confidence: Some("medium".to_string()),
            supersedes: Vec::new(),
        }
    }

    fn sample_knowledge_node_ref(label: &str) -> RadrootsKnowledgeNodeRef {
        RadrootsKnowledgeNodeRef {
            node_type: "event".to_string(),
            event_ref: Some(knowledge_event_ref('6', KIND_KNOWLEDGE_CLAIM)),
            address_ref: None,
            external_id: None,
            label: Some(label.to_string()),
        }
    }

    fn sample_knowledge_relation() -> RadrootsKnowledgeRelation {
        RadrootsKnowledgeRelation {
            schema: RADROOTS_KNOWLEDGE_RELATION_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            subject: sample_knowledge_node_ref("cover crops"),
            predicate: "supports".to_string(),
            object: sample_knowledge_node_ref("soil structure"),
            support_refs: vec![knowledge_event_ref('7', KIND_KNOWLEDGE_CLAIM)],
            author_asserted_confidence: Some("medium".to_string()),
            supersedes: Vec::new(),
        }
    }

    fn sample_knowledge_review() -> RadrootsKnowledgeReview {
        RadrootsKnowledgeReview {
            schema: RADROOTS_KNOWLEDGE_REVIEW_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            target: RadrootsKnowledgeReviewTarget {
                event_id: synthetic_event_id('8'),
                author_pubkey: synthetic_pubkey('a'),
                kind: KIND_KNOWLEDGE_CLAIM,
                address: None,
                relays: vec!["wss://relay.example.test".to_string()],
                review_scope: RadrootsKnowledgeReviewScope::SpecificVersion,
            },
            reviewer_role: "peer".to_string(),
            verdict: "needs_more_evidence".to_string(),
            scores: vec![RadrootsKnowledgeReviewScore {
                dimension: "evidence".to_string(),
                value: "partial".to_string(),
                note: None,
            }],
            notes: Some("Synthetic review".to_string()),
            evidence_refs: vec![knowledge_event_ref('9', KIND_KNOWLEDGE_SOURCE)],
        }
    }

    fn sample_knowledge_field_report() -> RadrootsKnowledgeFieldReport {
        RadrootsKnowledgeFieldReport {
            schema: RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA.to_string(),
            schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
            report_type: "observation".to_string(),
            title: "Field observation".to_string(),
            summary: Some("Observed cover crop residue.".to_string()),
            context: RadrootsKnowledgeFieldContext {
                location_precision: RadrootsKnowledgeLocationPrecision::CoarseGeohash,
                public_location: Some(RadrootsKnowledgeLocation {
                    label: Some("watershed".to_string()),
                    region: Some("synthetic-region".to_string()),
                    locality: None,
                    geohash: Some("c23".to_string()),
                }),
                private_location_ref: None,
                topics: vec!["field".to_string()],
                context_tags: vec!["observation".to_string()],
            },
            observations: vec![RadrootsKnowledgeObservation {
                observation_type: "residue".to_string(),
                text: "Residue was visible across beds.".to_string(),
                observed_at: Some("2026-07-05".to_string()),
                values: vec![RadrootsKnowledgeObservationValue {
                    key: "coverage".to_string(),
                    value: "medium".to_string(),
                    unit: None,
                }],
            }],
            artifact_refs: vec![knowledge_event_ref('c', KIND_FILE_METADATA)],
            related_refs: vec![knowledge_event_ref('d', KIND_KNOWLEDGE_CLAIM)],
            limitations: vec!["single observer".to_string()],
        }
    }

    fn signed_claim_event_json() -> String {
        let claim_json = serde_json::to_string(&sample_knowledge_claim()).expect("claim json");
        let tags = serde_json::from_str::<Vec<Vec<String>>>(
            &knowledge_claim_tags(&claim_json).expect("claim tags"),
        )
        .expect("tags");
        let tags = tags
            .into_iter()
            .map(nostr::Tag::parse)
            .collect::<Result<Vec<_>, _>>()
            .expect("parsed tags");
        let keys =
            nostr::Keys::parse("0101010101010101010101010101010101010101010101010101010101010101")
                .expect("keys");
        let event =
            nostr::EventBuilder::new(nostr::Kind::Custom(KIND_KNOWLEDGE_CLAIM as u16), claim_json)
                .tags(tags)
                .custom_created_at(nostr::Timestamp::from_secs(1_800_000_000))
                .sign_with_keys(&keys)
                .expect("signed event");
        let envelope = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: event.id.to_hex(),
            author: event.pubkey.to_hex(),
            created_at: event.created_at.as_secs(),
            kind: u32::from(event.kind.as_u16()),
            tags: event
                .tags
                .as_slice()
                .iter()
                .map(|tag| tag.as_slice().to_vec())
                .collect(),
            content: event.content,
            sig: event.sig.to_string(),
        })
        .expect("event envelope");
        serde_json::to_string(&envelope).expect("event json")
    }

    fn social_location() -> RadrootsSocialLocation {
        RadrootsSocialLocation {
            name: Some("field edge".to_string()),
            geohash: Some("c23nb62w20st".to_string()),
        }
    }

    fn sample_post() -> RadrootsPost {
        RadrootsPost {
            content: "field update".to_string(),
            farm: Some(social_farm_anchor()),
            address_refs: Some(vec![address_target(30023, "AAAAAAAAAAAAAAAAAAAAAQ")]),
            location: Some(social_location()),
            topics: Some(vec!["soil".to_string(), "market".to_string()]),
            quote_refs: Some(vec![event_target(30023, 'd')]),
            media: Some(vec![RadrootsSocialMediaMetadata {
                url: Some("https://media.example.test/field.jpg".to_string()),
                mime_type: Some("image/jpeg".to_string()),
                sha256: Some(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                ),
                original_sha256: None,
                size: Some(4096),
                dimensions: Some(RadrootsSocialMediaDimensions {
                    width: 1200,
                    height: 800,
                }),
                blurhash: None,
                thumbnails: None,
                image: None,
                summary: Some("field photo".to_string()),
                alt: Some("rows after harvest".to_string()),
                fallback: None,
                magnet: Some("magnet:?xt=urn:btih:abc".to_string()),
                content_hashes: Some(vec!["sha256:field".to_string()]),
                services: Some(vec!["https://media.example.test".to_string()]),
                imeta: None,
            }]),
        }
    }

    fn sample_article() -> RadrootsArticle {
        RadrootsArticle {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
            title: "soil notes".to_string(),
            content: "# soil notes".to_string(),
            summary: Some("cover crop observations".to_string()),
            image: Some("https://media.example.test/article.jpg".to_string()),
            published_at: Some(1_780_000_000),
            farm: Some(social_farm_anchor()),
            location: Some(social_location()),
            topics: Some(vec!["soil".to_string(), "cover-crops".to_string()]),
        }
    }

    fn sample_public_file_metadata() -> RadrootsFileMetadata {
        RadrootsFileMetadata {
            url: "https://media.example.test/public.jpg".to_string(),
            mime_type: "image/jpeg".to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            original_sha256: Some(
                "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
            ),
            size: Some(4096),
            dimensions: Some(RadrootsSocialMediaDimensions {
                width: 1200,
                height: 800,
            }),
            blurhash: None,
            thumbnails: None,
            summary: Some("public field photo".to_string()),
            alt: Some("rows after harvest".to_string()),
            fallback: Some("https://media.example.test/fallback.jpg".to_string()),
            magnet: Some("magnet:?xt=urn:btih:abc".to_string()),
            content_hashes: Some(vec!["sha256:field".to_string()]),
            services: Some(vec!["https://media.example.test".to_string()]),
            content: Some("caption".to_string()),
        }
    }

    fn sample_calendar_date_event() -> RadrootsCalendarDateEvent {
        RadrootsCalendarDateEvent {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
            title: "market day".to_string(),
            start: "2026-06-20".to_string(),
            description: Some("Farm stand pickup window.".to_string()),
            end: Some("2026-06-21".to_string()),
            days: Some(vec![RadrootsCalendarDateValue {
                value: "2026-06-20".to_string(),
            }]),
            location: Some(social_location()),
            summary: Some("weekly pickup".to_string()),
            image: None,
            participants: Some(vec![RadrootsCalendarParticipant {
                pubkey: synthetic_pubkey('e'),
                relay: Some("wss://relay.example.test".to_string()),
                role: Some("host".to_string()),
            }]),
        }
    }

    fn sample_calendar_time_event() -> RadrootsCalendarTimeEvent {
        RadrootsCalendarTimeEvent {
            d_tag: "AAAAAAAAAAAAAAAAAAAA-A".to_string(),
            title: "wash pack shift".to_string(),
            start: 1_781_895_600,
            dates: vec![RadrootsCalendarDateValue {
                value: "2026-06-20".to_string(),
            }],
            description: Some("Prepare CSA bins before pickup.".to_string()),
            end: Some(1_781_899_200),
            start_tzid: Some("America/Vancouver".to_string()),
            end_tzid: Some("America/Vancouver".to_string()),
            location: Some(social_location()),
            summary: Some("field crew".to_string()),
            image: None,
            participants: None,
        }
    }

    fn sample_calendar() -> RadrootsCalendar {
        RadrootsCalendar {
            d_tag: "AAAAAAAAAAAAAAAAAAAA_A".to_string(),
            title: "farm calendar".to_string(),
            events: vec![address_target(31923, "AAAAAAAAAAAAAAAAAAAA-A")],
            description: Some("Shared schedule for farm operations.".to_string()),
            summary: Some("field schedule".to_string()),
            image: None,
        }
    }

    fn sample_calendar_rsvp() -> RadrootsCalendarEventRsvp {
        RadrootsCalendarEventRsvp {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
            event: address_target(31923, "AAAAAAAAAAAAAAAAAAAA-A"),
            event_id: Some(synthetic_event_id('f')),
            status: RadrootsCalendarEventRsvpStatus::Tentative,
            free_busy: Some(RadrootsCalendarEventFreeBusy::Busy),
            note: Some("depends on harvest".to_string()),
            participants: None,
        }
    }

    fn sample_comment() -> RadrootsComment {
        RadrootsComment {
            root: event_target(30023, 'a'),
            parent: address_target(30023, "AAAAAAAAAAAAAAAAAAAAAg"),
            content: "great notes".to_string(),
        }
    }

    fn sample_reaction() -> RadrootsReaction {
        RadrootsReaction {
            target: address_target(30023, "AAAAAAAAAAAAAAAAAAAAAg"),
            content: String::new(),
        }
    }

    fn sample_repost() -> RadrootsRepost {
        RadrootsRepost {
            target: event_target(1, 'b'),
            content: Some("field update".to_string()),
        }
    }

    fn sample_generic_repost() -> RadrootsGenericRepost {
        RadrootsGenericRepost {
            target: address_target(30023, "AAAAAAAAAAAAAAAAAAAAAg"),
            target_kind: 30023,
            content: Some("article share".to_string()),
        }
    }

    fn sample_report() -> RadrootsReport {
        RadrootsReport {
            reported_pubkey: synthetic_pubkey('b'),
            report_type: RadrootsReportType::Spam,
            event: Some(event_target(1, 'c')),
            file: Some(RadrootsReportFileTarget {
                sha256: Some(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                ),
                url: Some("https://media.example.test/bad.jpg".to_string()),
                magnet: None,
            }),
            content: Some("spam report".to_string()),
        }
    }

    fn sample_job_request() -> RadrootsJobRequest {
        RadrootsJobRequest {
            kind: 5100,
            inputs: vec![RadrootsJobInput {
                data: "alpha".to_string(),
                input_type: JobInputType::Text,
                relay: None,
                marker: None,
            }],
            output: None,
            params: vec![RadrootsJobParam {
                key: "mode".to_string(),
                value: "fast".to_string(),
            }],
            bid_sat: Some(42),
            relays: vec!["wss://relay.example.com".to_string()],
            providers: vec!["provider-a".to_string()],
            topics: vec!["topic-a".to_string()],
            encrypted: false,
        }
    }

    fn sample_workspace_manifest() -> RadrootsFarmWorkspaceManifest {
        RadrootsFarmWorkspaceManifest {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            schema: RADROOTS_FARM_WORKSPACE_SCHEMA.to_string(),
            farm_group_id: "field-group".to_string(),
            name: "Small Regen Farm".to_string(),
            owner_pubkey: "workspace_owner_pubkey".to_string(),
            farm: Some(RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
            }),
            relays: vec![RadrootsFarmWorkspaceRelay {
                url: "wss://relay.example.invalid/farm/field-group".to_string(),
                mode: RadrootsFarmWorkspaceRelayMode::ReadWrite,
            }],
            media_servers: vec![RadrootsFarmWorkspaceMediaServer {
                url: "https://media.example.invalid/farm/field-group".to_string(),
                service: "RadrootsPrivateMedia".to_string(),
            }],
            supported_kinds: vec![78, 30078, KIND_FARM_FILE_METADATA],
            protocol_version: RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION.to_string(),
            created_at_ms: 1_780_000_000_000,
            updated_at_ms: None,
        }
    }

    fn sample_crdt_change() -> RadrootsFarmCrdtChange {
        RadrootsFarmCrdtChange {
            schema: RADROOTS_FARM_CRDT_CHANGE_SCHEMA.to_string(),
            workspace: RadrootsFarmWorkspaceRef {
                pubkey: "workspace_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            farm_group_id: "field-group".to_string(),
            document_id: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
            document_kind: RadrootsFarmCrdtDocumentKind::FarmTask,
            crdt_backend: RadrootsCrdtBackend::Automerge,
            crdt_backend_version: Some("0.x".to_string()),
            actor_id: "actor_abc".to_string(),
            change_hash: "crdt_hash_abc".to_string(),
            dependencies: Vec::new(),
            encoded_change: "abc-DEF_012".to_string(),
            semantic_kind: RadrootsFarmSemanticKind::FarmTaskCreate,
            business_time_ms: 1_780_000_000_000,
            author_member_id: Some("member_abc".to_string()),
            app_version: Some("0.1.0".to_string()),
        }
    }

    fn sample_file_metadata() -> RadrootsFarmFileMetadata {
        RadrootsFarmFileMetadata {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
            workspace: RadrootsFarmWorkspaceRef {
                pubkey: "workspace_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            farm_group_id: "field-group".to_string(),
            owner_document_id: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
            owner_document_kind: RadrootsFarmCrdtDocumentKind::FarmTask,
            caption: Some("Tomatoes harvested from Patch Y.".to_string()),
            url: "https://media.example.invalid/blob/sha256".to_string(),
            mime_type: "image/jpeg".to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            original_sha256: None,
            size_bytes: Some(123_456),
            dimensions: Some(RadrootsFarmFileDimensions { w: 1600, h: 1200 }),
            blurhash: None,
            thumb: Some(RadrootsFarmFileSource {
                url: "https://media.example.invalid/thumb/sha256".to_string(),
                mime_type: Some("image/jpeg".to_string()),
                dimensions: Some(RadrootsFarmFileDimensions { w: 320, h: 240 }),
            }),
            image: None,
            alt: Some("Harvested tomatoes in a crate".to_string()),
            fallbacks: Vec::new(),
        }
    }

    fn sample_group_metadata() -> RadrootsGroupEditableMetadata {
        RadrootsGroupEditableMetadata {
            name: Some("Small Regen Farm".to_string()),
            about: Some("Field app group".to_string()),
            picture: Some("https://media.example.invalid/group.png".to_string()),
            is_private: false,
            is_restricted: true,
            is_closed: false,
            is_hidden: false,
            supported_kinds: Some(vec![78, 30078, KIND_FARM_FILE_METADATA]),
        }
    }

    fn sample_group_user(role: &str) -> RadrootsGroupUserRef {
        RadrootsGroupUserRef {
            pubkey: format!("{role}_pubkey"),
            roles: vec![role.to_string()],
        }
    }

    fn sample_group_role() -> RadrootsGroupRole {
        RadrootsGroupRole {
            name: "member".to_string(),
            description: Some("can read and write group events".to_string()),
            permissions: vec!["read".to_string(), "write".to_string()],
        }
    }

    fn assert_tags_json(value: Result<String, RadrootsJsValue>) {
        let tags = tags_json(value);
        assert!(!tags.is_empty());
    }

    fn tags_json(value: Result<String, RadrootsJsValue>) -> Vec<Vec<String>> {
        let json = value.expect("tags json");
        serde_json::from_str(&json).expect("tags")
    }

    fn has_tag(tags: &[Vec<String>], key: &str, value: &str) -> bool {
        tags.iter().any(|tag| {
            tag.first().map(|entry| entry.as_str()) == Some(key)
                && tag.get(1).map(|entry| entry.as_str()) == Some(value)
        })
    }

    fn has_exact_tag(tags: &[Vec<String>], expected: &[&str]) -> bool {
        tags.iter().any(|tag| {
            tag.iter()
                .map(|entry| entry.as_str())
                .eq(expected.iter().copied())
        })
    }

    type BindingEncoder = fn(&str) -> Result<String, RadrootsJsValue>;

    #[test]
    fn bindings_reject_invalid_json() {
        let bindings: [BindingEncoder; 46] = [
            listing_tags,
            listing_tags_full,
            post_tags,
            comment_tags,
            article_tags,
            file_metadata_tags,
            calendar_date_event_tags,
            calendar_time_event_tags,
            calendar_tags,
            calendar_event_rsvp_tags,
            repost_tags,
            generic_repost_tags,
            report_tags,
            follow_tags,
            document_tags,
            coop_tags,
            farm_tags,
            list_tags,
            list_set_tags,
            plot_tags,
            job_request_tags,
            job_result_tags,
            job_feedback_tags,
            reaction_tags,
            message_tags,
            message_file_tags,
            seal_tags,
            gift_wrap_tags,
            farm_workspace_manifest_tags,
            farm_crdt_change_tags,
            farm_file_metadata_tags,
            relay_auth_tags,
            http_auth_tags,
            group_put_user_tags,
            group_remove_user_tags,
            group_create_group_tags,
            group_edit_metadata_tags,
            group_delete_group_tags,
            group_delete_event_tags,
            group_create_invite_tags,
            group_join_request_tags,
            group_leave_request_tags,
            group_metadata_tags,
            group_admins_tags,
            group_members_tags,
            group_roles_tags,
        ];

        for binding in bindings {
            assert!(binding("{").is_err());
        }
        assert!(listing_tags("").is_err());
    }

    #[test]
    fn bindings_encode_to_json_when_input_is_valid() {
        let listing_json = serde_json::to_string(&sample_listing()).expect("listing json");
        let listing_tags_json = listing_tags(&listing_json).expect("listing tags");
        let listing_tags: Vec<Vec<String>> =
            serde_json::from_str(&listing_tags_json).expect("listing tags json");
        assert!(!listing_tags.is_empty());

        let request_json = serde_json::to_string(&sample_job_request()).expect("request json");
        let request_tags_json = job_request_tags(&request_json).expect("request tags");
        let request_tags: Vec<Vec<String>> =
            serde_json::from_str(&request_tags_json).expect("request tags json");
        assert!(!request_tags.is_empty());
    }

    #[test]
    fn social_bindings_encode_to_json_when_input_is_valid() {
        assert_tags_json(post_tags(
            &serde_json::to_string(&sample_post()).expect("post json"),
        ));
        assert_tags_json(comment_tags(
            &serde_json::to_string(&sample_comment()).expect("comment json"),
        ));
        assert_tags_json(article_tags(
            &serde_json::to_string(&sample_article()).expect("article json"),
        ));
        assert_tags_json(file_metadata_tags(
            &serde_json::to_string(&sample_public_file_metadata()).expect("file json"),
        ));
        assert_tags_json(calendar_date_event_tags(
            &serde_json::to_string(&sample_calendar_date_event()).expect("date json"),
        ));
        let time_tags = tags_json(calendar_time_event_tags(
            &serde_json::to_string(&sample_calendar_time_event()).expect("time json"),
        ));
        assert!(has_tag(&time_tags, "D", "2026-06-20"));
        assert_tags_json(calendar_tags(
            &serde_json::to_string(&sample_calendar()).expect("calendar json"),
        ));
        assert_tags_json(calendar_event_rsvp_tags(
            &serde_json::to_string(&sample_calendar_rsvp()).expect("rsvp json"),
        ));
        assert_tags_json(reaction_tags(
            &serde_json::to_string(&sample_reaction()).expect("reaction json"),
        ));
        assert_tags_json(repost_tags(
            &serde_json::to_string(&sample_repost()).expect("repost json"),
        ));
        assert_tags_json(generic_repost_tags(
            &serde_json::to_string(&sample_generic_repost()).expect("generic repost json"),
        ));
        assert_tags_json(report_tags(
            &serde_json::to_string(&sample_report()).expect("report json"),
        ));
    }

    #[test]
    fn social_bindings_surface_builder_errors() {
        let mut article = sample_article();
        article.d_tag.clear();
        assert!(article_tags(&serde_json::to_string(&article).expect("article json")).is_err());

        let mut comment = sample_comment();
        comment.root = event_target(1, 'a');
        assert!(comment_tags(&serde_json::to_string(&comment).expect("comment json")).is_err());

        let mut reaction = sample_reaction();
        reaction.target = RadrootsSocialTarget::External {
            id: "https://example.test/object".to_string(),
            external_kind: "web".to_string(),
            hint: None,
        };
        assert!(reaction_tags(&serde_json::to_string(&reaction).expect("reaction json")).is_err());

        let mut rsvp = sample_calendar_rsvp();
        rsvp.event = event_target(31923, 'f');
        assert!(
            calendar_event_rsvp_tags(&serde_json::to_string(&rsvp).expect("rsvp json")).is_err()
        );

        let mut report = sample_report();
        report.reported_pubkey.clear();
        assert!(report_tags(&serde_json::to_string(&report).expect("report json")).is_err());
    }

    #[test]
    fn knowledge_bindings_encode_to_json_when_input_is_valid() {
        let article_tags = tags_json(wiki_article_tags(
            &serde_json::to_string(&sample_wiki_article()).expect("wiki article json"),
        ));
        assert!(has_tag(&article_tags, "title", "Soil health"));
        let fork_address = format!(
            "{}:{}:soil-health",
            KIND_WIKI_ARTICLE,
            synthetic_pubkey('a')
        );
        let fork_event_id = synthetic_event_id('b');
        assert!(has_exact_tag(
            &article_tags,
            &[
                "a",
                fork_address.as_str(),
                "wss://relay.example.test",
                "fork"
            ]
        ));
        assert!(has_exact_tag(
            &article_tags,
            &[
                "e",
                fork_event_id.as_str(),
                "wss://relay.example.test",
                "fork"
            ]
        ));

        let redirect_tags = tags_json(wiki_redirect_tags(
            &serde_json::to_string(&sample_wiki_redirect()).expect("wiki redirect json"),
        ));
        let redirect_address = format!(
            "{}:{}:soil-health",
            KIND_WIKI_ARTICLE,
            synthetic_pubkey('a')
        );
        assert!(has_exact_tag(
            &redirect_tags,
            &["a", redirect_address.as_str(), "wss://relay.example.test"]
        ));

        let merge_request = sample_wiki_merge_request();
        let merge_parts =
            radroots_event_codec::knowledge::wiki_merge_request_to_wire_parts(&merge_request)
                .expect("merge request parts");
        assert_eq!(merge_parts.content, "Merge synthetic soil article updates");
        let merge_tags = tags_json(wiki_merge_request_tags(
            &serde_json::to_string(&sample_wiki_merge_request()).expect("merge request json"),
        ));
        let source_event_id = synthetic_event_id('f');
        assert!(has_exact_tag(
            &merge_tags,
            &["e", source_event_id.as_str(), "", "source"]
        ));

        let source_tags = tags_json(knowledge_source_tags(
            &serde_json::to_string(&sample_knowledge_source()).expect("source json"),
        ));
        assert!(has_tag(
            &source_tags,
            "contract",
            RADROOTS_KNOWLEDGE_SOURCE_SCHEMA
        ));

        let claim_tags = tags_json(knowledge_claim_tags(
            &serde_json::to_string(&sample_knowledge_claim()).expect("claim json"),
        ));
        assert!(has_tag(
            &claim_tags,
            "contract",
            RADROOTS_KNOWLEDGE_CLAIM_SCHEMA
        ));

        assert_tags_json(knowledge_relation_tags(
            &serde_json::to_string(&sample_knowledge_relation()).expect("relation json"),
        ));
        assert_tags_json(knowledge_review_tags(
            &serde_json::to_string(&sample_knowledge_review()).expect("review json"),
        ));
        assert_tags_json(knowledge_field_report_tags(
            &serde_json::to_string(&sample_knowledge_field_report()).expect("field report json"),
        ));
    }

    #[test]
    fn wiki_article_tags_accept_missing_title() {
        let mut article = sample_wiki_article();
        article.title = None;
        let tags = tags_json(wiki_article_tags(
            &serde_json::to_string(&article).expect("wiki article json"),
        ));
        assert!(
            !tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some("title"))
        );
    }

    #[test]
    fn knowledge_claim_tags_enforce_citation_rules() {
        let mut claim = sample_knowledge_claim();
        claim.citation_spans.clear();
        let error = knowledge_claim_tags(
            &serde_json::to_string(&claim).expect("uncited source-backed claim json"),
        )
        .expect_err("source-backed claim requires citations");
        assert!(error.contains("citation_spans"));

        assert_tags_json(knowledge_claim_tags(
            &serde_json::to_string(&sample_knowledge_claim()).expect("claim json"),
        ));

        for claim_type in ["hypothesis", "observation", "question"] {
            let mut uncited = sample_knowledge_claim();
            uncited.claim_type = claim_type.to_string();
            uncited.citation_spans.clear();
            assert_tags_json(knowledge_claim_tags(
                &serde_json::to_string(&uncited).expect("uncited claim json"),
            ));
        }
    }

    #[test]
    fn knowledge_bindings_verify_decode_and_manifest_json() {
        let decoded = verify_and_decode_event_json(&signed_claim_event_json()).expect("decoded");
        let decoded: serde_json::Value = serde_json::from_str(&decoded).expect("decoded json");
        assert_eq!(decoded["event_type"], "knowledge_claim");
        assert_eq!(decoded["contract_id"], RADROOTS_KNOWLEDGE_CLAIM_SCHEMA);
        assert_eq!(
            decoded["payload"]["text"],
            "Cover crops improve soil structure."
        );

        let manifest = contract_manifest_json().expect("manifest");
        let manifest: serde_json::Value = serde_json::from_str(&manifest).expect("manifest json");
        assert_eq!(manifest["schema_version"], 2);
        assert_eq!(manifest["contract_count"], 11);
        let claim_contract = manifest["contracts"]
            .as_array()
            .expect("contracts")
            .iter()
            .find(|contract| contract["contract_id"] == RADROOTS_KNOWLEDGE_CLAIM_SCHEMA)
            .expect("claim contract");
        assert_eq!(claim_contract["sdk_builder_support"], true);
        assert_eq!(claim_contract["sdk_draft_support"], true);
        assert_eq!(claim_contract["wasm_tag_builder_support"], true);
        assert_eq!(claim_contract["wasm_verified_decode_support"], true);
    }

    #[test]
    fn knowledge_bindings_verify_decode_errors_are_coded_json() {
        let invalid_json = verify_and_decode_event_json("not json").expect_err("invalid json");
        let invalid_json: serde_json::Value =
            serde_json::from_str(&invalid_json).expect("error json");
        assert_eq!(invalid_json["code"], "invalid_json");
        assert_eq!(invalid_json["inner_code"], "event_json");

        let mut event: serde_json::Value =
            serde_json::from_str(&signed_claim_event_json()).expect("event json");
        event["sig"] = serde_json::Value::String("0".repeat(128));
        let signature_error =
            verify_and_decode_event_json(&event.to_string()).expect_err("signature error");
        let signature_error: serde_json::Value =
            serde_json::from_str(&signature_error).expect("signature error json");
        assert_eq!(signature_error["code"], "nip01_verification");
        assert_eq!(signature_error["inner_code"], "signature_invalid");
    }

    #[test]
    fn field_bindings_encode_to_json_when_input_is_valid() {
        let workspace_json =
            serde_json::to_string(&sample_workspace_manifest()).expect("workspace json");
        assert_tags_json(farm_workspace_manifest_tags(&workspace_json));

        let crdt_json = serde_json::json!({
            "change": sample_crdt_change(),
            "author_pubkey": "author_pubkey"
        })
        .to_string();
        assert_tags_json(farm_crdt_change_tags(&crdt_json));

        let file_json = serde_json::to_string(&sample_file_metadata()).expect("file json");
        assert_tags_json(farm_file_metadata_tags(&file_json));

        let relay_auth_json = serde_json::to_string(&RadrootsRelayAuth {
            relay: "wss://relay.example.invalid/farm/field-group".to_string(),
            challenge: "relay-provided-challenge".to_string(),
        })
        .expect("relay auth json");
        assert_tags_json(relay_auth_tags(&relay_auth_json));

        let http_auth_json = serde_json::to_string(&RadrootsHttpAuth {
            url: "https://media.example.invalid/upload".to_string(),
            method: "POST".to_string(),
            payload_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
        })
        .expect("http auth json");
        assert_tags_json(http_auth_tags(&http_auth_json));
    }

    #[test]
    fn field_bindings_surface_builder_errors() {
        let crdt_json = serde_json::json!({
            "change": sample_crdt_change(),
            "author_pubkey": " "
        })
        .to_string();

        assert!(farm_crdt_change_tags(&crdt_json).is_err());
    }

    #[test]
    fn group_bindings_encode_to_json_when_input_is_valid() {
        let metadata = sample_group_metadata();
        assert_tags_json(group_put_user_tags(
            &serde_json::to_string(&RadrootsGroupPutUser {
                group_id: "field-group".to_string(),
                message: Some("add member".to_string()),
                pubkey: "member_pubkey".to_string(),
                roles: vec!["member".to_string()],
            })
            .expect("put user json"),
        ));
        assert_tags_json(group_remove_user_tags(
            &serde_json::to_string(&RadrootsGroupRemoveUser {
                group_id: "field-group".to_string(),
                message: Some("remove member".to_string()),
                pubkey: "member_pubkey".to_string(),
            })
            .expect("remove user json"),
        ));
        assert_tags_json(group_create_group_tags(
            &serde_json::to_string(&RadrootsGroupCreateGroup {
                group_id: "field-group".to_string(),
                message: Some("create group".to_string()),
                metadata: metadata.clone(),
            })
            .expect("create group json"),
        ));
        assert_tags_json(group_edit_metadata_tags(
            &serde_json::to_string(&RadrootsGroupEditMetadata {
                group_id: "field-group".to_string(),
                message: Some("edit metadata".to_string()),
                metadata: metadata.clone(),
            })
            .expect("edit metadata json"),
        ));
        assert_tags_json(group_delete_group_tags(
            &serde_json::to_string(&RadrootsGroupDeleteGroup {
                group_id: "field-group".to_string(),
                message: Some("delete group".to_string()),
            })
            .expect("delete group json"),
        ));
        assert_tags_json(group_delete_event_tags(
            &serde_json::to_string(&RadrootsGroupDeleteEvent {
                group_id: "field-group".to_string(),
                message: Some("delete event".to_string()),
                event_id: "event_id".to_string(),
            })
            .expect("delete event json"),
        ));
        let invite_tags = tags_json(group_create_invite_tags(
            &serde_json::to_string(&RadrootsGroupCreateInvite {
                group_id: "field-group".to_string(),
                message: Some("join the field group".to_string()),
                code: "invite-code".to_string(),
            })
            .expect("invite json"),
        ));
        assert!(invite_tags.contains(&vec!["code".to_string(), "invite-code".to_string()]));
        assert_tags_json(group_join_request_tags(
            &serde_json::to_string(&RadrootsGroupJoinRequest {
                group_id: "field-group".to_string(),
                message: Some("requesting access".to_string()),
                code: Some("invite-code".to_string()),
            })
            .expect("join json"),
        ));
        assert_tags_json(group_leave_request_tags(
            &serde_json::to_string(&RadrootsGroupLeaveRequest {
                group_id: "field-group".to_string(),
                message: Some("leaving".to_string()),
            })
            .expect("leave json"),
        ));
        let metadata_tags = tags_json(group_metadata_tags(
            &serde_json::to_string(&RadrootsGroupMetadata {
                d_tag: "field-group".to_string(),
                metadata,
            })
            .expect("metadata json"),
        ));
        assert!(metadata_tags.contains(&vec!["restricted".to_string()]));
        assert!(metadata_tags.contains(&vec![
            "supported_kinds".to_string(),
            "78".to_string(),
            "30078".to_string(),
            KIND_FARM_FILE_METADATA.to_string()
        ]));
        assert_tags_json(group_admins_tags(
            &serde_json::to_string(&RadrootsGroupAdmins {
                d_tag: "field-group".to_string(),
                description: Some("group admins".to_string()),
                admins: vec![sample_group_user("admin")],
            })
            .expect("admins json"),
        ));
        assert_tags_json(group_members_tags(
            &serde_json::to_string(&RadrootsGroupMembers {
                d_tag: "field-group".to_string(),
                description: Some("group members".to_string()),
                members: vec![sample_group_user("member")],
            })
            .expect("members json"),
        ));
        assert_tags_json(group_roles_tags(
            &serde_json::to_string(&RadrootsGroupRoles {
                d_tag: "field-group".to_string(),
                description: Some("group roles".to_string()),
                roles: vec![sample_group_role()],
            })
            .expect("roles json"),
        ));
    }

    #[test]
    fn listing_bindings_surface_builder_errors() {
        let mut listing_json = serde_json::to_value(sample_listing()).expect("listing value");
        listing_json["bins"] = serde_json::Value::Array(Vec::new());
        let listing_json = serde_json::to_string(&listing_json).expect("listing json");

        assert!(listing_tags(&listing_json).is_err());
        assert!(listing_tags_full(&listing_json).is_err());
    }
}
