#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub mod account;
pub mod app_data;
pub mod article;
pub mod calendar;
pub mod comment;
pub mod contract;
pub mod coop;
pub mod document;
pub mod farm;
pub mod farm_crdt;
pub mod farm_file;
pub mod farm_workspace;
pub mod file_metadata;
pub mod follow;
pub mod geochat;
pub mod gift_wrap;
pub mod group;
pub mod http_auth;
pub mod ids;
pub mod job;
pub mod job_feedback;
pub mod job_request;
pub mod job_result;
pub mod kinds;
pub mod list;
pub mod list_set;
pub mod listing;
pub mod message;
pub mod message_file;
pub mod order;
pub mod order_economics;
pub mod plot;
pub mod post;
pub mod profile;
pub mod reaction;
pub mod relay_auth;
pub mod relay_document;
pub mod report;
pub mod repost;
pub mod resource_area;
pub mod resource_cap;
pub mod seal;
pub mod social;
pub mod tags;
pub mod trade_validation;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNostrEvent {
    pub id: String,
    pub author: String,
    pub created_at: u32,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsNostrEventRef {
    pub id: String,
    pub author: String,
    pub kind: u32,
    pub d_tag: Option<String>,
    pub relays: Option<Vec<String>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNostrEventPtr {
    pub id: String,
    pub relays: Option<String>,
}
