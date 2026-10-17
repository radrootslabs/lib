use super::*;
#[cfg(feature = "nip17")]
use crate::test_fixtures::FIXTURE_CAROL;
use crate::{
    key::parse_secret_key,
    test_fixtures::{FIXTURE_ALICE, FIXTURE_BOB},
};
use core::num::NonZeroU64;
#[cfg(feature = "nip17")]
use radroots_signing::provider::PRIVATE_CRYPTO_MAX_BYTES;
use radroots_signing::{host::Shared, recovery::ReplayCapability, request::CancellationSignal};
use std::sync::atomic::{AtomicUsize, Ordering};

fn clock() -> Result<u64, radroots_signing::Error> {
    Ok(100)
}
#[cfg(feature = "nip17")]
fn unavailable_clock() -> Result<u64, radroots_signing::Error> {
    Err(radroots_signing::Error::new(
        radroots_signing::error::Kind::InternalError,
    ))
}
fn signer(secret: &str) -> LocalSigner {
    LocalSigner::with_clock(parse_secret_key(secret).unwrap(), clock).unwrap()
}
fn call(signer: &LocalSigner) -> ProviderCall {
    ProviderCall::new(
        signer.public_key(),
        NonZeroU64::new(200).unwrap(),
        CancellationSignal::new(),
    )
}

struct FixedNonce {
    calls: AtomicUsize,
    fail: bool,
    nonce: [u8; 32],
}
impl NonceSource for FixedNonce {
    fn next_nonce(&self) -> Result<[u8; 32], ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(ProviderError::CapabilityUnavailable)
        } else {
            Ok(self.nonce)
        }
    }
}
fn nonce_source(fail: bool) -> Shared<FixedNonce> {
    let mut nonce = [0; 32];
    nonce[31] = 1;
    Shared::new(FixedNonce {
        calls: AtomicUsize::new(0),
        fail,
        nonce,
    })
}

#[tokio::test]
async fn identity_and_caps_report_the_actual_configuration() {
    let local = signer(FIXTURE_ALICE.secret_key_hex);
    let state = local.provider_state().await.unwrap();
    assert_eq!(state.identity(), local.public_key());
    assert_eq!(state.encryption(), PrivateCryptoCapability::Unavailable);
    assert_eq!(
        state.decryption(),
        if cfg!(feature = "nip17") {
            PrivateCryptoCapability::Nip44V2
        } else {
            PrivateCryptoCapability::Unavailable
        }
    );
    assert_eq!(state.signing_replay(), ReplayCapability::LocalReplaySafe);
    let peer = signer(FIXTURE_BOB.secret_key_hex).public_key();
    assert_eq!(
        local
            .encrypt_private(
                call(&local),
                peer,
                PrivateText::new("private question").unwrap()
            )
            .await
            .unwrap_err(),
        ProviderError::CapabilityUnavailable
    );
    let local = local.with_nonce_source(nonce_source(false));
    assert_eq!(
        local.provider_state().await.unwrap().encryption(),
        if cfg!(feature = "nip17") {
            PrivateCryptoCapability::Nip44V2
        } else {
            PrivateCryptoCapability::Unavailable
        }
    );
    if !cfg!(feature = "nip17") {
        let encoded = PrivateCiphertext::parse(&"A".repeat(132)).unwrap();
        assert_eq!(
            local
                .decrypt_private(call(&local), peer, encoded)
                .await
                .unwrap_err(),
            ProviderError::CapabilityUnavailable
        );
    }
}

#[cfg(feature = "nip17")]
#[tokio::test]
async fn exact_primary_vector_and_both_participant_directions() {
    let vector: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/nip44_provider.v1.json"
    ))
    .unwrap();
    let encoded_nonce = vector["nonce"].as_str().unwrap();
    assert_eq!(encoded_nonce.len(), 64);
    let mut nonce_bytes = [0; 32];
    for (index, byte) in nonce_bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&encoded_nonce[index * 2..index * 2 + 2], 16).unwrap();
    }
    let nonce = Shared::new(FixedNonce {
        calls: AtomicUsize::new(0),
        fail: false,
        nonce: nonce_bytes,
    });
    let alice = signer(vector["secret_a"].as_str().unwrap()).with_nonce_source(nonce.clone());
    let bob = signer(vector["secret_b"].as_str().unwrap()).with_nonce_source(nonce_source(false));
    let encrypted = alice
        .encrypt_private(
            call(&alice),
            bob.public_key(),
            PrivateText::new(vector["plaintext"].as_str().unwrap()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(encrypted.as_str(), vector["ciphertext"].as_str().unwrap());
    assert_eq!(nonce.calls.load(Ordering::SeqCst), 1);
    let opened = bob
        .decrypt_private(call(&bob), alice.public_key(), encrypted)
        .await
        .unwrap();
    assert_eq!(opened.expose(), vector["plaintext"].as_str().unwrap());
    let reply = bob
        .encrypt_private(
            call(&bob),
            alice.public_key(),
            PrivateText::new("fresh tomatoes — pickup tomorrow").unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        alice
            .decrypt_private(call(&alice), bob.public_key(), reply)
            .await
            .unwrap()
            .expose(),
        "fresh tomatoes — pickup tomorrow"
    );
}

#[cfg(feature = "nip17")]
#[tokio::test]
async fn tampering_foreign_key_and_unsupported_version_remain_distinct() {
    use base64::Engine;
    let alice = signer(FIXTURE_ALICE.secret_key_hex).with_nonce_source(nonce_source(false));
    let bob = signer(FIXTURE_BOB.secret_key_hex);
    let carol = signer(FIXTURE_CAROL.secret_key_hex);
    let encrypted = alice
        .encrypt_private(
            call(&alice),
            bob.public_key(),
            PrivateText::new("private food inquiry").unwrap(),
        )
        .await
        .unwrap();
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encrypted.as_str())
        .unwrap();
    assert_eq!(
        bob.decrypt_private(
            call(&bob),
            carol.public_key(),
            PrivateCiphertext::parse(encrypted.as_str()).unwrap()
        )
        .await
        .unwrap_err(),
        ProviderError::AuthenticationFailed
    );
    let mut damaged = raw.clone();
    *damaged.last_mut().unwrap() ^= 1;
    let encoded = base64::engine::general_purpose::STANDARD.encode(damaged);
    assert_eq!(
        bob.decrypt_private(
            call(&bob),
            alice.public_key(),
            PrivateCiphertext::parse(&encoded).unwrap()
        )
        .await
        .unwrap_err(),
        ProviderError::AuthenticationFailed
    );
    let mut future = raw;
    future[0] = 3;
    let encoded = base64::engine::general_purpose::STANDARD.encode(future);
    assert_eq!(
        bob.decrypt_private(
            call(&bob),
            alice.public_key(),
            PrivateCiphertext::parse(&encoded).unwrap()
        )
        .await
        .unwrap_err(),
        ProviderError::UnsupportedVersion
    );
}

#[cfg(feature = "nip17")]
#[tokio::test]
async fn identity_cancel_deadline_and_capacity_refuse_before_nonce_use() {
    let entropy = nonce_source(false);
    let alice = signer(FIXTURE_ALICE.secret_key_hex).with_nonce_source(entropy.clone());
    let bob = signer(FIXTURE_BOB.secret_key_hex);
    let mismatched = ProviderCall::new(
        bob.public_key(),
        NonZeroU64::new(200).unwrap(),
        CancellationSignal::new(),
    );
    assert_eq!(
        alice
            .encrypt_private(
                mismatched,
                bob.public_key(),
                PrivateText::new("private").unwrap()
            )
            .await
            .unwrap_err(),
        ProviderError::IdentityChanged
    );
    let cancelled = CancellationSignal::new();
    cancelled.cancel();
    let context = ProviderCall::new(alice.public_key(), NonZeroU64::new(200).unwrap(), cancelled);
    assert_eq!(
        alice
            .encrypt_private(
                context,
                bob.public_key(),
                PrivateText::new("private").unwrap()
            )
            .await
            .unwrap_err(),
        ProviderError::Cancelled
    );
    let expired = ProviderCall::new(
        alice.public_key(),
        NonZeroU64::new(100).unwrap(),
        CancellationSignal::new(),
    );
    assert_eq!(
        alice
            .encrypt_private(
                expired,
                bob.public_key(),
                PrivateText::new("private").unwrap()
            )
            .await
            .unwrap_err(),
        ProviderError::DeadlineExceeded
    );
    let oversized = "x".repeat(PRIVATE_CRYPTO_MAX_BYTES);
    assert_eq!(
        alice
            .encrypt_private(
                call(&alice),
                bob.public_key(),
                PrivateText::new(&oversized).unwrap()
            )
            .await
            .unwrap_err(),
        ProviderError::LimitExceeded
    );
    assert_eq!(entropy.calls.load(Ordering::SeqCst), 0);
    let failed = LocalSigner::with_clock(
        parse_secret_key(FIXTURE_ALICE.secret_key_hex).unwrap(),
        unavailable_clock,
    )
    .unwrap()
    .with_nonce_source(entropy.clone());
    assert_eq!(
        failed
            .encrypt_private(
                call(&failed),
                bob.public_key(),
                PrivateText::new("private").unwrap()
            )
            .await
            .unwrap_err(),
        ProviderError::ProviderFailure
    );
    assert_eq!(entropy.calls.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "nip17")]
#[tokio::test]
async fn bounded_full_rumor_and_entropy_failure_execute_the_real_codec() {
    let entropy = nonce_source(false);
    let alice = signer(FIXTURE_ALICE.secret_key_hex).with_nonce_source(entropy.clone());
    let bob = signer(FIXTURE_BOB.secret_key_hex);
    for length in [1, 32, 33, 256, 257, 4096, 16384, 24576] {
        let text = "a".repeat(length);
        let encrypted = alice
            .encrypt_private(
                call(&alice),
                bob.public_key(),
                PrivateText::new(&text).unwrap(),
            )
            .await
            .unwrap();
        assert!(encrypted.as_str().len() <= PRIVATE_CRYPTO_MAX_BYTES);
        assert_eq!(
            bob.decrypt_private(call(&bob), alice.public_key(), encrypted)
                .await
                .unwrap()
                .expose(),
            text
        );
    }
    let failed_source = nonce_source(true);
    let failed = signer(FIXTURE_ALICE.secret_key_hex).with_nonce_source(failed_source.clone());
    assert_eq!(
        failed
            .encrypt_private(
                call(&failed),
                bob.public_key(),
                PrivateText::new("private").unwrap()
            )
            .await
            .unwrap_err(),
        ProviderError::CapabilityUnavailable
    );
    assert_eq!(failed_source.calls.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "nip17")]
#[tokio::test]
async fn malformed_payloads_and_encoded_size_refuse_without_entropy() {
    let entropy = nonce_source(false);
    let alice = signer(FIXTURE_ALICE.secret_key_hex).with_nonce_source(entropy.clone());
    let bob = signer(FIXTURE_BOB.secret_key_hex);
    // This fits the codec's plaintext bound, but its encoded output would
    // exceed the independent public-event boundary.
    assert_eq!(
        alice
            .encrypt_private(
                call(&alice),
                bob.public_key(),
                PrivateText::new(&"x".repeat(60_000)).unwrap(),
            )
            .await
            .unwrap_err(),
        ProviderError::LimitExceeded
    );
    for encoded in ["=".repeat(132), "A".repeat(130) + "=="] {
        assert_eq!(
            bob.decrypt_private(
                call(&bob),
                alice.public_key(),
                PrivateCiphertext::parse(&encoded).unwrap(),
            )
            .await
            .unwrap_err(),
            ProviderError::InvalidInput
        );
    }
    assert_eq!(entropy.calls.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "nip17")]
#[test]
fn codec_nonce_consumption_is_exact_and_has_no_rng_quality_claim() {
    use nostr::secp256k1::rand::RngCore;
    let mut source = NonceTranscript {
        nonce: [7; 32],
        consumed: false,
        invalid: false,
    };
    let mut bytes = [0; 32];
    source.try_fill_bytes(&mut bytes).unwrap();
    assert_eq!(bytes, [7; 32]);
    assert!(source.try_fill_bytes(&mut bytes).is_err());
    for length in [0, 1, 31, 33, 64] {
        let mut source = NonceTranscript {
            nonce: [7; 32],
            consumed: false,
            invalid: false,
        };
        source.fill_bytes(&mut vec![0; length]);
        assert!(source.invalid);
    }
    let mut source = NonceTranscript {
        nonce: [7; 32],
        consumed: false,
        invalid: false,
    };
    assert_eq!(source.next_u32(), 0);
    assert_eq!(source.next_u64(), 0);
    assert!(source.invalid);
}
