#![cfg(target_arch = "wasm32")]
#![forbid(unsafe_code)]

use radroots_events::listing::RadrootsListing;
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
