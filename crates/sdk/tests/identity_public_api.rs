#![cfg(feature = "identity-models")]

#[test]
fn identity_models_are_public_through_identity_module() {
    use radroots_sdk::identity::{
        AccountId, Error, IdentityId, Profile, PublicIdentity, PublicKey, Username,
        username::{MAX_LENGTH, MIN_LENGTH},
    };

    const { assert!(MIN_LENGTH <= MAX_LENGTH) };

    let username = Username::parse(" Field_User ").expect("normalized username");
    assert_eq!(username.as_str(), "field_user");

    let public_key =
        PublicKey::from_hex("585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df")
            .expect("valid public key");
    let identity_id = IdentityId::from(public_key);
    let public_identity = PublicIdentity::new(public_key)
        .with_profile(Profile::new().with_username(username.clone()));
    let account_id = AccountId::from(&public_identity);

    assert_eq!(identity_id, public_identity.id());
    assert_eq!(account_id.to_hex(), public_key.to_hex());
    assert_eq!(
        public_identity.profile().and_then(Profile::username),
        Some(&username)
    );
    assert!(matches!(
        PublicKey::from_hex("not-a-public-key"),
        Err(Error::InvalidHexLength { .. })
    ));
}
