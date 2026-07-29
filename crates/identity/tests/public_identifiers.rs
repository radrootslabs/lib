use core::str::FromStr;

use radroots_identity::{AccountId, Error, IdentityId, PublicKey};

const ALICE_PUBLIC_KEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const BOB_PUBLIC_KEY: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

#[test]
fn public_key_canonicalizes_hex_and_round_trips_bytes() {
    let uppercase = ALICE_PUBLIC_KEY.to_ascii_uppercase();
    let public_key = PublicKey::from_hex(&uppercase).expect("valid fixture public key");

    assert_eq!(public_key.to_string(), ALICE_PUBLIC_KEY);
    assert_eq!(public_key.to_hex(), ALICE_PUBLIC_KEY);
    assert_eq!(PublicKey::from_str(ALICE_PUBLIC_KEY).unwrap(), public_key);
    assert_eq!(
        PublicKey::try_from(public_key.into_bytes()).unwrap(),
        public_key
    );
    assert_eq!(
        PublicKey::try_from(public_key.as_ref()).unwrap(),
        public_key
    );
}

#[test]
fn public_key_rejects_noncanonical_lengths_characters_and_points() {
    assert!(matches!(
        PublicKey::from_hex("00"),
        Err(Error::InvalidHexLength {
            expected: PublicKey::HEX_LENGTH,
            actual: 2,
        })
    ));

    let mut invalid_hex = ALICE_PUBLIC_KEY.as_bytes().to_vec();
    invalid_hex[17] = b'g';
    let invalid_hex = core::str::from_utf8(&invalid_hex).unwrap();
    assert!(matches!(
        PublicKey::from_hex(invalid_hex),
        Err(Error::InvalidHexCharacter { index: 17 })
    ));

    assert!(matches!(
        PublicKey::from_slice(&[1; PublicKey::BYTE_LENGTH - 1]),
        Err(Error::InvalidByteLength {
            expected: PublicKey::BYTE_LENGTH,
            actual,
        }) if actual == PublicKey::BYTE_LENGTH - 1
    ));
    assert!(matches!(
        PublicKey::from_bytes([0; PublicKey::BYTE_LENGTH]),
        Err(Error::InvalidPublicKeyBytes)
    ));
}

#[test]
fn identity_and_account_identifiers_preserve_semantics_and_ordering() {
    let alice_key = PublicKey::from_hex(ALICE_PUBLIC_KEY).unwrap();
    let bob_key = PublicKey::from_hex(BOB_PUBLIC_KEY).unwrap();
    let alice_identity = IdentityId::from_public_key(alice_key);
    let alice_account = AccountId::from_identity_id(alice_identity);

    assert_eq!(alice_identity.to_hex(), ALICE_PUBLIC_KEY);
    assert_eq!(alice_account.to_hex(), ALICE_PUBLIC_KEY);
    assert_eq!(
        IdentityId::from_hex(ALICE_PUBLIC_KEY).unwrap(),
        alice_identity
    );
    assert_eq!(
        AccountId::from_hex(ALICE_PUBLIC_KEY).unwrap(),
        alice_account
    );
    assert!(alice_identity < IdentityId::from_public_key(bob_key));
}

#[cfg(feature = "serde")]
#[test]
fn public_identifiers_use_checked_canonical_serde_strings() {
    let public_key = PublicKey::from_hex(ALICE_PUBLIC_KEY).unwrap();
    let identity_id = IdentityId::from(public_key);
    let account_id = AccountId::from(identity_id);

    for encoded in [
        serde_json::to_string(&public_key).unwrap(),
        serde_json::to_string(&identity_id).unwrap(),
        serde_json::to_string(&account_id).unwrap(),
    ] {
        assert_eq!(encoded, format!("\"{ALICE_PUBLIC_KEY}\""));
    }

    assert_eq!(
        serde_json::from_str::<PublicKey>(&format!("\"{}\"", ALICE_PUBLIC_KEY.to_uppercase()))
            .unwrap(),
        public_key
    );
    assert!(serde_json::from_str::<IdentityId>("\"invalid\"").is_err());
    assert!(serde_json::from_str::<AccountId>("null").is_err());
}
