#![cfg(target_arch = "wasm32")]
#![forbid(unsafe_code)]

use radroots_events::comment::RadrootsComment;
use radroots_events::follow::RadrootsFollow;
use radroots_events::job_feedback::RadrootsJobFeedback;
use radroots_events::job_request::RadrootsJobRequest;
use radroots_events::job_result::RadrootsJobResult;
use radroots_events::listing::RadrootsListing;
use radroots_events::message::RadrootsMessage;
use radroots_events::reaction::RadrootsReaction;
use radroots_events_codec::comment::encode::comment_build_tags;
use radroots_events_codec::follow::encode::follow_build_tags;
use radroots_events_codec::job::feedback::encode::job_feedback_build_tags;
use radroots_events_codec::job::request::encode::job_request_build_tags;
use radroots_events_codec::job::result::encode::job_result_build_tags;
use radroots_events_codec::message::encode::message_build_tags;
use radroots_events_codec::reaction::encode::reaction_build_tags;
use radroots_events_codec::listing::tags::{
    listing_tags as listing_tags_impl,
    listing_tags_full as listing_tags_full_impl,
};
use wasm_bindgen::prelude::*;

fn err_js<E: ToString>(err: E) -> JsValue {
    JsValue::from_str(&err.to_string())
}

fn parse_listing(listing_json: &str) -> Result<RadrootsListing, JsValue> {
    serde_json::from_str(listing_json).map_err(err_js)
}

fn parse_comment(comment_json: &str) -> Result<RadrootsComment, JsValue> {
    serde_json::from_str(comment_json).map_err(err_js)
}

fn parse_follow(follow_json: &str) -> Result<RadrootsFollow, JsValue> {
    serde_json::from_str(follow_json).map_err(err_js)
}

fn parse_job_request(job_json: &str) -> Result<RadrootsJobRequest, JsValue> {
    serde_json::from_str(job_json).map_err(err_js)
}

fn parse_job_result(job_json: &str) -> Result<RadrootsJobResult, JsValue> {
    serde_json::from_str(job_json).map_err(err_js)
}

fn parse_job_feedback(job_json: &str) -> Result<RadrootsJobFeedback, JsValue> {
    serde_json::from_str(job_json).map_err(err_js)
}

fn parse_reaction(reaction_json: &str) -> Result<RadrootsReaction, JsValue> {
    serde_json::from_str(reaction_json).map_err(err_js)
}

fn parse_message(message_json: &str) -> Result<RadrootsMessage, JsValue> {
    serde_json::from_str(message_json).map_err(err_js)
}

fn tags_to_json(tags: Vec<Vec<String>>) -> Result<String, JsValue> {
    serde_json::to_string(&tags).map_err(err_js)
}

#[wasm_bindgen(js_name = listing_tags)]
pub fn listing_tags(listing_json: &str) -> Result<String, JsValue> {
    let listing = parse_listing(listing_json)?;
    let tags = listing_tags_impl(&listing).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = listing_tags_full)]
pub fn listing_tags_full(listing_json: &str) -> Result<String, JsValue> {
    let listing = parse_listing(listing_json)?;
    let tags = listing_tags_full_impl(&listing).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = comment_tags)]
pub fn comment_tags(comment_json: &str) -> Result<String, JsValue> {
    let comment = parse_comment(comment_json)?;
    let tags = comment_build_tags(&comment).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = follow_tags)]
pub fn follow_tags(follow_json: &str) -> Result<String, JsValue> {
    let follow = parse_follow(follow_json)?;
    let tags = follow_build_tags(&follow).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = job_request_tags)]
pub fn job_request_tags(job_json: &str) -> Result<String, JsValue> {
    let job = parse_job_request(job_json)?;
    let tags = job_request_build_tags(&job);
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = job_result_tags)]
pub fn job_result_tags(job_json: &str) -> Result<String, JsValue> {
    let job = parse_job_result(job_json)?;
    let tags = job_result_build_tags(&job);
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = job_feedback_tags)]
pub fn job_feedback_tags(job_json: &str) -> Result<String, JsValue> {
    let job = parse_job_feedback(job_json)?;
    let tags = job_feedback_build_tags(&job);
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = reaction_tags)]
pub fn reaction_tags(reaction_json: &str) -> Result<String, JsValue> {
    let reaction = parse_reaction(reaction_json)?;
    let tags = reaction_build_tags(&reaction).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = message_tags)]
pub fn message_tags(message_json: &str) -> Result<String, JsValue> {
    let message = parse_message(message_json)?;
    let tags = message_build_tags(&message).map_err(err_js)?;
    tags_to_json(tags)
}
