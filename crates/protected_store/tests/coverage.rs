use radroots_protected_store::{
    RADROOTS_PROTECTED_STORE_KEY_LENGTH, RADROOTS_PROTECTED_STORE_NONCE_LENGTH,
    RadrootsProtectedStoreEnvelope,
};
use radroots_secret_vault::RadrootsSecretKeyWrapping;

#[derive(Default)]
struct TestVault;

impl RadrootsSecretKeyWrapping for TestVault {
    type Error = ();

    fn wrap_data_key(&self, key_slot: &str, plaintext_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let mut wrapped = key_slot.as_bytes().to_vec();
        wrapped.push(0);
        wrapped.extend(plaintext_key.iter().map(|byte| byte ^ 0x5a));
        Ok(wrapped)
    }

    fn unwrap_data_key(&self, key_slot: &str, wrapped_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let separator = wrapped_key.iter().position(|byte| *byte == 0).ok_or(())?;
        if &wrapped_key[..separator] != key_slot.as_bytes() {
            return Err(());
        }

        Ok(wrapped_key[separator + 1..]
            .iter()
            .map(|byte| byte ^ 0x5a)
            .collect())
    }
}

#[test]
fn public_roundtrip_apis_cover_external_lib_regions() {
    let vault = TestVault;
    let generated = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
        &vault,
        "drafts/default",
        b"generated roundtrip",
    )
    .expect("seal with runtime entropy succeeds");
    let generated_plaintext = generated
        .open_with_wrapped_key(&vault)
        .expect("generated envelope opens");
    assert_eq!(generated_plaintext, b"generated roundtrip");

    let deterministic = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key_and_material(
        &vault,
        "drafts/default",
        b"deterministic roundtrip",
        [7_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH],
        [9_u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
    )
    .expect("deterministic seal succeeds");
    let encoded = deterministic.encode_json().expect("encode succeeds");
    let decoded = RadrootsProtectedStoreEnvelope::decode_json(&encoded).expect("decode succeeds");
    let deterministic_plaintext = decoded
        .open_with_wrapped_key(&vault)
        .expect("deterministic envelope opens");
    assert_eq!(deterministic_plaintext, b"deterministic roundtrip");

    let malformed = RadrootsProtectedStoreEnvelope {
        header: decoded.header.clone(),
        wrapped_key: vec![1, 2, 3, 4],
        ciphertext: decoded.ciphertext.clone(),
    };
    let err = malformed
        .open_with_wrapped_key(&vault)
        .expect_err("wrapped key without separator must fail");
    assert_eq!(
        format!("{err:?}"),
        "KeyUnwrapFailed",
        "public wrapper should surface the vault unwrap failure",
    );

    let mismatched = RadrootsProtectedStoreEnvelope {
        header: decoded.header.clone(),
        wrapped_key: TestVault
            .wrap_data_key("drafts/other", &[7_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH])
            .expect("alternate slot wrap succeeds"),
        ciphertext: decoded.ciphertext,
    };
    let err = mismatched
        .open_with_wrapped_key(&vault)
        .expect_err("wrapped key slot mismatch must fail");
    assert_eq!(
        format!("{err:?}"),
        "KeyUnwrapFailed",
        "public wrapper should surface the slot mismatch unwrap failure",
    );
}
