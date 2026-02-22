use radroots_net_core::error::NetError;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn msg_constructor_creates_msg_variant() {
    let err = NetError::msg("hello");
    assert!(matches!(err, NetError::Msg(ref message) if message == "hello"));
}

#[test]
fn clone_covers_non_io_variants() {
    let variants = [
        NetError::msg("hello"),
        NetError::Poisoned,
        NetError::MissingKey,
        NetError::InvalidHex32,
        NetError::InvalidBech32,
        NetError::InvalidKeyFile,
        NetError::KeyIo,
        NetError::OverwriteDenied,
        NetError::PersistenceUnsupported,
        NetError::LoggingInit("init"),
    ];
    for variant in variants {
        let cloned = variant.clone();
        assert_eq!(format!("{variant}"), format!("{cloned}"));
    }
}

#[test]
fn clone_panics_for_io_variant() {
    let io_err = NetError::Io(std::io::Error::other("io"));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = io_err.clone();
    }));
    assert!(result.is_err());
}
