#[allow(unused_imports)]
use radroots_signing::{
    Actor, actor as _, capability as _, error as _, receipt as _, request as _, signer as _,
    status as _,
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
    assert!(ROOT.contains("pub use actor::Actor;"));
}
