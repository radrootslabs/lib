//! Deterministic GeoNames asset management and locality lookup.
//!
//! This crate never chooses runtime paths or performs work during
//! construction. Hosts explicitly provide every asset source and destination.

#![forbid(unsafe_code)]

pub mod asset;
pub mod database;
pub mod download;
mod error;
pub mod model;
pub mod query;

pub use asset::{AssetSpec, AssetStatus};
pub use database::Geocoder;
pub use error::Error;
pub use model::{Candidate, Point};
pub use query::Query;
