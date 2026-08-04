use core::fmt::Debug;
use radroots_transport::{EventSink, EventSource};
use radroots_transport_nostr::{Config, Error, NostrTransport, RelayUrl, RelayUrlPolicy};

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const AUTH: &str = include_str!("../src/auth.rs");
const RELAY: &str = include_str!("../src/relay.rs");
const SINK: &str = include_str!("../src/sink.rs");
const SOURCE: &str = include_str!("../src/source.rs");
const STATUS: &str = include_str!("../src/status.rs");

fn assert_adapter_contract<T>()
where
    T: EventSource + EventSink + Clone + Debug + Send + Sync,
{
}

fn assert_public_value<T>()
where
    T: Clone + Debug + Send + Sync,
{
}

#[test]
fn public_types_and_split_spis_satisfy_the_adapter_contract() {
    assert_adapter_contract::<NostrTransport>();
    assert_public_value::<Config>();
    assert_public_value::<RelayUrl>();
    assert_public_value::<RelayUrlPolicy>();
    assert_public_value::<Error>();
}

#[test]
fn root_and_manifest_expose_no_traits_features_or_ambient_runtime() {
    assert!(!MANIFEST.contains("[features]"));
    assert!(MANIFEST.contains("publish = [\"crates-io\"]"));
    for forbidden in [
        "pub trait ",
        "pub mod ",
        "tokio::runtime",
        "tracing_subscriber",
        "radroots_event_store",
        "radroots_outbox",
        "radroots_storage",
        "radroots_sync",
    ] {
        assert!(
            !ROOT.contains(forbidden),
            "forbidden root witness `{forbidden}`"
        );
        assert!(
            !MANIFEST.contains(forbidden),
            "forbidden manifest witness `{forbidden}`"
        );
    }
}

#[test]
fn complete_mocked_conformance_matrix_remains_reachable() {
    let witnesses = [
        (AUTH, "challenge_response_is_exact_bounded_and_single_use"),
        (
            AUTH,
            "wrong_relay_timeout_rejection_and_no_signer_fail_closed",
        ),
        (RELAY, "address_policies_fail_closed_for_special_use_ranges"),
        (SINK, "sink_returns_normalized_per_relay_partial_success"),
        (SINK, "expired_delivery_deadline_performs_no_relay_work"),
        (SINK, "dropping_an_unpolled_delivery_performs_no_relay_work"),
        (
            SOURCE,
            "source_deduplicates_relays_reports_malformed_and_paginates",
        ),
        (SOURCE, "dropping_an_unpolled_fetch_performs_no_relay_work"),
        (
            STATUS,
            "every_upstream_class_maps_to_stable_secret_safe_output",
        ),
    ];
    for (source, witness) in witnesses {
        assert!(
            source.contains(witness),
            "missing conformance witness `{witness}`"
        );
    }
}
