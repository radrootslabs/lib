use radroots_event::contract::AuthorRole;
use radroots_signing::{Actor, actor::ActorSource};
use serde::{Serialize, ser::SerializeStruct};

pub(crate) struct SdkActorContextJson<'a>(pub(crate) &'a Actor);

pub(crate) fn serialize_actor_context<S>(actor: &Actor, serializer: S) -> Result<S::Ok, S::Error>
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
        let account_id = self.0.account_id().map(|account_id| account_id.to_hex());
        let pubkey = self.0.public_key().to_hex();
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

fn actor_source_code(source: ActorSource) -> &'static str {
    match source {
        ActorSource::LocalAccount(_) => "local_account",
        ActorSource::ExplicitPublicKey => "explicit_public_key",
        ActorSource::RemoteSigner(_) => "remote_signer",
        ActorSource::Service(_) => "service",
        _ => "unknown",
    }
}

#[cfg(test)]
#[path = "../tests/unit/actor_json_tests.rs"]
mod tests;
