use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListingOrderRequestQuantity {
    pub amount: f64,
    pub unit: String,
    pub label: String,
    pub count: u32,
}

#[typeshare]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListingOrderRequestPrice {
    pub amount: f64,
    pub currency: String,
    pub quantity_amount: f64,
    pub quantity_unit: String,
}

#[typeshare]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListingOrderRequestPayload {
    pub price: ListingOrderRequestPrice,
    pub quantity: ListingOrderRequestQuantity,
}

#[typeshare]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListingOrderRequestEvent {
    pub id: String,
}

#[typeshare]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListingOrderRequest {
    pub event: ListingOrderRequestEvent,
    pub payload: ListingOrderRequestPayload,
}
