#![cfg(feature = "protected_outbox")]
use radroots_domain::{
    ActorRef, ActorRole, ExecutorId, GrantId, PublicKey, UnixSeconds, WorkspaceId,
};
use radroots_storage::workspace::{
    AttachmentProof, WorkspaceError, WorkspaceScope, grant::WorkspaceGrant,
    local_outbox::LocalOutboxConfig,
};

fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn enrolled(
    seller: bool,
) -> (
    radroots_storage::workspace::AttachmentInput,
    AttachmentProof,
    WorkspaceGrant,
) {
    let constructor = if seller {
        LocalOutboxConfig::seller
    } else {
        LocalOutboxConfig::new
    };
    constructor(
        key(),
        key(),
        "synthetic-device",
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
    .unwrap()
}

#[test]
fn enrollment_separates_scope_routes_genesis_and_credential_commitments() {
    let (buyer, _, buyer_grant) = enrolled(false);
    let (seller, _, seller_grant) = enrolled(true);
    assert_ne!(buyer.route_digest(), seller.route_digest());
    assert_ne!(buyer.checkpoint(), seller.checkpoint());
    let buyer: serde_json::Value = serde_json::from_slice(&buyer_grant.encode().unwrap()).unwrap();
    let seller: serde_json::Value =
        serde_json::from_slice(&seller_grant.encode().unwrap()).unwrap();
    assert_eq!(buyer["scope"], "new_buyer_outbox");
    assert_eq!(seller["scope"], "new_seller_outbox");
    assert_ne!(buyer["verifier"], seller["verifier"]);
}

#[test]
fn ordinary_outbox_admission_refuses_structurally_valid_stock_owner_metadata() {
    let (_, _, grant) = enrolled(true);
    let original = grant.encode().unwrap();
    let mut wire: serde_json::Value = serde_json::from_slice(&original).unwrap();
    wire["scope"] = serde_json::json!("seller_stock_owner");
    let encoded = serde_json::to_vec(&wire).unwrap();
    assert!(WorkspaceGrant::decode(&encoded).is_ok());
    assert!(matches!(
        WorkspaceGrant::decode_outbox(&encoded),
        Err(WorkspaceError::RecoveryRequired)
    ));
    assert!(WorkspaceGrant::decode_outbox(&original).is_ok());
}

#[test]
fn both_scopes_retain_protected_access_expiry_revocation_and_exact_role() {
    for (seller, scope, role) in [
        (false, WorkspaceScope::NewBuyerOutbox, ActorRole::Buyer),
        (true, WorkspaceScope::NewSellerOutbox, ActorRole::Seller),
    ] {
        let (input, proof, grant) = enrolled(seller);
        let mut grant = WorkspaceGrant::decode(&grant.encode().unwrap()).unwrap();
        let bound = grant
            .authenticate(&input, &proof, UnixSeconds::new(100).unwrap())
            .unwrap();
        assert_eq!(bound.scope(), scope);
        assert_eq!(bound.actor().role(), role);
        assert_eq!(bound.actor().organization(), None);
        assert_eq!(
            bound.input().checkpoint().coverage(),
            radroots_domain::Coverage::Partial
        );
        let wrong = AttachmentProof::from_protected_input(&[8; 32]).unwrap();
        assert_eq!(
            grant
                .authenticate(&input, &wrong, UnixSeconds::new(100).unwrap())
                .unwrap_err(),
            WorkspaceError::Unauthorized
        );
        assert_eq!(
            grant
                .authenticate(&input, &proof, UnixSeconds::new(0).unwrap())
                .unwrap_err(),
            WorkspaceError::Unavailable
        );
        assert_eq!(
            grant
                .authenticate(&input, &proof, UnixSeconds::new(200).unwrap())
                .unwrap_err(),
            WorkspaceError::GrantExpired
        );
        grant.revoke().unwrap();
        let grant = WorkspaceGrant::decode(&grant.encode().unwrap()).unwrap();
        assert_eq!(
            grant
                .authenticate(&input, &proof, UnixSeconds::new(100).unwrap())
                .unwrap_err(),
            WorkspaceError::GrantRevoked
        );
        // Capture is limited to the original credential and never renews access.
        grant.authenticate_retained_capture(&input, &proof).unwrap();
    }
}

#[test]
fn changing_scope_and_role_cannot_upgrade_an_existing_protected_credential() {
    for seller in [false, true] {
        let (input, proof, grant) = enrolled(seller);
        let original: serde_json::Value = serde_json::from_slice(&grant.encode().unwrap()).unwrap();
        let mut changed = original.clone();
        changed["actor"]["role"] = serde_json::json!(if seller { "buyer" } else { "seller" });
        assert!(matches!(
            WorkspaceGrant::decode(&serde_json::to_vec(&changed).unwrap()),
            Err(WorkspaceError::RecoveryRequired)
        ));
        changed["scope"] = serde_json::json!(if seller {
            "new_buyer_outbox"
        } else {
            "new_seller_outbox"
        });
        let changed = WorkspaceGrant::decode(&serde_json::to_vec(&changed).unwrap()).unwrap();
        assert_eq!(
            changed
                .authenticate(&input, &proof, UnixSeconds::new(100).unwrap())
                .unwrap_err(),
            WorkspaceError::Unauthorized
        );
        for scope in ["seller_stock", "remote_executor", "organization", "unknown"] {
            let mut changed = original.clone();
            changed["scope"] = serde_json::json!(scope);
            assert!(WorkspaceGrant::decode(&serde_json::to_vec(&changed).unwrap()).is_err());
        }
        // The original buyer constructor never accepts a seller actor.
        assert!(
            WorkspaceGrant::new(
                input.clone(),
                ActorRef::new(key(), ActorRole::Seller, None, Some(input.grant_id())),
                "device".into(),
                UnixSeconds::new(200).unwrap(),
                &proof
            )
            .is_err()
        );
    }
}
