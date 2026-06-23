use super::{SdkRelayTargetPolicy, SdkRelayTargetSet, SdkRelayUrlPolicy};
use crate::{RadrootsSdkError, SDK_RELAY_TARGET_MAX_COUNT};

#[path = "../support/serializer_failure.rs"]
mod serializer_failure;

use serializer_failure::assert_struct_serialize_error_paths;

fn is_local_ws_relay(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("ws://") else {
        return false;
    };
    let authority = rest
        .split_once('/')
        .map(|(authority, _)| authority)
        .unwrap_or(rest);
    let host = relay_authority_host(authority);
    matches!(host.as_deref(), Some("localhost" | "127.0.0.1" | "[::1]"))
}

fn relay_authority_host(authority: &str) -> Option<String> {
    if let Some(after_open) = authority.strip_prefix('[') {
        let close_index = after_open.find(']')?;
        return Some(format!("[{}]", &after_open[..close_index]));
    }
    Some(
        authority
            .split_once(':')
            .map(|(host, _)| host)
            .unwrap_or(authority)
            .to_owned(),
    )
}

#[test]
fn use_configured_policy_serializes_as_kind_only() {
    let policy = SdkRelayTargetPolicy::UseConfiguredRelays;
    assert_eq!(
        serde_json::to_value(&policy).expect("json"),
        serde_json::json!({ "kind": "use_configured_relays" })
    );
    assert_struct_serialize_error_paths(&policy, 1);

    let publish_transport_policy = SdkRelayTargetPolicy::use_publish_transport();
    assert_eq!(
        serde_json::to_value(&publish_transport_policy).expect("json"),
        serde_json::json!({ "kind": "use_publish_transport" })
    );
    assert_struct_serialize_error_paths(&publish_transport_policy, 1);
}

#[test]
fn target_set_accessors_and_configured_relays_cover_empty_and_dedupe_paths() {
    assert_eq!(
        SdkRelayTargetSet::from_configured_relays(Vec::<String>::new(), SdkRelayUrlPolicy::Public)
            .expect("empty configured"),
        Vec::<String>::new()
    );

    let targets = SdkRelayTargetSet::from_normalized_relays(vec![
        "wss://relay-a.example.com".to_owned(),
        "wss://relay-a.example.com".to_owned(),
        "wss://relay-b.example.com".to_owned(),
    ])
    .expect("targets");

    assert_eq!(targets.len(), 2);
    assert!(!targets.is_empty());
    assert_struct_serialize_error_paths(&targets, 2);
    assert_struct_serialize_error_paths(&SdkRelayTargetPolicy::explicit(targets.clone()), 3);
    assert_eq!(
        serde_json::to_value(&targets).expect("targets json"),
        serde_json::json!({
            "relays": ["wss://relay-a.example.com", "wss://relay-b.example.com"],
            "canonical_relays": ["wss://relay-a.example.com", "wss://relay-b.example.com"]
        })
    );
    assert_eq!(
        targets.into_vec(),
        vec![
            "wss://relay-a.example.com".to_owned(),
            "wss://relay-b.example.com".to_owned()
        ]
    );

    assert_eq!(
        SdkRelayTargetPolicy::try_explicit(
            vec!["wss://relay-c.example.com".to_owned()],
            SdkRelayUrlPolicy::Public,
        )
        .expect("explicit policy"),
        SdkRelayTargetPolicy::Explicit(
            SdkRelayTargetSet::new(["wss://relay-c.example.com"], SdkRelayUrlPolicy::Public)
                .expect("target set"),
        )
    );
}

#[test]
fn normalized_relays_reject_empty_and_over_limit_sets() {
    assert!(matches!(
        SdkRelayTargetSet::from_normalized_relays(Vec::new()),
        Err(RadrootsSdkError::EmptyTargetRelays { .. })
    ));

    let too_many = (0..=SDK_RELAY_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example.com"))
        .collect::<Vec<_>>();
    assert!(matches!(
        SdkRelayTargetSet::from_normalized_relays(too_many),
        Err(RadrootsSdkError::RelayTargetLimitExceeded { actual, .. })
            if actual == SDK_RELAY_TARGET_MAX_COUNT + 1
    ));
}

#[test]
fn local_ws_authority_parser_handles_ipv6_ports_and_non_ws_values() {
    assert!(is_local_ws_relay("ws://localhost:8080/path"));
    assert!(is_local_ws_relay("ws://127.0.0.1:8080"));
    assert!(is_local_ws_relay("ws://[::1]:8080"));
    assert!(!is_local_ws_relay("wss://relay.example.com"));
    assert!(!is_local_ws_relay("ws://relay.example.com"));
    assert!(matches!(
        SdkRelayTargetSet::new(["ws://relay.example.com"], SdkRelayUrlPolicy::Localhost),
        Err(RadrootsSdkError::InvalidRelayUrl { reason, .. })
            if reason.contains("localhost")
    ));
    assert!(matches!(
        SdkRelayTargetSet::new(["ws://relay.example.com"], SdkRelayUrlPolicy::Public),
        Err(RadrootsSdkError::InvalidRelayUrl { reason, .. })
            if reason.contains("localhost")
    ));
    assert_eq!(relay_authority_host("[::1]:8080"), Some("[::1]".to_owned()));
    assert_eq!(
        relay_authority_host("relay.example.com:443"),
        Some("relay.example.com".to_owned())
    );
    assert_eq!(relay_authority_host("[::1"), None);
}
