#![forbid(unsafe_code)]

use radroots_events::comment::RadrootsComment;
use radroots_events::coop::RadrootsCoop;
use radroots_events::document::RadrootsDocument;
use radroots_events::farm::RadrootsFarm;
use radroots_events::farm_crdt::RadrootsFarmCrdtChange;
use radroots_events::farm_file::RadrootsFarmFileMetadata;
use radroots_events::farm_workspace::RadrootsFarmWorkspaceManifest;
use radroots_events::follow::RadrootsFollow;
use radroots_events::gift_wrap::RadrootsGiftWrap;
use radroots_events::group::{
    RadrootsGroupAdmins, RadrootsGroupCreateGroup, RadrootsGroupCreateInvite,
    RadrootsGroupDeleteEvent, RadrootsGroupDeleteGroup, RadrootsGroupEditMetadata,
    RadrootsGroupJoinRequest, RadrootsGroupLeaveRequest, RadrootsGroupMembers,
    RadrootsGroupMetadata, RadrootsGroupPutUser, RadrootsGroupRemoveUser, RadrootsGroupRoles,
};
use radroots_events::http_auth::RadrootsHttpAuth;
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
use radroots_events::relay_auth::RadrootsRelayAuth;
use radroots_events::seal::RadrootsSeal;
use radroots_events_codec::comment::encode::comment_build_tags;
use radroots_events_codec::coop::encode::coop_build_tags;
use radroots_events_codec::document::encode::document_build_tags;
use radroots_events_codec::farm::encode::farm_build_tags;
use radroots_events_codec::farm_crdt::encode::farm_crdt_change_build_tags_with_author;
use radroots_events_codec::farm_file::encode::farm_file_metadata_build_tags;
use radroots_events_codec::farm_workspace::encode::farm_workspace_build_tags;
use radroots_events_codec::follow::encode::follow_build_tags;
use radroots_events_codec::gift_wrap::encode::gift_wrap_build_tags;
use radroots_events_codec::group::encode::{
    group_admins_build_tags, group_create_group_build_tags, group_create_invite_build_tags,
    group_delete_event_build_tags, group_delete_group_build_tags, group_edit_metadata_build_tags,
    group_join_request_build_tags, group_leave_request_build_tags, group_members_build_tags,
    group_metadata_build_tags, group_put_user_build_tags, group_remove_user_build_tags,
    group_roles_build_tags,
};
use radroots_events_codec::http_auth::encode::http_auth_build_tags;
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
use radroots_events_codec::relay_auth::encode::relay_auth_build_tags;
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = farm_workspace_tags))]
pub fn farm_workspace_tags(workspace_json: &str) -> Result<String, RadrootsJsValue> {
    build_tags_json::<RadrootsFarmWorkspaceManifest, _, _>(
        workspace_json,
        farm_workspace_build_tags,
    )
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = farm_crdt_tags))]
pub fn farm_crdt_tags(input_json: &str) -> Result<String, RadrootsJsValue> {
    let input = parse_json::<FarmCrdtTagsInput>(input_json)?;
    let tags = farm_crdt_change_build_tags_with_author(&input.change, Some(&input.author_pubkey))
        .map_err(err_js)?;
    tags_to_json(tags)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = farm_file_tags))]
pub fn farm_file_tags(file_json: &str) -> Result<String, RadrootsJsValue> {
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
    use radroots_events::farm::RadrootsFarmRef;
    use radroots_events::farm_crdt::{
        RADROOTS_FARM_CRDT_CHANGE_SCHEMA, RadrootsCrdtBackend, RadrootsFarmCrdtDocumentKind,
        RadrootsFarmSemanticKind,
    };
    use radroots_events::farm_file::{
        RadrootsFarmFileDimensions, RadrootsFarmFileMetadata, RadrootsFarmFileSource,
    };
    use radroots_events::farm_workspace::{
        RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION, RADROOTS_FARM_WORKSPACE_SCHEMA,
        RadrootsFarmWorkspaceManifest, RadrootsFarmWorkspaceMediaServer, RadrootsFarmWorkspaceRef,
        RadrootsFarmWorkspaceRelay, RadrootsFarmWorkspaceRelayMode,
    };
    use radroots_events::group::{
        RadrootsGroupAdmins, RadrootsGroupCreateGroup, RadrootsGroupCreateInvite,
        RadrootsGroupDeleteEvent, RadrootsGroupDeleteGroup, RadrootsGroupEditMetadata,
        RadrootsGroupEditableMetadata, RadrootsGroupJoinRequest, RadrootsGroupLeaveRequest,
        RadrootsGroupMembers, RadrootsGroupMetadata, RadrootsGroupPutUser, RadrootsGroupRemoveUser,
        RadrootsGroupRole, RadrootsGroupRoles, RadrootsGroupUserRef,
    };
    use radroots_events::http_auth::RadrootsHttpAuth;
    use radroots_events::job::JobInputType;
    use radroots_events::job_request::{RadrootsJobInput, RadrootsJobParam};
    use radroots_events::listing::{RadrootsListingBin, RadrootsListingProduct};
    use radroots_events::relay_auth::RadrootsRelayAuth;

    fn sample_listing() -> RadrootsListing {
        let quantity =
            RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
        let price = RadrootsCoreQuantityPrice::new(
            RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
            quantity.clone(),
        );

        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
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
            supported_kinds: vec![78, 30078],
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
            is_closed: false,
            is_hidden: false,
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
        let json = value.expect("tags json");
        let tags: Vec<Vec<String>> = serde_json::from_str(&json).expect("tags");
        assert!(!tags.is_empty());
    }

    #[test]
    fn bindings_reject_invalid_json() {
        let bindings: [fn(&str) -> Result<String, RadrootsJsValue>; 36] = [
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
            farm_workspace_tags,
            farm_crdt_tags,
            farm_file_tags,
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
    fn field_bindings_encode_to_json_when_input_is_valid() {
        let workspace_json =
            serde_json::to_string(&sample_workspace_manifest()).expect("workspace json");
        assert_tags_json(farm_workspace_tags(&workspace_json));

        let crdt_json = serde_json::json!({
            "change": sample_crdt_change(),
            "author_pubkey": "author_pubkey"
        })
        .to_string();
        assert_tags_json(farm_crdt_tags(&crdt_json));

        let file_json = serde_json::to_string(&sample_file_metadata()).expect("file json");
        assert_tags_json(farm_file_tags(&file_json));

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
    fn group_bindings_encode_to_json_when_input_is_valid() {
        let metadata = sample_group_metadata();
        assert_tags_json(group_put_user_tags(
            &serde_json::to_string(&RadrootsGroupPutUser {
                group_id: "field-group".to_string(),
                pubkey: "member_pubkey".to_string(),
                roles: vec!["member".to_string()],
            })
            .expect("put user json"),
        ));
        assert_tags_json(group_remove_user_tags(
            &serde_json::to_string(&RadrootsGroupRemoveUser {
                group_id: "field-group".to_string(),
                pubkey: "member_pubkey".to_string(),
            })
            .expect("remove user json"),
        ));
        assert_tags_json(group_create_group_tags(
            &serde_json::to_string(&RadrootsGroupCreateGroup {
                group_id: "field-group".to_string(),
                metadata: metadata.clone(),
            })
            .expect("create group json"),
        ));
        assert_tags_json(group_edit_metadata_tags(
            &serde_json::to_string(&RadrootsGroupEditMetadata {
                group_id: "field-group".to_string(),
                metadata: metadata.clone(),
            })
            .expect("edit metadata json"),
        ));
        assert_tags_json(group_delete_group_tags(
            &serde_json::to_string(&RadrootsGroupDeleteGroup {
                group_id: "field-group".to_string(),
            })
            .expect("delete group json"),
        ));
        assert_tags_json(group_delete_event_tags(
            &serde_json::to_string(&RadrootsGroupDeleteEvent {
                group_id: "field-group".to_string(),
                event_id: "event_id".to_string(),
            })
            .expect("delete event json"),
        ));
        assert_tags_json(group_create_invite_tags(
            &serde_json::to_string(&RadrootsGroupCreateInvite {
                group_id: "field-group".to_string(),
                invitee_pubkey: Some("member_pubkey".to_string()),
                roles: vec!["member".to_string()],
                expires_at: Some(1_780_000_000),
                claim: Some("claim-token".to_string()),
            })
            .expect("invite json"),
        ));
        assert_tags_json(group_join_request_tags(
            &serde_json::to_string(&RadrootsGroupJoinRequest {
                group_id: "field-group".to_string(),
                message: Some("requesting access".to_string()),
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
        assert_tags_json(group_metadata_tags(
            &serde_json::to_string(&RadrootsGroupMetadata {
                d_tag: "field-group".to_string(),
                metadata,
            })
            .expect("metadata json"),
        ));
        assert_tags_json(group_admins_tags(
            &serde_json::to_string(&RadrootsGroupAdmins {
                d_tag: "field-group".to_string(),
                admins: vec![sample_group_user("admin")],
            })
            .expect("admins json"),
        ));
        assert_tags_json(group_members_tags(
            &serde_json::to_string(&RadrootsGroupMembers {
                d_tag: "field-group".to_string(),
                members: vec![sample_group_user("member")],
            })
            .expect("members json"),
        ));
        assert_tags_json(group_roles_tags(
            &serde_json::to_string(&RadrootsGroupRoles {
                d_tag: "field-group".to_string(),
                roles: vec![sample_group_role()],
            })
            .expect("roles json"),
        ));
    }

    #[test]
    fn listing_bindings_surface_builder_errors() {
        let mut listing = sample_listing();
        listing.d_tag.clear();
        let listing_json = serde_json::to_string(&listing).expect("listing json");

        assert!(listing_tags(&listing_json).is_err());
        assert!(listing_tags_full(&listing_json).is_err());
    }
}
