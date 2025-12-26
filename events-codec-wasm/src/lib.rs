#![cfg(target_arch = "wasm32")]
#![forbid(unsafe_code)]

use radroots_events::comment::RadrootsComment;
use radroots_events::follow::RadrootsFollow;
use radroots_events::farm::RadrootsFarm;
use radroots_events::job_feedback::RadrootsJobFeedback;
use radroots_events::job_request::RadrootsJobRequest;
use radroots_events::job_result::RadrootsJobResult;
use radroots_events::listing::RadrootsListing;
use radroots_events::list::RadrootsList;
use radroots_events::list_set::RadrootsListSet;
use radroots_events::message::RadrootsMessage;
use radroots_events::message_file::RadrootsMessageFile;
use radroots_events::plot::RadrootsPlot;
use radroots_events::reaction::RadrootsReaction;
use radroots_events::gift_wrap::RadrootsGiftWrap;
use radroots_events::seal::RadrootsSeal;
use radroots_events_codec::comment::encode::comment_build_tags;
use radroots_events_codec::follow::encode::follow_build_tags;
use radroots_events_codec::farm::encode::farm_build_tags;
use radroots_events_codec::gift_wrap::encode::gift_wrap_build_tags;
use radroots_events_codec::job::feedback::encode::job_feedback_build_tags;
use radroots_events_codec::job::request::encode::job_request_build_tags;
use radroots_events_codec::job::result::encode::job_result_build_tags;
use radroots_events_codec::list::encode::list_build_tags;
use radroots_events_codec::list_set::encode::list_set_build_tags;
use radroots_events_codec::message::encode::message_build_tags;
use radroots_events_codec::message_file::encode::message_file_build_tags;
use radroots_events_codec::plot::encode::plot_build_tags;
use radroots_events_codec::reaction::encode::reaction_build_tags;
use radroots_events_codec::seal::encode::seal_build_tags;
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

fn parse_farm(farm_json: &str) -> Result<RadrootsFarm, JsValue> {
    serde_json::from_str(farm_json).map_err(err_js)
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

fn parse_message_file(message_json: &str) -> Result<RadrootsMessageFile, JsValue> {
    serde_json::from_str(message_json).map_err(err_js)
}

fn parse_plot(plot_json: &str) -> Result<RadrootsPlot, JsValue> {
    serde_json::from_str(plot_json).map_err(err_js)
}

fn parse_gift_wrap(gift_wrap_json: &str) -> Result<RadrootsGiftWrap, JsValue> {
    serde_json::from_str(gift_wrap_json).map_err(err_js)
}

fn parse_seal(seal_json: &str) -> Result<RadrootsSeal, JsValue> {
    serde_json::from_str(seal_json).map_err(err_js)
}

fn parse_list(list_json: &str) -> Result<RadrootsList, JsValue> {
    serde_json::from_str(list_json).map_err(err_js)
}

fn parse_list_set(list_json: &str) -> Result<RadrootsListSet, JsValue> {
    serde_json::from_str(list_json).map_err(err_js)
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

#[wasm_bindgen(js_name = farm_tags)]
pub fn farm_tags(farm_json: &str) -> Result<String, JsValue> {
    let farm = parse_farm(farm_json)?;
    let tags = farm_build_tags(&farm).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = list_tags)]
pub fn list_tags(list_json: &str) -> Result<String, JsValue> {
    let list = parse_list(list_json)?;
    let tags = list_build_tags(&list).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = list_set_tags)]
pub fn list_set_tags(list_json: &str) -> Result<String, JsValue> {
    let list = parse_list_set(list_json)?;
    let tags = list_set_build_tags(&list).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = plot_tags)]
pub fn plot_tags(plot_json: &str) -> Result<String, JsValue> {
    let plot = parse_plot(plot_json)?;
    let tags = plot_build_tags(&plot).map_err(err_js)?;
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

#[wasm_bindgen(js_name = message_file_tags)]
pub fn message_file_tags(message_json: &str) -> Result<String, JsValue> {
    let message = parse_message_file(message_json)?;
    let tags = message_file_build_tags(&message).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = seal_tags)]
pub fn seal_tags(seal_json: &str) -> Result<String, JsValue> {
    let seal = parse_seal(seal_json)?;
    let tags = seal_build_tags(&seal).map_err(err_js)?;
    tags_to_json(tags)
}

#[wasm_bindgen(js_name = gift_wrap_tags)]
pub fn gift_wrap_tags(gift_wrap_json: &str) -> Result<String, JsValue> {
    let gift_wrap = parse_gift_wrap(gift_wrap_json)?;
    let tags = gift_wrap_build_tags(&gift_wrap).map_err(err_js)?;
    tags_to_json(tags)
}
