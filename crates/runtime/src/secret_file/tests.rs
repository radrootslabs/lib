use super::{
    LocalWrappedKeySource, RuntimeProtectedFileError, WRAPPED_KEY_VERSION, local_wrapping_key_path,
    open_local_secret_file, seal_local_secret_file,
};
use radroots_secret_vault::RadrootsSecretKeyWrapping;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn secret_file_round_trips_with_sidecar_key() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("identity.secret.json");

    seal_local_secret_file(
        &path,
        "runtime_test_identity",
        br#"{"secret_key":"secret"}"#,
    )
    .expect("seal local secret file");

    let payload =
        open_local_secret_file(&path, "runtime_test_identity").expect("open local secret file");
    assert_eq!(payload, br#"{"secret_key":"secret"}"#);
    assert!(local_wrapping_key_path(&path).is_file());
}

#[test]
fn secret_file_open_fails_when_wrapping_key_is_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("identity.secret.json");

    seal_local_secret_file(&path, "runtime_test_identity", b"payload")
        .expect("seal local secret file");
    std::fs::remove_file(local_wrapping_key_path(&path)).expect("remove wrapping key");

    let err =
        open_local_secret_file(&path, "runtime_test_identity").expect_err("missing wrapping key");
    assert!(err.to_string().contains("identity.secret.json"));
}

#[test]
fn secret_file_open_fails_when_key_slot_does_not_match() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("identity.secret.json");

    seal_local_secret_file(&path, "runtime_test_identity", b"payload")
        .expect("seal local secret file");

    let err =
        open_local_secret_file(&path, "unexpected_slot").expect_err("slot mismatch should fail");
    assert!(
        err.to_string()
            .contains("expected key slot unexpected_slot")
    );
}

#[test]
fn local_wrapped_key_source_reuses_existing_key_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("identity.secret.json");
    let key_path = local_wrapping_key_path(&path);
    let expected = [7_u8; super::RADROOTS_PROTECTED_STORE_KEY_LENGTH];
    std::fs::write(&key_path, expected).expect("write sidecar key");

    let source = LocalWrappedKeySource::new(&path);
    let loaded = source
        .load_or_create_wrapping_key()
        .expect("existing key should be reused");

    assert_eq!(loaded, expected);
}

#[test]
fn local_wrapped_key_source_rejects_invalid_key_length() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("identity.secret.json");
    let key_path = local_wrapping_key_path(&path);
    std::fs::write(
        &key_path,
        [7_u8; super::RADROOTS_PROTECTED_STORE_KEY_LENGTH - 1],
    )
    .expect("write short sidecar key");

    let source = LocalWrappedKeySource::new(&path);
    let err = source
        .load_wrapping_key()
        .expect_err("short wrapping key must fail");

    assert!(
        err.to_string().contains("invalid length"),
        "unexpected error: {err}"
    );
}

#[test]
fn local_wrapped_key_source_rejects_truncated_invalid_and_tampered_wrapped_keys() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("identity.secret.json");
    let source = LocalWrappedKeySource::new(&path);

    let wrapped = source
        .wrap_data_key("runtime_test_identity", b"payload")
        .expect("wrap succeeds");

    let err = source
        .unwrap_data_key(
            "runtime_test_identity",
            &wrapped[..=super::RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
        )
        .expect_err("truncated wrapped key must fail");
    assert!(err.to_string().contains("truncated"));

    let mut invalid_version = wrapped.clone();
    invalid_version[0] = WRAPPED_KEY_VERSION + 1;
    let err = source
        .unwrap_data_key("runtime_test_identity", &invalid_version)
        .expect_err("invalid wrapped key version must fail");
    assert!(
        err.to_string()
            .contains("unsupported wrapped protected secret data key version")
    );

    let mut tampered = wrapped;
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    let err = source
        .unwrap_data_key("runtime_test_identity", &tampered)
        .expect_err("tampered ciphertext must fail");
    assert!(
        err.to_string()
            .contains("failed to unwrap protected secret data key")
    );
}

#[test]
fn seal_local_secret_file_reports_create_dir_failure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let blocked_parent = temp.path().join("not-a-dir");
    std::fs::write(&blocked_parent, b"blocker").expect("write blocker file");
    let path = blocked_parent.join("identity.secret.json");

    let err = seal_local_secret_file(&path, "runtime_test_identity", b"payload")
        .expect_err("parent file must block directory creation");

    assert!(matches!(err, RuntimeProtectedFileError::CreateDir { .. }));
    if let RuntimeProtectedFileError::CreateDir { path: err_path, .. } = &err {
        assert_eq!(err_path, &blocked_parent);
    }
}

#[test]
fn seal_local_secret_file_reports_seal_failure_for_invalid_existing_wrapping_key() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("identity.secret.json");
    std::fs::write(local_wrapping_key_path(&path), [1_u8; 3]).expect("write invalid sidecar");

    let err = seal_local_secret_file(&path, "runtime_test_identity", b"payload")
        .expect_err("invalid sidecar should fail sealing");

    assert!(matches!(err, RuntimeProtectedFileError::Seal { .. }));
    if let RuntimeProtectedFileError::Seal {
        path: err_path,
        message,
    } = &err
    {
        assert_eq!(err_path, &path);
        assert!(!message.is_empty());
    }
}

#[test]
fn seal_local_secret_file_reports_io_error_when_target_is_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("identity.secret.json");
    std::fs::create_dir(&path).expect("create directory target");

    let err = seal_local_secret_file(&path, "runtime_test_identity", b"payload")
        .expect_err("directory target must fail write");

    assert!(matches!(err, RuntimeProtectedFileError::Io { .. }));
    if let RuntimeProtectedFileError::Io { path: err_path, .. } = &err {
        assert_eq!(err_path, &path);
    }
}

#[test]
fn open_local_secret_file_reports_io_error_for_missing_payload_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("missing.secret.json");

    let err =
        open_local_secret_file(&path, "runtime_test_identity").expect_err("missing file must fail");

    assert!(matches!(err, RuntimeProtectedFileError::Io { .. }));
    if let RuntimeProtectedFileError::Io { path: err_path, .. } = &err {
        assert_eq!(err_path, &path);
    }
}

#[test]
fn open_local_secret_file_reports_decode_error_for_invalid_payload() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("identity.secret.json");
    std::fs::write(&path, b"not-json").expect("write invalid payload");

    let err = open_local_secret_file(&path, "runtime_test_identity")
        .expect_err("invalid json payload must fail");

    assert!(matches!(err, RuntimeProtectedFileError::Decode { .. }));
    if let RuntimeProtectedFileError::Decode { path: err_path, .. } = &err {
        assert_eq!(err_path, &path);
    }
}

#[test]
fn local_wrapped_key_source_creates_key_for_parentless_paths() {
    let _guard = cwd_lock().lock().expect("cwd lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let original = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp.path()).expect("switch cwd");

    let path = PathBuf::from("identity.secret.json");
    let source = LocalWrappedKeySource::new(&path);
    let loaded = source
        .load_or_create_wrapping_key()
        .expect("parentless path should create key");

    assert_eq!(loaded.len(), super::RADROOTS_PROTECTED_STORE_KEY_LENGTH);
    assert!(local_wrapping_key_path(&path).is_file());

    std::env::set_current_dir(original).expect("restore cwd");
}

#[test]
fn seal_local_secret_file_allows_parentless_paths() {
    let _guard = cwd_lock().lock().expect("cwd lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let original = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp.path()).expect("switch cwd");

    let path = PathBuf::from("identity.secret.json");
    seal_local_secret_file(&path, "runtime_test_identity", b"payload")
        .expect("parentless path should seal");
    let payload =
        open_local_secret_file(&path, "runtime_test_identity").expect("parentless path opens");
    assert_eq!(payload, b"payload");

    std::env::set_current_dir(original).expect("restore cwd");
}
