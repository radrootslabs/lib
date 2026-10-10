//! Immutable explicit registry v8: unchanged v7 contracts plus complete-profile edit.

use super::{EventContract, all_event_contracts_registry_v7};

pub const PROFILE_METADATA_EDIT_CONTRACT_ID: &str = "radroots.profile.metadata_edit.v1";

const COUNT: usize = all_event_contracts_registry_v7().len() + 1;
static CONTRACTS: [EventContract; COUNT] = {
    let inherited = all_event_contracts_registry_v7();
    let mut contracts = [inherited[0]; COUNT];
    let mut index = 0;
    while index < inherited.len() {
        contracts[index] = inherited[index];
        index += 1;
    }
    let mut edit = inherited[0];
    edit.id = PROFILE_METADATA_EDIT_CONTRACT_ID;
    edit.name = "Complete Profile Metadata Edit";
    edit.payload_type = "ProfileEdit / RadrootsAdmittedProfileEvent";
    contracts[index] = edit;
    contracts
};

pub const fn all_event_contracts_registry_v8() -> &'static [EventContract] {
    &CONTRACTS
}

pub fn event_contract_registry_v8(id: &str) -> Option<&'static EventContract> {
    CONTRACTS.iter().find(|contract| contract.id == id)
}
