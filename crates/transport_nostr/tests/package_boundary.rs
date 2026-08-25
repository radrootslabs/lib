use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const EXAMPLE: &str = include_str!("../examples/configure_transport.rs");
const PUBLIC_API: &str =
    include_str!("../../../contracts/api_baselines/radroots_transport_nostr.txt");
const ROOT: &str = include_str!("../src/lib.rs");
const ERROR: &str = include_str!("../src/error.rs");
const PROFILE: &str = include_str!("../src/profile.rs");
const SINK: &str = include_str!("../src/sink.rs");
const SUBSCRIPTION: &str = include_str!("../src/subscription.rs");

#[test]
fn manifest_and_root_match_the_governed_transport_boundary() {
    for required in [
        "name = \"radroots_transport_nostr\"",
        "version = \"0.1.0-alpha\"",
        "publish = [\"crates-io\"]",
        "[lib]\nname = \"radroots_transport_nostr\"",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }
    assert!(!MANIFEST.contains("[features]"));
    assert_eq!(
        radroots_dependency_keys(MANIFEST),
        BTreeSet::from([
            "radroots_event_codec",
            "radroots_nostr",
            "radroots_protocol",
            "radroots_transport",
        ])
    );
    assert_eq!(
        private_modules(ROOT),
        BTreeSet::from([
            "auth",
            "client",
            "cursor",
            "error",
            "profile",
            "relay",
            "sink",
            "source",
            "status",
            "subscription"
        ])
    );
    for export in [
        "pub use client::{Config, NostrTransport, ReconnectBackoff};",
        "pub use cursor::RelayCursor;",
        "pub use error::Error;",
        "pub use profile::{",
        "pub use relay::{RelayUrl, RelayUrlPolicy};",
        "pub use sink::PreparedDelivery;",
    ] {
        assert!(ROOT.contains(export), "crate root is missing `{export}`");
    }
}

#[test]
fn documentation_example_and_reviewed_api_baseline_are_complete() {
    for required in [
        "## Configure without connecting",
        "## Public surface",
        "## Relay and network security",
        "## Fetch, live subscription, delivery, and outcome behavior",
        "## Prepared delivery boundary",
        "## Deadlines, cancellation, and commit points",
        "## Serialization and diagnostics",
        "## Features and runtime requirements",
        "## Intended consumers",
        "radroots_crates_release_v1.toml",
        "examples/configure_transport.rs",
        "contracts/api_baselines/radroots_transport_nostr.txt",
        "Live subscriptions use the same explicit readable targets",
        "exact indexed single-letter tag",
        "Fetch reports `Complete` only after the exact subscription receives EOSE",
        "Deadline expiry is `Cancelled`",
        "exceeds the bounded inventory is `Partial`",
        "inclusive `since` timestamp",
        "permits at-least-once",
        "same-second events in relay arrival order",
        "never regresses the canonical target checkpoint",
        "upstream auto-close deadline",
        "adapter-owned worker",
        "validates the exact request, writable\nrelay bindings, and signed-event conversion without reading a clock, polling\nstatus, or performing relay I/O",
        "Executing a capability\nthrough a differently configured transport fails closed",
        "let _forged = PreparedDelivery {",
        "request: panic!(),",
        "skipped: panic!(),",
        "literal RFC1918 IPv4 or ULA IPv6 destinations",
        "relay URLs and resolved addresses",
    ] {
        assert!(README.contains(required), "README is missing `{required}`");
    }
    for required in [
        "Config::from_profile(",
        "RelayEndpoint::new(",
        "RelayProfile::explicit(",
        "NostrTransport::new(config)",
        "let source: &dyn EventSource",
        "let subscriber: &dyn EventSubscriber",
        "let sink: &dyn EventSink",
        "drop(source.status())",
        "let _ = subscriber",
        "drop(sink.status())",
    ] {
        assert!(
            EXAMPLE.contains(required),
            "example is missing `{required}`"
        );
    }
    for required in [
        "pub struct radroots_transport_nostr::Config",
        "pub struct radroots_transport_nostr::NostrTransport",
        "pub struct radroots_transport_nostr::PreparedDelivery",
        "pub enum radroots_transport_nostr::RelayAggregateState",
        "pub struct radroots_transport_nostr::RelayProfile",
        "pub fn radroots_transport_nostr::RelayEndpoint::new(",
        "pub fn radroots_transport_nostr::RelayProfile::explicit<I>(",
        "pub struct radroots_transport_nostr::RelayStatusReport",
        "pub struct radroots_transport_nostr::RelayUrl(_)",
        "pub enum radroots_transport_nostr::RelayUrlPolicy",
        "pub enum radroots_transport_nostr::Error",
        "impl radroots_transport::sink::EventSink for radroots_transport_nostr::NostrTransport",
        "impl radroots_transport::source::EventSource for radroots_transport_nostr::NostrTransport",
        "impl radroots_transport::source::EventSubscriber for radroots_transport_nostr::NostrTransport",
        "NostrTransport::begin_authentication",
        "NostrTransport::complete_authentication",
        "NostrTransport::reject_authentication",
        "NostrTransport::prepare_delivery",
        "NostrTransport::execute_prepared_delivery",
    ] {
        assert!(
            PUBLIC_API.contains(required),
            "public API baseline is missing `{required}`"
        );
    }
    for forbidden in [
        "nostr_sdk",
        "nostr_relay_pool",
        "tokio::",
        "radroots_storage",
        "radroots_outbox",
        "pub trait radroots_transport_nostr",
        "impl core::clone::Clone for radroots_transport_nostr::PreparedDelivery",
        "impl serde_core::ser::Serialize for radroots_transport_nostr::PreparedDelivery",
    ] {
        assert!(
            !PUBLIC_API.contains(forbidden),
            "reviewed public API baseline exposes `{forbidden}`"
        );
    }
}

#[test]
fn public_errors_retain_no_network_or_dependency_owned_material() {
    let declaration = ERROR
        .split_once("pub enum Error {")
        .expect("error declaration")
        .1
        .split_once("\n}")
        .expect("error declaration end")
        .0;
    for forbidden in [
        "url:",
        "address:",
        "reason:",
        "String",
        "url::",
        "nostr_sdk::",
        "nostr_relay_pool::",
    ] {
        assert!(
            !declaration.contains(forbidden),
            "public error retains forbidden network material `{forbidden}`"
        );
    }
}

#[test]
fn preparation_is_sealed_and_separated_from_execution_io() {
    let prepare = SINK
        .split_once("pub fn prepare_delivery(")
        .expect("prepared delivery function")
        .1
        .split_once("pub fn execute_prepared_delivery(")
        .expect("execution boundary")
        .0;
    for forbidden in [".await", "unix_time_ms", "self.status", ".publish("] {
        assert!(
            !prepare.contains(forbidden),
            "delivery preparation contains I/O authority `{forbidden}`"
        );
    }
    for required in [
        "pub struct PreparedDelivery",
        "#[must_use = \"prepared delivery must be durably bound before execution or deliberately discarded\"]",
        "request: DeliveryRequest",
        "event: Event",
        "config: crate::Config",
        "formatter.write_str(\"PreparedDelivery([redacted])\")",
        "pub const fn request(&self) -> &DeliveryRequest",
    ] {
        assert!(
            SINK.contains(required),
            "prepared boundary is missing `{required}`"
        );
    }
    for forbidden in [
        "impl Clone for PreparedDelivery",
        "derive(Clone",
        "pub fn new(",
        "pub request:",
        "pub event:",
        "pub config:",
    ] {
        assert!(
            !prepare.contains(forbidden),
            "prepared boundary exposes `{forbidden}`"
        );
    }
}

#[test]
fn relay_profiles_have_no_implicit_destination_or_policy_constructor() {
    for forbidden in [
        "DEFAULT_PUBLIC_RELAY",
        "pub fn public",
        "pub fn simulator",
        "pub fn device",
    ] {
        assert!(!ROOT.contains(forbidden));
        assert!(!PROFILE.contains(forbidden));
        assert!(!PUBLIC_API.contains(forbidden));
    }
    for required in [
        "pub fn new(",
        "policy: RelayUrlPolicy",
        "access: RelayAccess",
        "IntoIterator<Item = RelayEndpoint>",
        ".take(crate::client::MAX_RELAYS + 1)",
        "Error::RelayProfilePolicyMismatch",
    ] {
        assert!(
            PROFILE.contains(required),
            "profile is missing `{required}`"
        );
    }
}

fn radroots_dependency_keys(manifest: &str) -> BTreeSet<&str> {
    dependency_keys(manifest)
        .into_iter()
        .filter(|key| key.starts_with("radroots_"))
        .collect()
}

fn dependency_keys(manifest: &str) -> BTreeSet<&str> {
    manifest
        .split_once("[dependencies]")
        .map(|(_, dependencies)| dependencies)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| !key.is_empty())
        .collect()
}

fn private_modules(root: &str) -> BTreeSet<&str> {
    root.lines()
        .filter_map(|line| line.trim().strip_prefix("mod "))
        .filter_map(|module| module.strip_suffix(';'))
        .collect()
}

#[test]
fn adapter_owns_no_storage_outbox_or_orchestration_surface() {
    for forbidden in [
        "radroots_event_store",
        "radroots_outbox",
        "radroots_storage",
        "publish_claimed",
        "fetch_and_ingest",
        "projection_refresh",
        "retry_schedule",
    ] {
        assert!(!MANIFEST.contains(forbidden));
        assert!(!ROOT.contains(forbidden));
    }

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source_files = fs::read_dir(source_root)
        .expect("source directory")
        .map(|entry| {
            entry
                .expect("source entry")
                .file_name()
                .into_string()
                .expect("utf-8 source name")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        source_files,
        BTreeSet::from([
            "auth.rs".to_owned(),
            "client.rs".to_owned(),
            "cursor.rs".to_owned(),
            "error.rs".to_owned(),
            "lib.rs".to_owned(),
            "profile.rs".to_owned(),
            "relay.rs".to_owned(),
            "sink.rs".to_owned(),
            "source.rs".to_owned(),
            "status.rs".to_owned(),
            "subscription.rs".to_owned(),
        ])
    );

    for required in [
        "impl EventSubscriber for NostrTransport",
        "SubscribeAutoCloseOptions::default()",
        "ReqExitPolicy::WaitDurationAfterEOSE(query.timeout)",
        "self.seen_event_ids.contains(event_id.as_str())",
        "resume_cursors",
        "let cursor_advances = self",
        "self.terminate(SubscriptionEndReason::Cancelled)",
    ] {
        assert!(
            SUBSCRIPTION.contains(required),
            "subscription adapter is missing `{required}`"
        );
    }
    for forbidden in [
        "tokio::spawn",
        "spawn_blocking",
        "std::thread",
        "process::",
        "global_default",
    ] {
        assert!(
            !SUBSCRIPTION.contains(forbidden),
            "subscription adapter contains forbidden authority `{forbidden}`"
        );
    }
}
