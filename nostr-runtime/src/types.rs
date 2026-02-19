use alloc::string::String;
#[cfg(feature = "nostr-client")]
use radroots_nostr::prelude::RadrootsNostrFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsNostrSubscriptionPolicy {
    Streaming,
    OneShotOnEose,
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrSubscriptionSpec {
    pub name: Option<String>,
    #[cfg(feature = "nostr-client")]
    pub filter: RadrootsNostrFilter,
    pub policy: RadrootsNostrSubscriptionPolicy,
    pub stream_timeout_secs: u64,
    pub reconnect_delay_millis: u64,
}

impl RadrootsNostrSubscriptionSpec {
    pub const DEFAULT_STREAM_TIMEOUT_SECS: u64 = 30;
    pub const DEFAULT_RECONNECT_DELAY_MILLIS: u64 = 2_000;

    #[cfg(feature = "nostr-client")]
    pub fn streaming(filter: RadrootsNostrFilter) -> Self {
        Self {
            name: None,
            filter,
            policy: RadrootsNostrSubscriptionPolicy::Streaming,
            stream_timeout_secs: Self::DEFAULT_STREAM_TIMEOUT_SECS,
            reconnect_delay_millis: Self::DEFAULT_RECONNECT_DELAY_MILLIS,
        }
    }

    #[cfg(feature = "nostr-client")]
    pub fn one_shot(filter: RadrootsNostrFilter) -> Self {
        Self {
            name: None,
            filter,
            policy: RadrootsNostrSubscriptionPolicy::OneShotOnEose,
            stream_timeout_secs: Self::DEFAULT_STREAM_TIMEOUT_SECS,
            reconnect_delay_millis: Self::DEFAULT_RECONNECT_DELAY_MILLIS,
        }
    }

    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn stream_timeout_secs(mut self, value: u64) -> Self {
        self.stream_timeout_secs = value;
        self
    }

    pub fn reconnect_delay_millis(mut self, value: u64) -> Self {
        self.reconnect_delay_millis = value;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RadrootsNostrSubscriptionHandle {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RadrootsNostrRuntimeEvent {
    RuntimeStarted,
    RuntimeStopped,
    SubscriptionOpened {
        id: String,
    },
    SubscriptionClosed {
        id: String,
    },
    Note {
        subscription_id: String,
        id: String,
        author: String,
        kind: u16,
        relay: Option<String>,
    },
    Notice {
        relay: String,
        message: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsNostrTrafficLight {
    Red,
    Yellow,
    Green,
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrConnectionSnapshot {
    pub light: RadrootsNostrTrafficLight,
    pub connected: usize,
    pub connecting: usize,
    pub last_error: Option<String>,
}

impl Default for RadrootsNostrConnectionSnapshot {
    fn default() -> Self {
        Self {
            light: RadrootsNostrTrafficLight::Red,
            connected: 0,
            connecting: 0,
            last_error: None,
        }
    }
}
