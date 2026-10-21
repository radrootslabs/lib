use super::Error;
use wasm_bindgen::prelude::*;

/// Actual owned installed family value. Projections grant no execution authority.
#[derive(Clone)]
#[wasm_bindgen]
pub struct Value {
    inner: radroots_execution::OperationValue,
}
impl Value {
    pub(super) fn listing(inner: &radroots_domain::listing::Listing) -> Self {
        Self {
            inner: radroots_execution::OperationValue::Listing(Box::new(inner.clone())),
        }
    }
    pub(super) fn chat(inner: &radroots_domain::ChatMessage) -> Self {
        Self {
            inner: radroots_execution::OperationValue::Chat(Box::new(inner.clone())),
        }
    }
}
#[wasm_bindgen]
impl Value {
    pub fn listing_value(&self) -> Option<ListingValue> {
        match &self.inner {
            radroots_execution::OperationValue::Listing(inner) => Some(ListingValue {
                inner: inner.as_ref().clone(),
            }),
            _ => None,
        }
    }
    pub fn chat_value(&self) -> Option<ChatValue> {
        match &self.inner {
            radroots_execution::OperationValue::Chat(inner) => Some(ChatValue {
                inner: inner.as_ref().clone(),
            }),
            _ => None,
        }
    }
}
#[wasm_bindgen]
pub struct ListingValue {
    inner: radroots_domain::listing::Listing,
}
#[wasm_bindgen]
impl ListingValue {
    pub fn id(&self) -> String {
        self.inner.id().to_string()
    }
    pub fn author(&self) -> String {
        self.inner.author().to_hex()
    }
    pub fn revision(&self) -> String {
        self.inner.revision().to_string()
    }
    pub fn created_at(&self) -> String {
        self.inner.created_at().to_string()
    }
    pub fn published_at(&self) -> String {
        self.inner.published_at().to_string()
    }
    pub fn title(&self) -> String {
        self.inner.text().title().to_owned()
    }
    pub fn summary(&self) -> String {
        self.inner.text().summary().to_owned()
    }
    pub fn description_markdown(&self) -> String {
        self.inner.text().description_markdown().to_owned()
    }
    pub fn terms_json(&self) -> Result<Vec<u8>, JsValue> {
        serde_json::to_vec(self.inner.terms()).map_err(|_| {
            Error::UnsupportedState {
                axis: "listing_terms".into(),
                operation_id: None,
            }
            .into()
        })
    }
}
#[wasm_bindgen]
pub struct ChatValue {
    inner: radroots_domain::ChatMessage,
}
#[wasm_bindgen]
impl ChatValue {
    pub fn id(&self) -> String {
        self.inner.id().to_string()
    }
    pub fn thread_id(&self) -> String {
        self.inner.thread_id().to_string()
    }
    pub fn author(&self) -> String {
        self.inner.author().to_hex()
    }
    pub fn peer(&self) -> String {
        self.inner.peer().to_hex()
    }
    pub fn created_at(&self) -> String {
        self.inner.created_at().to_string()
    }
    /// Closed relation kind. A future source relation refuses projection.
    pub fn relation(&self) -> Result<String, JsValue> {
        match self.inner.relation() {
            radroots_domain::ChatRelation::Inquire { .. } => Ok("inquire".into()),
            radroots_domain::ChatRelation::Reply { .. } => Ok("reply".into()),
            _ => Err(Error::UnsupportedState {
                axis: "chat_relation".into(),
                operation_id: None,
            }
            .into()),
        }
    }
    pub fn source(&self) -> Result<Option<String>, JsValue> {
        match self.inner.relation() {
            radroots_domain::ChatRelation::Inquire { source } => Ok(source.map(|v| v.to_string())),
            radroots_domain::ChatRelation::Reply { .. } => Ok(None),
            _ => Err(Error::UnsupportedState {
                axis: "chat_relation".into(),
                operation_id: None,
            }
            .into()),
        }
    }
    pub fn parent(&self) -> Result<Option<String>, JsValue> {
        match self.inner.relation() {
            radroots_domain::ChatRelation::Reply { parent } => Ok(Some(parent.to_string())),
            radroots_domain::ChatRelation::Inquire { .. } => Ok(None),
            _ => Err(Error::UnsupportedState {
                axis: "chat_relation".into(),
                operation_id: None,
            }
            .into()),
        }
    }
    /// Explicit presentation copy; JS controls its own resulting string lifetime.
    pub fn text(&self) -> String {
        self.inner.text().to_owned()
    }
}
