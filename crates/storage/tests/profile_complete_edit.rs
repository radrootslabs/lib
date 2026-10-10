use futures_executor::block_on;
use radroots_event::{
    profile::edit::{ProfileEdit, ProfileFieldChange},
    wire::Nip01EventWire,
};
use radroots_event_codec::{
    admission::profile::verify_and_admit_profile_event, authoring::AuthoredEventPlan,
};
use radroots_storage::{
    Error,
    authored::DurableAuthoredPlan,
    authored_draft::{
        AuthoredDraft, AuthoredDraftId, AuthoredDraftStage, AuthoredDraftStore,
        DraftAppendDisposition,
    },
    memory::MemoryStorage,
};

// Exact canonical profile_admit_tagless_tolerant_001 vector, packaged locally.
fn edit_draft() -> (AuthoredEventPlan, AuthoredDraft) {
    let baseline = verify_and_admit_profile_event(
        Nip01EventWire::parse_json(include_str!("fixtures/profile_edit_baseline.v1.json"))
            .unwrap()
            .into_envelope()
            .unwrap(),
    )
    .unwrap();
    let plan = AuthoredEventPlan::from_profile_edit(
        &baseline,
        &baseline,
        &ProfileEdit {
            display_name: ProfileFieldChange::Set("Updated farm".into()),
            ..Default::default()
        },
        baseline.event().created_at_u64() + 1,
        baseline.event().author().to_hex(),
    )
    .unwrap();
    let durable = DurableAuthoredPlan::from_plan(&plan).unwrap();
    let draft = AuthoredDraft::initial(
        AuthoredDraftId::new([41; 16]).unwrap(),
        *plan.author().as_bytes(),
        "radroots.authored_event_plan.v1",
        durable.wire_json().to_vec(),
        AuthoredDraftStage::Draft,
        None,
        100,
    )
    .unwrap();
    (plan, draft)
}

#[test]
fn profile_complete_edit_memory_durable_payload_replay_conflict_and_reconstruction_preserve_metadata()
 {
    let (plan, draft) = edit_draft();
    let store = MemoryStorage::default();
    block_on(store.append_authored_draft(draft.clone(), None)).unwrap();
    assert_eq!(
        block_on(store.append_authored_draft(draft.clone(), None))
            .unwrap()
            .disposition(),
        DraftAppendDisposition::Replay
    );
    let conflicting = AuthoredDraft::initial(
        draft.draft_id(),
        *draft.author(),
        draft.payload_schema(),
        b"conflicting".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        100,
    )
    .unwrap();
    assert_eq!(
        block_on(store.append_authored_draft(conflicting, None)),
        Err(Error::DraftRevisionConflict)
    );
    let stored = block_on(store.authored_draft_head(draft.draft_id()))
        .unwrap()
        .unwrap();
    assert_eq!(stored, draft);
    let restored = DurableAuthoredPlan::reconstruct(stored.payload().to_vec()).unwrap();
    assert_eq!(restored.decode().unwrap().plan(), &plan);
    let fields: serde_json::Value = serde_json::from_str(plan.body().content()).unwrap();
    assert_eq!(fields["display_name"], "Updated farm");
    assert_eq!(fields["picture"], 42);
    assert_eq!(fields["website"], "https://mossstreet.example");
    assert_eq!(fields["bot"], false);
}
