#![cfg(feature = "listing")]
use radroots_domain::{
    ActorRef, ActorRole, AuthorityRef, CommandContextData, ExecutorId, Generation, GrantId,
    OperationId, PublicKey, RouteDigest, ThreadId, UnixSeconds, WorkspaceId,
};
use radroots_protocol::{
    command::{CapturedChatCommand, CapturedCommand, CommandError, InquireInput},
    listing::{CapturedListingCommand, PublishInput},
};
use serde_json::{Value, json};
fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn context() -> CommandContextData {
    CommandContextData::new(
        OperationId::from_bytes([7; 32]),
        ActorRef::new(
            key(),
            ActorRole::Seller,
            None,
            Some(GrantId::from_bytes([3; 32])),
        ),
        WorkspaceId::from_bytes([1; 32]),
        AuthorityRef::new(ExecutorId::from_bytes([2; 32]), Generation::new(1).unwrap()),
        RouteDigest::from_bytes([5; 32]),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap()
}
fn input() -> Value {
    json!({"id":"01".repeat(16),"title":"Carrots","summary":"One bunch; quote required","description_markdown":"Public terms","published_at":"100","profile":{"profile":"radroots.listing","version":1,"state":"active","product":{"name":"Carrots","description":"Public description","producer":null,"origin_summary":null},"default_option":"02".repeat(32),"options":[{"id":"02".repeat(32),"label":"One bunch","unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},"increment":"1","minimum":"1","price":{"type":"quote_required"},"supply":{"type":"available","amount":"10"},"lead_seconds":"0"}],"fulfillment":{"modes":["pickup"],"area":"Example region","lead_seconds":"0"},"authority":null,"media":[]}})
}

#[test]
fn both_families_round_trip_without_rehashing_or_fabricating_chat_data() {
    let chat = CapturedChatCommand::inquire(
        context(),
        InquireInput::new(
            ThreadId::from_bytes([6; 32]),
            key(),
            None,
            "Original private text",
        )
        .unwrap(),
    )
    .unwrap();
    let wire = chat.protected_wire().to_vec();
    let digest = chat.digest();
    let intent = chat.approval_intent_digest();
    let common = CapturedCommand::from(chat);
    assert_eq!(common.protected_wire(), wire);
    assert_eq!(common.digest(), digest);
    assert_eq!(common.approval_intent_digest(), intent);
    assert!(common.listing().is_none());
    let replay = CapturedCommand::parse(&wire).unwrap();
    assert_eq!(replay.protected_wire(), wire);
    assert_eq!(replay.chat().unwrap().text(), "Original private text");
    let public = CapturedListingCommand::capture(
        context(),
        PublishInput::parse(&serde_json::to_vec(&input()).unwrap()).unwrap(),
    )
    .unwrap();
    let common = CapturedCommand::from(public);
    let replay = CapturedCommand::parse(common.protected_wire()).unwrap();
    assert_eq!(replay.digest(), common.digest());
    assert_eq!(replay.protected_wire(), common.protected_wire());
    assert_eq!(replay.kind().as_str(), "listing.publish");
    assert!(replay.chat().is_none());
    assert!(!replay.requires_history());
    assert_eq!(
        replay.listing().unwrap().input().id().to_string(),
        "01".repeat(16)
    );
    assert!(!format!("{replay:?}").contains("Carrots"));
}

#[test]
fn diagnostic_correlation_and_approval_projections_reuse_the_common_context_contract() {
    let public = CapturedListingCommand::capture(
        context(),
        PublishInput::parse(&serde_json::to_vec(&input()).unwrap()).unwrap(),
    )
    .unwrap();
    let mut wire: Value = serde_json::from_slice(public.protected_wire()).unwrap();
    wire["context"]["correlation_id"] = json!("diagnostic-two");
    let diagnostic = CapturedListingCommand::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert_eq!(diagnostic.digest(), public.digest());
    assert_eq!(
        diagnostic.approval_intent_digest(),
        public.approval_intent_digest()
    );
    assert_ne!(diagnostic.protected_wire(), public.protected_wire());
    wire["context"]["approval"] = json!({"intent_digest":"11".repeat(32),"scope":"synthetic approval metadata","expires_at":"140"});
    let approved = CapturedListingCommand::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert_ne!(approved.digest(), public.digest());
    assert_eq!(
        approved.approval_intent_digest(),
        public.approval_intent_digest()
    );
}

#[test]
fn closed_input_refuses_unknown_duplicate_missing_nullable_and_invalid_value_paths() {
    let original = input();
    let bytes = serde_json::to_vec(&original).unwrap();
    PublishInput::parse(&bytes).unwrap();
    for pointer in [
        "/profile/authority",
        "/profile/product/producer",
        "/profile/product/origin_summary",
    ] {
        let mut value = original.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            PublishInput::parse(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{pointer}"
        );
    }
    for (pointer, bad) in [
        ("/profile/options/0/minimum", json!("1.5")),
        ("/profile/options/0/supply/amount", json!("0.5")),
        ("/profile/default_option", json!("03".repeat(32))),
        ("/profile/fulfillment/modes", json!(["pickup", "pickup"])),
        ("/published_at", json!(100)),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(pointer).unwrap() = bad;
        assert!(
            PublishInput::parse(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{pointer}"
        );
    }
    let mut unknown = original.clone();
    unknown["profile"]["options"][0]["pool_id"] = json!("private inventory selector");
    assert!(PublishInput::parse(&serde_json::to_vec(&unknown).unwrap()).is_err());
    let raw = String::from_utf8(bytes).unwrap();
    let duplicate = raw.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
    assert!(PublishInput::parse(duplicate.as_bytes()).is_err());
    assert_eq!(
        CapturedCommand::parse(&vec![b' '; 65537]).unwrap_err(),
        CommandError::LimitExceeded
    );
}

#[test]
fn unsupported_full_profile_features_are_refused_without_silent_quote_or_media_fallback() {
    for (pointer, bad) in [
        ("/profile/version", json!(2)),
        ("/profile/profile", json!("other.profile")),
        (
            "/profile/media",
            json!([{"url":"https://example.invalid/image"}]),
        ),
        (
            "/profile/options/0/price",
            json!({"type":"fixed","rate":{"currency":"CAD","minor":"325","exponent":2}}),
        ),
        (
            "/profile/options/0/supply",
            json!({"type":"estimated","amount":"10","window":null}),
        ),
        (
            "/profile/options/0/unit/contents",
            json!({"type":"fixed","quantity":{"amount":"1","unit":"kg"}}),
        ),
    ] {
        let mut value = input();
        *value.pointer_mut(pointer).unwrap() = bad;
        assert_eq!(
            PublishInput::parse(&serde_json::to_vec(&value).unwrap()).unwrap_err(),
            CommandError::UnsupportedProfile,
            "{pointer}"
        );
    }
}

#[test]
fn exact_public_plan_reconstructs_original_event_and_refuses_changed_framing_or_command() {
    use radroots_protocol::{
        listing::PublicSigningPlan,
        plan::{CapturedSigningPlan, SigningReplay},
    };
    let public = CapturedListingCommand::capture(
        context(),
        PublishInput::parse(&serde_json::to_vec(&input()).unwrap()).unwrap(),
    )
    .unwrap();
    let targets = vec![
        "wss://listing.fixture.example/a".to_string(),
        "wss://listing.fixture.example/b".to_string(),
    ];
    let plan = PublicSigningPlan::capture(
        &public,
        UnixSeconds::new(100).unwrap(),
        &targets,
        SigningReplay::NonReplayable,
    )
    .unwrap();
    let wire = plan.protected_wire().to_vec();
    let digest = *plan.digest();
    let event_id = *plan.event().event_id();
    assert_eq!(
        plan.selected_targets()
            .iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>(),
        targets
    );
    let common = CapturedCommand::from(public);
    let retained = CapturedSigningPlan::parse(&common, &wire).unwrap();
    assert_eq!(retained.protected_wire(), wire);
    assert_eq!(retained.digest(), &digest);
    assert_eq!(retained.listing().unwrap().event().event_id(), &event_id);
    #[cfg(feature = "chat")]
    assert!(retained.chat().is_none());
    assert!(!format!("{retained:?}").contains("Carrots"));
    let changed = PublicSigningPlan::capture(
        common.listing().unwrap(),
        UnixSeconds::new(101).unwrap(),
        &targets,
        SigningReplay::NonReplayable,
    )
    .unwrap();
    assert_ne!(changed.digest(), &digest);
    for replay in [
        SigningReplay::LocalReplaySafe,
        SigningReplay::ExactReplayByRequestId,
    ] {
        let changed = PublicSigningPlan::capture(
            common.listing().unwrap(),
            UnixSeconds::new(100).unwrap(),
            &targets,
            replay,
        )
        .unwrap();
        assert_ne!(changed.digest(), &digest);
        assert_eq!(
            PublicSigningPlan::parse(common.listing().unwrap(), changed.protected_wire())
                .unwrap()
                .replay(),
            replay
        );
    }
    let prefix = b"radroots.listing.signing_plan.v1\0".len();
    for offset in [
        0,
        prefix,
        prefix + 32,
        prefix + 64 + 8,
        prefix + 64 + 9,
        prefix + 64 + 10,
    ] {
        let mut changed = wire.clone();
        changed[offset] ^= 0x80;
        assert!(CapturedSigningPlan::parse(&common, &changed).is_err());
    }
    let mut changed = wire.clone();
    let offset = changed.windows(7).position(|w| w == b"Carrots").unwrap();
    changed[offset] = b'P';
    assert!(CapturedSigningPlan::parse(&common, &changed).is_err());
    let mut changed = wire.clone();
    changed.push(0);
    assert!(CapturedSigningPlan::parse(&common, &changed).is_err());
    for end in [0, 1, prefix, prefix + 32, wire.len() - 1] {
        assert!(CapturedSigningPlan::parse(&common, &wire[..end]).is_err());
    }
    let chat = CapturedCommand::from(
        CapturedChatCommand::inquire(
            context(),
            InquireInput::new(ThreadId::from_bytes([6; 32]), key(), None, "Private").unwrap(),
        )
        .unwrap(),
    );
    assert!(CapturedSigningPlan::parse(&chat, &wire).is_err());
    assert!(
        PublicSigningPlan::capture(
            common.listing().unwrap(),
            UnixSeconds::new(100).unwrap(),
            &[],
            SigningReplay::NonReplayable
        )
        .is_err()
    );
    assert!(
        PublicSigningPlan::capture(
            common.listing().unwrap(),
            UnixSeconds::new(100).unwrap(),
            &vec![targets[0].clone(); 2],
            SigningReplay::NonReplayable
        )
        .is_err()
    );
}

#[test]
fn artifact_selection_has_no_public_participant_and_preserves_private_keys() {
    use radroots_protocol::plan::SigningTarget;
    let public = SigningTarget::PublicEvent;
    assert_eq!(public.recipient(), None);
    assert_eq!(public.key(), [0]);
    assert_eq!(SigningTarget::from_key(&[0]).unwrap(), public);
    let key = radroots_domain::PublicKey::from_hex(
        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    )
    .unwrap();
    let private = SigningTarget::BusinessSeal(key);
    assert_eq!(private.recipient(), Some(key));
    assert_eq!(private.key(), key.as_bytes());
    assert_eq!(SigningTarget::from_key(key.as_bytes()).unwrap(), private);
    for invalid in [&[][..], &[1][..], &[0, 0][..], &[1; 31][..], &[1; 33][..]] {
        assert!(SigningTarget::from_key(invalid).is_err());
    }
}
