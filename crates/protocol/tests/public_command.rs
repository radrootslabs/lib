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
fn listing_objects_reject_positional_arrays_at_every_struct_boundary() {
    let mut accepted = Vec::new();
    for (pointer, fields) in [
        (
            "",
            &[
                "id",
                "title",
                "summary",
                "description_markdown",
                "published_at",
                "profile",
            ][..],
        ),
        (
            "/profile",
            &[
                "profile",
                "version",
                "state",
                "product",
                "default_option",
                "options",
                "fulfillment",
                "authority",
                "media",
            ][..],
        ),
        (
            "/profile/product",
            &["name", "description", "producer", "origin_summary"][..],
        ),
        (
            "/profile/options/0",
            &[
                "id",
                "label",
                "unit",
                "increment",
                "minimum",
                "price",
                "supply",
                "lead_seconds",
            ][..],
        ),
        (
            "/profile/fulfillment",
            &["modes", "area", "lead_seconds"][..],
        ),
    ] {
        let mut value = input();
        let object = value.pointer(pointer).unwrap();
        let array = Value::Array(fields.iter().map(|key| object[*key].clone()).collect());
        *value.pointer_mut(pointer).unwrap() = array;
        if PublishInput::parse(&serde_json::to_vec(&value).unwrap()).is_ok() {
            accepted.push(pointer);
        }
    }
    assert!(
        accepted.is_empty(),
        "object-only fields accepted arrays: {accepted:?}"
    );
}

fn fixed_price(code: &str, minor: &str, exponent: u8) -> Value {
    json!({"type":"fixed","rate":{"currency":code,"minor":minor,"exponent":exponent},
        "basis":"2","discount_bps":1000,"tiers":[]})
}

#[test]
fn fixed_default_projection_preserves_basis_currency_and_full_command_plan_replay() {
    use radroots_protocol::{
        listing::{ListingEvent, PublicSigningPlan},
        plan::SigningReplay,
    };
    for (code, minor, exponent, amount) in [
        ("JPY", "101", 0, "101"),
        ("CAD", "101", 2, "1.01"),
        ("KWD", "101", 3, "0.101"),
        ("CLF", "101", 4, "0.0101"),
        ("CAD", "0", 2, "0.00"),
    ] {
        let mut wire = input();
        wire["profile"]["options"][0]["price"] = fixed_price(code, minor, exponent);
        wire["summary"] = json!("Carrot offering");
        wire["profile"]["options"][0]["unit"]["contents"] =
            json!({"type":"fixed","quantity":{"amount":"0.5","unit":"kg"}});
        let mut cheaper = wire["profile"]["options"][0].clone();
        cheaper["id"] = json!("03".repeat(32));
        cheaper["price"]["rate"]["minor"] = json!("0");
        wire["profile"]["options"]
            .as_array_mut()
            .unwrap()
            .push(cheaper);
        let parsed = PublishInput::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&parsed).unwrap(), wire);
        let event = ListingEvent::new(&parsed, key(), UnixSeconds::new(101).unwrap()).unwrap();
        let fields = event.fields();
        let prices: Vec<_> = fields.tags.iter().filter(|tag| tag[0] == "price").collect();
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].as_slice(), ["price", amount, code]);
        assert_eq!(fields.tags.len(), 9);
        assert_eq!(fields.tags.iter().map(Vec::len).sum::<usize>(), 19);
        let summary = &fields.tags.iter().find(|tag| tag[0] == "summary").unwrap()[1];
        assert!(summary.contains("per 2 bunch"));
        assert!(summary.contains("discount 1000 bps"));
        assert!(summary.contains("One bunch"));
        let public = CapturedListingCommand::capture(context(), parsed).unwrap();
        let captured = CapturedListingCommand::parse(public.protected_wire()).unwrap();
        assert_eq!(captured.digest(), public.digest());
        assert_eq!(captured.protected_wire(), public.protected_wire());
        let plan = PublicSigningPlan::capture(
            &public,
            UnixSeconds::new(101).unwrap(),
            &["wss://listing.fixture.example".into()],
            SigningReplay::LocalReplaySafe,
        )
        .unwrap();
        let restored = PublicSigningPlan::parse(&captured, plan.protected_wire()).unwrap();
        assert_eq!(restored.event().event_id(), event.event_id());
        assert_eq!(restored.protected_wire(), plan.protected_wire());
    }
}

#[test]
fn variable_per_sale_unit_profile_preserves_all_bounds_and_estimated_contents_without_fixed_price()
{
    use radroots_protocol::listing::ListingEvent;
    let mut wire = input();
    wire["profile"]["options"][0]["price"] = json!({"type":"variable_weight","measurement_unit":"kg",
        "minimum_per_sale_unit":"0.8","maximum_per_sale_unit":"1.2","rate":{"currency":"CAD","minor":"325","exponent":2},
        "basis":"1","maximum_per_sale_unit_total":{"currency":"CAD","minor":"400","exponent":2}});
    wire["profile"]["options"][0]["unit"]["contents"] =
        json!({"type":"estimated","quantity":{"amount":"1","unit":"kg"}});
    let parsed = PublishInput::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), wire);
    let event = ListingEvent::new(&parsed, key(), UnixSeconds::new(101).unwrap()).unwrap();
    assert!(!event.fields().tags.iter().any(|tag| tag[0] == "price"));
    let summary = &event
        .fields()
        .tags
        .iter()
        .find(|tag| tag[0] == "summary")
        .unwrap()[1];
    assert!(summary.contains("0.8..1.2 kg per sale unit"));
    assert!(summary.contains("cap 4.00 CAD"));
    assert!(summary.contains("per 1 kg"));
    assert!(
        parsed
            .terms()
            .default_option()
            .unit()
            .contents()
            .unwrap()
            .fixed_quantity()
            .is_none()
    );
    let public = CapturedListingCommand::capture(context(), parsed).unwrap();
    assert_eq!(
        CapturedListingCommand::parse(public.protected_wire())
            .unwrap()
            .digest(),
        public.digest()
    );
}

#[test]
fn monetary_profile_admission_rejects_incomplete_inconsistent_duplicate_and_array_values() {
    let mut original = input();
    original["profile"]["options"][0]["price"] = fixed_price("CAD", "325", 2);
    for (pointer, invalid) in [
        (
            "/profile/options/0/price",
            json!({"type":"fixed","rate":{"currency":"CAD","minor":"325","exponent":2}}),
        ),
        ("/profile/options/0/price/basis", json!("0")),
        ("/profile/options/0/price/discount_bps", json!(10001)),
        ("/profile/options/0/price/rate/exponent", json!(3)),
        ("/profile/options/0/price/rate/minor", json!(325)),
        ("/profile/options/0/price/rate", json!(["CAD", "325", 2])),
        (
            "/profile/options/0/price/tiers",
            json!([["1",{"currency":"CAD","minor":"300","exponent":2}]]),
        ),
        (
            "/profile/options/0/price/tiers",
            json!([{"from":"1","rate":{"currency":"USD","minor":"300","exponent":2}}]),
        ),
        (
            "/profile/options/0/unit/contents",
            json!({"type":"fixed","quantity":["1","kg"]}),
        ),
        (
            "/profile/options/0/unit/contents",
            json!({"type":"fixed","quantity":{"amount":"0.5","unit":"each"}}),
        ),
    ] {
        let mut wire = original.clone();
        *wire.pointer_mut(pointer).unwrap() = invalid;
        assert!(
            PublishInput::parse(&serde_json::to_vec(&wire).unwrap()).is_err(),
            "{pointer}"
        );
    }
    let raw = serde_json::to_string(&original).unwrap();
    for (needle, duplicate) in [
        ("\"basis\":\"2\"", "\"basis\":\"2\",\"basis\":\"2\""),
        ("\"minor\":\"325\"", "\"minor\":\"325\",\"minor\":\"325\""),
    ] {
        let changed = raw.replacen(needle, duplicate, 1);
        assert_ne!(changed, raw);
        assert!(PublishInput::parse(changed.as_bytes()).is_err());
    }
    let mut extra = original.clone();
    extra["profile"]["options"][0]["price"]["unapproved"] = json!(true);
    assert!(PublishInput::parse(&serde_json::to_vec(&extra).unwrap()).is_err());
}

#[test]
fn variable_wire_terms_refuse_inverted_ranges_infeasible_caps_and_currency_changes() {
    let mut original = input();
    original["profile"]["options"][0]["price"] = json!({"type":"variable_weight","measurement_unit":"kg",
        "minimum_per_sale_unit":"0.8","maximum_per_sale_unit":"1.2","rate":{"currency":"CAD","minor":"325","exponent":2},
        "basis":"1","maximum_per_sale_unit_total":{"currency":"CAD","minor":"400","exponent":2}});
    for (pointer, bad) in [
        (
            "/profile/options/0/price/minimum_per_sale_unit",
            json!("1.3"),
        ),
        ("/profile/options/0/price/maximum_per_sale_unit", json!("0")),
        ("/profile/options/0/price/basis", json!("0")),
        ("/profile/options/0/price/measurement_unit", json!("each")),
        (
            "/profile/options/0/price/maximum_per_sale_unit_total/minor",
            json!("259"),
        ),
        (
            "/profile/options/0/price/maximum_per_sale_unit_total/currency",
            json!("USD"),
        ),
    ] {
        let mut wire = original.clone();
        *wire.pointer_mut(pointer).unwrap() = bad;
        assert_eq!(
            PublishInput::parse(&serde_json::to_vec(&wire).unwrap()).unwrap_err(),
            CommandError::InvalidInput
        );
    }
    let mut extra = original;
    extra["profile"]["options"][0]["price"]["discount_bps"] = json!(0);
    assert!(PublishInput::parse(&serde_json::to_vec(&extra).unwrap()).is_err());
}

#[test]
fn monetary_listing_limits_include_projection_profile_and_complete_signed_framing() {
    use radroots_protocol::{
        listing::{LISTING_EVENT_MAX_BYTES, ListingEvent},
        nip01::{self, WireError},
    };
    let mut wire = input();
    wire["description_markdown"] = json!("");
    wire["profile"]["options"][0]["price"] = fixed_price("CAD", "325", 2);
    let make = |wire: &Value| PublishInput::parse(&serde_json::to_vec(wire).unwrap()).unwrap();
    let base = ListingEvent::new(&make(&wire), key(), UnixSeconds::new(101).unwrap()).unwrap();
    let free = LISTING_EVENT_MAX_BYTES
        - nip01::signed_json_len(&base.fields(), LISTING_EVENT_MAX_BYTES).unwrap();
    let body = "\u{0001}".repeat(free / 6) + &"a".repeat(free % 6);
    assert!(body.chars().count() <= 16_384);
    wire["description_markdown"] = json!(body);
    let exact = ListingEvent::new(&make(&wire), key(), UnixSeconds::new(101).unwrap()).unwrap();
    assert_eq!(
        nip01::signed_json_len(&exact.fields(), LISTING_EVENT_MAX_BYTES).unwrap(),
        LISTING_EVENT_MAX_BYTES
    );
    let larger = wire["description_markdown"].as_str().unwrap().to_owned() + "a";
    wire["description_markdown"] = json!(larger);
    assert!(matches!(
        ListingEvent::new(&make(&wire), key(), UnixSeconds::new(101).unwrap()),
        Err(WireError::LimitExceeded)
    ));
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
            json!({"type":"future_price","rate":{"currency":"CAD","minor":"325","exponent":2}}),
        ),
        (
            "/profile/options/0/supply",
            json!({"type":"estimated","amount":"10","window":null}),
        ),
        (
            "/profile/options/0/unit/contents",
            json!({"type":"future_contents","quantity":{"amount":"1","unit":"kg"}}),
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

#[test]
fn public_plans_require_bounded_explicit_targets_without_scheme_or_whitespace_fallback() {
    use radroots_protocol::{listing::PublicSigningPlan, plan::SigningReplay};
    let command = CapturedListingCommand::capture(
        context(),
        PublishInput::parse(&serde_json::to_vec(&input()).unwrap()).unwrap(),
    )
    .unwrap();
    let capture = |targets: &[String]| {
        PublicSigningPlan::capture(
            &command,
            UnixSeconds::new(100).unwrap(),
            targets,
            SigningReplay::NonReplayable,
        )
    };
    assert!(capture(&["wss://route.example/".into()]).is_ok());
    let maximum = format!("wss://{}", "x".repeat(2048 - "wss://".len()));
    assert_eq!(maximum.len(), 2048);
    assert!(capture(std::slice::from_ref(&maximum)).is_ok());
    let excessive = (0..17)
        .map(|i| format!("wss://route.example/{i}"))
        .collect::<Vec<_>>();
    assert!(capture(&excessive).is_err());
    for target in [
        format!("{maximum}x"),
        "https://route.example/".into(),
        "wss://route.example/a ".into(),
        "wss://route.example/a\nb".into(),
    ] {
        assert!(
            capture(std::slice::from_ref(&target)).is_err(),
            "{target:?}"
        );
    }
}
