use radroots_event::contract::{
    RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION, event_contract_registry_v7,
};

#[test]
fn current_admission_registry_still_maps_to_v7() {
    assert_eq!(RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION, 7);
}

#[test]
fn frozen_typed_contract_ids_resolve_in_registry_v7() {
    for contract_id in [
        super::PROFILE_CONTRACT_ID,
        "radroots.social.update.v1",
        "radroots.social.photo_update.v1",
        "radroots.social.ask.v1",
        "radroots.social.reply.v1",
        "radroots.social.comment.v1",
        "radroots.social.deletion_request.v1",
        radroots_event::food::availability::RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID,
    ] {
        assert_eq!(
            event_contract_registry_v7(contract_id).map(|contract| contract.id),
            Some(contract_id)
        );
    }
}
