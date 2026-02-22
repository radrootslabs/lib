#![forbid(unsafe_code)]

use radroots_events::comment::RadrootsComment;
use radroots_events::coop::RadrootsCoop;
use radroots_events::document::RadrootsDocument;
use radroots_events::farm::RadrootsFarm;
use radroots_events::follow::RadrootsFollow;
use radroots_events::gift_wrap::RadrootsGiftWrap;
use radroots_events::job_feedback::RadrootsJobFeedback;
use radroots_events::job_request::RadrootsJobRequest;
use radroots_events::job_result::RadrootsJobResult;
use radroots_events::list::RadrootsList;
use radroots_events::list_set::RadrootsListSet;
use radroots_events::listing::RadrootsListing;
use radroots_events::message::RadrootsMessage;
use radroots_events::message_file::RadrootsMessageFile;
use radroots_events::plot::RadrootsPlot;
use radroots_events::reaction::RadrootsReaction;
use radroots_events::seal::RadrootsSeal;
use radroots_events_codec::comment::encode::comment_build_tags;
use radroots_events_codec::coop::encode::coop_build_tags;
use radroots_events_codec::document::encode::document_build_tags;
use radroots_events_codec::farm::encode::farm_build_tags;
use radroots_events_codec::follow::encode::follow_build_tags;
use radroots_events_codec::gift_wrap::encode::gift_wrap_build_tags;
use radroots_events_codec::job::feedback::encode::job_feedback_build_tags;
use radroots_events_codec::job::request::encode::job_request_build_tags;
use radroots_events_codec::job::result::encode::job_result_build_tags;
use radroots_events_codec::list::encode::list_build_tags;
use radroots_events_codec::list_set::encode::list_set_build_tags;
use radroots_events_codec::listing::tags::{
    listing_tags as listing_tags_impl, listing_tags_full as listing_tags_full_impl,
};
use radroots_events_codec::message::encode::message_build_tags;
use radroots_events_codec::message_file::encode::message_file_build_tags;
use radroots_events_codec::plot::encode::plot_build_tags;
use radroots_events_codec::reaction::encode::reaction_build_tags;
use radroots_events_codec::seal::encode::seal_build_tags;
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

fn normalized_payload(input: &str) -> &str {
    if input.is_empty() { "{}" } else { input }
}

fn parse_json<T: DeserializeOwned>(input: &str) -> Result<T, RadrootsJsValue> {
    serde_json::from_str(normalized_payload(input)).map_err(err_js)
}

fn tags_to_json(tags: Vec<Vec<String>>) -> Result<String, RadrootsJsValue> {
    serde_json::to_string(&tags).map_err(err_js)
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = listing_tags))]
pub fn listing_tags(listing_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsListing, _, _>(listing_json, listing_tags_impl)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = listing_tags_full))]
pub fn listing_tags_full(listing_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsListing, _, _>(listing_json, listing_tags_full_impl)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = comment_tags))]
pub fn comment_tags(comment_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsComment, _, _>(comment_json, comment_build_tags)
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

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::job::JobInputType;
    use radroots_events::job_request::{RadrootsJobInput, RadrootsJobParam};
    use radroots_events::listing::{
        RadrootsListingBin, RadrootsListingFarmRef, RadrootsListingProduct,
    };

    fn sample_listing() -> RadrootsListing {
        let quantity =
            RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
        let price = RadrootsCoreQuantityPrice::new(
            RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
            quantity.clone(),
        );

        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
            farm: RadrootsListingFarmRef {
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
            primary_bin_id: "bin-1".to_string(),
            bins: vec![RadrootsListingBin {
                bin_id: "bin-1".to_string(),
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

    #[test]
    fn bindings_reject_invalid_json() {
        let bindings: [fn(&str) -> Result<String, RadrootsJsValue>; 18] = [
            listing_tags,
            listing_tags_full,
            comment_tags,
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
}
