use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEvent {
    pub id: String,
    pub author: String,
    pub created_at: u32,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEventRef {
    pub ref_id: String,
    pub ref_author: String,
    pub ref_kind: u32,
    pub ref_d_tag: Option<String>,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsMetadataEvent {
    pub event: RadrootsNostrEvent,
    pub data: RadrootsMetadataEventData,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsMetadataEventDataMetadata {
    pub name: String,
    pub display_name: Option<String>,
    pub nip05: Option<String>,
    pub about: Option<String>,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsMetadataEventData {
    pub id: String,
    pub public_key: String,
    pub published_at: u32,
    pub metadata: RadrootsMetadataEventDataMetadata,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEvent3 {
    pub event: RadrootsNostrEvent,
    pub data: RadrootsNostrEvent3Data,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEvent3Data {
    pub following: Vec<RadrootsNostrEvent3DataFollow>,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEvent3DataFollow {
    pub published_at: u32,
    pub pubkey: String,
    pub relay_url: Option<String>,
    pub petname: Option<String>,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEvent7 {
    pub event: RadrootsNostrEvent,
    pub data: RadrootsNostrEvent7Data,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEvent7Data {
    pub published_at: u32,
    pub root: RadrootsNostrEventRef,
    pub content: String,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEvent1111 {
    pub event: RadrootsNostrEvent,
    pub data: RadrootsNostrEvent1111Data,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEvent1111Data {
    pub published_at: u32,
    pub root: RadrootsNostrEventRef,
    pub parent: RadrootsNostrEventRef,
    pub content: String,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingEvent {
    pub event: RadrootsNostrEvent,
    pub data: RadrootsListingEventData,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingEventData {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub d_tag: String,
    pub title: String,
    pub summary: String,
    pub images: Vec<String>,
    pub location_address: String,
    pub location_city: String,
    pub location_region: String,
    pub location_country: String,
    pub location_lat: String,
    pub location_lng: String,
    pub location_geohash: String,
    pub product_kind: String,
    pub product_category: String,
    pub product_process: String,
    pub product_lot: String,
    pub product_profile: String,
    pub product_year: String,
    pub product_quantity_amt: String,
    pub product_quantity_unit: String,
    pub product_price_amt: String,
    pub product_price_cur: String,
    pub product_price_qty_amt: String,
    pub product_price_qty_unit: String,
}
