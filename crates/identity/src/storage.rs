//! Transitional filesystem helpers for public profile snapshots.
//!
//! Filesystem ownership is removed from this package in the next ordered
//! migration checkpoint.

use std::{fs, path::Path};

use crate::{IdentityError, PublicIdentity};

/// Stores a validated public identity profile as JSON.
pub fn store_identity_profile(
    path: impl AsRef<Path>,
    identity: &PublicIdentity,
) -> Result<(), IdentityError> {
    store_identity_profile_path(path.as_ref(), identity)
}

fn store_identity_profile_path(
    path: &Path,
    identity: &PublicIdentity,
) -> Result<(), IdentityError> {
    if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|source| IdentityError::CreateDir(parent.to_path_buf(), source))?;
    }
    let encoded = serde_json::to_vec_pretty(identity)?;
    fs::write(path, encoded).map_err(|source| IdentityError::Write(path.to_path_buf(), source))
}

/// Loads and revalidates a public identity profile from JSON.
pub fn load_identity_profile(path: impl AsRef<Path>) -> Result<PublicIdentity, IdentityError> {
    load_identity_profile_path(path.as_ref())
}

fn load_identity_profile_path(path: &Path) -> Result<PublicIdentity, IdentityError> {
    let encoded = fs::read(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            IdentityError::NotFound(path.to_path_buf())
        } else {
            IdentityError::Read(path.to_path_buf(), source)
        }
    })?;
    serde_json::from_slice(&encoded).map_err(IdentityError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Profile, PublicKey, Username};

    const ALICE: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    fn fixture_identity() -> PublicIdentity {
        PublicIdentity::new(PublicKey::from_hex(ALICE).unwrap())
            .with_profile(Profile::new().with_username(Username::parse("alice.farm").unwrap()))
    }

    #[test]
    fn public_profile_file_round_trip_revalidates_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("profiles/alice.json");
        let identity = fixture_identity();

        store_identity_profile(&path, &identity).unwrap();
        assert_eq!(load_identity_profile(&path).unwrap(), identity);
    }

    #[test]
    fn public_profile_file_rejects_mismatched_or_missing_data() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("alice.json");
        let mut value = serde_json::to_value(fixture_identity()).unwrap();
        value["id"] = serde_json::Value::String(
            "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af".into(),
        );
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        assert!(matches!(
            load_identity_profile(&path),
            Err(IdentityError::InvalidJson(_))
        ));
        assert!(matches!(
            load_identity_profile(directory.path().join("missing.json")),
            Err(IdentityError::NotFound(_))
        ));
    }
}
