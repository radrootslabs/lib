use radroots_identity::{AccountId, Profile, PublicIdentity, PublicKey, Username};

fn main() -> Result<(), radroots_identity::Error> {
    let public_key =
        PublicKey::from_hex("585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df")?;
    let username = Username::parse(" Alice.Farm ")?;
    let identity =
        PublicIdentity::new(public_key).with_profile(Profile::new().with_username(username));
    let account_id = AccountId::from_public_identity(&identity);

    assert_eq!(identity.id().as_bytes(), public_key.as_bytes());
    assert_eq!(account_id.as_bytes(), public_key.as_bytes());
    assert_eq!(
        identity
            .profile()
            .and_then(Profile::username)
            .map(Username::as_str),
        Some("alice.farm"),
    );
    Ok(())
}
