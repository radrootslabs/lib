use radroots_types::types::IResultPass;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;
use crate::nostr_profile::NostrProfileQueryBindValues;
use crate::nostr_relay::NostrRelayQueryBindValues;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct INostrProfileRelayRelation {
    pub nostr_profile: NostrProfileQueryBindValues,
    pub nostr_relay: NostrRelayQueryBindValues,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename = "INostrProfileRelayResolve",
        type = "IResultPass"
    )
)]
pub struct INostrProfileRelayResolveTs;
pub type INostrProfileRelayResolve = IResultPass;
