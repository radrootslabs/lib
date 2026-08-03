//! Deterministic GeoNames asset management and locality lookup.
//!
//! This crate never chooses runtime paths or performs work during
//! construction. Hosts explicitly provide every asset source and destination.
//!
//! # Inert query construction
//!
//! ```
//! use radroots_geonames::{Point, Query};
//!
//! let locality = Query::locality("Victoria")?
//!     .with_region("BC")?
//!     .with_country("CA")?;
//! assert_eq!(locality.limit(), 10);
//!
//! let reverse = Query::reverse(Point::new(48.4284, -123.3656)?)
//!     .with_radius_degrees(0.25)?;
//! assert_eq!(reverse.limit(), 1);
//! # Ok::<(), radroots_geonames::Error>(())
//! ```

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
