#[allow(unused_imports)]
use radroots_signing::{
    Actor, Error, SignReceipt, SignRequest, Signer, SignerStatus, actor as _, capability as _,
    error as _, receipt as _, request as _, signer as _, status as _,
};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn manifest_has_final_identity_features_and_dependencies() {
    for required in [
        "name = \"radroots_signing\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "default = [\"std\", \"serde\"]",
        "radroots_event = { workspace = true, default-features = false }",
        "radroots_identity = { workspace = true, default-features = false }",
        "radroots_protocol = { workspace = true, default-features = false }",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing {required}"
        );
    }
}

#[test]
fn crate_root_declares_the_approved_module_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    for module in [
        "actor",
        "capability",
        "error",
        "request",
        "receipt",
        "signer",
        "status",
    ] {
        let declaration = format!("pub mod {module};");
        assert!(
            ROOT.contains(&declaration),
            "crate root is missing {module}"
        );
    }
    let _ = core::mem::size_of::<Actor>();
    let _ = core::mem::size_of::<SignRequest>();
    let _ = core::mem::size_of::<SignReceipt>();
    let _ = core::mem::size_of::<SignerStatus>();
    let _ = core::mem::size_of::<Error>();
    fn assert_object_safe(_: &dyn Signer) {}
    let _ = assert_object_safe;
    for root_export in [
        "pub use actor::Actor;",
        "pub use error::Error;",
        "pub use receipt::SignReceipt;",
        "pub use request::SignRequest;",
        "pub use signer::Signer;",
        "pub use status::SignerStatus;",
    ] {
        assert!(ROOT.contains(root_export), "missing {root_export}");
    }
}
