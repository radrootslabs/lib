use radroots_event::{
    envelope::kind::KIND_DELETION_REQUEST,
    post::deletion::{
        RadrootsAuthoredNip09DeletionRequest, RadrootsNip09DeletionAddressTarget,
        RadrootsNip09DeletionEventTarget,
    },
};
use radroots_event_codec::deletion::admission::verify_and_admit_nip09_deletion_request_event;
use radroots_nostr::prelude::{
    RadrootsNostrError, RadrootsNostrGenericEventBuilder, RadrootsNostrKeys, RadrootsNostrKind,
    RadrootsNostrSecretKey, RadrootsNostrTimestamp, radroots_event_from_nostr,
    radroots_nostr_build_nip09_deletion_request_event,
};
use radroots_test_fixtures::{FIXTURE_ALICE_SECRET_KEY_HEX, FIXTURE_BOB_PUBLIC_KEY_HEX};

#[cfg(feature = "client")]
use radroots_nostr::prelude::{
    RadrootsNostrClient, radroots_nostr_send_nip09_deletion_request_event,
};

const CREATED_AT: u64 = 1_784_347_200;
const TARGET_EVENT_ID: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

#[test]
fn typed_nip09_deletion_request_signs_exact_tags_and_admits() {
    let request = request();
    let created_at = RadrootsNostrTimestamp::from_secs(CREATED_AT);
    let profile_coordinate = format!("0:{FIXTURE_BOB_PUBLIC_KEY_HEX}:");
    let listing_coordinate = format!("30402:{FIXTURE_BOB_PUBLIC_KEY_HEX}:carrots");
    let expected_tags = vec![
        tag(&["e", TARGET_EVENT_ID]),
        tag(&["a", &profile_coordinate]),
        tag(&["a", &listing_coordinate]),
        tag(&["k", "0"]),
        tag(&["k", "1"]),
        tag(&["k", "30402"]),
    ];

    let event = radroots_nostr_build_nip09_deletion_request_event(&request)
        .expect("typed NIP-09 builder")
        .custom_created_at(created_at)
        .sign_with_keys(&fixture_keys())
        .expect("signed NIP-09 deletion request");

    assert_eq!(event.kind.as_u16(), KIND_DELETION_REQUEST as u16);
    assert_eq!(event.created_at, created_at);
    assert_eq!(event.content, "superseded");
    assert_eq!(event_tags(&event), expected_tags);
    event.verify().expect("valid deletion-request signature");

    let admitted = verify_and_admit_nip09_deletion_request_event(
        radroots_event_from_nostr(&event).expect("deletion-request event adapter"),
    )
    .expect("deletion-request admission");
    assert_eq!(
        admitted.contract().id,
        "radroots.social.deletion_request.v1"
    );
    assert_eq!(admitted.projection().event_targets().len(), 1);
    assert_eq!(admitted.projection().address_targets().len(), 2);
    assert_eq!(
        admitted
            .projection()
            .kind_advisories()
            .iter()
            .map(|advisory| advisory.kind())
            .collect::<Vec<_>>(),
        vec![0, 1, 30_402]
    );
    assert!(admitted.projection().diagnostics().is_empty());
    assert_eq!(admitted.projection().raw_tags(), expected_tags.as_slice());
}

#[test]
fn generic_kind_five_builder_cannot_bypass_typed_deletion_authoring() {
    let error = RadrootsNostrGenericEventBuilder::new(
        RadrootsNostrKind::Custom(KIND_DELETION_REQUEST as u16),
        "Raw deletion request",
    )
    .sign_with_keys(&fixture_keys())
    .expect_err("every generic kind 5 must be reserved");

    assert!(matches!(
        error,
        RadrootsNostrError::TypedAuthoringRequired { kind }
            if kind == KIND_DELETION_REQUEST as u16
    ));
}

#[cfg(feature = "client")]
#[tokio::test]
async fn generic_kind_five_client_rejection_precedes_signer_access() {
    let client = RadrootsNostrClient::new_signerless();
    let builder = RadrootsNostrGenericEventBuilder::new(
        RadrootsNostrKind::Custom(KIND_DELETION_REQUEST as u16),
        "Raw deletion request",
    );

    let error = client
        .send_event_builder(builder)
        .await
        .expect_err("generic kind 5 must fail before signer access");

    assert!(matches!(
        error,
        RadrootsNostrError::TypedAuthoringRequired { kind }
            if kind == KIND_DELETION_REQUEST as u16
    ));
}

#[cfg(feature = "client")]
#[tokio::test]
async fn typed_nip09_deletion_request_reaches_client_publication() {
    let client = RadrootsNostrClient::new(fixture_keys());
    let method_builder = radroots_nostr_build_nip09_deletion_request_event(&request())
        .expect("typed NIP-09 builder");
    let helper_builder = radroots_nostr_build_nip09_deletion_request_event(&request())
        .expect("typed NIP-09 builder");

    let method_error = client
        .send_nip09_deletion_request_event_builder(method_builder)
        .await
        .expect_err("no relay is configured");
    let helper_error = radroots_nostr_send_nip09_deletion_request_event(&client, helper_builder)
        .await
        .expect_err("no relay is configured");

    assert!(matches!(method_error, RadrootsNostrError::ClientError(_)));
    assert!(matches!(helper_error, RadrootsNostrError::ClientError(_)));
}

fn request() -> RadrootsAuthoredNip09DeletionRequest {
    RadrootsAuthoredNip09DeletionRequest::new(
        "superseded",
        vec![RadrootsNip09DeletionEventTarget::parse(TARGET_EVENT_ID, 1).expect("event target")],
        vec![
            RadrootsNip09DeletionAddressTarget::parse(format!(
                "30402:{FIXTURE_BOB_PUBLIC_KEY_HEX}:carrots"
            ))
            .expect("listing target"),
            RadrootsNip09DeletionAddressTarget::parse(format!("0:{FIXTURE_BOB_PUBLIC_KEY_HEX}:"))
                .expect("profile target"),
        ],
    )
    .expect("authored deletion request")
}

fn fixture_keys() -> RadrootsNostrKeys {
    RadrootsNostrKeys::new(
        RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("fixture secret key"),
    )
}

fn event_tags(event: &nostr::Event) -> Vec<Vec<String>> {
    event
        .tags
        .iter()
        .map(|tag| tag.as_slice().to_vec())
        .collect()
}

fn tag(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
