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
    event::SourceGeneration,
    memory::MemoryStorage,
};
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths, SqliteStorage};

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

async fn exercise(store: &dyn AuthoredDraftStore, draft: &AuthoredDraft) {
    store
        .append_authored_draft(draft.clone(), None)
        .await
        .unwrap();
    assert_eq!(
        store
            .append_authored_draft(draft.clone(), None)
            .await
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
        store.append_authored_draft(conflicting, None).await,
        Err(Error::DraftRevisionConflict)
    );
    assert_eq!(
        store
            .authored_draft_head(draft.draft_id())
            .await
            .unwrap()
            .unwrap(),
        *draft
    );
}

#[tokio::test]
async fn profile_complete_edit_sqlite_and_memory_durable_replay_and_readonly_reopen_preserve_metadata()
 {
    let (plan, draft) = edit_draft();
    let directory = tempfile::tempdir().unwrap();
    let paths = Paths::from_directory(directory.path()).unwrap();
    let store = SqliteStorage::open(
        OpenOptions::new(paths.clone(), OpenMode::Create)
            .with_source_generation(SourceGeneration::new([41; 32]).unwrap(), 100)
            .unwrap(),
    )
    .await
    .unwrap();
    exercise(&MemoryStorage::default(), &draft).await;
    exercise(&store, &draft).await;
    store.close().await.unwrap();
    let reader = SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadOnly))
        .await
        .unwrap();
    let stored = reader
        .authored_draft_head(draft.draft_id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored, draft);
    let restored = DurableAuthoredPlan::reconstruct(stored.payload().to_vec()).unwrap();
    assert_eq!(restored.decode().unwrap().plan(), &plan);
    assert_eq!(
        reader.append_authored_draft(draft.clone(), None).await,
        Err(Error::BackendUnavailable)
    );
    let fields: serde_json::Value = serde_json::from_str(plan.body().content()).unwrap();
    assert_eq!(fields["display_name"], "Updated farm");
    assert_eq!(fields["picture"], 42);
    assert_eq!(fields["website"], "https://mossstreet.example");
    assert_eq!(fields["bot"], false);
    reader.close().await.unwrap();
}
