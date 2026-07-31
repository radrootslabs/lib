use std::collections::BTreeSet;

use radroots_nostr_connect::{
    BunkerUri, Client, ClientUri, Error, Method, Permission, Request, Response, Server,
    client::Transport,
};

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const EXAMPLE: &str = include_str!("../examples/prepare_request.rs");
const PUBLIC_API: &str = include_str!("../../../docs/api/radroots_nostr_connect.txt");
const CLIENT: &str = include_str!("../src/client.rs");
const METHOD: &str = include_str!("../src/method.rs");
const PERMISSION: &str = include_str!("../src/permission.rs");
const ROOT: &str = include_str!("../src/lib.rs");
const SERVER: &str = include_str!("../src/server.rs");
const URI: &str = include_str!("../src/uri.rs");
const SIGNER_CONSUMERS: &[&str] = &[
    include_str!("../../nostr_signer/src/backend.rs"),
    include_str!("../../nostr_signer/src/capability.rs"),
    include_str!("../../nostr_signer/src/error.rs"),
    include_str!("../../nostr_signer/src/evaluation.rs"),
    include_str!("../../nostr_signer/src/manager.rs"),
    include_str!("../../nostr_signer/src/model.rs"),
    include_str!("../../nostr_signer/src/nip46.rs"),
    include_str!("../../nostr_signer/src/store.rs"),
];

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
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]"),
        BTreeSet::from([
            "nostr",
            "radroots_event",
            "radroots_identity",
            "radroots_nostr",
            "radroots_protocol",
            "serde",
            "serde_json",
            "thiserror",
            "url",
        ])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dev-dependencies]"),
        BTreeSet::from(["tokio"])
    );
    for forbidden in [
        "keyring",
        "nostr-sdk",
        "reqwest",
        "sqlx",
        "radroots_secrets",
        "radroots_storage",
        "radroots_transport_nostr",
    ] {
        assert!(
            !table_keys(MANIFEST, "[dependencies]").contains(forbidden),
            "protocol package acquired forbidden production dependency `{forbidden}`"
        );
    }
}

#[test]
fn crate_root_contains_the_approved_module_skeleton() {
    let final_root = ROOT
        .split("// Transitional compatibility surface")
        .next()
        .expect("final root declarations");
    assert_eq!(
        declarations(final_root, "pub mod "),
        BTreeSet::from([
            "client",
            "error",
            "message",
            "method",
            "permission",
            "server",
            "uri",
        ])
    );
    assert_eq!(
        final_root
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub use "))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "pub use client::Client;",
            "pub use error::RadrootsNostrConnectError as Error;",
            "pub use message::{Request, Response};",
            "pub use method::Method;",
            "pub use permission::Permission;",
            "pub use server::Server;",
            "pub use uri::{BunkerUri, ClientUri};",
        ])
    );

    assert!(SERVER.starts_with("//! Relay- and persistence-independent NIP-46 server state."));
    assert!(ROOT.contains("pub use server::Server;"));
    assert!(ROOT.contains("#[doc(hidden)]\npub mod prelude"));
    assert!(
        ROOT.contains("Step 143 removes this module"),
        "the temporary prelude must carry an exact removal checkpoint"
    );
}

#[test]
fn approved_root_exports_and_transport_trait_compile() {
    fn assert_value<T: Send + Sync>() {}
    fn assert_error<T: core::error::Error + Send + Sync>() {}
    fn accept_transport(_: &mut dyn Transport) {}

    assert_value::<BunkerUri>();
    assert_value::<Client>();
    assert_value::<ClientUri>();
    assert_value::<Method>();
    assert_value::<Permission>();
    assert_value::<Request>();
    assert_value::<Response>();
    assert_value::<Server>();
    assert_error::<Error>();
    let _ = accept_transport;

    let public_traits = CLIENT
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("pub trait "))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        public_traits,
        BTreeSet::from([
            "pub trait RadrootsNostrConnectClientTransport {",
            "pub trait Transport: Send {",
        ]),
        "only the final host transport SPI and Step 143 compatibility trait may remain"
    );
    for forbidden in [
        "Relay", "Runtime", "Session", "Storage", "Secret", "Approval",
    ] {
        assert!(
            public_traits
                .iter()
                .all(|trait_line| !trait_line.contains(forbidden)),
            "protocol package exposes forbidden owner trait containing `{forbidden}`"
        );
    }
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

#[test]
fn protocol_transport_boundary_has_no_relay_pool_or_runtime_owner() {
    for forbidden in [
        "nostr-sdk",
        "nostr_sdk",
        "tokio::runtime",
        "RelayPool",
        "Client::start",
    ] {
        assert!(
            !CLIENT.contains(forbidden) && !MANIFEST.contains(forbidden),
            "protocol transport retains forbidden owner `{forbidden}`"
        );
    }
    assert!(CLIENT.contains("pub trait Transport: Send"));
    assert!(CLIENT.contains("T: Transport + ?Sized"));
}

#[test]
fn workspace_signer_consumers_use_only_final_protocol_paths() {
    for source in SIGNER_CONSUMERS {
        for retired in [
            "radroots_nostr_connect::prelude",
            "RadrootsNostrConnectClient",
            "RadrootsNostrConnectMethod",
            "RadrootsNostrConnectPermission",
            "RadrootsNostrConnectRequest",
            "RadrootsNostrConnectResponse",
            "RADROOTS_NOSTR_CONNECT_",
        ] {
            assert!(
                !source.contains(retired),
                "workspace signer consumer retains retired protocol surface `{retired}`"
            );
        }
    }
}

#[test]
fn package_documentation_and_reviewed_api_baseline_cover_the_public_contract() {
    for section in [
        "## Canonical surface",
        "## Features and supported targets",
        "## Serialization and compatibility",
        "## Security, side effects, and commit points",
        "## Intended consumers",
        "## Package charter",
    ] {
        assert!(README.contains(section), "README is missing `{section}`");
    }
    for required in [
        "Client::generate",
        "Target::try_new",
        "Request::Ping",
        "operation.publication()",
    ] {
        assert!(
            EXAMPLE.contains(required),
            "example is missing `{required}`"
        );
    }
    for export in [
        "pub struct radroots_nostr_connect::Client",
        "pub struct radroots_nostr_connect::Server",
        "pub enum radroots_nostr_connect::Method",
        "pub struct radroots_nostr_connect::Permission",
        "pub enum radroots_nostr_connect::Request",
        "pub enum radroots_nostr_connect::Response",
        "pub struct radroots_nostr_connect::BunkerUri",
        "pub struct radroots_nostr_connect::ClientUri",
        "pub enum radroots_nostr_connect::Error",
    ] {
        assert!(
            PUBLIC_API.contains(export),
            "reviewed API baseline is missing `{export}`"
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

fn declarations<'a>(source: &'a str, prefix: &str) -> BTreeSet<&'a str> {
    source
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            line.strip_prefix(prefix)
                .and_then(|name| name.strip_suffix(';'))
        })
        .collect()
}
