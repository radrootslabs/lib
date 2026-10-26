#![cfg(feature = "commerce")]
use radroots_nostr::{commerce::VerifiedCommerceSigningPlan, key::SecretKey, signing::LocalSigner};
use radroots_protocol::{
    command::CapturedOrderAcceptance,
    commerce::{self, CommerceSeal, CommerceSigningPlan},
    intent::SigningReplay,
    nip59::CapturedDmRoute,
};
use radroots_signing::{
    issuance::{OutcomeKind, SigningIntent},
    provider::{NonceSource, ParticipantProvider, PrivateText, ProviderCall, ProviderError},
    request::CancellationSignal,
};
use serde_json::Value;
use std::{num::NonZeroU64, sync::Arc};
struct TestNonce;
impl NonceSource for TestNonce {
    fn next_nonce(&self) -> Result<[u8; 32], ProviderError> {
        Ok([42; 32])
    }
}
fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[tokio::test]
async fn independent_peer_self_ciphertexts_routes_and_exact_intents_verify_and_actual_signer_matches()
 {
    let value: Value =
        serde_json::from_str(include_str!("fixtures/commerce_signing.v1.json")).unwrap();
    let command =
        CapturedOrderAcceptance::parse(value["command_wire"].as_str().unwrap().as_bytes()).unwrap();
    let record = commerce::decode(value["record_wire"].as_str().unwrap().as_bytes()).unwrap();
    let rumor = commerce::EncodedCommerceRumor::capture(record.content()).unwrap();
    let wire = bytes(value["plan_hex"].as_str().unwrap());
    let plan = || {
        CommerceSigningPlan::parse(
            command.context().operation_id(),
            command.digest(),
            &rumor,
            &wire,
        )
        .unwrap()
    };
    let provider =
        LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 1)).unwrap(), || {
            Ok(451000)
        })
        .unwrap()
        .with_nonce_source(Arc::new(TestNonce));
    let call = || {
        ProviderCall::new(
            provider.public_key(),
            NonZeroU64::new(600000).unwrap(),
            CancellationSignal::new(),
        )
    };
    let verified = VerifiedCommerceSigningPlan::verify(&provider, call(), &rumor, plan())
        .await
        .unwrap();
    assert_eq!(verified.plan().protected_wire(), wire);
    assert_eq!(verified.plan().replay(), SigningReplay::NonReplayable);
    for (seal, row) in verified
        .plan()
        .seals()
        .iter()
        .zip(value["seals"].as_array().unwrap())
    {
        let intent = SigningIntent::from_commerce_seal(seal, call()).unwrap();
        assert_eq!(hex(intent.binding().digest()), row["intent_digest"]);
        let outcome = provider.sign_exact(intent).await;
        let OutcomeKind::Obtained(artifact) = outcome.kind() else {
            panic!("{outcome:?}");
        };
        assert_eq!(artifact.event_id().as_bytes(), &seal.event_id());
    }
    let unavailable =
        LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 1)).unwrap(), || {
            Ok(451000)
        })
        .unwrap();
    assert!(
        VerifiedCommerceSigningPlan::verify(&unavailable, call(), &rumor, plan())
            .await
            .is_err()
    );
    let mut forged = wire.clone();
    let ciphertext = value["seals"][0]["ciphertext"].as_str().unwrap().as_bytes();
    let at = forged
        .windows(ciphertext.len())
        .position(|part| part == ciphertext)
        .unwrap();
    forged[at + 50] = if forged[at + 50] == b'A' { b'B' } else { b'A' };
    // A stale event ID is refused structurally before any crypto qualification.
    assert!(
        CommerceSigningPlan::parse(
            command.context().operation_id(),
            command.digest(),
            &rumor,
            &forged
        )
        .is_err()
    );
    let rebuild = |first: &str, replay, bad_route| {
        let seals = value["seals"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, row)| {
                CommerceSeal::new(
                    command.context().operation_id(),
                    command.digest(),
                    &rumor,
                    row["recipient"].as_str().unwrap().parse().unwrap(),
                    serde_json::from_value(serde_json::json!("450")).unwrap(),
                    if index == 0 {
                        first
                    } else {
                        row["ciphertext"].as_str().unwrap()
                    },
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let routes = value["seals"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, row)| {
                CapturedDmRoute::from_observed_descriptor(
                    row["recipient"].as_str().unwrap().parse().unwrap(),
                    row["route"]["event_id"].as_str().unwrap().parse().unwrap(),
                    if index == 0 && bad_route {
                        "{}"
                    } else {
                        row["route"]["raw"].as_str().unwrap()
                    },
                    &["wss://relay.example".into()],
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        CommerceSigningPlan::new(
            command.context().operation_id(),
            command.digest(),
            &rumor,
            seals,
            routes,
            replay,
        )
        .unwrap()
    };
    let peer_cipher = value["seals"][1]["ciphertext"].as_str().unwrap();
    assert!(
        VerifiedCommerceSigningPlan::verify(
            &provider,
            call(),
            &rumor,
            rebuild(peer_cipher, SigningReplay::NonReplayable, false)
        )
        .await
        .is_err()
    );
    let wrong = provider
        .encrypt_private(
            call(),
            rumor.recipients()[0],
            PrivateText::new(&"x".repeat(rumor.protected_wire().len())).unwrap(),
        )
        .await
        .unwrap();
    assert!(
        VerifiedCommerceSigningPlan::verify(
            &provider,
            call(),
            &rumor,
            rebuild(wrong.as_str(), SigningReplay::NonReplayable, false)
        )
        .await
        .is_err()
    );
    let original = value["seals"][0]["ciphertext"].as_str().unwrap();
    assert!(
        VerifiedCommerceSigningPlan::verify(
            &provider,
            call(),
            &rumor,
            rebuild(original, SigningReplay::NonReplayable, true)
        )
        .await
        .is_err()
    );
    assert!(
        VerifiedCommerceSigningPlan::verify(
            &provider,
            call(),
            &rumor,
            rebuild(original, SigningReplay::LocalReplaySafe, false)
        )
        .await
        .is_ok()
    );
}
