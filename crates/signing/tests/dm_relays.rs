#![cfg(feature = "chat")]

use nostr::JsonUtil;
use radroots_domain::PublicKey;
use radroots_signing::{issuance::VerifiedDmRelayAdvertisement, provider::ProviderError};

fn key() -> nostr::Keys {
    let mut scalar = [0; 32];
    scalar[31] = 1; // Published synthetic identity, never host custody.
    nostr::Keys::new(nostr::SecretKey::from_slice(&scalar).unwrap())
}
fn participant() -> PublicKey {
    PublicKey::from_hex(&key().public_key().to_hex()).unwrap()
}
fn event(kind: u16, tags: Vec<Vec<String>>) -> String {
    nostr::UnsignedEvent {
        id: None,
        pubkey: key().public_key(),
        created_at: nostr::Timestamp::from_secs(1_700_000_000),
        kind: nostr::Kind::Custom(kind),
        tags: nostr::Tags::from_list(
            tags.into_iter()
                .map(|tag| nostr::Tag::parse(tag).unwrap())
                .collect(),
        ),
        content: "Preserved signed metadata content does not supply a route.".into(),
    }
    .sign_with_keys(&key())
    .unwrap()
    .as_json()
}

#[test]
fn actual_verified_advertisement_retains_original_evidence_and_preference_without_readiness_claims()
{
    let raw = event(
        10050,
        vec![
            vec!["relay".into(), "wss://first.example/".into()],
            vec!["alt".into(), "public route information".into()],
            vec!["relay".into(), "wss://second.example/".into()],
            vec!["relay".into(), "wss://first.example/".into()],
        ],
    );
    let verified = VerifiedDmRelayAdvertisement::verify(&raw, participant()).unwrap();
    let decoded = nostr::Event::from_json(&raw).unwrap();
    decoded.verify().unwrap();
    assert_eq!(verified.event_id().as_bytes(), decoded.id.as_bytes());
    assert_eq!(verified.raw_json(), raw);
    assert_eq!(verified.advertisement().participant(), participant());
    assert_eq!(verified.advertisement().created_at().get(), 1_700_000_000);
    let urls: Vec<_> = verified
        .advertisement()
        .advertised_relays()
        .iter()
        .map(|url| url.as_ref())
        .collect();
    assert_eq!(urls, ["wss://first.example/", "wss://second.example/"]);
    let selected = vec![String::from("wss://second.example/")];
    let route = verified.capture_route(&selected).unwrap();
    assert_eq!(route.participant(), participant());
    assert_eq!(route.source_id(), verified.event_id());
    assert_eq!(route.raw_json(), raw);
    assert_eq!(route.selected_relays()[0].as_ref(), selected[0]);
    assert!(!format!("{route:?}").contains("example"));
    for refused in [
        vec![],
        vec![String::from("wss://unadvertised.example/")],
        vec![selected[0].clone(), selected[0].clone()],
    ] {
        assert_eq!(
            verified.capture_route(&refused).unwrap_err(),
            ProviderError::InvalidInput
        );
    }
    assert!(!format!("{verified:?} {:?}", verified.advertisement()).contains("example"));
    let absent =
        VerifiedDmRelayAdvertisement::verify(&event(10050, vec![]), participant()).unwrap();
    assert!(absent.advertisement().advertised_relays().is_empty());
    assert!(absent.capture_route(&selected).is_err());
}

#[test]
fn correct_source_signature_is_required_for_the_exact_participant_and_profile() {
    let raw = event(
        10050,
        vec![vec!["relay".into(), "wss://first.example/".into()]],
    );
    let other =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    assert_eq!(
        VerifiedDmRelayAdvertisement::verify(&raw, other).unwrap_err(),
        ProviderError::AuthenticationFailed
    );
    assert_eq!(
        VerifiedDmRelayAdvertisement::verify(&event(10051, vec![]), participant()).unwrap_err(),
        ProviderError::UnsupportedVersion
    );
    let mut changed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    changed["sig"] = "00".repeat(64).into();
    assert_eq!(
        VerifiedDmRelayAdvertisement::verify(&changed.to_string(), participant()).unwrap_err(),
        ProviderError::AuthenticationFailed
    );
    changed = serde_json::from_str(&raw).unwrap();
    changed["id"] = "11".repeat(32).into();
    assert_eq!(
        VerifiedDmRelayAdvertisement::verify(&changed.to_string(), participant()).unwrap_err(),
        ProviderError::AuthenticationFailed
    );
    let duplicate = format!("{{\"\\u0069d\":\"untrusted\",{}", &raw[1..]);
    assert_eq!(
        VerifiedDmRelayAdvertisement::verify(&duplicate, participant()).unwrap_err(),
        ProviderError::AuthenticationFailed
    );
    assert_eq!(
        VerifiedDmRelayAdvertisement::verify(&"x".repeat(65537), participant()).unwrap_err(),
        ProviderError::LimitExceeded
    );
}

#[test]
fn authenticated_malformed_or_over_budget_relay_tags_never_supply_targets() {
    for fields in [
        vec!["relay".into()],
        vec!["relay".into(), "https://wrong.example/".into()],
        vec![
            "relay".into(),
            "wss://first.example/".into(),
            "extra".into(),
        ],
    ] {
        assert_eq!(
            VerifiedDmRelayAdvertisement::verify(&event(10050, vec![fields]), participant())
                .unwrap_err(),
            ProviderError::InvalidInput
        );
    }
    let raw = event(
        10050,
        vec![vec![
            "relay".into(),
            format!("wss://host.example/{}", "a".repeat(2048)),
        ]],
    );
    assert_eq!(
        VerifiedDmRelayAdvertisement::verify(&raw, participant()).unwrap_err(),
        ProviderError::LimitExceeded
    );
}
