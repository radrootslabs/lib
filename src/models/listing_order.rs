use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListingOrder {
    pub quantity: ListingOrderQuantity,
    pub price: ListingOrderPrice,
    pub discounts: Vec<ListingOrderDiscount>,
    pub subtotal: ListingOrderSubtotal,
    pub total: ListingOrderTotal,
}

#[typeshare]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListingOrderQuantity {
    pub amount: f64,
    pub unit: String,
    pub label: String,
    pub count: u32,
}

#[typeshare]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListingOrderPrice {
    pub amount: f64,
    pub currency: String,
    pub quantity_amount: f64,
    pub quantity_unit: String,
}

#[typeshare]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListingOrderDiscount {
    pub discount_type: String,
    pub threshold: Option<f64>,
    pub threshold_unit: Option<String>,
    pub discount_per_unit: Option<f64>,
    pub discount_unit: Option<String>,
    pub discount_percent: Option<f64>,
    pub discount_amount: f64,
    pub currency: String,
}

#[typeshare]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListingOrderSubtotal {
    pub price_amount: f64,
    pub price_currency: String,
    pub quantity_amount: f64,
    pub quantity_unit: String,
}

#[typeshare]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListingOrderTotal {
    pub price_amount: f64,
    pub price_currency: String,
    pub quantity_amount: f64,
    pub quantity_unit: String,
}
