use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const CLIENT: &str = include_str!("../src/client.rs");
const METHOD: &str = include_str!("../src/method.rs");
const PERMISSION: &str = include_str!("../src/permission.rs");
const ROOT: &str = include_str!("../src/lib.rs");
const SERVER: &str = include_str!("../src/server.rs");
const URI: &str = include_str!("../src/uri.rs");

#[test]
fn manifest_has_final_identity_feature_vocabulary_and_radroots_dependencies() {
    for required in [
        "name = \"radroots_nostr_connect\"",
        "version = \"0.1.0-alpha\"",
        "publish = false",
        "[lib]\nname = \"radroots_nostr_connect\"",
        "default = [\"serde\"]",
        "radroots_event = { workspace = true, default-features = false }",
        "radroots_identity = { workspace = true, default-features = false }",
        "radroots_nostr = { workspace = true, default-features = false }",
        "radroots_protocol = { workspace = true, default-features = false }",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }

    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "serde"])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]")
            .into_iter()
            .filter(|dependency| dependency.starts_with("radroots_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "radroots_event",
            "radroots_identity",
            "radroots_nostr",
            "radroots_protocol",
        ])
    );
}

#[test]
fn crate_root_contains_the_approved_module_skeleton() {
    for module in [
        "client",
        "error",
        "message",
        "method",
        "permission",
        "server",
        "uri",
    ] {
        assert!(
            ROOT.contains(&format!("pub mod {module};")),
            "crate root is missing `{module}`"
        );
    }

    assert!(SERVER.starts_with("//! NIP-46 server-side protocol state."));
    assert!(ROOT.contains("#[doc(hidden)]\npub mod prelude"));
    assert!(
        ROOT.contains("Step 143 removes this module"),
        "the temporary prelude must carry an exact removal checkpoint"
    );
}

#[test]
fn uri_method_and_permission_types_use_canonical_owners_and_names() {
    for root_export in [
        "pub use method::Method;",
        "pub use permission::Permission;",
        "pub use uri::{BunkerUri, ClientUri};",
    ] {
        assert!(ROOT.contains(root_export), "missing `{root_export}`");
    }
    for forbidden in [
        "pub enum RadrootsNostrConnectMethod",
        "pub struct RadrootsNostrConnectPermission",
        "pub struct RadrootsNostrConnectPermissions",
    ] {
        assert!(!METHOD.contains(forbidden));
        assert!(!PERMISSION.contains(forbidden));
    }
    for forbidden in [
        "pub struct RadrootsNostrConnectBunkerUri",
        "pub struct RadrootsNostrConnectClientUri",
        "pub enum RadrootsNostrConnectUri",
        "use nostr::{PublicKey",
    ] {
        assert!(!URI.contains(forbidden), "URI source retains `{forbidden}`");
    }
    assert!(URI.contains("use radroots_identity::PublicKey;"));
    assert!(URI.contains("radroots_nostr::key::parse_public_key"));
}

#[test]
fn client_root_and_transport_use_package_owned_state_machine_types() {
    assert!(ROOT.contains("pub use client::Client;"));
    for required in [
        "pub struct Client {",
        "pub struct ClientEvent(Event);",
        "pub struct Target {",
        "pub trait Transport: Send {",
        "pub enum CancellationPhase {",
    ] {
        assert!(CLIENT.contains(required), "client is missing `{required}`");
    }
    for forbidden in [
        "pub struct Client {\n    pub ",
        "pub struct ClientEvent(pub ",
        "pub struct Target {\n    pub ",
    ] {
        assert!(
            !CLIENT.contains(forbidden),
            "client exposes representation through `{forbidden}`"
        );
    }
}

fn table_keys<'a>(source: &'a str, header: &str) -> BTreeSet<&'a str> {
    let Some((_, tail)) = source.split_once(header) else {
        panic!("manifest is missing {header}");
    };

    tail.lines()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('"') {
                return None;
            }
            line.split_once('=').map(|(key, _)| key.trim())
        })
        .collect()
}
