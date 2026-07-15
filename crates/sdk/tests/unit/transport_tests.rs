use super::{
    MeshScopeId, NostrProfile, NostrRelayUrlPolicy, PublishMode, ReticulumAgentEndpoint,
    ReticulumBehavior, ReticulumProfile, SatisfactionPolicy, TargetPolicy, TargetSet,
    TransportProfile,
};
use crate::{RadrootsSdkError, SDK_TRANSPORT_TARGET_MAX_COUNT};
use radroots_transport::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RadrootsTransportError, RadrootsTransportKind,
    RadrootsTransportTarget, RadrootsTransportTargetFingerprint, RadrootsTransportTargetUri,
};

use crate::serializer_failure::assert_struct_serialize_error_paths;

#[test]
fn publish_mode_and_ack_policy_serialize_explicit_product_contracts() {
    assert_eq!(
        serde_json::to_value(PublishMode::DryRun).expect("json"),
        serde_json::json!("dry_run")
    );
    assert_eq!(
        serde_json::to_value(PublishMode::EnqueueOnly).expect("json"),
        serde_json::json!("enqueue_only")
    );
    assert_eq!(
        serde_json::to_value(PublishMode::EnqueueAndPublish).expect("json"),
        serde_json::json!("enqueue_and_publish")
    );
    assert_eq!(
        serde_json::to_value(SatisfactionPolicy::NoWait).expect("json"),
        serde_json::json!("no_wait")
    );
    assert_eq!(
        serde_json::to_value(SatisfactionPolicy::AnyAccepted).expect("json"),
        serde_json::json!("any_accepted")
    );
    assert_eq!(
        serde_json::to_value(SatisfactionPolicy::AllAccepted).expect("json"),
        serde_json::json!("all_accepted")
    );
    assert_eq!(
        serde_json::to_value(SatisfactionPolicy::quorum_accepted(2).expect("satisfaction policy"))
            .expect("json"),
        serde_json::json!({ "quorum_accepted": { "threshold": 2 } })
    );
    assert_eq!(
        serde_json::to_value(SatisfactionPolicy::AnyDelivered).expect("json"),
        serde_json::json!("any_delivered")
    );
    assert_eq!(
        serde_json::to_value(SatisfactionPolicy::AllDelivered).expect("json"),
        serde_json::json!("all_delivered")
    );
    assert_eq!(
        serde_json::to_value(SatisfactionPolicy::quorum_delivered(3).expect("satisfaction policy"))
            .expect("json"),
        serde_json::json!({ "quorum_delivered": { "threshold": 3 } })
    );
    assert_eq!(
        serde_json::to_value(
            SatisfactionPolicy::required_accepted_targets(["a".repeat(64)])
                .expect("satisfaction policy")
        )
        .expect("json"),
        serde_json::json!({
            "required_accepted_targets": {
                "target_fingerprints": ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
            }
        })
    );
    assert!(matches!(
        SatisfactionPolicy::quorum_accepted(0),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message == "satisfaction policy threshold must require at least one target"
    ));
}

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
fn transport_profile_policy_serializes_as_kind_only() {
    let transport_profile_policy = TargetPolicy::default_profile();
    assert_eq!(
        serde_json::to_value(&transport_profile_policy).expect("json"),
        serde_json::json!({ "kind": "default_profile" })
    );
    assert_struct_serialize_error_paths(&transport_profile_policy, 1);
}

#[test]
fn target_set_accessors_and_configured_relays_cover_empty_paths() {
    assert!(
        TransportProfile::local_only()
            .configured_nostr_relay_urls()
            .is_empty()
    );

    let targets = TargetSet::nostr_relays(
        ["wss://relay-a.example.com", "wss://relay-b.example.com"],
        NostrRelayUrlPolicy::Public,
    )
    .expect("targets");

    assert_eq!(targets.len(), 2);
    assert!(!targets.is_empty());
    assert_struct_serialize_error_paths(&targets, 2);
    assert_struct_serialize_error_paths(&TargetPolicy::explicit(targets.clone()), 3);
    let targets_json = serde_json::to_value(&targets).expect("targets json");
    assert_eq!(
        targets_json["targets"].as_array().expect("targets").len(),
        2
    );
    assert_eq!(
        targets_json["canonical_targets"]
            .as_array()
            .expect("canonical targets")
            .len(),
        2
    );
    assert_eq!(
        targets.nostr_relay_urls(),
        vec![
            "wss://relay-a.example.com".to_owned(),
            "wss://relay-b.example.com".to_owned()
        ]
    );

    assert_eq!(
        TargetPolicy::try_nostr_relays(
            vec!["wss://relay-c.example.com".to_owned()],
            NostrRelayUrlPolicy::Public,
        )
        .expect("explicit policy"),
        TargetPolicy::Explicit(
            TargetSet::nostr_relays(["wss://relay-c.example.com"], NostrRelayUrlPolicy::Public)
                .expect("target set"),
        )
    );
    assert_eq!(
        serde_json::to_value(
            TargetPolicy::try_nostr_relays(
                vec!["wss://relay-c.example.com".to_owned()],
                NostrRelayUrlPolicy::Public,
            )
            .expect("trade explicit policy")
        )
        .expect("trade policy json"),
        serde_json::json!({
            "kind": "explicit",
            "targets": [{
                "kind": "nostr",
                "uri": "wss://relay-c.example.com",
                "scope": null,
                "label": null,
                "fingerprint": "ec4b5005dd1fcf0d949045e3d5524f9a6a95209ecc888f582ae2e9bf69e5b8e6"
            }],
            "canonical_targets": ["ec4b5005dd1fcf0d949045e3d5524f9a6a95209ecc888f582ae2e9bf69e5b8e6"]
        })
    );

    let nostr_profile = TransportProfile::nostr(
        NostrProfile::new(["wss://relay-d.example.com"], NostrRelayUrlPolicy::Public)
            .expect("Nostr profile"),
    );
    assert_eq!(
        nostr_profile.configured_nostr_relay_urls(),
        vec!["wss://relay-d.example.com".to_owned()]
    );
}

#[test]
fn target_sets_reject_duplicate_transport_fingerprints() {
    let duplicate_relays = TargetSet::nostr_relays(
        [
            "wss://relay-a.example.com/path",
            "WSS://RELAY-A.EXAMPLE.COM/path",
        ],
        NostrRelayUrlPolicy::Public,
    )
    .expect_err("duplicate relays");

    assert!(matches!(
        duplicate_relays,
        RadrootsSdkError::Transport { ref message }
            if message == "transport target set contains duplicate fingerprints"
    ));

    let first = RadrootsTransportTarget::nostr_relay("wss://relay-a.example.com/path")
        .expect("first target");
    let second = RadrootsTransportTarget::nostr_relay("WSS://RELAY-A.EXAMPLE.COM/path")
        .expect("second target");
    let duplicate_targets =
        TargetSet::transport_targets(vec![first, second]).expect_err("duplicate targets");

    assert!(matches!(
        duplicate_targets,
        RadrootsSdkError::Transport { ref message }
            if message == "transport target set contains duplicate fingerprints"
    ));
}

#[test]
fn reticulum_profile_uses_canonical_endpoint_and_behavior_names() {
    let profile = ReticulumProfile::deferred_until_implemented();

    assert_eq!(profile.endpoint_uri(), RADROOTS_RETICULUM_ENDPOINT_URI);
    assert_eq!(
        profile.behavior(),
        ReticulumBehavior::RejectDeliveryAttempts
    );
    assert_eq!(
        ReticulumBehavior::RejectDeliveryAttempts.as_str(),
        "reject_delivery_attempts"
    );
    assert_eq!(
        ReticulumBehavior::DeferDeliveryPlans.as_str(),
        "defer_delivery_plans"
    );
    assert_eq!(
        serde_json::to_value(profile).expect("profile json"),
        serde_json::json!({
            "endpoint_uri": "reticulum:local",
            "scope": "local",
            "agent_endpoint": null,
            "behavior": "reject_delivery_attempts"
        })
    );
}

#[test]
fn reticulum_profile_preserves_explicit_scope_and_agent_endpoint() {
    let profile = ReticulumProfile::deferred_until_implemented()
        .with_scope(MeshScopeId::parse("farmers_market").expect("scope"))
        .with_agent_endpoint(
            ReticulumAgentEndpoint::parse("reticulum-agent:local").expect("agent endpoint"),
        );

    assert_eq!(profile.scope().as_str(), "farmers_market");
    assert_eq!(
        profile.agent_endpoint().expect("agent endpoint").as_str(),
        "reticulum-agent:local"
    );
    assert_eq!(
        serde_json::to_value(profile).expect("profile json"),
        serde_json::json!({
            "endpoint_uri": "reticulum:local",
            "scope": "farmers_market",
            "agent_endpoint": "reticulum-agent:local",
            "behavior": "reject_delivery_attempts"
        })
    );
}

#[test]
fn reticulum_agent_endpoint_rejects_non_agent_endpoint_families() {
    for invalid in [
        "",
        "reticulum-agent:",
        " reticulum-agent:local",
        "reticulum-agent:local ",
        "RETICULUM-AGENT:local",
        "reticulum:local",
        "https://reticulum.example.com",
        "ws://127.0.0.1:9735",
    ] {
        assert!(matches!(
            ReticulumAgentEndpoint::parse(invalid),
            Err(RadrootsSdkError::InvalidRequest { ref message })
                if message == "Reticulum agent endpoint is invalid"
        ));
    }
}

#[test]
fn explicit_target_sets_reject_noncanonical_reticulum_endpoints() {
    for invalid in [
        " reticulum:local".to_owned(),
        "reticulum:local ".to_owned(),
        "RETICULUM:deferred-until-implemented".to_owned(),
        "reticulum:Preview-Unavailable".to_owned(),
        ["reticulum:", "pre", "view"].concat(),
        "reticulum:local-alt".to_owned(),
        "reticulum:custom".to_owned(),
    ] {
        assert_eq!(
            RadrootsTransportTarget::new(RadrootsTransportKind::Reticulum, invalid.as_str())
                .expect_err("invalid Reticulum target"),
            RadrootsTransportError::InvalidTargetUri
        );
    }

    let uri = RadrootsTransportTargetUri::parse("reticulum:local-alt").expect("target uri");
    let fingerprint = RadrootsTransportTargetFingerprint::from_target(
        &RadrootsTransportKind::Reticulum,
        &uri,
        None,
    );
    let err = TargetSet::transport_targets(vec![RadrootsTransportTarget {
        kind: RadrootsTransportKind::Reticulum,
        uri,
        scope: None,
        label: None,
        fingerprint,
    }])
    .expect_err("noncanonical Reticulum endpoint");

    assert!(matches!(err, RadrootsSdkError::InvalidRequest { .. }));
}

#[test]
fn normalized_relays_reject_empty_and_over_limit_sets() {
    assert!(matches!(
        TargetSet::nostr_relays(Vec::<String>::new(), NostrRelayUrlPolicy::Public),
        Err(RadrootsSdkError::EmptyTransportTargets { .. })
    ));

    let too_many = (0..=SDK_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example.com"))
        .collect::<Vec<_>>();
    assert!(matches!(
        TargetSet::nostr_relays(too_many, NostrRelayUrlPolicy::Public),
        Err(RadrootsSdkError::TransportTargetLimitExceeded { actual, .. })
            if actual == SDK_TRANSPORT_TARGET_MAX_COUNT + 1
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
        TargetSet::nostr_relays(["ws://relay.example.com"], NostrRelayUrlPolicy::Localhost),
        Err(RadrootsSdkError::InvalidRelayUrl { reason, .. })
            if reason.contains("localhost")
    ));
    assert!(matches!(
        TargetSet::nostr_relays(["ws://relay.example.com"], NostrRelayUrlPolicy::Public),
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
