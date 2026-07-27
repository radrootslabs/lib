//! Public, transport-neutral identity profiles.

use crate::{Error, IdentityId, PublicKey, Username};

/// Public identity metadata that is independent of any transport event.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Profile {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    username: Option<Username>,
}

impl Profile {
    /// Creates an empty public profile.
    #[must_use]
    pub const fn new() -> Self {
        Self { username: None }
    }

    /// Returns a profile with its canonical username set.
    #[must_use]
    pub fn with_username(mut self, username: Username) -> Self {
        self.username = Some(username);
        self
    }

    /// Borrows the canonical username, when present.
    #[must_use]
    pub fn username(&self) -> Option<&Username> {
        self.username.as_ref()
    }

    /// Reports whether the profile contains no public metadata.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.username.is_none()
    }
}

/// A public identity with an invariant-matched identifier and public key.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PublicIdentity {
    id: IdentityId,
    public_key: PublicKey,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    profile: Option<Profile>,
}

impl PublicIdentity {
    /// Creates a public identity whose identifier is derived from its key.
    #[must_use]
    pub const fn new(public_key: PublicKey) -> Self {
        Self {
            id: IdentityId::from_public_key(public_key),
            public_key,
            profile: None,
        }
    }

    /// Validates an identity assembled from separately decoded parts.
    pub fn try_from_parts(
        id: IdentityId,
        public_key: PublicKey,
        profile: Option<Profile>,
    ) -> Result<Self, Error> {
        if id != IdentityId::from_public_key(public_key) {
            return Err(Error::IdentityIdMismatch);
        }
        Ok(Self {
            id,
            public_key,
            profile: profile.filter(|value| !value.is_empty()),
        })
    }

    /// Returns a public identity with non-empty profile metadata attached.
    #[must_use]
    pub fn with_profile(mut self, profile: Profile) -> Self {
        self.profile = (!profile.is_empty()).then_some(profile);
        self
    }

    /// Returns the canonical identity identifier.
    #[must_use]
    pub const fn id(&self) -> IdentityId {
        self.id
    }

    /// Returns the canonical public key.
    #[must_use]
    pub const fn public_key(&self) -> PublicKey {
        self.public_key
    }

    /// Borrows public profile metadata, when present.
    #[must_use]
    pub fn profile(&self) -> Option<&Profile> {
        self.profile.as_ref()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PublicIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PublicIdentityRepr {
            id: IdentityId,
            public_key: PublicKey,
            #[serde(default)]
            profile: Option<Profile>,
        }

        let value = PublicIdentityRepr::deserialize(deserializer)?;
        Self::try_from_parts(value.id, value.public_key, value.profile)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const BOB: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    #[test]
    fn public_identity_derives_and_protects_its_identifier() {
        let public_key = PublicKey::from_hex(ALICE).unwrap();
        let identity = PublicIdentity::new(public_key);

        assert_eq!(identity.id(), IdentityId::from(public_key));
        assert_eq!(identity.public_key(), public_key);
        assert!(identity.profile().is_none());

        let mismatched_id = IdentityId::from_hex(BOB).unwrap();
        assert!(matches!(
            PublicIdentity::try_from_parts(mismatched_id, public_key, None),
            Err(Error::IdentityIdMismatch)
        ));
    }

    #[test]
    fn public_identity_discards_empty_profiles_and_retains_usernames() {
        let public_key = PublicKey::from_hex(ALICE).unwrap();
        let empty = PublicIdentity::new(public_key).with_profile(Profile::new());
        assert!(empty.profile().is_none());

        let username = Username::parse("Alice.Farm").unwrap();
        let identity = PublicIdentity::new(public_key)
            .with_profile(Profile::new().with_username(username.clone()));
        assert_eq!(
            identity.profile().and_then(Profile::username),
            Some(&username)
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn public_identity_serde_revalidates_key_identity_and_profile() {
        let public_key = PublicKey::from_hex(ALICE).unwrap();
        let identity = PublicIdentity::new(public_key)
            .with_profile(Profile::new().with_username(Username::parse("Alice.Farm").unwrap()));
        let encoded = serde_json::to_string(&identity).unwrap();
        assert_eq!(
            serde_json::from_str::<PublicIdentity>(&encoded).unwrap(),
            identity
        );

        let mismatched = encoded.replace(ALICE, BOB).replacen(BOB, ALICE, 1);
        assert!(serde_json::from_str::<PublicIdentity>(&mismatched).is_err());
        assert!(!encoded.contains("metadata"));
        assert!(!encoded.contains("application_handler"));
    }
}
