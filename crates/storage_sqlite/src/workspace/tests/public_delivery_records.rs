//! Actual synthetic native T0/T1/S/T2/D records, without network dispatch.
//! Codec-only report fixtures retain their separate, narrower claims.
use super::*;
use crate::workspace::{listing_installation, snapshot::RetainedGraph};
use radroots_protocol::listing::PublicSigningPlan;
use radroots_storage::command::{
    DeliveryAttemptOutcome, DeliveryAttemptRequest, DeliveryIntent, InstallationOutcome,
    InstallationRequest, OperationError, SigningTarget,
};
use radroots_transport::{
    Target, TargetSet,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
};

async fn actual() -> (DeliveryData, WorkspaceBinding) {
    let fixture = CryptoFixture::new();
    let path = fixture
        ._temp
        .path()
        .canonicalize()
        .unwrap()
        .join("actual-public");
    let (store, selected, proof) = NativeWorkspace::initialize_local_outbox(
        &path,
        LocalOutboxConfig::seller(
            participant(),
            participant(),
            "public-records",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap(),
        fixture.crypto(),
        Arc::new(FixedClock(100)),
    )
    .await
    .unwrap();
    let binding = store.authenticate(selected.clone(), &proof).await.unwrap();
    let copied = || proof.with_secret(|raw| AttachmentProof::from_protected_input(raw).unwrap());
    let time = UnixSeconds::new(100).unwrap();
    let context = CommandContextData::new(
        OperationId::from_bytes([91; 32]),
        binding.actor(),
        selected.workspace_id(),
        selected.authority(),
        selected.route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    let wire = json!({"type":"listing.publish","context":context,"input":{
        "id":"04".repeat(16),"title":"Actual public records","summary":"Quote required",
        "description_markdown":"Retained public terms","published_at":"100",
        "profile":{"profile":"radroots.listing","version":1,"state":"active",
        "product":{"name":"Carrots","description":"Public description","producer":null,"origin_summary":null},
        "default_option":"02".repeat(32),"options":[{"id":"02".repeat(32),"label":"Bunch",
        "unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},
        "increment":"1","minimum":"1","price":{"type":"quote_required"},
        "supply":{"type":"available","amount":"10"},"lead_seconds":"0"}],
        "fulfillment":{"modes":["pickup"],"area":"Example","lead_seconds":"0"},"authority":null,"media":[]}}});
    let request =
        AdmissionRequest::from_protected_wire(binding.clone(), &serde_json::to_vec(&wire).unwrap())
            .unwrap();
    let plan = PublicSigningPlan::capture(
        request.command().listing().unwrap(),
        time,
        &["wss://route.example/".into()],
        SigningReplay::NonReplayable,
    )
    .unwrap();
    let plan_digest = *plan.digest();
    let command_digest = request.command().digest();
    assert!(matches!(
        store
            .admit_command_and_hold(request, copied())
            .await
            .unwrap(),
        AdmissionOutcome::Committed(_)
    ));
    assert!(matches!(
        store
            .bind_signing_intents(binding.clone(), plan.into(), copied())
            .await
            .unwrap(),
        PlanBindingOutcome::Committed(_)
    ));
    let attempt = match store
        .claim_attempt(
            ClaimAttemptRequest::for_artifact(
                binding.clone(),
                OperationId::from_bytes([91; 32]),
                command_digest,
                plan_digest,
                SigningTarget::PublicEvent,
            ),
            copied(),
        )
        .await
        .unwrap()
    {
        ClaimAttemptOutcome::Claimed(value) => value,
        _ => panic!("actual public signing claim"),
    };
    let artifact = attempt.artifact().unwrap();
    let fields = artifact.fields();
    let mut scalar = [0; 32];
    scalar[31] = 1;
    let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&scalar).unwrap());
    let signed = nostr::UnsignedEvent {
        id: None,
        pubkey: keys.public_key(),
        created_at: nostr::Timestamp::from_secs(fields.created_at),
        kind: nostr::Kind::Custom(fields.kind.try_into().unwrap()),
        tags: nostr::Tags::from_list(
            fields
                .tags
                .iter()
                .map(|v| nostr::Tag::parse(v).unwrap())
                .collect(),
        ),
        content: fields.content.into(),
    }
    .sign_with_keys(&keys)
    .unwrap();
    signed.verify().unwrap();
    let evidence = ArtifactEvidence::verify(&attempt, &signed.as_json(), None).unwrap();
    assert!(matches!(
        store
            .capture_evidence(selected.clone(), &proof, &evidence)
            .await
            .unwrap(),
        EvidenceCaptureOutcome::Committed(_)
    ));
    assert!(matches!(
        store
            .install_transition_and_outbox(
                InstallationRequest::new(
                    binding.clone(),
                    OperationId::from_bytes([91; 32]),
                    command_digest,
                    plan_digest
                ),
                copied()
            )
            .await
            .unwrap(),
        InstallationOutcome::ListingCommitted(_)
    ));
    let snapshot = store
        .operation_snapshot(
            binding.clone(),
            OperationId::from_bytes([91; 32]),
            Some(command_digest),
            copied(),
        )
        .await
        .unwrap()
        .unwrap();
    let work = snapshot.public_publications()[0];
    let targets =
        TargetSet::new(vec![Target::nostr_relay("wss://route.example/").unwrap()]).unwrap();
    let intent = DeliveryIntent::public(
        &"ab".repeat(32),
        work.evidence().event_id(),
        &targets,
        &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        core::num::NonZeroU64::new(110_000).unwrap(),
    )
    .unwrap();
    let report = match store
        .prepare_delivery_attempt(
            DeliveryAttemptRequest::new(binding.clone(), work, intent).unwrap(),
            copied(),
            None,
        )
        .await
        .unwrap()
    {
        DeliveryAttemptOutcome::Claimed(value) => *value,
        _ => panic!("actual public delivery claim"),
    };
    store.close().await.unwrap();
    let issued = Issued {
        fixture,
        selected,
        attempt,
        evidence,
        proof,
        path,
    };
    let database = issued.database().await;
    let crypto = issued.fixture.crypto();
    (
        DeliveryData {
            _issued: issued,
            database,
            crypto,
            report,
        },
        binding,
    )
}

#[tokio::test]
async fn actual_public_records_refuse_reencoded_scope_and_nonatomic_initial_work() {
    let (mut data, binding) = actual().await;
    let selected = data.selection();
    let e = data.report.work().evidence();
    let graph = RetainedGraph::read(
        &mut data.database,
        &selected,
        &binding,
        e.operation_id(),
        Some(e.command_digest()),
        &data.crypto,
    )
    .await
    .unwrap();
    let retained = listing_installation::Records::read(
        &mut data.database,
        &selected,
        e.operation_id(),
        &graph,
        &data.crypto,
    )
    .await
    .unwrap();
    let scope = crate::workspace::crypto::IssuanceScope {
        workspace: e.workspace_id(),
        operation: e.operation_id(),
        plan_digest: *data.report.work().plan_digest(),
        attempt: e.attempt_id(),
        target: SigningTarget::PublicEvent,
    };
    for (table, payload, changes) in [
        (
            "listing_installations",
            IssuancePayload::ListingInstallation,
            vec![
                ("/version", json!(2)),
                ("/plan", json!(vec![9u8; 32])),
                ("/preparation", json!("99".repeat(32))),
                ("/plan_transaction", json!("99".repeat(32))),
                ("/evidence/operation", json!("99".repeat(32))),
            ],
        ),
        (
            "public_publication_work",
            IssuancePayload::Publication,
            vec![
                ("/version", json!(2)),
                ("/installation", json!("99".repeat(32))),
                ("/plan", json!(vec![9u8; 32])),
                ("/evidence/attempt", json!("99".repeat(32))),
                ("/queued_at", json!("101")),
            ],
        ),
    ] {
        let (read, update) = match table {
            "listing_installations" => (
                "SELECT protected FROM listing_installations",
                "UPDATE listing_installations SET protected=?",
            ),
            "public_publication_work" => (
                "SELECT protected FROM public_publication_work",
                "UPDATE public_publication_work SET protected=?",
            ),
            _ => unreachable!("fixed fixture tables"),
        };
        let original: Vec<u8> = sqlx::query_scalar(read)
            .fetch_one(&mut data.database)
            .await
            .unwrap();
        let decoded: Value = data
            .crypto
            .open_issuance(scope, payload, 0, &original)
            .await
            .unwrap()
            .expose_secret(|raw| serde_json::from_slice(raw).unwrap());
        for (pointer, value) in changes {
            let mut changed = decoded.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            let protected = data
                .crypto
                .seal_issuance(scope, payload, 0, &serde_json::to_vec(&changed).unwrap())
                .await
                .unwrap();
            sqlx::query(update)
                .bind(&protected)
                .execute(&mut data.database)
                .await
                .unwrap();
            assert!(
                matches!(
                    listing_installation::Records::read(
                        &mut data.database,
                        &selected,
                        e.operation_id(),
                        &graph,
                        &data.crypto
                    )
                    .await,
                    Err(OperationError::RecoveryRequired)
                ),
                "{table}: {pointer}"
            );
            assert!(
                retained
                    .unchanged(&mut data.database, &selected, e.operation_id())
                    .await
                    .is_err()
            );
            let untouched: Vec<u8> = sqlx::query_scalar(read)
                .fetch_one(&mut data.database)
                .await
                .unwrap();
            assert_eq!(
                untouched, protected,
                "validation must not repair persisted data"
            );
            sqlx::query(update)
                .bind(&original)
                .execute(&mut data.database)
                .await
                .unwrap();
        }
        // Equivalent plaintext with a new protected representation still
        // invalidates an already observed exact-fact fence.
        let different = data
            .crypto
            .seal_issuance(scope, payload, 0, &serde_json::to_vec(&decoded).unwrap())
            .await
            .unwrap();
        assert_ne!(different, original);
        sqlx::query(update)
            .bind(&different)
            .execute(&mut data.database)
            .await
            .unwrap();
        assert!(
            listing_installation::Records::read(
                &mut data.database,
                &selected,
                e.operation_id(),
                &graph,
                &data.crypto
            )
            .await
            .is_ok()
        );
        assert!(
            retained
                .unchanged(&mut data.database, &selected, e.operation_id())
                .await
                .is_err()
        );
        sqlx::query(update)
            .bind(&original)
            .execute(&mut data.database)
            .await
            .unwrap();
    }
    let original: Vec<u8> = sqlx::query_scalar("SELECT protected FROM public_publication_work")
        .fetch_one(&mut data.database)
        .await
        .unwrap();
    let mut changed: Value = data
        .crypto
        .open_issuance(scope, IssuancePayload::Publication, 0, &original)
        .await
        .unwrap()
        .expose_secret(|raw| serde_json::from_slice(raw).unwrap());
    changed["transaction"] = json!("99".repeat(32));
    let protected = data
        .crypto
        .seal_issuance(
            scope,
            IssuancePayload::Publication,
            0,
            &serde_json::to_vec(&changed).unwrap(),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE public_publication_work SET transaction_id=?,protected=?")
        .bind([0x99u8; 32].as_slice())
        .bind(protected)
        .execute(&mut data.database)
        .await
        .unwrap();
    assert!(matches!(
        listing_installation::Records::read(
            &mut data.database,
            &selected,
            e.operation_id(),
            &graph,
            &data.crypto
        )
        .await,
        Err(OperationError::RecoveryRequired)
    ));
    sqlx::query("UPDATE public_publication_work SET transaction_id=?,protected=?")
        .bind(data.report.work().transaction_id().as_bytes().as_slice())
        .bind(original)
        .execute(&mut data.database)
        .await
        .unwrap();
    retained
        .unchanged(&mut data.database, &selected, e.operation_id())
        .await
        .unwrap();
    assert!(
        listing_installation::absent(&mut data.database, &selected, e.operation_id())
            .await
            .is_err()
    );
    data.database.close().await.unwrap();
}
