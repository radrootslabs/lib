use super::*;
use alloc::{string::String, vec::Vec};
use nostr::nips::nip44::v2::{ConversationKey, decrypt_to_bytes};
use radroots_signing::provider::ProviderError;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

const NOW: u64 = 1_700_200_000;
fn time(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}

#[test]
fn public_past_time_boundary_is_positive_bounded_and_refuses_invalid_time_or_entropy() {
    for now in [1, 2, PAST_SECONDS, NOW, MAX_TIME] {
        let selected = randomized_past_time(time(now), &Entropy::default())
            .unwrap()
            .get();
        assert!((now.saturating_sub(PAST_SECONDS).max(1)..=now).contains(&selected));
    }
    let source = Entropy::default();
    assert_eq!(
        randomized_past_time(time(MAX_TIME + 1), &source),
        Err(WrapError::InvalidTime)
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
    let source = Entropy {
        fail_at: Some(0),
        ..Default::default()
    };
    assert_eq!(
        randomized_past_time(time(NOW), &source),
        Err(WrapError::EntropyUnavailable)
    );
}
fn keys(value: u8) -> nostr::Keys {
    let mut key = [0; 32];
    key[31] = value;
    nostr::Keys::new(nostr::SecretKey::from_slice(&key).unwrap())
}
fn identity(keys: &nostr::Keys) -> PublicKey {
    PublicKey::from_hex(&keys.public_key().to_hex()).unwrap()
}
#[derive(Default)]
struct Entropy {
    calls: AtomicUsize,
    fail_at: Option<usize>,
    draws: Mutex<Vec<[u8; 32]>>,
}
impl NonceSource for Entropy {
    fn next_nonce(&self) -> Result<[u8; 32], ProviderError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_at == Some(call) {
            return Err(ProviderError::CapabilityUnavailable);
        }
        let mut draws = self.draws.lock().unwrap();
        if !draws.is_empty() {
            Ok(draws.remove(0))
        } else {
            Ok([42 + call as u8; 32])
        }
    }
}
fn seal_with(
    author: &nostr::Keys,
    peer: &nostr::Keys,
    kind: u16,
    at: u64,
    tags: Vec<nostr::Tag>,
    content: Option<String>,
) -> (String, String) {
    let mut rumor = nostr::UnsignedEvent {
        id: None,
        pubkey: author.public_key(),
        created_at: nostr::Timestamp::from_secs(1_700_000_000),
        kind: nostr::Kind::PrivateDirectMessage,
        tags: nostr::Tags::from_list(vec![nostr::Tag::public_key(peer.public_key())]),
        content: "original café 🌱\u{0000} message".into(),
    };
    let _ = rumor.id();
    let original = rumor.as_json();
    let content = content.unwrap_or_else(|| {
        base64::engine::general_purpose::STANDARD.encode(
            super::super::private::encrypt_payload(
                author.secret_key(),
                &peer.public_key(),
                &original,
                [31; 32],
            )
            .unwrap(),
        )
    });
    let seal = nostr::EventBuilder::new(nostr::Kind::Custom(kind), content)
        .tags(tags)
        .custom_created_at(nostr::Timestamp::from_secs(at))
        .sign_with_keys(author)
        .unwrap();
    (seal.as_json(), original)
}
fn seal(author: &nostr::Keys, peer: &nostr::Keys) -> (String, String) {
    seal_with(author, peer, 13, 1_699_999_990, vec![], None)
}
fn decrypt(peer: &nostr::Keys, outer: &nostr::Event) -> Vec<u8> {
    let key = ConversationKey::derive(peer.secret_key(), &outer.pubkey).unwrap();
    decrypt_to_bytes(
        &key,
        &base64::engine::general_purpose::STANDARD
            .decode(&outer.content)
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn peer_and_self_wrap_exact_retained_seals_without_participant_signing() {
    let author = keys(1);
    let peer = keys(2);
    for recipient in [&peer, &author] {
        let (raw, rumor) = seal(&author, recipient);
        // Whitespace belongs to obtained representation, though not NIP-01 ID.
        let original = format!(" \n{raw}\t");
        let entropy = Entropy::default();
        let pending =
            wrap_retained_seal(&original, identity(recipient), time(NOW), &entropy).unwrap();
        let outer = nostr::Event::from_json(pending.signed_json()).unwrap();
        outer.verify().unwrap();
        assert_eq!(outer.id.to_bytes(), pending.event_id());
        assert_eq!(outer.kind, nostr::Kind::GiftWrap);
        assert_eq!(
            outer.tags.as_slice(),
            &[nostr::Tag::public_key(recipient.public_key())]
        );
        assert_ne!(outer.pubkey, author.public_key());
        assert_ne!(outer.pubkey, recipient.public_key());
        assert!((NOW - PAST_SECONDS..=NOW).contains(&outer.created_at.as_secs()));
        assert_eq!(decrypt(recipient, &outer), original.as_bytes());
        let recovered = nostr::nips::nip59::extract_rumor(recipient, &outer)
            .await
            .unwrap();
        assert_eq!(recovered.sender, author.public_key());
        assert_eq!(recovered.rumor.as_json(), rumor);
        assert_eq!(pending.recipient(), identity(recipient));
        assert_eq!(
            pending.seal_digest(),
            sha256::Hash::hash(original.as_bytes()).to_byte_array()
        );
        assert_eq!(
            pending.seal_event_id(),
            nostr::Event::from_json(&raw).unwrap().id.to_bytes()
        );
        pending.with_wrapping_key(|key| {
            assert_eq!(keys_from(key).public_key(), outer.pubkey);
        });
        assert_eq!(entropy.calls.load(Ordering::SeqCst), 4);
        assert_eq!(format!("{pending:?}"), "PendingGiftWrap(<redacted>)");
        assert!(pending.signed_json().len() <= SIGNED_MAX_BYTES);
    }
}
fn keys_from(key: &[u8; 32]) -> nostr::Keys {
    nostr::Keys::new(nostr::SecretKey::from_slice(key).unwrap())
}

#[test]
fn malformed_oversized_and_wrong_profile_seals_refuse_before_entropy() {
    let author = keys(1);
    let peer = keys(2);
    let recipient = identity(&peer);
    let (raw, _) = seal(&author, &peer);
    let mut altered: serde_json::Value = serde_json::from_str(&raw).unwrap();
    altered["sig"] = serde_json::Value::String("0".repeat(128));
    let mut extra: serde_json::Value = serde_json::from_str(&raw).unwrap();
    extra["extra"] = 1.into();
    let mut bad_id: serde_json::Value = serde_json::from_str(&raw).unwrap();
    bad_id["id"] = "0".repeat(64).into();
    let mut malformed_payload = vec![2; 98];
    malformed_payload[0] = 3;
    let cases = vec![
        ("".into(), WrapError::InvalidSeal),
        ("{}".into(), WrapError::InvalidSeal),
        ("x".repeat(SIGNED_MAX_BYTES + 1), WrapError::LimitExceeded),
        (
            format!("{{\"kind\":13,{}", &raw[1..]),
            WrapError::InvalidSeal,
        ),
        (altered.to_string(), WrapError::InvalidSeal),
        (extra.to_string(), WrapError::InvalidSeal),
        (bad_id.to_string(), WrapError::InvalidSeal),
        (
            seal_with(&author, &peer, 1059, NOW, vec![], None).0,
            WrapError::InvalidSeal,
        ),
        (
            seal_with(&author, &peer, 13, 0, vec![], None).0,
            WrapError::InvalidSeal,
        ),
        (
            seal_with(&author, &peer, 13, NOW + 1, vec![], None).0,
            WrapError::InvalidSeal,
        ),
        (
            seal_with(
                &author,
                &peer,
                13,
                NOW,
                vec![nostr::Tag::public_key(peer.public_key())],
                None,
            )
            .0,
            WrapError::InvalidSeal,
        ),
        (
            seal_with(&author, &peer, 13, NOW, vec![], Some("not base64".into())).0,
            WrapError::InvalidSeal,
        ),
        (
            seal_with(
                &author,
                &peer,
                13,
                NOW,
                vec![],
                Some(base64::engine::general_purpose::STANDARD.encode(&malformed_payload)),
            )
            .0,
            WrapError::InvalidSeal,
        ),
    ];
    for (raw, expected) in cases {
        let entropy = Entropy::default();
        assert_eq!(
            wrap_retained_seal(&raw, recipient, time(NOW), &entropy).unwrap_err(),
            expected
        );
        assert_eq!(entropy.calls.load(Ordering::SeqCst), 0);
    }
    let entropy = Entropy::default();
    assert_eq!(
        wrap_retained_seal(&raw, recipient, time(MAX_TIME + 1), &entropy).unwrap_err(),
        WrapError::InvalidTime
    );
    assert_eq!(entropy.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn exact_large_representation_cannot_hide_behind_small_canonical_seal() {
    let author = keys(1);
    let peer = keys(2);
    let (raw, _) = seal(&author, &peer);
    let original = format!("{}{raw}", " ".repeat(60_000));
    assert!(original.len() < SIGNED_MAX_BYTES);
    let entropy = Entropy::default();
    assert_eq!(
        wrap_retained_seal(&original, identity(&peer), time(NOW), &entropy).unwrap_err(),
        WrapError::LimitExceeded
    );
    assert_eq!(entropy.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn large_supported_representation_and_latest_profile_time_execute_real_crypto() {
    let author = keys(1);
    let peer = keys(2);
    let (raw, _) = seal(&author, &peer);
    let original = format!("{}{raw}", " ".repeat(40_960 - raw.len()));
    let pending = wrap_retained_seal(
        &original,
        identity(&peer),
        time(MAX_TIME),
        &Entropy::default(),
    )
    .unwrap();
    let outer = nostr::Event::from_json(pending.signed_json()).unwrap();
    outer.verify().unwrap();
    assert_eq!(decrypt(&peer, &outer), original.as_bytes());
    assert!((MAX_TIME - PAST_SECONDS..=MAX_TIME).contains(&outer.created_at.as_secs()));
    assert!(
        pending.signed_json().len()
            <= GiftWrapBudget::for_seal(original.len(), identity(&peer))
                .unwrap()
                .signed_max_bytes()
    );
}

#[test]
fn failure_at_each_entropy_boundary_preserves_original_input() {
    let author = keys(1);
    let peer = keys(2);
    let (raw, _) = seal(&author, &peer);
    let before = raw.clone();
    for fail_at in 0..4 {
        let entropy = Entropy {
            fail_at: Some(fail_at),
            ..Default::default()
        };
        assert_eq!(
            wrap_retained_seal(&raw, identity(&peer), time(NOW), &entropy).unwrap_err(),
            WrapError::EntropyUnavailable
        );
        assert_eq!(entropy.calls.load(Ordering::SeqCst), fail_at + 1);
        assert_eq!(raw, before);
    }
}

#[test]
fn rejection_sampling_is_bounded_and_never_uses_participant_keys() {
    let author = keys(1);
    let peer = keys(2);
    let mut one = [0; 32];
    one[31] = 1;
    let mut two = [0; 32];
    two[31] = 2;
    let entropy = Entropy {
        draws: Mutex::new(vec![[0; 32], [255; 32], one, two, [3; 32]]),
        ..Default::default()
    };
    let (_, generated) = wrapping_keys(&entropy, identity(&author), identity(&peer)).unwrap();
    assert_ne!(generated.public_key(), author.public_key());
    assert_ne!(generated.public_key(), peer.public_key());
    assert_eq!(entropy.calls.load(Ordering::SeqCst), 5);
    let entropy = Entropy {
        draws: Mutex::new(vec![[0; 32]; ENTROPY_DRAWS]),
        ..Default::default()
    };
    assert!(matches!(
        wrapping_keys(&entropy, identity(&author), identity(&peer)),
        Err(WrapError::EntropyUnavailable)
    ));
    assert_eq!(entropy.calls.load(Ordering::SeqCst), ENTROPY_DRAWS);
    let entropy = Entropy {
        draws: Mutex::new(vec![[0; 32]; ENTROPY_DRAWS]),
        ..Default::default()
    };
    assert_eq!(
        past_time(NOW, &entropy).unwrap_err(),
        WrapError::EntropyUnavailable
    );
    assert_eq!(entropy.calls.load(Ordering::SeqCst), ENTROPY_DRAWS);
    assert_eq!(past_time(1, &Entropy::default()).unwrap(), 1);
    let entropy = Entropy {
        draws: Mutex::new(vec![[0; 32], [255; 32]]),
        ..Default::default()
    };
    let at = past_time(NOW, &entropy).unwrap();
    assert!((NOW - PAST_SECONDS..=NOW).contains(&at));
    assert_eq!(entropy.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn temporary_wraps_are_separate_and_wrong_recipient_cannot_decrypt() {
    let author = keys(1);
    let peer = keys(2);
    let (raw, _) = seal(&author, &peer);
    let entropy = Entropy::default();
    let first = wrap_retained_seal(&raw, identity(&peer), time(NOW), &entropy).unwrap();
    let second = wrap_retained_seal(&raw, identity(&peer), time(NOW), &entropy).unwrap();
    assert_ne!(first.event_id(), second.event_id());
    assert_eq!(first.seal_digest(), second.seal_digest());
    let outer = nostr::Event::from_json(first.signed_json()).unwrap();
    let key = ConversationKey::derive(author.secret_key(), &outer.pubkey).unwrap();
    assert!(
        decrypt_to_bytes(
            &key,
            &base64::engine::general_purpose::STANDARD
                .decode(&outer.content)
                .unwrap()
        )
        .is_err()
    );
    for error in [
        WrapError::InvalidSeal,
        WrapError::LimitExceeded,
        WrapError::InvalidTime,
        WrapError::EntropyUnavailable,
        WrapError::CryptoFailure,
    ] {
        assert!(error.to_string().len() < 40);
        assert!(!error.to_string().contains(&raw));
    }
}
