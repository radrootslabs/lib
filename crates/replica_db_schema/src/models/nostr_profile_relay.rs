use crate::nostr_profile::NostrProfileQueryBindValues;
use crate::nostr_relay::NostrRelayQueryBindValues;
use radroots_types::types::IResultPass;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct INostrProfileRelayRelation {
    pub nostr_profile: NostrProfileQueryBindValues,
    pub nostr_relay: NostrRelayQueryBindValues,
}

pub struct INostrProfileRelayResolveTs;
pub type INostrProfileRelayResolve = IResultPass;
