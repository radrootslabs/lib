use radroots_identity::PublicKey;
use radroots_signing::{provider::*, recovery::ReplayCapability, request::CancellationSignal};
use std::{error::Error as _, num::NonZeroU64, str::FromStr};

fn identity() -> PublicKey {
    PublicKey::from_str("585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df").unwrap()
}
fn other() -> PublicKey {
    PublicKey::from_str("e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af").unwrap()
}

#[test]
fn private_values_admit_exact_byte_bounds_without_diagnostic_or_wire_exposure() {
    let private = "private buyer question — do not log";
    let text = PrivateText::new(private).unwrap();
    assert_eq!(text.expose(), private);
    assert_eq!(format!("{text:?}"), "PrivateText([redacted])");
    assert!(matches!(
        PrivateText::new(""),
        Err(ProviderError::InvalidInput)
    ));
    assert!(matches!(
        PrivateText::from_owned(String::new()),
        Err(ProviderError::InvalidInput)
    ));
    for text in [
        "a".repeat(PRIVATE_CRYPTO_MAX_BYTES),
        "é".repeat(PRIVATE_CRYPTO_MAX_BYTES / 2),
    ] {
        assert_eq!(
            PrivateText::new(&text).unwrap().expose().len(),
            PRIVATE_CRYPTO_MAX_BYTES
        );
        assert_eq!(
            PrivateText::from_owned(text).unwrap().expose().len(),
            PRIVATE_CRYPTO_MAX_BYTES
        );
    }
    let too_long = "a".repeat(PRIVATE_CRYPTO_MAX_BYTES + 1);
    assert!(matches!(
        PrivateText::new(&too_long),
        Err(ProviderError::LimitExceeded)
    ));
    assert!(matches!(
        PrivateText::from_owned(too_long),
        Err(ProviderError::LimitExceeded)
    ));
    for bad in ["", "abc", "!", "a".repeat(131).as_str()] {
        assert!(matches!(
            PrivateCiphertext::parse(bad),
            Err(ProviderError::InvalidInput)
        ));
    }
    assert!(matches!(
        PrivateCiphertext::parse("#future"),
        Err(ProviderError::UnsupportedVersion)
    ));
    assert!(matches!(
        PrivateCiphertext::parse(&"a".repeat(PRIVATE_CRYPTO_MAX_BYTES + 1)),
        Err(ProviderError::LimitExceeded)
    ));
    assert!(matches!(
        PrivateCiphertext::parse(&(String::from("!") + &"a".repeat(132))),
        Err(ProviderError::InvalidInput)
    ));
    let encoded = "a".repeat(PRIVATE_CRYPTO_MAX_BYTES);
    let ciphertext = PrivateCiphertext::parse(&encoded).unwrap();
    assert_eq!(ciphertext.as_str(), encoded);
    assert_eq!(format!("{ciphertext:?}"), "PrivateCiphertext([redacted])");
}

#[test]
fn call_binding_is_not_interchangeable_with_another_identity_or_expired_budget() {
    let signal = CancellationSignal::new();
    let call = ProviderCall::new(identity(), NonZeroU64::new(200).unwrap(), signal.clone());
    assert_eq!(call.identity(), identity());
    assert_eq!(call.check_before_effect(identity(), 100), Ok(()));
    assert_eq!(
        call.check_before_effect(other(), 100),
        Err(ProviderError::IdentityChanged)
    );
    assert_eq!(
        call.check_before_effect(identity(), 0),
        Err(ProviderError::ProviderFailure)
    );
    assert_eq!(
        call.check_before_effect(identity(), 200),
        Err(ProviderError::DeadlineExceeded)
    );
    assert_eq!(
        call.check_before_effect(identity(), 201),
        Err(ProviderError::DeadlineExceeded)
    );
    signal.cancel();
    assert_eq!(
        call.check_before_effect(identity(), 100),
        Err(ProviderError::Cancelled)
    );
    assert_eq!(
        call.clone().check_before_effect(identity(), 100),
        Err(ProviderError::Cancelled)
    );
}

#[test]
fn capabilities_are_separate_method_observations_and_errors_retain_no_cause() {
    for encryption in [
        PrivateCryptoCapability::Unavailable,
        PrivateCryptoCapability::Nip44V2,
    ] {
        for decryption in [
            PrivateCryptoCapability::Unavailable,
            PrivateCryptoCapability::Nip44V2,
        ] {
            for replay in [
                ReplayCapability::ExactReplayByRequestId,
                ReplayCapability::LocalReplaySafe,
                ReplayCapability::NonReplayable,
            ] {
                let state = ProviderState::new(identity(), encryption, decryption, replay);
                assert_eq!(state.identity(), identity());
                assert_eq!(state.encryption(), encryption);
                assert_eq!(state.decryption(), decryption);
                assert_eq!(state.signing_replay(), replay);
            }
        }
    }
    for error in [
        ProviderError::InvalidInput,
        ProviderError::LimitExceeded,
        ProviderError::UnsupportedVersion,
        ProviderError::CapabilityUnavailable,
        ProviderError::IdentityChanged,
        ProviderError::AuthenticationFailed,
        ProviderError::Cancelled,
        ProviderError::DeadlineExceeded,
        ProviderError::ProviderFailure,
    ] {
        assert_eq!(error.to_string(), error.as_str());
        assert!(error.source().is_none());
        assert!(format!("{error:?}").len() < 32);
    }
}
