#![forbid(unsafe_code)]

use crate::RadrootsRelayTransportError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsRelayUrlPolicy {
    Public,
    LocalDev,
}

impl RadrootsRelayUrlPolicy {
    fn accepts_ws(self) -> bool {
        matches!(self, Self::LocalDev)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RadrootsRelayUrl(String);

impl RadrootsRelayUrl {
    pub fn parse(
        value: impl AsRef<str>,
        policy: RadrootsRelayUrlPolicy,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let original = value.as_ref().trim();
        let parsed =
            Url::parse(original).map_err(|error| RadrootsRelayTransportError::RelayUrlParse {
                url: original.to_owned(),
                reason: error.to_string(),
            })?;
        let scheme = parsed.scheme();
        match scheme {
            "wss" => {}
            "ws" if policy.accepts_ws() => {}
            "ws" => {
                return Err(RadrootsRelayTransportError::WsRequiresLocalPolicy {
                    url: original.to_owned(),
                });
            }
            other => {
                return Err(RadrootsRelayTransportError::UnsupportedRelayScheme {
                    url: original.to_owned(),
                    scheme: other.to_owned(),
                });
            }
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(RadrootsRelayTransportError::RelayUrlUserinfo {
                url: original.to_owned(),
            });
        }
        if parsed.host_str().is_none_or(str::is_empty) {
            return Err(RadrootsRelayTransportError::EmptyRelayHost {
                url: original.to_owned(),
            });
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(RadrootsRelayTransportError::RelayUrlQueryOrFragment {
                url: original.to_owned(),
            });
        }
        let mut normalized = parsed.to_string();
        if parsed.path() == "/" {
            normalized.pop();
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RadrootsRelayUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayTargetSet {
    relays: Vec<RadrootsRelayUrl>,
}

impl RadrootsRelayTargetSet {
    pub fn new<I, S>(
        relays: I,
        policy: RadrootsRelayUrlPolicy,
    ) -> Result<Self, RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let relays = relays
            .into_iter()
            .map(|relay| RadrootsRelayUrl::parse(relay, policy))
            .collect::<Result<BTreeSet<_>, _>>()?
            .into_iter()
            .collect::<Vec<_>>();
        if relays.is_empty() {
            return Err(RadrootsRelayTransportError::EmptyTargetSet);
        }
        Ok(Self { relays })
    }

    pub fn from_urls(relays: Vec<RadrootsRelayUrl>) -> Result<Self, RadrootsRelayTransportError> {
        let relays = relays
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if relays.is_empty() {
            return Err(RadrootsRelayTransportError::EmptyTargetSet);
        }
        Ok(Self { relays })
    }

    pub fn relays(&self) -> &[RadrootsRelayUrl] {
        &self.relays
    }

    pub fn relay_strings(&self) -> Vec<String> {
        self.relays
            .iter()
            .map(|relay| relay.as_str().to_owned())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.relays.len()
    }

    pub fn is_empty(&self) -> bool {
        self.relays.is_empty()
    }
}
