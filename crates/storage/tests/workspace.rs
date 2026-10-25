use radroots_domain::{
    ActorRef, ActorRole, AuthorityRef, CheckpointDigest, CheckpointRef, Coverage, ExecutorId,
    Generation, GrantId, PublicKey, RouteDigest, Sequence, UnixSeconds, WorkspaceId,
};
use radroots_storage::workspace::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceError,
};
#[cfg(feature = "protected_outbox")]
#[path = "workspace/inventory_abort.rs"]
mod inventory_abort;
fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn input() -> AttachmentInput {
    AttachmentInput::new(
        WorkspaceId::from_bytes([1; 32]),
        AuthorityRef::new(ExecutorId::from_bytes([2; 32]), Generation::new(1).unwrap()),
        key(),
        "local://workspace/selector",
        key(),
        GrantId::from_bytes([3; 32]),
        CheckpointRef::new(
            CheckpointDigest::from_bytes([4; 32]),
            Sequence::new(0).unwrap(),
            Coverage::Partial,
            vec![],
        )
        .unwrap(),
        RouteDigest::from_bytes([5; 32]),
    )
    .unwrap()
}

#[cfg(feature = "protected_outbox")]
#[test]
fn declared_stock_grants_are_explicit_bounded_and_never_expand_empty_or_reordered_scopes() {
    use radroots_domain::PoolId;
    use radroots_storage::workspace::seller_stock::{
        StockAction::*, StockGrantScope, StockResources,
    };
    let first = PoolId::from_bytes([1; 32]);
    let second = PoolId::from_bytes([2; 32]);
    assert!(StockResources::pools(vec![]).is_err());
    assert!(StockResources::pools(vec![first, first]).is_err());
    assert!(StockResources::pools(vec![second, first]).is_err());
    assert!(StockResources::pools((0..65).map(|n| PoolId::from_bytes([n; 32])).collect()).is_err());
    assert!(StockGrantScope::new(StockResources::all_pools(), vec![]).is_err());
    for actions in [
        vec![AddStock],
        vec![ReadPools, AddStock],
        vec![ReadOperations, AddStock],
        vec![ReadOperations, ReadPools],
        vec![ReadPools, ReadPools],
    ] {
        assert!(StockGrantScope::new(StockResources::all_pools(), actions).is_err());
    }
    let scope = StockGrantScope::new(
        StockResources::pools(vec![first]).unwrap(),
        vec![ReadPools, ReadOperations, AddStock],
    )
    .unwrap();
    assert!(scope.allows(AddStock, first));
    assert!(!scope.allows(AddStock, second));
    assert!(!scope.allows(RemoveStock, first));
    assert!(!scope.allows(SpoilStock, first));
    assert_eq!(scope.actions(), vec![ReadPools, ReadOperations, AddStock]);
    assert_eq!(scope.resources().selected_pools(), Some(&[first][..]));
    let readonly =
        StockGrantScope::new(StockResources::pools(vec![first]).unwrap(), vec![ReadPools]).unwrap();
    assert!(readonly.allows(ReadPools, first));
    assert!(!readonly.allows(ReadOperations, first));
    assert!(!readonly.allows(AddStock, first));
    let full = StockGrantScope::full_inventory();
    assert_eq!(
        full.actions(),
        vec![
            ReadPools,
            ReadOperations,
            CreatePool,
            AddStock,
            RemoveStock,
            SpoilStock
        ]
    );
    assert!(full.allows(SpoilStock, second));
    assert_eq!(
        format!("{scope:?}"),
        "StockGrantScope(<untrusted descriptor>)"
    );
    assert_eq!(
        format!("{:?}", scope.resources()),
        "StockResources(<redacted>)"
    );
}

#[cfg(all(feature = "protected_outbox", feature = "serde"))]
#[test]
fn closed_stock_scope_codecs_reject_aliases_extras_and_implicit_wildcards() {
    use radroots_storage::workspace::seller_stock::StockGrantScope;
    use serde_json::json;
    let valid = json!({"actions":["read_pools","read_operations","add_stock"],"resources":{"type":"pools","ids":["01".repeat(32)]}});
    let scope: StockGrantScope = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(serde_json::to_value(scope).unwrap(), valid);
    for invalid in [
        json!({"actions":[],"resources":{"type":"all_pools"}}),
        json!({"actions":["add_stock"],"resources":{"type":"all_pools"}}),
        json!({"actions":["read_pools"],"resources":{"type":"pools","ids":[]}}),
        json!({"actions":["read_pools"],"resources":{"type":"all_pools","ids":null}}),
        json!({"actions":["read_pools"],"resources":{"type":"all_pools","ids":[]}}),
        json!({"actions":["read_pools"],"resources":{"type":"all_pools","ignored":true}}),
        json!({"actions":[{"read_pools":null}],"resources":{"type":"all_pools"}}),
        json!({"actions":["admin"],"resources":{"type":"all_pools"}}),
        json!({"actions":["read_pools"],"resources":{"type":"all_pools"},"admin":true}),
        json!([{"type":"all_pools"},["read_pools"]]),
        json!({"actions":vec!["read_pools";7],"resources":{"type":"all_pools"}}),
    ] {
        assert!(serde_json::from_value::<StockGrantScope>(invalid).is_err());
    }
    let mut excessive = valid;
    excessive["resources"]["ids"] = json!(
        (0..65)
            .map(|n| format!("{n:02x}").repeat(32))
            .collect::<Vec<_>>()
    );
    assert!(serde_json::from_value::<StockGrantScope>(excessive).is_err());
}

#[cfg(feature = "protected_outbox")]
#[test]
fn seller_namespace_selection_excludes_only_the_caller_grant_without_minting_permission() {
    use radroots_storage::workspace::seller_stock::{SellerAuthoritySelection, SellerStockConfig};
    let (original, proof, record) = SellerStockConfig::new(
        key(),
        key(),
        "namespace-owner",
        UnixSeconds::new(200).unwrap(),
    )
    .unwrap()
    .enroll(
        UnixSeconds::new(100).unwrap(),
        WorkspaceId::from_bytes([1; 32]),
        ExecutorId::from_bytes([2; 32]),
        GrantId::from_bytes([3; 32]),
        AttachmentProof::from_protected_input(&[7; 32]).unwrap(),
    )
    .unwrap();
    let selected = SellerAuthoritySelection::from_attachment(&original);
    let caller = selected.attachment(GrantId::from_bytes([9; 32])).unwrap();
    assert_ne!(caller.grant_id(), original.grant_id());
    assert_eq!(SellerAuthoritySelection::from_attachment(&caller), selected);
    assert!(selected.matches_attachment(&original));
    assert!(selected.matches_attachment(&caller));
    assert!(
        record
            .authenticate(&original, &proof, UnixSeconds::new(100).unwrap())
            .is_ok()
    );
    assert!(
        record
            .authenticate(&caller, &proof, UnixSeconds::new(100).unwrap())
            .is_err()
    );
    assert_eq!(selected.workspace_id(), original.workspace_id());
    assert_eq!(selected.authority(), original.authority());
    assert_eq!(selected.participant(), original.participant());
    assert_eq!(selected.endpoint(), original.endpoint());
    assert_eq!(selected.endpoint_identity(), original.endpoint_identity());
    assert_eq!(selected.checkpoint(), original.checkpoint());
    assert_eq!(selected.route_digest(), original.route_digest());
    assert_eq!(
        format!("{selected:?}"),
        "SellerAuthoritySelection(<untrusted descriptor>)"
    );
}

#[cfg(all(feature = "protected_outbox", feature = "serde"))]
#[test]
fn seller_namespace_selection_is_a_closed_object_and_retains_every_namespace_binding() {
    use radroots_storage::workspace::seller_stock::SellerAuthoritySelection;
    use serde_json::json;
    let original = input();
    let selection = SellerAuthoritySelection::from_attachment(&original);
    let value = serde_json::to_value(&selection).unwrap();
    assert!(value.get("grant_id").is_none());
    assert_eq!(
        serde_json::from_value::<SellerAuthoritySelection>(value.clone()).unwrap(),
        selection
    );
    for field in [
        "workspace_id",
        "authority",
        "participant",
        "endpoint",
        "endpoint_identity",
        "checkpoint",
        "route_digest",
    ] {
        let mut changed = value.clone();
        changed.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<SellerAuthoritySelection>(changed).is_err(),
            "{field}"
        );
    }
    let other_key = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    for (path, bad) in [
        ("/workspace_id", json!("0b".repeat(32))),
        ("/authority/executor_id", json!("0b".repeat(32))),
        ("/authority/generation", json!("2")),
        ("/participant", json!(other_key)),
        ("/endpoint", json!("local://another-namespace")),
        ("/endpoint_identity", json!(other_key)),
        ("/checkpoint/digest", json!("0b".repeat(32))),
        ("/route_digest", json!("0b".repeat(32))),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        let changed: SellerAuthoritySelection = serde_json::from_value(changed).unwrap();
        assert!(!changed.matches_attachment(&original), "{path}");
    }
    for bad in [json!(""), json!("x".repeat(2049)), json!("local://bad\n")] {
        let mut changed = value.clone();
        changed["endpoint"] = bad;
        assert!(serde_json::from_value::<SellerAuthoritySelection>(changed).is_err());
    }
    let mut changed = value.clone();
    changed["grant_id"] = json!(null);
    for bad in [
        json!(null),
        json!({"id":"radroots.seller_stock.native_inventory","version":"2"}),
        json!({"id":"radroots.seller_stock.native_inventory","version":1}),
        json!(["radroots.seller_stock.native_inventory", "1"]),
    ] {
        let mut invalid = value.clone();
        invalid["capability_contract"] = bad;
        assert!(serde_json::from_value::<SellerAuthoritySelection>(invalid).is_err());
    }

    assert!(serde_json::from_value::<SellerAuthoritySelection>(changed).is_err());
    assert!(
        serde_json::from_value::<SellerAuthoritySelection>(json!([
            value["workspace_id"],
            value["authority"]
        ]))
        .is_err()
    );
}
fn actor() -> ActorRef {
    ActorRef::new(key(), ActorRole::Buyer, None, Some(input().grant_id()))
}

#[test]
fn proof_has_exact_width_explicit_exposure_and_redacted_diagnostics() {
    for bytes in [&[][..], &[7; 31][..], &[7; 33][..]] {
        assert_eq!(
            AttachmentProof::from_protected_input(bytes).unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
    let proof = AttachmentProof::from_protected_input(&[7; 32]).unwrap();
    proof.with_secret(|bytes| assert_eq!(bytes, &[7; 32]));
    assert_eq!(format!("{proof:?}"), "AttachmentProof(<redacted>)");
}
#[test]
fn selections_are_bounded_untrusted_data_with_complete_getters() {
    let input = input();
    assert_eq!(input.workspace_id(), WorkspaceId::from_bytes([1; 32]));
    assert_eq!(
        input.authority().executor_id(),
        ExecutorId::from_bytes([2; 32])
    );
    assert_eq!(input.participant(), key());
    assert_eq!(input.endpoint_identity(), key());
    assert_eq!(input.endpoint(), "local://workspace/selector");
    assert_eq!(input.grant_id(), GrantId::from_bytes([3; 32]));
    assert_eq!(input.checkpoint().sequence().get(), 0);
    assert_eq!(input.route_digest(), RouteDigest::from_bytes([5; 32]));
    for endpoint in [String::new(), "x".repeat(2049), "local://bad\n".into()] {
        assert_eq!(
            AttachmentInput::new(
                input.workspace_id(),
                input.authority(),
                input.participant(),
                endpoint,
                input.endpoint_identity(),
                input.grant_id(),
                input.checkpoint().clone(),
                input.route_digest()
            )
            .unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
    assert_eq!(
        format!("{input:?}"),
        "AttachmentInput(<untrusted descriptor>)"
    );
}
#[test]
fn host_observations_validate_internal_binding_without_authenticating_metadata() {
    let binding = |actor, principal: &str, revision, expiry, observed| {
        WorkspaceBinding::from_authenticated_host(
            input(),
            radroots_storage::workspace::WorkspaceScope::NewBuyerOutbox,
            actor,
            principal,
            Sequence::new(revision).unwrap(),
            UnixSeconds::new(expiry).unwrap(),
            UnixSeconds::new(observed).unwrap(),
        )
    };
    let observed = binding(actor(), "device", 1, 200, 100).unwrap();
    assert_eq!(observed.input(), &input());
    assert_eq!(
        observed.scope(),
        radroots_storage::workspace::WorkspaceScope::NewBuyerOutbox
    );
    assert_eq!(observed.actor(), actor());
    assert_eq!(observed.principal_id(), "device");
    assert_eq!(observed.grant_revision().get(), 1);
    assert_eq!(observed.expires_at().get(), 200);
    assert_eq!(observed.observed_at().get(), 100);
    assert_eq!(format!("{observed:?}"), "WorkspaceBinding(<redacted>)");
    for principal in [String::new(), "x".repeat(129), "device\n".into()] {
        assert_eq!(
            binding(actor(), principal.as_str(), 1, 200, 100).unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
    for (revision, expiry, observed) in [
        (0, 200, 100),
        (1, 0, 100),
        (1, 100, 0),
        (1, 100, 100),
        (1, 99, 100),
    ] {
        assert_eq!(
            binding(actor(), "device", revision, expiry, observed).unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
    let other_key =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    for invalid in [
        ActorRef::new(key(), ActorRole::Seller, None, Some(input().grant_id())),
        ActorRef::new(
            key(),
            ActorRole::Coordinator,
            None,
            Some(input().grant_id()),
        ),
        ActorRef::new(
            key(),
            ActorRole::Buyer,
            Some(key()),
            Some(input().grant_id()),
        ),
        ActorRef::new(other_key, ActorRole::Buyer, None, Some(input().grant_id())),
        ActorRef::new(key(), ActorRole::Buyer, None, None),
        ActorRef::new(
            key(),
            ActorRole::Buyer,
            None,
            Some(GrantId::from_bytes([9; 32])),
        ),
    ] {
        assert_eq!(
            binding(invalid, "device", 1, 200, 100).unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
}
#[cfg(feature = "serde")]
#[test]
fn selection_json_rejects_missing_duplicate_unknown_and_invalid_data() {
    let input = input();
    let encoded = serde_json::to_string(&input).unwrap();
    assert_eq!(
        serde_json::from_str::<AttachmentInput>(&encoded).unwrap(),
        input
    );
    let value = serde_json::to_value(&input).unwrap();
    for field in value.as_object().unwrap().keys() {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<AttachmentInput>(missing).is_err());
        let duplicate = format!("{{\"{field}\":{},{}", value[field], &encoded[1..]);
        assert!(serde_json::from_str::<AttachmentInput>(&duplicate).is_err());
    }
    let mut unknown = value.clone();
    unknown["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<AttachmentInput>(unknown).is_err());
    for invalid in ["", "bad\n"] {
        let mut data = value.clone();
        data["endpoint"] = serde_json::json!(invalid);
        assert!(serde_json::from_value::<AttachmentInput>(data).is_err());
    }
}
#[test]
fn workspace_failures_have_fixed_bounded_codes_and_no_sources() {
    use WorkspaceError::*;
    for error in [
        InvalidInput,
        Unavailable,
        NamespaceUnavailable,
        WriterActive,
        WrongNamespace,
        RecoveryRequired,
        Unauthorized,
        GrantExpired,
        GrantRevoked,
        StaleAuthority,
        StaleCheckpoint,
        Closed,
        StorageFailure,
        CommitUnknown,
    ] {
        assert_eq!(error.to_string(), error.code());
        assert!(error.code().len() <= 32);
        let source: &dyn core::error::Error = &error;
        assert!(source.source().is_none());
    }
}
