use radroots_net::error::NetError;

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
        NetError::PersistencePathRequired,
        NetError::PersistenceUnsupported,
        NetError::LoggingInit("init"),
    ];
    for variant in variants {
        let cloned = variant.clone();
        assert_eq!(format!("{variant}"), format!("{cloned}"));
    }
}

#[test]
fn clone_preserves_io_variant_without_panicking() {
    let io_err = NetError::Io(std::io::Error::other("io"));
    let cloned = io_err.clone();

    assert!(matches!(cloned, NetError::Io(_)));
    assert_eq!(format!("{io_err}"), format!("{cloned}"));
}
