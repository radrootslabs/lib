#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RadrootsNostrNdbIngestSource {
    Client,
    Relay { relay_url: Option<String> },
}

impl RadrootsNostrNdbIngestSource {
    pub fn client() -> Self {
        Self::Client
    }

    pub fn relay(relay_url: impl Into<String>) -> Self {
        Self::Relay {
            relay_url: Some(relay_url.into()),
        }
    }

    pub fn relay_unknown() -> Self {
        Self::Relay { relay_url: None }
    }

    pub(crate) fn to_ndb_metadata(&self) -> nostrdb::IngestMetadata {
        match self {
            Self::Client => nostrdb::IngestMetadata::new().client(true),
            Self::Relay { relay_url } => {
                let meta = nostrdb::IngestMetadata::new().client(false);
                if let Some(relay_url) = relay_url {
                    meta.relay(relay_url.as_str())
                } else {
                    meta
                }
            }
        }
    }
}

impl Default for RadrootsNostrNdbIngestSource {
    fn default() -> Self {
        Self::Client
    }
}
