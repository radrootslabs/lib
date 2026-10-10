#![cfg(feature = "json")]

use nostr::{EventBuilder, Keys, Kind, Timestamp};
use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};
use radroots_event::{
    contract::{
        ContractId, ContractKey, RegistryVersion, all_event_contracts,
        registry_v8::{PROFILE_METADATA_EDIT_CONTRACT_ID, all_event_contracts_registry_v8},
    },
    envelope::{EventEnvelope, EventEnvelopeParts},
    media::AuthoredImage,
    profile::{
        AuthoredProfile, Nip05Identifier, RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES,
        edit::{ProfileEdit, ProfileFieldChange as Change, ProfileMediaChange as Media},
    },
};
use radroots_event_codec::{
    admission::profile::{RadrootsAdmittedProfileEvent, verify_and_admit_profile_event},
    authoring::{AuthoredEventPlan, PLAN_WIRE_MAX_BYTES, PlanWireV1},
    encode::profile::ProfileEditError,
};
use serde_json::{Value, value::RawValue};
use std::collections::BTreeMap;

fn keys() -> Keys {
    Keys::parse("0101010101010101010101010101010101010101010101010101010101010101").unwrap()
}

fn envelope(content: &str, time: u64, tags: Vec<Vec<String>>, signer: &Keys) -> EventEnvelope {
    let event = EventBuilder::new(Kind::Metadata, content)
        .custom_created_at(Timestamp::from_secs(time))
        .tags(tags.into_iter().map(|tag| nostr::Tag::parse(tag).unwrap()))
        .sign_with_keys(signer)
        .unwrap();
    EventEnvelope::new(EventEnvelopeParts {
        id: event.id.to_hex(),
        author: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs(),
        kind: 0,
        tags: event
            .tags
            .as_slice()
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect(),
        content: event.content,
        sig: event.sig.to_string(),
    })
    .unwrap()
}

fn admitted(content: &str, time: u64) -> RadrootsAdmittedProfileEvent {
    verify_and_admit_profile_event(envelope(content, time, vec![], &keys())).unwrap()
}

fn plan(
    baseline: &RadrootsAdmittedProfileEvent,
    edit: &ProfileEdit,
) -> Result<AuthoredEventPlan, ProfileEditError> {
    AuthoredEventPlan::from_profile_edit(
        baseline,
        baseline,
        edit,
        baseline.event().created_at_u64() + 1,
        keys().public_key().to_hex(),
    )
}

fn image(bytes: &[u8]) -> AuthoredImage {
    let hash = Sha256::digest(bytes);
    let media = MediaType::parse("image/webp").unwrap();
    AuthoredImage::try_from(
        BlobDescriptor::new(
            BlobUrl::parse(&format!("https://media.example/{hash}.webp")).unwrap(),
            hash,
            bytes.len() as u64,
            media.clone(),
            100,
        )
        .unwrap()
        .approve_reference()
        .unwrap()
        .verify_bytes(bytes, &media)
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn profile_complete_edit_display_only_preserves_all_untouched_metadata_and_signed_roundtrip() {
    let picture = image(b"picture");
    let banner = image(b"banner");
    let content = format!(
        r#"{{"name":"farm","display_name":"Old","about":"grows food","picture":"{}","banner":"{}","nip05":"farm@EXAMPLE.COM","bot":false,"website":"https://example.com","birthday":{{ "year":1900, "year":2000 }},"custom":900719925474099312345,"lud16":null}}"#,
        picture.descriptor().url(),
        banner.descriptor().url()
    );
    let base = admitted(&content, 100);
    let edit = ProfileEdit {
        display_name: Change::Set("New".into()),
        picture: Media::KeepVerified(picture),
        banner: Media::KeepVerified(banner),
        ..Default::default()
    };
    let candidate = plan(&base, &edit).unwrap();
    let before: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&content).unwrap();
    let after: BTreeMap<String, Box<RawValue>> =
        serde_json::from_str(candidate.body().content()).unwrap();
    assert_eq!(before.len(), after.len());
    for (name, value) in before
        .iter()
        .filter(|(name, _)| name.as_str() != "display_name")
    {
        assert_eq!(value.get(), after[name].get(), "{name}");
    }
    assert_eq!(after["display_name"].get(), r#""New""#);
    assert_eq!(
        candidate.body().contract().registry_version(),
        RegistryVersion::V8
    );
    assert_eq!(
        candidate.body().contract().contract_id().as_str(),
        PROFILE_METADATA_EDIT_CONTRACT_ID
    );
    let wire = PlanWireV1::from_plan(&candidate).to_json().unwrap();
    let restored = PlanWireV1::from_json(&wire).unwrap();
    assert_eq!(restored.plan(), &candidate);
    let signed = verify_and_admit_profile_event(envelope(
        candidate.body().content(),
        candidate.created_at(),
        vec![],
        &keys(),
    ))
    .unwrap();
    assert_eq!(signed.event().id(), candidate.expected_event_id());
    assert_eq!(signed.metadata().display_name(), Some("New"));
    assert!(signed.metadata().residual_fields().contains_key("custom"));
    assert_eq!(
        signed.metadata().nip05().unwrap().as_str(),
        "farm@example.com"
    );
}

#[test]
fn profile_complete_edit_sets_and_clears_each_typed_field_without_unknown_authoring() {
    let base = admitted(
        r#"{"name":"old","display_name":7,"about":null,"picture":false,"banner":[],"nip05":"invalid","bot":"wrong","unknown":{"x":1}}"#,
        100,
    );
    let edit = ProfileEdit {
        name: Change::Set("new".into()),
        display_name: Change::Set("Display".into()),
        about: Change::Set("About".into()),
        picture: Media::Set(image(b"picture")),
        banner: Media::Set(image(b"banner")),
        nip05: Change::Set(Nip05Identifier::parse("farmer@EXAMPLE.COM").unwrap()),
        bot: Change::Set(true),
    };
    let result: Value = serde_json::from_str(plan(&base, &edit).unwrap().body().content()).unwrap();
    assert_eq!(result["name"], "new");
    assert_eq!(result["display_name"], "Display");
    assert_eq!(result["about"], "About");
    assert_eq!(result["nip05"], "farmer@example.com");
    assert_eq!(result["bot"], true);
    assert!(
        result["picture"]
            .as_str()
            .unwrap()
            .starts_with("https://media.example/")
    );
    assert!(
        result["banner"]
            .as_str()
            .unwrap()
            .starts_with("https://media.example/")
    );
    let clear = ProfileEdit {
        name: Change::Clear,
        display_name: Change::Clear,
        about: Change::Clear,
        picture: Media::Clear,
        banner: Media::Clear,
        nip05: Change::Clear,
        bot: Change::Clear,
    };
    assert_eq!(
        plan(&base, &clear).unwrap().body().content(),
        r#"{"unknown":{"x":1}}"#
    );
    assert_eq!(
        plan(&base, &ProfileEdit::default())
            .unwrap()
            .body()
            .content(),
        r#"{"about":null,"banner":[],"bot":"wrong","display_name":7,"name":"old","nip05":"invalid","picture":false,"unknown":{"x":1}}"#
    );
    assert_eq!(ProfileEdit::default().name, Change::Keep);
}

#[test]
fn profile_complete_edit_stale_foreign_tagged_and_nonadvancing_baselines_refuse() {
    let base = admitted(r#"{"name":"old"}"#, 100);
    let newer = admitted(r#"{"name":"new"}"#, 101);
    let edit = ProfileEdit::default();
    let author = keys().public_key().to_hex();
    assert_eq!(
        AuthoredEventPlan::from_profile_edit(&base, &newer, &edit, 102, &author).unwrap_err(),
        ProfileEditError::StaleBaseline
    );
    let equal_time = admitted(r#"{"name":"changed"}"#, 100);
    assert_eq!(
        AuthoredEventPlan::from_profile_edit(&base, &equal_time, &edit, 102, &author).unwrap_err(),
        ProfileEditError::StaleBaseline
    );
    for time in [99, 100] {
        assert_eq!(
            AuthoredEventPlan::from_profile_edit(&base, &base, &edit, time, &author).unwrap_err(),
            ProfileEditError::TimestampNotAdvancing
        );
    }
    for wrong in ["bad", &"ff".repeat(32)] {
        assert_eq!(
            AuthoredEventPlan::from_profile_edit(&base, &base, &edit, 102, wrong).unwrap_err(),
            ProfileEditError::AuthorMismatch
        );
    }
    let other =
        Keys::parse("0202020202020202020202020202020202020202020202020202020202020202").unwrap();
    let foreign =
        verify_and_admit_profile_event(envelope(r#"{"name":"foreign"}"#, 100, vec![], &other))
            .unwrap();
    assert_eq!(
        AuthoredEventPlan::from_profile_edit(&base, &foreign, &edit, 102, &author).unwrap_err(),
        ProfileEditError::AuthorMismatch
    );
    assert_eq!(
        AuthoredEventPlan::from_profile_edit(&foreign, &base, &edit, 102, &author).unwrap_err(),
        ProfileEditError::AuthorMismatch
    );
    let tagged = verify_and_admit_profile_event(envelope(
        r#"{"name":"old"}"#,
        100,
        vec![vec!["x".into(), "keep".into()]],
        &keys(),
    ))
    .unwrap();
    assert_eq!(
        plan(&tagged, &edit).unwrap_err(),
        ProfileEditError::BaselineTagsUnsupported
    );
}

#[test]
fn profile_complete_edit_kept_media_requires_exact_proof_or_explicit_clear() {
    let picture = image(b"picture");
    let base = admitted(
        &format!(r#"{{"picture":"{}"}}"#, picture.descriptor().url()),
        100,
    );
    assert_eq!(
        plan(&base, &ProfileEdit::default()).unwrap_err(),
        ProfileEditError::MediaVerificationRequired { field: "picture" }
    );
    let mismatch = ProfileEdit {
        picture: Media::KeepVerified(image(b"other")),
        ..Default::default()
    };
    assert_eq!(
        plan(&base, &mismatch).unwrap_err(),
        ProfileEditError::MediaVerificationMismatch { field: "picture" }
    );
    let empty = admitted("{}", 100);
    assert_eq!(
        plan(&empty, &mismatch).unwrap_err(),
        ProfileEditError::MediaVerificationMismatch { field: "picture" }
    );
    let unsupported = admitted(r#"{"picture":"https://legacy.example/photo"}"#, 100);
    assert_eq!(
        plan(&unsupported, &ProfileEdit::default())
            .unwrap_err()
            .code(),
        "profile_media_verification_required"
    );
    assert_eq!(
        plan(
            &unsupported,
            &ProfileEdit {
                picture: Media::Clear,
                ..Default::default()
            }
        )
        .unwrap()
        .body()
        .content(),
        "{}"
    );
    let malformed = admitted(r#"{"picture":{"url":"untrusted"}}"#, 100);
    assert_eq!(
        plan(&malformed, &ProfileEdit::default())
            .unwrap()
            .body()
            .content(),
        r#"{"picture":{"url":"untrusted"}}"#
    );
}

#[test]
fn profile_complete_edit_content_and_durable_wire_bounds_are_exact() {
    let empty = admitted("{}", 100);
    let overhead = r#"{"about":""}"#.len();
    let max = RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES;
    let exact = ProfileEdit {
        about: Change::Set("a".repeat(max - overhead)),
        ..Default::default()
    };
    let accepted = plan(&empty, &exact).unwrap();
    assert_eq!(accepted.body().content().len(), max);
    assert!(PlanWireV1::from_plan(&accepted).to_json().unwrap().len() <= PLAN_WIRE_MAX_BYTES);
    let over = ProfileEdit {
        about: Change::Set("a".repeat(max - overhead + 1)),
        ..Default::default()
    };
    assert_eq!(plan(&empty, &over).unwrap_err().code(), "content_too_large");
    let early = ProfileEdit {
        name: Change::Set("a".repeat(max + 1)),
        ..Default::default()
    };
    assert_eq!(
        plan(&empty, &early).unwrap_err(),
        ProfileEditError::ContentTooLarge {
            max,
            actual: max + 1
        }
    );
    for name in ["", " ", "bad\nname"] {
        assert_eq!(
            plan(
                &empty,
                &ProfileEdit {
                    name: Change::Set(name.into()),
                    ..Default::default()
                }
            )
            .unwrap_err(),
            ProfileEditError::InvalidName
        );
    }
    // Valid input content can expand beyond the durable-wire bound when escaped again.
    let escaped = ProfileEdit {
        about: Change::Set("\"".repeat((max - overhead) / 2)),
        ..Default::default()
    };
    assert_eq!(
        plan(&empty, &escaped).unwrap_err().code(),
        "profile_durable_wire_too_large"
    );
}

#[test]
fn profile_complete_edit_registry_is_explicit_and_v7_defaults_and_strict_wire_are_unchanged() {
    assert_eq!(RegistryVersion::CURRENT, RegistryVersion::V7);
    let inherited = all_event_contracts();
    let successor = all_event_contracts_registry_v8();
    assert_eq!(&successor[..inherited.len()], inherited);
    assert_eq!(successor.len(), inherited.len() + 1);
    assert_eq!(
        successor.last().unwrap().id,
        PROFILE_METADATA_EDIT_CONTRACT_ID
    );
    assert!(ContractKey::current(PROFILE_METADATA_EDIT_CONTRACT_ID).is_err());
    let key = ContractKey::new(
        RegistryVersion::V8,
        ContractId::parse(PROFILE_METADATA_EDIT_CONTRACT_ID).unwrap(),
    )
    .unwrap();
    assert_eq!(key.contract().kind, 0);
    assert_eq!(
        serde_json::from_slice::<ContractKey>(&serde_json::to_vec(&key).unwrap()).unwrap(),
        key
    );
    assert!(
        ContractKey::new(
            RegistryVersion::V8,
            ContractId::parse("radroots.profile.unknown.v1").unwrap()
        )
        .is_err()
    );
    let strict = AuthoredEventPlan::from_profile(
        &AuthoredProfile::new("farm").unwrap(),
        101,
        keys().public_key().to_hex(),
    )
    .unwrap();
    assert_eq!(
        strict.body().contract().registry_version(),
        RegistryVersion::V7
    );
    assert_eq!(strict.body().content(), r#"{"name":"farm"}"#);
    let baseline = admitted(r#"{"custom":true,"name":"farm"}"#, 100);
    let edit = plan(&baseline, &ProfileEdit::default()).unwrap();
    let mut forged: Value =
        serde_json::from_slice(&PlanWireV1::from_plan(&edit).to_json().unwrap()).unwrap();
    forged["contract_registry_version"] = Value::from(7);
    assert!(PlanWireV1::from_json(&serde_json::to_vec(&forged).unwrap()).is_err());
    forged["contract_id"] = Value::from("radroots.profile.metadata.v1");
    assert!(PlanWireV1::from_json(&serde_json::to_vec(&forged).unwrap()).is_err());
}

#[test]
fn profile_complete_edit_historical_reconstruction_refuses_ambiguous_or_unsupported_shape() {
    use radroots_event_codec::authoring::PlanDecodeError;
    let baseline = admitted(r#"{"name":"farm","unknown":true}"#, 100);
    let candidate = plan(&baseline, &ProfileEdit::default()).unwrap();
    let original: Value =
        serde_json::from_slice(&PlanWireV1::from_plan(&candidate).to_json().unwrap()).unwrap();
    for (content, code) in [
        (
            r#"{"unknown":true,"name":"farm"}"#,
            "profile_edit_noncanonical",
        ),
        (r#"{"name":"farm","name":"other"}"#, "duplicate_field"),
        (
            r#"{"picture":"https://legacy.example/image"}"#,
            "profile_media_url_invalid",
        ),
        ("[]", "root_not_object"),
        ("broken", "invalid_json"),
    ] {
        let mut value = original.clone();
        value["content"] = Value::from(content);
        assert_eq!(
            PlanWireV1::from_json(&serde_json::to_vec(&value).unwrap()),
            Err(PlanDecodeError::HistoricalShape(code.into()))
        );
    }
    let mut tagged = original.clone();
    tagged["tags"] = serde_json::json!([["x", "extra"]]);
    assert_eq!(
        PlanWireV1::from_json(&serde_json::to_vec(&tagged).unwrap()),
        Err(PlanDecodeError::HistoricalShape(
            "profile_tags_forbidden".into()
        ))
    );
    let mut unsupported = original;
    unsupported["contract_registry_version"] = Value::from(9);
    assert!(PlanWireV1::from_json(&serde_json::to_vec(&unsupported).unwrap()).is_err());
}
