use super::{
    NostrProfile, NostrRelayUrlPolicy, PublishMode, ReticulumPreviewBehavior,
    ReticulumPreviewProfile, SatisfactionPolicy, TargetPolicy, TargetSet, TransportProfile,
};
use crate::{RadrootsSdkError, SDK_TRANSPORT_TARGET_MAX_COUNT};

#[path = "../support/serializer_failure.rs"]
mod serializer_failure;

use serializer_failure::assert_struct_serialize_error_paths;

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
        serde_json::to_value(SatisfactionPolicy::AtLeastOneTarget).expect("json"),
        serde_json::json!("at_least_one_target")
    );
    assert_eq!(
        serde_json::to_value(SatisfactionPolicy::AllTargets).expect("json"),
        serde_json::json!("all_targets")
    );
    assert_eq!(
        serde_json::to_value(SatisfactionPolicy::at_least(2).expect("satisfaction policy"))
            .expect("json"),
        serde_json::json!({ "at_least": { "required": 2 } })
    );
    assert!(matches!(
        SatisfactionPolicy::at_least(0),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message == "satisfaction policy must require at least one target"
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
fn use_configured_policy_serializes_as_kind_only() {
    let policy = TargetPolicy::UseConfiguredProfile;
    assert_eq!(
        serde_json::to_value(&policy).expect("json"),
        serde_json::json!({ "kind": "use_configured_profile" })
    );
    assert_struct_serialize_error_paths(&policy, 1);

    let transport_profile_policy = TargetPolicy::use_transport_profile();
    assert_eq!(
        serde_json::to_value(&transport_profile_policy).expect("json"),
        serde_json::json!({ "kind": "use_transport_profile" })
    );
    assert_struct_serialize_error_paths(&transport_profile_policy, 1);
}

#[test]
fn target_set_accessors_and_configured_relays_cover_empty_and_dedupe_paths() {
    assert!(
        TransportProfile::local_only()
            .configured_nostr_relay_urls()
            .is_empty()
    );

    let targets = TargetSet::from_normalized_nostr_relays(vec![
        "wss://relay-a.example.com".to_owned(),
        "wss://relay-a.example.com".to_owned(),
        "wss://relay-b.example.com".to_owned(),
    ])
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
            TargetSet::new(["wss://relay-c.example.com"], NostrRelayUrlPolicy::Public)
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
                "kind": "Nostr",
                "uri": "wss://relay-c.example.com",
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
fn reticulum_preview_profile_uses_canonical_endpoint_and_behavior_names() {
    let profile = ReticulumPreviewProfile::preview_unavailable();

    assert_eq!(profile.endpoint_uri(), "reticulum:preview-unavailable");
    assert_eq!(
        profile.behavior(),
        ReticulumPreviewBehavior::RejectDeliveryAttempts
    );
    assert_eq!(
        ReticulumPreviewBehavior::RejectDeliveryAttempts.as_str(),
        "reject_delivery_attempts"
    );
    assert_eq!(
        ReticulumPreviewBehavior::DeferDeliveryPlans.as_str(),
        "defer_delivery_plans"
    );
    assert_eq!(
        serde_json::to_value(profile).expect("profile json"),
        serde_json::json!({
            "endpoint_uri": "reticulum:preview-unavailable",
            "behavior": "reject_delivery_attempts"
        })
    );
}

#[test]
fn normalized_relays_reject_empty_and_over_limit_sets() {
    assert!(matches!(
        TargetSet::from_normalized_nostr_relays(Vec::new()),
        Err(RadrootsSdkError::EmptyTransportTargets { .. })
    ));

    let too_many = (0..=SDK_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example.com"))
        .collect::<Vec<_>>();
    assert!(matches!(
        TargetSet::from_normalized_nostr_relays(too_many),
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
        TargetSet::new(["ws://relay.example.com"], NostrRelayUrlPolicy::Localhost),
        Err(RadrootsSdkError::InvalidRelayUrl { reason, .. })
            if reason.contains("localhost")
    ));
    assert!(matches!(
        TargetSet::new(["ws://relay.example.com"], NostrRelayUrlPolicy::Public),
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
