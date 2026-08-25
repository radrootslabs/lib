use core::str::FromStr;

use radroots_transport::{
    Error, TARGET_SET_MAX_ITEMS, Target, TargetNetworkPolicy, TargetSet, TransportId,
    endpoint::{
        ENDPOINT_URI_MAX_BYTES, EndpointUri, TARGET_LABEL_MAX_BYTES, TARGET_SCOPE_MAX_BYTES,
        TargetLabel, TargetScope,
    },
    target::TargetFingerprint,
};
use serde_json::Value;

#[test]
fn endpoint_scope_and_label_values_are_canonical_and_bounded() {
    let endpoint = EndpointUri::from_str("MESH://Node.Example/path").expect("endpoint");
    assert_eq!(endpoint.as_ref(), "mesh://node.example/path");
    assert_eq!(EndpointUri::try_from(endpoint.as_str()).unwrap(), endpoint);
    assert_eq!(
        EndpointUri::parse("a".repeat(ENDPOINT_URI_MAX_BYTES + 1)).unwrap_err(),
        Error::InvalidTargetUri
    );

    let scope = TargetScope::from_str("farm_1.alpha-beta").expect("scope");
    assert_eq!(scope.as_ref(), "farm_1.alpha-beta");
    assert_eq!(TargetScope::try_from(scope.as_str()).unwrap(), scope);
    assert_eq!(
        TargetScope::parse("a".repeat(TARGET_SCOPE_MAX_BYTES + 1)).unwrap_err(),
        Error::InvalidTargetScope
    );

    let label = TargetLabel::from_str(" Relay One ").expect("label");
    assert_eq!(label.as_ref(), "Relay One");
    assert_eq!(TargetLabel::try_from(label.as_str()).unwrap(), label);
    assert_eq!(
        TargetLabel::parse("a".repeat(TARGET_LABEL_MAX_BYTES + 1)).unwrap_err(),
        Error::InvalidTargetLabel
    );
}

#[test]
fn fingerprints_and_target_sets_are_deterministic_unique_and_bounded() {
    let target = Target::new(TransportId::LOCAL, "local:node-0").expect("target");
    let fingerprint =
        TargetFingerprint::from_str(target.fingerprint().as_str()).expect("target fingerprint");
    assert_eq!(&fingerprint, target.fingerprint());
    assert_eq!(
        TargetFingerprint::try_from(fingerprint.as_str()).unwrap(),
        fingerprint
    );

    let targets = (0..TARGET_SET_MAX_ITEMS)
        .map(|index| {
            Target::new(TransportId::LOCAL, format!("local:node-{index}")).expect("bounded target")
        })
        .collect();
    let target_set = TargetSet::new(targets).expect("maximum target set");
    assert_eq!(target_set.len(), TARGET_SET_MAX_ITEMS);

    let oversized = (0..=TARGET_SET_MAX_ITEMS)
        .map(|index| {
            Target::new(TransportId::LOCAL, format!("local:oversized-{index}"))
                .expect("oversized target member")
        })
        .collect();
    assert_eq!(
        TargetSet::new(oversized).unwrap_err(),
        Error::TargetSetTooLarge
    );

    assert_eq!(
        TargetSet::new(vec![target.clone(), target]).unwrap_err(),
        Error::DuplicateTargetFingerprint
    );
}

#[test]
fn transport_target_uri_vectors_match_the_canonical_parser() {
    let document: Value = serde_json::from_str(include_str!("fixtures/target_uri.v1.json"))
        .expect("transport target vectors");
    let vectors = document["vectors"].as_array().expect("vector array");

    for vector in vectors {
        let kind = vector["kind"].as_str().expect("vector kind");
        let raw = vector["input"]["uri"].as_str().expect("input URI");
        match kind {
            "transport.target_uri.valid" => assert_eq!(
                EndpointUri::parse(raw).expect("valid endpoint").as_str(),
                vector["expected"]["canonical_uri"]
                    .as_str()
                    .expect("canonical URI")
            ),
            "transport.target_uri.invalid" => assert!(EndpointUri::parse(raw).is_err()),
            "transport.nostr_relay_target.valid" => assert_eq!(
                Target::nostr_relay(raw)
                    .expect("valid relay")
                    .uri()
                    .as_str(),
                vector["expected"]["canonical_uri"]
                    .as_str()
                    .expect("canonical URI")
            ),
            "transport.nostr_relay_target.invalid" => {
                assert!(Target::nostr_relay(raw).is_err())
            }
            "transport.nostr_relay_private_device.valid" => assert_eq!(
                Target::nostr_relay_with_policy(raw, TargetNetworkPolicy::PrivateDevice)
                    .expect("valid private-device relay")
                    .uri()
                    .as_str(),
                vector["expected"]["canonical_uri"]
                    .as_str()
                    .expect("canonical URI")
            ),
            "transport.nostr_relay_private_device.invalid" => assert!(
                Target::nostr_relay_with_policy(raw, TargetNetworkPolicy::PrivateDevice).is_err()
            ),
            other => panic!("unknown vector kind {other}"),
        }
    }
}

#[test]
fn generic_endpoint_parser_covers_opaque_authority_and_scheme_edges() {
    let schemeless = EndpointUri::parse("transport-target").expect("schemeless URI");
    assert_eq!(schemeless.as_str(), "transport-target");
    assert_eq!(schemeless.to_string(), "transport-target");
    assert_eq!(
        EndpointUri::parse("RNS:PeerA")
            .expect("opaque URI")
            .as_str(),
        "rns:PeerA"
    );
    assert_eq!(
        EndpointUri::parse("MESH://Node.Example/path?q=1#frag")
            .expect("authority URI")
            .as_str(),
        "mesh://node.example/path?q=1#frag"
    );
    assert_eq!(EndpointUri::parse(" ").unwrap_err(), Error::EmptyTargetUri);

    for invalid in [
        "bad target",
        " transport-target ",
        ":target",
        "1bad:target",
        "bad_scheme://target",
        "bad\ttarget",
        "bad\ntarget",
    ] {
        assert_eq!(
            EndpointUri::parse(invalid).unwrap_err(),
            Error::InvalidTargetUri,
            "{invalid:?} must fail"
        );
    }
}

#[test]
fn nostr_targets_canonicalize_identity_and_local_websocket_endpoints() {
    let root = Target::nostr_relay("wss://relay.example").expect("root relay");
    let slash = Target::nostr_relay("WSS://RELAY.EXAMPLE/").expect("root slash relay");
    let default_port = Target::nostr_relay("wss://relay.example:443").expect("default port relay");
    let path = Target::nostr_relay("wss://relay.example/Events").expect("path relay");
    let generic =
        Target::new(TransportId::NOSTR, "wss://relay.example/").expect("generic Nostr target");

    assert_eq!(root.uri().as_str(), "wss://relay.example");
    assert_eq!(root.fingerprint(), slash.fingerprint());
    assert_eq!(root.fingerprint(), default_port.fingerprint());
    assert_eq!(root.fingerprint(), generic.fingerprint());
    assert_ne!(root.fingerprint(), path.fingerprint());
    assert_eq!(path.uri().as_str(), "wss://relay.example/Events");
    assert_eq!(
        TargetSet::new(vec![root, slash]).unwrap_err(),
        Error::DuplicateTargetFingerprint
    );

    for (raw, expected) in [
        ("ws://LOCALHOST:80/", "ws://localhost"),
        ("ws://127.0.0.1:7777", "ws://127.0.0.1:7777"),
        ("ws://[0:0:0:0:0:0:0:1]:7777", "ws://[::1]:7777"),
        ("wss://[2001:0DB8::1]:443", "wss://[2001:db8::1]"),
        (
            "wss://relay.example:7443/a%20b",
            "wss://relay.example:7443/a%20b",
        ),
    ] {
        assert_eq!(
            Target::nostr_relay(raw)
                .expect("valid relay")
                .uri()
                .as_str(),
            expected
        );
    }
}

#[test]
fn nostr_targets_reject_ambiguous_authorities_hosts_ports_and_paths() {
    let overlong = format!("wss://{}", "a".repeat(ENDPOINT_URI_MAX_BYTES));
    let overlong_dns_label = format!("wss://{}.example", "a".repeat(64));
    let overlong_host = format!("wss://{}.example", "a.".repeat(124));
    assert_eq!(Target::nostr_relay("").unwrap_err(), Error::EmptyTargetUri);
    let invalid = [
        " wss://relay.example",
        "wss://relay.example ",
        "https://relay.example",
        "wss:relay.example",
        "wss://",
        "wss://user@relay.example",
        "wss://user:password@relay.example",
        "wss://relay.example?subscription=1",
        "wss://relay.example#fragment",
        "wss://relay.example\\path",
        "wss://relay.example:",
        "wss://relay.example:0",
        "wss://relay.example:01",
        "wss://relay.example:65536",
        "wss://relay.example:999999999999999999999999999999999999",
        "wss://:443",
        "wss://[",
        "wss://[]",
        "wss://[::1",
        "wss://[::1]suffix",
        "wss://[::1]:",
        "wss://[not-ipv6]",
        "wss://[::ffff:127.0.0.1x]",
        "wss://relay[.]example",
        "wss://127.0.0.01",
        "wss://127.0.0.256",
        "wss://127.0.0",
        "wss://127.0.0.1.2",
        "wss://-relay.example",
        "wss://relay-.example",
        "wss://relay..example",
        "wss://xn--relay.example",
        "wss://relay.123",
        "wss://relay.0x10",
        "wss://relay.ex_ample",
        "ws://relay.example",
        "ws://192.168.1.2",
        "wss://relay.example/.",
        "wss://relay.example/..",
        "wss://relay.example/%2E",
        "wss://relay.example/%2E%2E",
        "wss://relay.example/%2e",
        "wss://relay.example/%",
        "wss://relay.example/%2",
        "wss://relay.example/a[b",
    ];
    for raw in invalid.into_iter().chain([
        overlong.as_str(),
        overlong_dns_label.as_str(),
        overlong_host.as_str(),
    ]) {
        assert_eq!(
            Target::nostr_relay(raw).unwrap_err(),
            Error::InvalidTargetUri,
            "{raw:?} must fail"
        );
    }
}

#[test]
fn private_device_relay_targets_require_an_explicit_literal_policy() {
    for (raw, canonical, requires_explicit_policy) in [
        ("ws://10.0.0.1:7447", "ws://10.0.0.1:7447", true),
        ("ws://172.16.0.1", "ws://172.16.0.1", true),
        ("wss://192.168.1.2", "wss://192.168.1.2", false),
        ("ws://[FD00::1]:7447", "ws://[fd00::1]:7447", true),
    ] {
        assert_eq!(
            Target::nostr_relay(raw).is_err(),
            requires_explicit_policy,
            "{raw}"
        );
        assert_eq!(
            Target::nostr_relay_with_policy(raw, TargetNetworkPolicy::PrivateDevice)
                .expect("private-device target")
                .uri()
                .as_str(),
            canonical
        );
    }

    for denied in [
        "ws://relay.example",
        "ws://8.8.8.8",
        "ws://127.0.0.1",
        "ws://169.254.1.1",
        "ws://224.0.0.1",
        "ws://[::1]",
        "ws://[fe80::1]",
        "ws://[ff02::1]",
    ] {
        assert!(
            Target::nostr_relay_with_policy(denied, TargetNetworkPolicy::PrivateDevice).is_err(),
            "{denied}"
        );
    }
}

#[test]
fn target_metadata_identity_and_collection_accessors_are_explicit() {
    let scope = TargetScope::parse("local").expect("scope");
    let label = TargetLabel::parse(" Local node ").expect("label");
    let local =
        Target::local_with_metadata("local:node-1", Some(scope.clone()), Some(label.clone()))
            .expect("local target");
    let relabeled = Target::new_with_metadata(
        TransportId::LOCAL,
        "local:node-1",
        Some(scope),
        Some(TargetLabel::parse("Renamed node").expect("label")),
    )
    .expect("relabeled target");
    let unscoped = Target::local("local:node-1").expect("unscoped target");

    assert_eq!(local.kind(), &TransportId::LOCAL);
    assert_eq!(local.uri().as_ref(), "local:node-1");
    assert_eq!(local.scope().map(TargetScope::as_str), Some("local"));
    assert_eq!(local.label().map(TargetLabel::as_str), Some("Local node"));
    assert_eq!(local.fingerprint(), relabeled.fingerprint());
    assert_ne!(local.fingerprint(), unscoped.fingerprint());

    let set = TargetSet::new(vec![local.clone(), unscoped]).expect("target set");
    assert!(set.contains(local.fingerprint()));
    assert!(!set.is_empty());
    assert_eq!(set.targets().len(), 2);
    let foreign = Target::local("local:foreign").expect("foreign target");
    assert!(!set.contains(foreign.fingerprint()));

    assert_eq!(TargetScope::parse("").unwrap_err(), Error::EmptyTargetScope);
    for value in [" scope", "scope ", "bad scope", "bad/scope", "bad\nscope"] {
        assert_eq!(
            TargetScope::parse(value).unwrap_err(),
            Error::InvalidTargetScope
        );
    }
    assert_eq!(
        TargetLabel::parse(" ").unwrap_err(),
        Error::EmptyTargetLabel
    );
    assert_eq!(
        TargetLabel::parse("bad\nlabel").unwrap_err(),
        Error::InvalidTargetLabel
    );
    assert_eq!(
        TargetFingerprint::parse("abc").unwrap_err(),
        Error::InvalidTargetFingerprint
    );
    assert_eq!(
        TargetFingerprint::parse("g".repeat(64)).unwrap_err(),
        Error::InvalidTargetFingerprint
    );
    let uppercase = TargetFingerprint::parse(local.fingerprint().as_str().to_ascii_uppercase())
        .expect("uppercase fingerprint");
    assert_eq!(uppercase.as_str(), local.fingerprint().as_str());
    assert_eq!(uppercase.to_string(), local.fingerprint().as_str());
    assert_eq!(
        TargetSet::new(Vec::new()).unwrap_err(),
        Error::EmptyTargetSet
    );
}

#[test]
#[cfg(feature = "serde")]
fn target_deserialization_revalidates_every_canonical_identity_field() {
    let target = Target::nostr_relay_with_metadata(
        "wss://relay.example/path",
        Some(TargetScope::parse("public").expect("scope")),
        Some(TargetLabel::parse("Relay").expect("label")),
    )
    .expect("target");
    let value = serde_json::to_value(&target).expect("target JSON");
    assert_eq!(
        serde_json::from_value::<Target>(value.clone()).expect("target round trip"),
        target
    );

    for (field, forged) in [
        ("uri", Value::String("WSS://RELAY.EXAMPLE/path".into())),
        ("scope", Value::String("bad scope".into())),
        ("label", Value::String(" Relay ".into())),
        ("fingerprint", Value::String("A".repeat(64))),
        ("fingerprint", Value::String("0".repeat(64))),
    ] {
        let mut invalid = value.clone();
        invalid[field] = forged;
        assert!(
            serde_json::from_value::<Target>(invalid).is_err(),
            "{field}"
        );
    }

    let set = TargetSet::new(vec![target]).expect("target set");
    let encoded = serde_json::to_value(&set).expect("target set JSON");
    assert_eq!(
        serde_json::from_value::<TargetSet>(encoded.clone()).expect("set round trip"),
        set
    );
    let mut duplicate = encoded;
    let first = duplicate["targets"][0].clone();
    duplicate["targets"].as_array_mut().unwrap().push(first);
    assert!(serde_json::from_value::<TargetSet>(duplicate).is_err());
}

#[test]
#[cfg(feature = "serde")]
fn private_device_cleartext_target_round_trip_remains_explicit_and_validated() {
    let target =
        Target::nostr_relay_with_policy("ws://[fd00::5]:7447", TargetNetworkPolicy::PrivateDevice)
            .expect("private-device target");
    let value = serde_json::to_value(&target).expect("private-device target JSON");
    assert_eq!(value["private_device_cleartext"], Value::Bool(true));
    assert_eq!(
        serde_json::from_value::<Target>(value.clone()).expect("private-device round trip"),
        target
    );

    let mut false_marker = value.clone();
    false_marker["private_device_cleartext"] = Value::Bool(false);
    assert!(serde_json::from_value::<Target>(false_marker).is_err());

    let mut public_address = value.clone();
    public_address["uri"] = Value::String("ws://8.8.8.8:7447".into());
    assert!(serde_json::from_value::<Target>(public_address).is_err());

    let mut named = value;
    named["uri"] = Value::String("ws://relay.internal:7447".into());
    assert!(serde_json::from_value::<Target>(named).is_err());
}
