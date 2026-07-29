use radroots_authority::{RadrootsActorContext, RadrootsActorSource};
use radroots_event::contract::AuthorRole;
use serde::{Serialize, ser::SerializeStruct};

pub(crate) struct SdkActorContextJson<'a>(pub(crate) &'a RadrootsActorContext);

pub(crate) fn serialize_actor_context<S>(
    actor: &RadrootsActorContext,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    SdkActorContextJson(actor).serialize(serializer)
}

impl serde::Serialize for SdkActorContextJson<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let roles = self
            .0
            .roles()
            .iter()
            .map(actor_role_code)
            .collect::<Vec<_>>();
        let account_id = self.0.account_id().map(|account_id| account_id.as_str());
        let pubkey = self.0.pubkey().to_hex();
        let mut state = serializer.serialize_struct("SdkActorContext", 4)?;
        state.serialize_field("pubkey", &pubkey)?;
        state.serialize_field("roles", &roles)?;
        state.serialize_field("account_id", &account_id)?;
        state.serialize_field("source", actor_source_code(self.0.source()))?;
        state.end()
    }
}

fn actor_role_code(role: &AuthorRole) -> &'static str {
    match role {
        AuthorRole::Any => "any",
        AuthorRole::Application => "application",
        AuthorRole::Buyer => "buyer",
        AuthorRole::Farmer => "farmer",
        AuthorRole::Member => "member",
        AuthorRole::Moderator => "moderator",
        AuthorRole::Relay => "relay",
        AuthorRole::Seller => "seller",
        AuthorRole::Service => "service",
    }
}

fn actor_source_code(source: RadrootsActorSource) -> &'static str {
    match source {
        RadrootsActorSource::LocalAccount => "local_account",
        RadrootsActorSource::ExplicitPubkey => "explicit_pubkey",
        RadrootsActorSource::RemoteSigner => "remote_signer",
        RadrootsActorSource::Service => "service",
        RadrootsActorSource::Test => "test",
    }
}

#[cfg(test)]
#[path = "../tests/unit/actor_json_tests.rs"]
mod tests;
