//! Explicit NIP-42 relay authentication state.

use crate::{Error, NostrTransport, RelayUrl};
use nostr_sdk::prelude::{ClientMessage, Event};
use radroots_transport::BoxFuture;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

const MAX_CHALLENGE_BYTES: usize = 1_024;
const MAX_CHALLENGE_LIFETIME_MS: u64 = 600_000;

#[derive(Clone, Eq, PartialEq)]
struct PendingAuth {
    challenge: String,
    required_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

impl fmt::Debug for PendingAuth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingAuth")
            .field("challenge", &"[redacted]")
            .field("required_at_unix_ms", &self.required_at_unix_ms)
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
            .finish()
    }
}

pub(crate) trait AuthClient: Send + Sync {
    fn submit<'a>(&'a self, relay: RelayUrl, event: Event) -> BoxFuture<'a, Result<(), Error>>;
}

#[derive(Clone, Debug)]
pub(crate) struct LiveAuthClient {
    client: nostr_sdk::Client,
}

impl LiveAuthClient {
    pub(crate) const fn new(client: nostr_sdk::Client) -> Self {
        Self { client }
    }
}

impl AuthClient for LiveAuthClient {
    // Relay submission is external SDK I/O; the state machine and submission
    // outcomes are covered through the injected AuthClient boundary.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn submit<'a>(&'a self, relay: RelayUrl, event: Event) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            let expected = relay.as_str().trim_end_matches('/');
            let output = self
                .client
                .send_msg_to([relay.as_str()], ClientMessage::auth(event))
                .await
                .map_err(|_| Error::AuthTransport)?;
            if output
                .success
                .iter()
                .any(|url| url.to_string().trim_end_matches('/') == expected)
            {
                Ok(())
            } else {
                Err(Error::AuthTransport)
            }
        })
    }
}

pub(crate) struct AuthFlow {
    client: Arc<dyn AuthClient>,
    pending: Mutex<BTreeMap<RelayUrl, PendingAuth>>,
}

impl AuthFlow {
    pub(crate) fn new(client: Arc<dyn AuthClient>) -> Self {
        Self {
            client,
            pending: Mutex::new(BTreeMap::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn isolated() -> Self {
        let client = nostr_sdk::Client::default();
        client.automatic_authentication(false);
        Self::new(Arc::new(LiveAuthClient::new(client)))
    }

    fn begin(
        &self,
        relay: RelayUrl,
        challenge: &str,
        required_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<String, Error> {
        validate_challenge(challenge, required_at_unix_ms, expires_at_unix_ms)?;
        let pending = PendingAuth {
            challenge: challenge.to_owned(),
            required_at_unix_ms,
            expires_at_unix_ms,
        };
        let mut state = self
            .pending
            .lock()
            .map_err(|_| Error::AuthStateUnavailable)?;
        match state.get(&relay) {
            Some(existing) if existing == &pending => {}
            Some(_) => return Err(Error::AuthChallengeConflict),
            None => {
                state.insert(relay.clone(), pending);
            }
        }
        serde_json::to_string(&serde_json::json!({
            "content": "",
            "created_at_max_unix_ms": expires_at_unix_ms,
            "created_at_min_unix_ms": required_at_unix_ms,
            "kind": 22242,
            "tags": [
                ["challenge", challenge],
                ["relay", relay.as_str()],
            ],
        }))
        .map_err(|_| Error::AuthStateUnavailable)
    }

    fn pending(&self, relay: &RelayUrl, challenge: &str) -> Result<PendingAuth, Error> {
        let state = self
            .pending
            .lock()
            .map_err(|_| Error::AuthStateUnavailable)?;
        let pending = state.get(relay).ok_or(Error::AuthChallengeMissing)?;
        if pending.challenge != challenge {
            return Err(Error::AuthResponseMismatch);
        }
        Ok(pending.clone())
    }

    fn remove(&self, relay: &RelayUrl, challenge: &str) -> Result<(), Error> {
        let mut state = self
            .pending
            .lock()
            .map_err(|_| Error::AuthStateUnavailable)?;
        match state.get(relay) {
            Some(pending) if pending.challenge == challenge => {
                state.remove(relay);
                Ok(())
            }
            Some(_) => Err(Error::AuthResponseMismatch),
            None => Err(Error::AuthChallengeMissing),
        }
    }
}

impl fmt::Debug for AuthFlow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pending_count = self
            .pending
            .lock()
            .map(|pending| pending.len())
            .unwrap_or_default();
        formatter
            .debug_struct("AuthFlow")
            .field("pending_count", &pending_count)
            .finish_non_exhaustive()
    }
}

impl NostrTransport {
    /// Records an exact relay challenge and returns bounded signing input.
    pub fn begin_authentication(
        &self,
        relay: &RelayUrl,
        challenge: impl AsRef<str>,
        required_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<String, Error> {
        if !self.config().relays().contains(relay) {
            return Err(Error::AuthResponseMismatch);
        }
        self.auth.begin(
            relay.clone(),
            challenge.as_ref(),
            required_at_unix_ms,
            expires_at_unix_ms,
        )
    }

    /// Verifies and submits one host-signed NIP-42 response without retrying.
    pub fn complete_authentication<'a>(
        &'a self,
        relay: &'a RelayUrl,
        challenge: &'a str,
        signed_event_json: Option<&'a str>,
        now_unix_ms: u64,
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            let pending = self.auth.pending(relay, challenge)?;
            if now_unix_ms > pending.expires_at_unix_ms {
                self.auth.remove(relay, challenge)?;
                return Err(Error::AuthChallengeExpired);
            }
            let signed_event_json = signed_event_json.ok_or(Error::AuthSignerUnavailable)?;
            let signed = radroots_event_codec::decode::signed_event(signed_event_json)
                .map_err(|_| Error::AuthResponseInvalid)?;
            if radroots_nostr::event::verify(signed.envelope())
                != radroots_nostr::event::Verification::Verified
            {
                return Err(Error::AuthResponseInvalid);
            }
            let created_at_ms = signed.created_at().saturating_mul(1_000);
            if signed.kind() != 22242
                || !signed.content().is_empty()
                || created_at_ms < pending.required_at_unix_ms
                || created_at_ms > pending.expires_at_unix_ms
                || !has_exact_tag(signed.tags_as_vec().as_slice(), "challenge", challenge)
                || !has_exact_tag(signed.tags_as_vec().as_slice(), "relay", relay.as_str())
            {
                return Err(Error::AuthResponseMismatch);
            }
            let event = radroots_nostr::event::to_nostr(signed.envelope())
                .map_err(|_| Error::AuthResponseInvalid)?;
            self.auth.client.submit(relay.clone(), event).await?;
            self.auth.remove(relay, challenge)
        })
    }

    /// Rejects and consumes one exact pending challenge without relay access.
    pub fn reject_authentication(&self, relay: &RelayUrl, challenge: &str) -> Result<(), Error> {
        self.auth.remove(relay, challenge)
    }

    #[cfg(test)]
    fn with_auth_client(mut self, client: Arc<dyn AuthClient>) -> Self {
        self.auth = Arc::new(AuthFlow::new(client));
        self
    }
}

fn validate_challenge(
    challenge: &str,
    required_at_unix_ms: u64,
    expires_at_unix_ms: u64,
) -> Result<(), Error> {
    if challenge.is_empty()
        || challenge.len() > MAX_CHALLENGE_BYTES
        || challenge != challenge.trim()
        || challenge.chars().any(char::is_control)
        || required_at_unix_ms == 0
        || expires_at_unix_ms <= required_at_unix_ms
        || expires_at_unix_ms - required_at_unix_ms > MAX_CHALLENGE_LIFETIME_MS
    {
        return Err(Error::InvalidAuthChallenge);
    }
    Ok(())
}

fn has_exact_tag(tags: &[Vec<String>], name: &str, value: &str) -> bool {
    tags.iter().any(|tag| {
        tag.len() == 2 && tag.first().is_some_and(|item| item == name) && tag[1] == value
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, RelayUrlPolicy};
    use nostr_sdk::prelude::{
        EventBuilder, JsonUtil, Keys, RelayUrl as UpstreamRelayUrl, Timestamp,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct MockAuthClient(AtomicUsize);

    impl AuthClient for MockAuthClient {
        fn submit<'a>(
            &'a self,
            _relay: RelayUrl,
            _event: Event,
        ) -> BoxFuture<'a, Result<(), Error>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }

    fn transport() -> (NostrTransport, Arc<MockAuthClient>, RelayUrl) {
        let relay =
            RelayUrl::parse("wss://relay.example.com", RelayUrlPolicy::Public).expect("relay");
        let config = Config::from_profile(
            crate::profile::test_profile(
                crate::RelayProfileKind::Public,
                RelayUrlPolicy::Public,
                [relay.as_str()],
            )
            .expect("profile"),
        );
        let client = Arc::new(MockAuthClient(AtomicUsize::new(0)));
        let transport = NostrTransport::new(config).with_auth_client(client.clone());
        (transport, client, relay)
    }

    fn signed_response(relay: &RelayUrl, challenge: &str, created_at: u64) -> String {
        EventBuilder::auth(
            challenge,
            relay
                .as_str()
                .parse::<UpstreamRelayUrl>()
                .expect("upstream relay"),
        )
        .custom_created_at(Timestamp::from_secs(created_at))
        .sign_with_keys(&Keys::generate())
        .expect("signed auth event")
        .as_json()
    }

    #[test]
    fn challenge_response_is_exact_bounded_and_single_use() {
        let (transport, client, relay) = transport();
        let input = transport
            .begin_authentication(&relay, "challenge-a", 1_000_000, 1_600_000)
            .expect("challenge");
        assert!(input.contains("\"kind\":22242"));
        assert!(input.contains("challenge-a"));
        let response = signed_response(&relay, "challenge-a", 1_200);
        futures::executor::block_on(transport.complete_authentication(
            &relay,
            "challenge-a",
            Some(response.as_str()),
            1_300_000,
        ))
        .expect("complete auth");
        assert_eq!(client.0.load(Ordering::SeqCst), 1);
        assert_eq!(
            futures::executor::block_on(transport.complete_authentication(
                &relay,
                "challenge-a",
                Some(response.as_str()),
                1_300_000,
            )),
            Err(Error::AuthChallengeMissing)
        );
    }

    #[test]
    fn wrong_relay_timeout_rejection_and_no_signer_fail_closed() {
        let (transport, client, relay) = transport();
        transport
            .begin_authentication(&relay, "challenge-a", 1_000_000, 1_600_000)
            .expect("challenge");
        assert_eq!(
            futures::executor::block_on(transport.complete_authentication(
                &relay,
                "challenge-a",
                None,
                1_100_000,
            )),
            Err(Error::AuthSignerUnavailable)
        );
        let wrong = signed_response(&relay, "challenge-b", 1_200);
        assert_eq!(
            futures::executor::block_on(transport.complete_authentication(
                &relay,
                "challenge-a",
                Some(wrong.as_str()),
                1_300_000,
            )),
            Err(Error::AuthResponseMismatch)
        );
        let wrong_relay = RelayUrl::parse("wss://other.example.com", RelayUrlPolicy::Public)
            .expect("other relay");
        let wrong = signed_response(&wrong_relay, "challenge-a", 1_200);
        assert_eq!(
            futures::executor::block_on(transport.complete_authentication(
                &relay,
                "challenge-a",
                Some(wrong.as_str()),
                1_300_000,
            )),
            Err(Error::AuthResponseMismatch)
        );
        let valid = signed_response(&relay, "challenge-a", 1_200);
        assert_eq!(
            futures::executor::block_on(transport.complete_authentication(
                &relay,
                "challenge-a",
                Some(valid.as_str()),
                1_700_000,
            )),
            Err(Error::AuthChallengeExpired)
        );
        assert_eq!(client.0.load(Ordering::SeqCst), 0);

        transport
            .begin_authentication(&relay, "challenge-b", 2_000_000, 2_600_000)
            .expect("second challenge");
        transport
            .reject_authentication(&relay, "challenge-b")
            .expect("reject");
        assert_eq!(
            transport.reject_authentication(&relay, "challenge-b"),
            Err(Error::AuthChallengeMissing)
        );
    }

    #[test]
    fn challenge_debug_is_redacted_and_conflicts_are_rejected() {
        let (transport, _, relay) = transport();
        transport
            .begin_authentication(&relay, "secret-challenge", 1_000, 2_000)
            .expect("challenge");
        assert!(!format!("{:?}", transport.auth).contains("secret-challenge"));
        assert_eq!(
            transport.begin_authentication(&relay, "different", 1_000, 2_000),
            Err(Error::AuthChallengeConflict)
        );
        assert_eq!(
            transport.reject_authentication(&relay, "different"),
            Err(Error::AuthResponseMismatch)
        );
        transport
            .begin_authentication(&relay, "secret-challenge", 1_000, 2_000)
            .expect("idempotent challenge");
    }

    #[test]
    fn challenge_validation_rejects_each_invalid_boundary() {
        for (challenge, required, expires) in [
            ("", 1, 2),
            (&"a".repeat(MAX_CHALLENGE_BYTES + 1), 1, 2),
            (" challenge", 1, 2),
            ("challenge ", 1, 2),
            ("chall\nenge", 1, 2),
            ("challenge", 0, 2),
            ("challenge", 2, 2),
            ("challenge", 2, 1),
            ("challenge", 1, MAX_CHALLENGE_LIFETIME_MS + 2),
        ] {
            assert_eq!(
                validate_challenge(challenge, required, expires),
                Err(Error::InvalidAuthChallenge)
            );
        }
        assert!(validate_challenge("challenge", 1, MAX_CHALLENGE_LIFETIME_MS + 1).is_ok());

        assert!(has_exact_tag(
            &[vec!["challenge".into(), "value".into()]],
            "challenge",
            "value"
        ));
        assert!(!has_exact_tag(&[], "challenge", "value"));
        assert!(!has_exact_tag(
            &[vec!["challenge".into()]],
            "challenge",
            "value"
        ));
        assert!(!has_exact_tag(
            &[vec!["other".into(), "value".into()]],
            "challenge",
            "value"
        ));
        assert!(!has_exact_tag(
            &[vec!["challenge".into(), "other".into()]],
            "challenge",
            "value"
        ));
    }

    #[test]
    fn authentication_rejects_unconfigured_and_malformed_responses() {
        let (transport, _, relay) = transport();
        let other =
            RelayUrl::parse("wss://other.example.com", RelayUrlPolicy::Public).expect("other");
        assert_eq!(
            transport.begin_authentication(&other, "challenge", 1, 2),
            Err(Error::AuthResponseMismatch)
        );
        transport
            .begin_authentication(&relay, "challenge", 1_000, 2_000)
            .expect("begin");
        assert_eq!(
            futures::executor::block_on(transport.complete_authentication(
                &relay,
                "wrong",
                Some("{}"),
                1_500
            )),
            Err(Error::AuthResponseMismatch)
        );
        assert_eq!(
            futures::executor::block_on(transport.complete_authentication(
                &relay,
                "challenge",
                Some("{}"),
                1_500
            )),
            Err(Error::AuthResponseInvalid)
        );
    }
}
