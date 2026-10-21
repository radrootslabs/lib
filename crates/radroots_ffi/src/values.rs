use crate::Error;
use std::sync::Arc;

#[derive(Clone, uniffi::Enum)]
pub enum ChatRelation {
    Inquire { source: Option<String> },
    Reply { parent: String },
}

/// An actual installed family-specific shared value, never a private placeholder.
#[derive(Clone, uniffi::Enum)]
pub enum Value {
    Chat { message: Arc<ChatValue> },
    Listing { listing: Arc<ListingValue> },
}

#[derive(uniffi::Object)]
pub struct ListingValue {
    inner: radroots::listings::Listing,
}
impl ListingValue {
    pub(crate) fn captured(inner: &radroots::listings::Listing) -> Arc<Self> {
        Arc::new(Self {
            inner: inner.clone(),
        })
    }
}
#[uniffi::export]
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
    /// Source-owned validated terms data; this is not a new wire producer.
    pub fn terms_json(&self) -> Result<Vec<u8>, Error> {
        serde_json::to_vec(self.inner.terms()).map_err(|_| Error::UnsupportedState {
            axis: "listing_terms".to_owned(),
            operation_id: None,
        })
    }
}

#[derive(uniffi::Object)]
pub struct ChatValue {
    inner: radroots::domain::ChatMessage,
}
impl ChatValue {
    pub(crate) fn captured(inner: &radroots::domain::ChatMessage) -> Arc<Self> {
        Arc::new(Self {
            inner: inner.clone(),
        })
    }
}
#[uniffi::export]
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
    pub fn relation(&self) -> Result<ChatRelation, Error> {
        match self.inner.relation() {
            radroots::domain::ChatRelation::Inquire { source } => Ok(ChatRelation::Inquire {
                source: source.map(|v| v.to_string()),
            }),
            radroots::domain::ChatRelation::Reply { parent } => Ok(ChatRelation::Reply {
                parent: parent.to_string(),
            }),
            _ => Err(Error::UnsupportedState {
                axis: "chat_relation".to_owned(),
                operation_id: None,
            }),
        }
    }
    /// Explicit authorized presentation copy; Rust retains zeroizing storage.
    pub fn text(&self) -> String {
        self.inner.text().to_owned()
    }
}
