use super::{SdkActorContextJson, actor_role_code, actor_source_code};
use radroots_authority::{RadrootsActorContext, RadrootsActorSource};
use radroots_event::contract::AuthorRole;

use crate::serializer_failure::assert_struct_serialize_error_paths;

const PUBKEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn actor_role_and_source_codes_cover_public_actor_taxonomy() {
    assert_eq!(actor_role_code(&AuthorRole::Any), "any");
    assert_eq!(actor_role_code(&AuthorRole::Application), "application");
    assert_eq!(actor_role_code(&AuthorRole::Buyer), "buyer");
    assert_eq!(actor_role_code(&AuthorRole::Farmer), "farmer");
    assert_eq!(actor_role_code(&AuthorRole::Member), "member");
    assert_eq!(actor_role_code(&AuthorRole::Moderator), "moderator");
    assert_eq!(actor_role_code(&AuthorRole::Relay), "relay");
    assert_eq!(actor_role_code(&AuthorRole::Seller), "seller");
    assert_eq!(actor_role_code(&AuthorRole::Service), "service");

    assert_eq!(
        actor_source_code(RadrootsActorSource::LocalAccount),
        "local_account"
    );
    assert_eq!(
        actor_source_code(RadrootsActorSource::ExplicitPubkey),
        "explicit_pubkey"
    );
    assert_eq!(
        actor_source_code(RadrootsActorSource::RemoteSigner),
        "remote_signer"
    );
    assert_eq!(actor_source_code(RadrootsActorSource::Service), "service");
    assert_eq!(actor_source_code(RadrootsActorSource::Test), "test");
}

#[test]
fn actor_context_json_preserves_source_roles_and_account_id() {
    let actor = RadrootsActorContext::local_account(
        PUBKEY,
        "acct-1",
        [AuthorRole::Buyer, AuthorRole::Seller],
    )
    .expect("actor");

    let json = serde_json::to_value(SdkActorContextJson(&actor)).expect("actor json");

    assert_eq!(
        json,
        serde_json::json!({
            "pubkey": PUBKEY,
            "roles": ["buyer", "seller"],
            "account_id": "acct-1",
            "source": "local_account"
        })
    );
}

#[test]
fn actor_context_json_reports_serializer_failures() {
    let actor = RadrootsActorContext::local_account(
        PUBKEY,
        "acct-1",
        [AuthorRole::Buyer, AuthorRole::Seller],
    )
    .expect("actor");

    assert_struct_serialize_error_paths(&SdkActorContextJson(&actor), 4);
}
