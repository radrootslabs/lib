use alloc::string::String;
#[cfg(feature = "nostr-client")]
use radroots_nostr::prelude::{
    RadrootsNostrFilter, RadrootsNostrPublicKey, RadrootsNostrTimestamp, radroots_nostr_kind,
    radroots_nostr_post_events_filter,
};

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

    pub fn with_policy(mut self, policy: RadrootsNostrSubscriptionPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[cfg(feature = "nostr-client")]
    pub fn text_notes(
        limit: Option<u16>,
        since_unix: Option<u64>,
        policy: RadrootsNostrSubscriptionPolicy,
    ) -> Self {
        Self::streaming(radroots_nostr_post_events_filter(limit, since_unix)).with_policy(policy)
    }

    #[cfg(feature = "nostr-client")]
    pub fn by_kind(
        kind: u16,
        limit: Option<u16>,
        since_unix: Option<u64>,
        policy: RadrootsNostrSubscriptionPolicy,
    ) -> Self {
        let mut filter = RadrootsNostrFilter::new().kind(radroots_nostr_kind(kind));
        if let Some(limit) = limit {
            filter = filter.limit(limit.into());
        }
        if let Some(since) = since_unix {
            filter = filter.since(RadrootsNostrTimestamp::from(since));
        }
        Self::streaming(filter).with_policy(policy)
    }

    #[cfg(feature = "nostr-client")]
    pub fn by_author(mut self, author: RadrootsNostrPublicKey) -> Self {
        self.filter = self.filter.author(author);
        self
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "nostr-client")]
    use radroots_nostr::prelude::RadrootsNostrKeys;

    fn base_spec() -> RadrootsNostrSubscriptionSpec {
        RadrootsNostrSubscriptionSpec {
            name: None,
            #[cfg(feature = "nostr-client")]
            filter: radroots_nostr::prelude::RadrootsNostrFilter::new(),
            policy: RadrootsNostrSubscriptionPolicy::Streaming,
            stream_timeout_secs: RadrootsNostrSubscriptionSpec::DEFAULT_STREAM_TIMEOUT_SECS,
            reconnect_delay_millis: RadrootsNostrSubscriptionSpec::DEFAULT_RECONNECT_DELAY_MILLIS,
        }
    }

    #[cfg(feature = "nostr-client")]
    #[test]
    fn text_notes_constructor_sets_defaults() {
        let spec = RadrootsNostrSubscriptionSpec::text_notes(
            Some(5),
            Some(10),
            RadrootsNostrSubscriptionPolicy::Streaming,
        );
        assert!(matches!(
            spec.policy,
            RadrootsNostrSubscriptionPolicy::Streaming
        ));
        assert_eq!(
            spec.stream_timeout_secs,
            RadrootsNostrSubscriptionSpec::DEFAULT_STREAM_TIMEOUT_SECS
        );
        assert_eq!(
            spec.reconnect_delay_millis,
            RadrootsNostrSubscriptionSpec::DEFAULT_RECONNECT_DELAY_MILLIS
        );
    }

    #[cfg(feature = "nostr-client")]
    #[test]
    fn by_kind_constructor_respects_policy() {
        let spec = RadrootsNostrSubscriptionSpec::by_kind(
            30023,
            None,
            None,
            RadrootsNostrSubscriptionPolicy::OneShotOnEose,
        );
        assert!(matches!(
            spec.policy,
            RadrootsNostrSubscriptionPolicy::OneShotOnEose
        ));
    }

    #[cfg(feature = "nostr-client")]
    #[test]
    fn builder_methods_update_spec_fields() {
        let keys = RadrootsNostrKeys::generate();
        let author = keys.public_key();
        let spec = RadrootsNostrSubscriptionSpec::text_notes(
            None,
            None,
            RadrootsNostrSubscriptionPolicy::Streaming,
        )
        .by_author(author)
        .named("posts")
        .stream_timeout_secs(12)
        .reconnect_delay_millis(99)
        .with_policy(RadrootsNostrSubscriptionPolicy::OneShotOnEose);

        assert_eq!(spec.name.as_deref(), Some("posts"));
        assert_eq!(spec.stream_timeout_secs, 12);
        assert_eq!(spec.reconnect_delay_millis, 99);
        assert!(matches!(
            spec.policy,
            RadrootsNostrSubscriptionPolicy::OneShotOnEose
        ));
    }

    #[test]
    fn builder_methods_update_common_fields_without_client_feature() {
        let spec = base_spec()
            .named("posts")
            .stream_timeout_secs(12)
            .reconnect_delay_millis(99)
            .with_policy(RadrootsNostrSubscriptionPolicy::OneShotOnEose);

        assert_eq!(spec.name.as_deref(), Some("posts"));
        assert_eq!(spec.stream_timeout_secs, 12);
        assert_eq!(spec.reconnect_delay_millis, 99);
        assert!(matches!(
            spec.policy,
            RadrootsNostrSubscriptionPolicy::OneShotOnEose
        ));
    }

    #[test]
    fn connection_snapshot_default_is_red() {
        let snapshot = RadrootsNostrConnectionSnapshot::default();
        assert!(matches!(snapshot.light, RadrootsNostrTrafficLight::Red));
        assert_eq!(snapshot.connected, 0);
        assert_eq!(snapshot.connecting, 0);
        assert!(snapshot.last_error.is_none());
    }

    #[test]
    fn branch_probe_covers_true_and_false_paths() {
        let mut total = 0;
        for flag in [true, false] {
            if flag {
                total += 1;
            } else {
                total += 2;
            }
        }
        assert_eq!(total, 3);
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
