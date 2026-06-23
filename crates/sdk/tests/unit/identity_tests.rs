use super::{RadrootsEncryptedIdentityFile, RadrootsIdentity};

#[test]
fn encrypted_identity_file_round_trips() {
    let temp = tempfile::tempdir().expect("tempdir");
    let file = RadrootsEncryptedIdentityFile::new(temp.path().join("identity.enc.json"));
    let identity = RadrootsIdentity::generate();

    file.store(&identity).expect("store identity");
    let loaded = file.load().expect("load identity");

    assert_eq!(loaded.public_key_hex(), identity.public_key_hex());
}
