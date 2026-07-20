use radroots_event::{
    contract::registry_v7::{
        RadrootsContractMatchError, RadrootsContractValidationError, RadrootsEventContract,
        event_contract_registry_v7, validate_event_contract_registry_v7,
    },
    food_availability::RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID,
    kinds::{
        KIND_CLASSIFIED_LISTING, KIND_COMMENT, KIND_DELETION_REQUEST, KIND_POST, KIND_PROFILE,
    },
};

use crate::{
    comment::inbound::registry_v7::project_verified_nip22_comment_event_registry_v7,
    deletion::reconciliation_v1::inbound::project_verified_nip09_deletion_request_event_v1,
    food_availability::inbound::registry_v7::{
        RadrootsFoodAvailabilityProjectionOutcome,
        project_verified_food_availability_event_registry_v7,
    },
    post::inbound::registry_v7::project_verified_post_event_registry_v7,
    profile::inbound::registry_v7::parse_inbound_profile_metadata_registry_v7,
    reply::inbound::registry_v7::project_verified_nip10_reply_event_registry_v7,
    verification::v1::RadrootsSignatureVerifiedEvent,
};

const PROFILE_CONTRACT_ID: &str = "radroots.profile.metadata.v1";
const INTERNAL_CONTRACT_MISSING: &str = "registry_v7_contract_missing";

/// The closed admission decision persisted by event-store reconciliation v1.
///
/// This intentionally excludes current typed admission enums and rich errors.
/// Later registries may grow those APIs without changing persisted v7 facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsRegistryV7AdmissionDecision {
    Admitted {
        contract: &'static RadrootsEventContract,
    },
    Unsupported {
        code: &'static str,
    },
    Invalid {
        code: &'static str,
    },
    Defect {
        code: &'static str,
    },
}

/// Admits through the immutable event-contract registry-v7 behavior graph.
///
/// Event-store migration 0002 calls this entry point. Later registries must
/// retain it and add a new versioned admission function.
pub fn admit_verified_event_registry_v7(
    event: &RadrootsSignatureVerifiedEvent,
) -> RadrootsRegistryV7AdmissionDecision {
    match event.event().kind_u32() {
        KIND_PROFILE => match parse_inbound_profile_metadata_registry_v7(event.event().content()) {
            Ok(_) => admitted(PROFILE_CONTRACT_ID),
            Err(error) => invalid(error.code()),
        },
        KIND_POST => match project_verified_post_event_registry_v7(event) {
            Ok(projection) if projection.classification().is_root_card() => {
                admitted(projection.classification().contract_id())
            }
            Ok(_) => match project_verified_nip10_reply_event_registry_v7(event) {
                Ok(projection) => admitted(projection.contract_id()),
                Err(error) => invalid(error.code()),
            },
            Err(error) => invalid(error.code()),
        },
        KIND_COMMENT => match project_verified_nip22_comment_event_registry_v7(event) {
            Ok(projection) => admitted(projection.contract_id()),
            Err(error) => invalid(error.code()),
        },
        KIND_DELETION_REQUEST => match project_verified_nip09_deletion_request_event_v1(event) {
            Ok(projection) => admitted(projection.contract_id()),
            Err(error) => invalid(error.code()),
        },
        KIND_CLASSIFIED_LISTING => {
            match project_verified_food_availability_event_registry_v7(event) {
                Ok(RadrootsFoodAvailabilityProjectionOutcome::Focused(_)) => {
                    admitted(RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID)
                }
                Ok(RadrootsFoodAvailabilityProjectionOutcome::Excluded(_)) => {
                    admit_registry_contract_v7(event)
                }
                Err(error) => invalid(error.code()),
            }
        }
        _ => admit_registry_contract_v7(event),
    }
}

fn admit_registry_contract_v7(
    event: &RadrootsSignatureVerifiedEvent,
) -> RadrootsRegistryV7AdmissionDecision {
    match validate_event_contract_registry_v7(event.event()) {
        Ok(contract) => RadrootsRegistryV7AdmissionDecision::Admitted { contract },
        Err(RadrootsContractValidationError::ContractMatch {
            error: RadrootsContractMatchError::UnsupportedKind(_),
        }) => unsupported("unsupported_kind"),
        Err(RadrootsContractValidationError::ContractMatch {
            error: RadrootsContractMatchError::UnsupportedShape(_),
        }) => unsupported("unsupported_shape"),
        Err(RadrootsContractValidationError::ContractMatch {
            error: RadrootsContractMatchError::AmbiguousShape(_),
        }) => invalid("ambiguous_shape"),
        Err(error) => invalid(error.code()),
    }
}

fn admitted(contract_id: &str) -> RadrootsRegistryV7AdmissionDecision {
    match event_contract_registry_v7(contract_id) {
        Some(contract) => RadrootsRegistryV7AdmissionDecision::Admitted { contract },
        None => RadrootsRegistryV7AdmissionDecision::Defect {
            code: INTERNAL_CONTRACT_MISSING,
        },
    }
}

const fn unsupported(code: &'static str) -> RadrootsRegistryV7AdmissionDecision {
    RadrootsRegistryV7AdmissionDecision::Unsupported { code }
}

const fn invalid(code: &'static str) -> RadrootsRegistryV7AdmissionDecision {
    RadrootsRegistryV7AdmissionDecision::Invalid { code }
}

#[cfg(test)]
mod tests;
