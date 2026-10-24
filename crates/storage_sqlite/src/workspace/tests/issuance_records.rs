use super::*;
use crate::workspace::{
    attempt as attempt_store, crypto::IssuancePayload, evidence as evidence_store, operation,
    plan as plan_store,
};
use nostr::{JsonUtil, nips::nip44};
use radroots_domain::{
    CommandContextData, EnvelopeEventId, OperationId, SigningAttemptId, TransactionId,
};
use radroots_protocol::{
    chat::{CapturedDmRoute, ChatRumor, ChatSeal, ChatSigningPlan},
    command::{CapturedChatCommand, InquireInput},
    plan::SigningReplay,
};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, ArtifactEvidence, BoundSigningPlan, ClaimAttemptOutcome,
    ClaimAttemptRequest, EvidenceCaptureOutcome, OperationJournal, PlanBindingOutcome,
    SigningAttempt,
};
use serde_json::{Value, json};
use sqlx::{Connection, SqliteConnection};
#[path = "public_delivery_records.rs"]
mod public_delivery_records;

struct Issued {
    fixture: CryptoFixture,
    selected: AttachmentInput,
    attempt: SigningAttempt,
    evidence: ArtifactEvidence,
    proof: AttachmentProof,
    path: std::path::PathBuf,
}

/// Codec-only data fixture. Structural reports and encrypted records here do
/// not establish native T2/D authorization; the facade owns those runtime cases.
#[cfg(test)]
pub(in crate::workspace) struct DeliveryData {
    _issued: Issued,
    pub(in crate::workspace) database: SqliteConnection,
    pub(in crate::workspace) crypto: WorkspaceCrypto,
    pub(in crate::workspace) report: radroots_storage::command::DeliveryAttempt,
}
#[cfg(test)]
impl DeliveryData {
    pub(in crate::workspace) fn selection(&self) -> AttachmentInput {
        self._issued.selected.clone()
    }
    pub(in crate::workspace) async fn bound(&mut self) -> BoundSigningPlan {
        self._issued.bound(&mut self.database).await
    }
}
#[cfg(test)]
pub(in crate::workspace) async fn delivery_data() -> DeliveryData {
    use radroots_storage::command::{
        ChatInstallation, DeliveryAttempt, DeliveryIntent, EvidenceReceipt, PublicationWork,
    };
    use radroots_transport::{
        Target, TargetSet,
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    };
    let issued = Issued::new().await;
    let mut database = issued.database().await;
    let bound = issued.bound(&mut database).await;
    let evidence = EvidenceReceipt::from_committed_store(
        &issued.evidence,
        TransactionId::from_bytes([70; 32]),
    );
    let time = UnixSeconds::new(100).unwrap();
    let installed = ChatInstallation::from_committed_store(
        &bound,
        evidence,
        TransactionId::from_bytes([71; 32]),
        time,
    )
    .unwrap();
    let work = PublicationWork::from_committed_store(
        &bound,
        installed,
        evidence,
        TransactionId::from_bytes([72; 32]),
        time,
    )
    .unwrap();
    let mut scalar = [0; 32];
    scalar[31] = 3;
    let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&scalar).unwrap());
    let recipient = nostr::PublicKey::from_slice(work.recipient().as_bytes()).unwrap();
    let raw = nostr::UnsignedEvent {
        id: None,
        pubkey: keys.public_key(),
        created_at: nostr::Timestamp::from_secs(99),
        kind: nostr::Kind::Custom(1059),
        tags: nostr::Tags::from_list(vec![nostr::Tag::public_key(recipient)]),
        content: nip44::encrypt(
            keys.secret_key(),
            &recipient,
            "codec private record ".repeat(1200),
            nip44::Version::V2,
        )
        .unwrap(),
    }
    .sign_with_keys(&keys)
    .unwrap()
    .as_json();
    let targets =
        TargetSet::new(vec![Target::nostr_relay("wss://route.example/").unwrap()]).unwrap();
    let intent = DeliveryIntent::new(
        &"01".repeat(32),
        EnvelopeEventId::from_bytes(*nostr::Event::from_json(&raw).unwrap().id.as_bytes()),
        &targets,
        &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        core::num::NonZeroU64::new(110_000).unwrap(),
    )
    .unwrap();
    let report = DeliveryAttempt::from_store_report(
        work,
        intent,
        &raw,
        TransactionId::from_bytes([73; 32]),
        time,
        vec![1],
    )
    .unwrap();
    let crypto = issued.fixture.crypto();
    DeliveryData {
        _issued: issued,
        database,
        crypto,
        report,
    }
}

/// Codec-only structural public reports. This fixture does not claim that the
/// database authorized public T0/S/T2/D; actual facade tests cover that path.
#[cfg(test)]
pub(in crate::workspace) async fn public_delivery_data() -> DeliveryData {
    use radroots_domain::{ActorRef, ActorRole, IssuanceWindow, Sequence};
    use radroots_protocol::{listing::PublicSigningPlan, plan::SigningReplay};
    use radroots_storage::{
        command::{
            DeliveryAttempt, DeliveryIntent, EvidenceReceipt, ListingInstallation,
            PreparedOperation, PublicPublicationWork, SigningTarget,
        },
        workspace::{WorkspaceBinding, WorkspaceScope},
    };
    let mut data = delivery_data().await;
    let selected = data.selection();
    let time = UnixSeconds::new(100).unwrap();
    let binding = WorkspaceBinding::from_authenticated_host(
        selected.clone(),
        WorkspaceScope::NewSellerOutbox,
        ActorRef::new(
            participant(),
            ActorRole::Seller,
            None,
            Some(selected.grant_id()),
        ),
        "codec-only",
        Sequence::new(1).unwrap(),
        UnixSeconds::new(200).unwrap(),
        time,
    )
    .unwrap();
    let context = CommandContextData::new(
        OperationId::from_bytes([62; 32]),
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
        "id":"01".repeat(16),"title":"Codec public listing","summary":"Quote required",
        "description_markdown":"Public terms","published_at":"100",
        "profile":{"profile":"radroots.listing","version":1,"state":"active",
        "product":{"name":"Carrots","description":"Public description","producer":null,"origin_summary":null},
        "default_option":"02".repeat(32),"options":[{"id":"02".repeat(32),"label":"Bunch",
        "unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},
        "increment":"1","minimum":"1","price":{"type":"quote_required"},
        "supply":{"type":"available","amount":"10"},"lead_seconds":"0"}],
        "fulfillment":{"modes":["pickup"],"area":"Example","lead_seconds":"0"},"authority":null,"media":[]}}});
    let request =
        AdmissionRequest::from_protected_wire(binding, &serde_json::to_vec(&wire).unwrap())
            .unwrap();
    let plan = PublicSigningPlan::capture(
        request.command().listing().unwrap(),
        time,
        &["wss://route.example/".into()],
        SigningReplay::NonReplayable,
    )
    .unwrap();
    let prepared = PreparedOperation::from_committed_store(
        request,
        TransactionId::from_bytes([80; 32]),
        Sequence::new(1).unwrap(),
        IssuanceWindow::new(time, UnixSeconds::new(150).unwrap()).unwrap(),
    )
    .unwrap();
    let bound = BoundSigningPlan::from_committed_store(
        prepared,
        plan,
        TransactionId::from_bytes([81; 32]),
        time,
    )
    .unwrap();
    let attempt = SigningAttempt::from_committed_store(
        bound,
        SigningTarget::PublicEvent,
        SigningAttemptId::from_bytes([82; 32]),
        TransactionId::from_bytes([83; 32]),
        time,
    )
    .unwrap();
    let fields = attempt
        .bound_plan()
        .plan()
        .listing()
        .unwrap()
        .event()
        .fields();
    let mut scalar = [0; 32];
    scalar[31] = 1;
    let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&scalar).unwrap());
    let raw = nostr::UnsignedEvent {
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
    .unwrap()
    .as_json();
    let evidence = ArtifactEvidence::verify(&attempt, &raw, None).unwrap();
    let receipt =
        EvidenceReceipt::from_committed_store(&evidence, TransactionId::from_bytes([84; 32]));
    let installed = ListingInstallation::from_committed_store(
        attempt.bound_plan(),
        receipt,
        TransactionId::from_bytes([85; 32]),
        time,
    )
    .unwrap();
    let work = PublicPublicationWork::from_committed_store(
        attempt.bound_plan(),
        installed,
        receipt,
        TransactionId::from_bytes([86; 32]),
        time,
    )
    .unwrap();
    let intent = DeliveryIntent::public(
        &"03".repeat(32),
        receipt.event_id(),
        data.report.intent().targets(),
        data.report.intent().policy(),
        data.report.intent().deadline_unix_ms(),
    )
    .unwrap();
    data.report = DeliveryAttempt::from_store_report(
        work,
        intent,
        &raw,
        TransactionId::from_bytes([87; 32]),
        time,
        vec![1],
    )
    .unwrap();
    data
}

impl Issued {
    async fn new() -> Self {
        let fixture = CryptoFixture::new();
        let path = fixture
            ._temp
            .path()
            .canonicalize()
            .unwrap()
            .join("issuance-records");
        let (store, selected, proof) = NativeWorkspace::initialize_local_outbox(
            &path,
            LocalOutboxConfig::new(
                participant(),
                participant(),
                "device",
                UnixSeconds::new(200).unwrap(),
            )
            .unwrap(),
            fixture.crypto(),
            Arc::new(FixedClock(100)),
        )
        .await
        .unwrap();
        let binding = store.authenticate(selected.clone(), &proof).await.unwrap();
        let operation = OperationId::from_bytes([61; 32]);
        let context = CommandContextData::new(
            operation,
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
        let command = CapturedChatCommand::inquire(
            context,
            InquireInput::new(
                radroots_domain::ThreadId::from_bytes([7; 32]),
                participant(),
                None,
                &"\0".repeat(3500),
            )
            .unwrap(),
        )
        .unwrap();
        let rumor = ChatRumor::from_command(&command, UnixSeconds::new(100).unwrap()).unwrap();
        let mut scalar = [0; 32];
        scalar[31] = 1;
        let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&scalar).unwrap());
        let ciphertext = nip44::encrypt(
            keys.secret_key(),
            &keys.public_key(),
            rumor.json(),
            nip44::Version::V2,
        )
        .unwrap();
        let seal = ChatSeal::new(
            &rumor,
            participant(),
            UnixSeconds::new(99).unwrap(),
            &ciphertext,
        )
        .unwrap();
        let source = nostr::UnsignedEvent {
            id: None,
            pubkey: keys.public_key(),
            created_at: nostr::Timestamp::from_secs(99),
            kind: nostr::Kind::Custom(10050),
            tags: nostr::Tags::from_list(vec![
                nostr::Tag::parse(["relay", "wss://route.example/"]).unwrap(),
            ]),
            content: "public routing provenance ".repeat(1000),
        }
        .sign_with_keys(&keys)
        .unwrap();
        let route = CapturedDmRoute::from_observed_descriptor(
            participant(),
            EnvelopeEventId::from_bytes(*source.id.as_bytes()),
            &source.as_json(),
            &["wss://route.example/".into()],
        )
        .unwrap();
        let plan = ChatSigningPlan::new(
            &command,
            UnixSeconds::new(100).unwrap(),
            vec![seal],
            vec![route],
            SigningReplay::NonReplayable,
        )
        .unwrap();
        let digest = *plan.digest();
        let command_digest = command.digest();
        let copied =
            || proof.with_secret(|value| AttachmentProof::from_protected_input(value).unwrap());
        assert!(matches!(
            store
                .admit_command_and_hold(
                    AdmissionRequest::new(binding.clone(), command).unwrap(),
                    copied()
                )
                .await
                .unwrap(),
            AdmissionOutcome::Committed(_)
        ));
        assert!(matches!(
            store
                .bind_signing_intents(binding.clone(), (plan).into(), copied())
                .await
                .unwrap(),
            PlanBindingOutcome::Committed(_)
        ));
        let attempt = match store
            .claim_attempt(
                ClaimAttemptRequest::new(binding, operation, command_digest, digest, participant()),
                copied(),
            )
            .await
            .unwrap()
        {
            ClaimAttemptOutcome::Claimed(value) => value,
            _ => panic!("first claim must confirm"),
        };
        let fields = attempt.seal().unwrap().fields();
        let signed = nostr::UnsignedEvent {
            id: Some(nostr::EventId::from_slice(&attempt.seal().unwrap().event_id()).unwrap()),
            pubkey: keys.public_key(),
            created_at: nostr::Timestamp::from_secs(fields.created_at),
            kind: nostr::Kind::Custom(13),
            tags: nostr::Tags::new(),
            content: fields.content.into(),
        }
        .sign_with_keys(&keys)
        .unwrap();
        signed.verify().unwrap();
        let evidence = ArtifactEvidence::verify(&attempt, &signed.as_json(), None).unwrap();
        assert!(evidence.raw_json().len() > 16384);
        assert!(matches!(
            store
                .capture_evidence(selected.clone(), &proof, &evidence)
                .await
                .unwrap(),
            EvidenceCaptureOutcome::Committed(_)
        ));
        store.close().await.unwrap();
        Self {
            fixture,
            selected,
            attempt,
            evidence,
            proof,
            path,
        }
    }
    async fn database(&self) -> SqliteConnection {
        let mut connection = SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(self.path.join("execution_v1.sqlite"))
                .create_if_missing(false)
                .foreign_keys(false),
        )
        .await
        .unwrap();
        sqlx::query("PRAGMA ignore_check_constraints=ON")
            .execute(&mut connection)
            .await
            .unwrap();
        connection
    }
    async fn bound(&self, connection: &mut SqliteConnection) -> BoundSigningPlan {
        let original = self.attempt.bound_plan().prepared().identity();
        let prepared = operation::read(
            connection,
            &self.selected,
            original.operation_id(),
            &self.fixture.crypto(),
        )
        .await
        .unwrap()
        .unwrap()
        .0;
        let (prepared, value) =
            plan_store::read(connection, &self.selected, prepared, &self.fixture.crypto())
                .await
                .unwrap();
        let (plan, transaction, time, _) = value.unwrap();
        BoundSigningPlan::from_committed_store(prepared, plan, transaction, time).unwrap()
    }
}
#[tokio::test]
async fn authenticated_attempt_headers_and_all_indexed_facts_are_checked_without_repair() {
    let issued = Issued::new().await;
    let mut database = issued.database().await;
    let crypto = issued.fixture.crypto();
    let scope = attempt_store::scope(&issued.attempt).unwrap();
    let original: Vec<u8> = sqlx::query_scalar("SELECT protected FROM signing_attempts")
        .fetch_one(&mut database)
        .await
        .unwrap();
    let encoded: Value = crypto
        .open_issuance(scope, IssuancePayload::Attempt, 0, &original)
        .await
        .unwrap()
        .expose_secret(|bytes| serde_json::from_slice(bytes).unwrap());
    let other =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    for (field, value) in [
        ("version", json!(2)),
        ("workspace", json!("99".repeat(32))),
        ("operation_id", json!("99".repeat(32))),
        ("command_digest", json!("99".repeat(32))),
        ("plan_digest", json!(vec![9u8; 32])),
        ("recipient", json!(other)),
        ("attempt_id", json!("99".repeat(32))),
        ("transaction_id", json!("99".repeat(32))),
        ("preparation_transaction", json!("99".repeat(32))),
        ("plan_transaction", json!("99".repeat(32))),
        ("grant_id", json!("99".repeat(32))),
        ("intent_digest", json!(vec![9u8; 32])),
        ("deadline_unix_ms", json!(149000)),
        ("expected_event", json!("99".repeat(32))),
        ("claimed_at", json!("99")),
        ("claimed_at", json!("150")),
    ] {
        let mut changed = encoded.clone();
        changed[field] = value;
        let protected = crypto
            .seal_issuance(
                scope,
                IssuancePayload::Attempt,
                0,
                &serde_json::to_vec(&changed).unwrap(),
            )
            .await
            .unwrap();
        sqlx::query("UPDATE signing_attempts SET protected=?")
            .bind(&protected)
            .execute(&mut database)
            .await
            .unwrap();
        let bound = issued.bound(&mut database).await;
        let error = attempt_store::read(
            &mut database,
            &issued.selected,
            bound,
            participant(),
            &crypto,
        )
        .await
        .expect_err("inconsistent authenticated claim must fail");
        assert_eq!(error.code(), "recovery_required", "{field}");
        let retained: Vec<u8> = sqlx::query_scalar("SELECT protected FROM signing_attempts")
            .fetch_one(&mut database)
            .await
            .unwrap();
        assert_eq!(retained, protected);
    }
    sqlx::query("UPDATE signing_attempts SET protected=?")
        .bind(&original)
        .execute(&mut database)
        .await
        .unwrap();
    for change in [
        "UPDATE signing_attempts SET plan_digest=zeroblob(32)",
        "UPDATE signing_attempts SET intent_digest=zeroblob(32)",
        "UPDATE signing_attempts SET protected=zeroblob(262145)",
    ] {
        sqlx::query(change).execute(&mut database).await.unwrap();
        let bound = issued.bound(&mut database).await;
        assert_eq!(
            attempt_store::read(
                &mut database,
                &issued.selected,
                bound,
                participant(),
                &crypto
            )
            .await
            .err()
            .unwrap()
            .code(),
            "recovery_required"
        );
        sqlx::query("UPDATE signing_attempts SET plan_digest=?,intent_digest=?,protected=?")
            .bind(issued.attempt.bound_plan().plan().digest().as_slice())
            .bind(issued.attempt.binding().digest().as_slice())
            .bind(&original)
            .execute(&mut database)
            .await
            .unwrap();
    }
    let identity = issued.attempt.bound_plan().prepared().identity();
    for change in [
        "UPDATE signing_attempts SET workspace=zeroblob(32)",
        "UPDATE signing_attempts SET operation_id=zeroblob(32)",
        "UPDATE signing_attempts SET artifact_key=zeroblob(32)",
        "UPDATE signing_attempts SET attempt_id=zeroblob(32)",
        "UPDATE signing_attempts SET transaction_id=zeroblob(32)",
        "UPDATE signing_attempts SET plan_digest=zeroblob(32)",
        "UPDATE signing_attempts SET intent_digest=zeroblob(32)",
        "UPDATE signing_attempts SET protected=zeroblob(1)",
    ] {
        sqlx::query(change).execute(&mut database).await.unwrap();
        let mut transaction = database.begin_with("BEGIN IMMEDIATE").await.unwrap();
        assert!(
            attempt_store::unchanged(&mut transaction, &issued.attempt, &original)
                .await
                .is_err(),
            "{change}"
        );
        transaction.rollback().await.unwrap();
        sqlx::query("UPDATE signing_attempts SET workspace=?,operation_id=?,artifact_key=?,attempt_id=?,transaction_id=?,plan_digest=?,intent_digest=?,protected=?").bind(identity.workspace_id().as_bytes().as_slice()).bind(identity.operation_id().as_bytes().as_slice()).bind(participant().as_bytes().as_slice()).bind(issued.attempt.attempt_id().as_bytes().as_slice()).bind(issued.attempt.transaction_id().as_bytes().as_slice()).bind(issued.attempt.bound_plan().plan().digest().as_slice()).bind(issued.attempt.binding().digest().as_slice()).bind(&original).execute(&mut database).await.unwrap();
    }
    database.close().await.unwrap();
}
#[tokio::test]
async fn authenticated_evidence_manifests_parts_and_exact_original_observations_are_checked() {
    let issued = Issued::new().await;
    let mut database = issued.database().await;
    let crypto = issued.fixture.crypto();
    let scope = attempt_store::scope(&issued.attempt).unwrap();
    let original: Vec<u8> = sqlx::query_scalar("SELECT protected FROM artifact_evidence")
        .fetch_one(&mut database)
        .await
        .unwrap();
    let manifest: Value = crypto
        .open_issuance(scope, IssuancePayload::Evidence, 0, &original)
        .await
        .unwrap()
        .expose_secret(|bytes| serde_json::from_slice(bytes).unwrap());
    let other =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    for (field, value) in [
        ("version", json!(2)),
        ("workspace", json!("99".repeat(32))),
        ("operation_id", json!("99".repeat(32))),
        ("command_digest", json!("99".repeat(32))),
        ("plan_digest", json!(vec![9u8; 32])),
        ("recipient", json!(other)),
        ("attempt_id", json!("99".repeat(32))),
        ("claim_transaction", json!("99".repeat(32))),
        ("intent_digest", json!(vec![9u8; 32])),
        ("event_id", json!("99".repeat(32))),
        ("evidence_digest", json!(vec![9u8; 32])),
        ("transaction_id", json!("99".repeat(32))),
        ("part_count", json!(4)),
        ("byte_count", json!(1)),
        ("observed_at", json!(0)),
        ("observed_at", json!(253_402_300_800_000u64)),
    ] {
        let mut changed = manifest.clone();
        changed[field] = value;
        let protected = crypto
            .seal_issuance(
                scope,
                IssuancePayload::Evidence,
                0,
                &serde_json::to_vec(&changed).unwrap(),
            )
            .await
            .unwrap();
        sqlx::query("UPDATE artifact_evidence SET protected=?")
            .bind(&protected)
            .execute(&mut database)
            .await
            .unwrap();
        let error = evidence_store::read(&mut database, &issued.attempt, &crypto)
            .await
            .err()
            .expect("inconsistent authenticated evidence must fail");
        assert_eq!(error.code(), "recovery_required", "{field}");
        let retained: Vec<u8> = sqlx::query_scalar("SELECT protected FROM artifact_evidence")
            .fetch_one(&mut database)
            .await
            .unwrap();
        assert_eq!(retained, protected);
    }
    // Internally consistent authenticated metadata must still match the
    // independent SQL indexes. Invalid metadata alone does not prove this.
    let original_bytes = manifest["byte_count"].as_u64().unwrap();
    let same_parts = if original_bytes.is_multiple_of(16_384) {
        original_bytes - 1
    } else {
        original_bytes + 1
    };
    for byte_count in [same_parts, 1] {
        let mut changed = manifest.clone();
        changed["byte_count"] = json!(byte_count);
        changed["part_count"] = json!(byte_count.div_ceil(16_384));
        let protected = crypto
            .seal_issuance(
                scope,
                IssuancePayload::Evidence,
                0,
                &serde_json::to_vec(&changed).unwrap(),
            )
            .await
            .unwrap();
        sqlx::query("UPDATE artifact_evidence SET protected=?")
            .bind(&protected)
            .execute(&mut database)
            .await
            .unwrap();
        assert_eq!(
            evidence_store::read(&mut database, &issued.attempt, &crypto)
                .await
                .err()
                .unwrap()
                .code(),
            "recovery_required"
        );
        let retained: Vec<u8> = sqlx::query_scalar("SELECT protected FROM artifact_evidence")
            .fetch_one(&mut database)
            .await
            .unwrap();
        assert_eq!(retained, protected);
    }
    sqlx::query("UPDATE artifact_evidence SET protected=?")
        .bind(&original)
        .execute(&mut database)
        .await
        .unwrap();
    let header:(Vec<u8>,Vec<u8>,Vec<u8>,Vec<u8>,i64,i64)=sqlx::query_as("SELECT attempt_id,event_id,evidence_digest,transaction_id,part_count,byte_count FROM artifact_evidence").fetch_one(&mut database).await.unwrap();
    for change in [
        "UPDATE artifact_evidence SET attempt_id=zeroblob(32)",
        "UPDATE artifact_evidence SET event_id=zeroblob(32)",
        "UPDATE artifact_evidence SET evidence_digest=zeroblob(32)",
        "UPDATE artifact_evidence SET transaction_id=zeroblob(32)",
        "UPDATE artifact_evidence SET part_count=5",
        "UPDATE artifact_evidence SET byte_count=65537",
        "UPDATE artifact_evidence SET part_count=1",
        "UPDATE artifact_evidence SET protected=zeroblob(262145)",
    ] {
        sqlx::query(change).execute(&mut database).await.unwrap();
        assert_eq!(
            evidence_store::read(&mut database, &issued.attempt, &crypto)
                .await
                .err()
                .unwrap()
                .code(),
            "recovery_required",
            "{change}"
        );
        sqlx::query("UPDATE artifact_evidence SET attempt_id=?,event_id=?,evidence_digest=?,transaction_id=?,part_count=?,byte_count=?,protected=?").bind(&header.0).bind(&header.1).bind(&header.2).bind(&header.3).bind(header.4).bind(header.5).bind(&original).execute(&mut database).await.unwrap();
    }
    // A valid protected manifest matching a changed SQL digest still cannot
    // redefine the hash of the original authenticated raw signature bytes.
    let mut changed = manifest.clone();
    changed["evidence_digest"] = json!(vec![9u8; 32]);
    let protected = crypto
        .seal_issuance(
            scope,
            IssuancePayload::Evidence,
            0,
            &serde_json::to_vec(&changed).unwrap(),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE artifact_evidence SET evidence_digest=?,protected=?")
        .bind([9u8; 32].as_slice())
        .bind(&protected)
        .execute(&mut database)
        .await
        .unwrap();
    assert_eq!(
        evidence_store::read(&mut database, &issued.attempt, &crypto)
            .await
            .err()
            .unwrap()
            .code(),
        "recovery_required"
    );
    sqlx::query("UPDATE artifact_evidence SET evidence_digest=?,protected=?")
        .bind(&header.2)
        .bind(&original)
        .execute(&mut database)
        .await
        .unwrap();
    let parts: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT part,protected FROM artifact_evidence_parts ORDER BY part")
            .fetch_all(&mut database)
            .await
            .unwrap();
    assert!(parts.len() > 1);
    for fault in 0..5 {
        sqlx::query("DELETE FROM artifact_evidence_parts")
            .execute(&mut database)
            .await
            .unwrap();
        for (part, protected) in &parts {
            sqlx::query("INSERT INTO artifact_evidence_parts(workspace,operation_id,artifact_key,part,protected) VALUES(?,?,?,?,?)").bind(scope.workspace.as_bytes().as_slice()).bind(scope.operation.as_bytes().as_slice()).bind(scope.target.key().as_slice()).bind(part).bind(protected).execute(&mut database).await.unwrap();
        }
        match fault {
            0 => {
                sqlx::query("DELETE FROM artifact_evidence_parts WHERE part=1")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            1 => {
                sqlx::query("UPDATE artifact_evidence_parts SET protected=zeroblob(0)")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            2 => {
                sqlx::query(
                    "UPDATE artifact_evidence_parts SET protected=zeroblob(1048577) WHERE part=1",
                )
                .execute(&mut database)
                .await
                .unwrap();
            }
            3 => {
                sqlx::query("UPDATE artifact_evidence_parts SET part=0 WHERE part=1")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            4 => {
                let protected = crypto
                    .seal_issuance(scope, IssuancePayload::Evidence, 1, &[0])
                    .await
                    .unwrap();
                sqlx::query("UPDATE artifact_evidence_parts SET protected=? WHERE part=1")
                    .bind(protected)
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            evidence_store::read(&mut database, &issued.attempt, &crypto)
                .await
                .err()
                .unwrap()
                .code(),
            "recovery_required",
            "part fault {fault}"
        );
    }
    sqlx::query("DELETE FROM artifact_evidence_parts")
        .execute(&mut database)
        .await
        .unwrap();
    for (part, protected) in &parts {
        sqlx::query("INSERT INTO artifact_evidence_parts(workspace,operation_id,artifact_key,part,protected) VALUES(?,?,?,?,?)").bind(scope.workspace.as_bytes().as_slice()).bind(scope.operation.as_bytes().as_slice()).bind(scope.target.key().as_slice()).bind(part).bind(protected).execute(&mut database).await.unwrap();
    }
    let (_, protected) = evidence_store::read(&mut database, &issued.attempt, &crypto)
        .await
        .unwrap()
        .unwrap();
    assert!(
        evidence_store::unchanged(&mut database, &issued.attempt, None)
            .await
            .is_err()
    );
    for fault in 0..4 {
        match fault {
            0 => {
                sqlx::query("UPDATE artifact_evidence SET transaction_id=zeroblob(32)")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            1 => {
                sqlx::query(
                    "UPDATE artifact_evidence_parts SET protected=zeroblob(1) WHERE part=1",
                )
                .execute(&mut database)
                .await
                .unwrap();
            }
            2 => {
                sqlx::query("DELETE FROM artifact_evidence_parts WHERE part=1")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            3 => {
                sqlx::query("DELETE FROM artifact_evidence")
                    .execute(&mut database)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            evidence_store::unchanged(&mut database, &issued.attempt, Some(&protected))
                .await
                .is_err()
        );
        if fault == 0 {
            sqlx::query("UPDATE artifact_evidence SET transaction_id=?")
                .bind(&header.3)
                .execute(&mut database)
                .await
                .unwrap();
        }
        if fault == 1 {
            sqlx::query("UPDATE artifact_evidence_parts SET protected=? WHERE part=1")
                .bind(&parts[0].1)
                .execute(&mut database)
                .await
                .unwrap();
        }
        if fault == 2 {
            sqlx::query("INSERT INTO artifact_evidence_parts(workspace,operation_id,artifact_key,part,protected) VALUES(?,?,?,1,?)").bind(scope.workspace.as_bytes().as_slice()).bind(scope.operation.as_bytes().as_slice()).bind(scope.target.key().as_slice()).bind(&parts[0].1).execute(&mut database).await.unwrap();
        }
    }
    assert!(
        evidence_store::unchanged(&mut database, &issued.attempt, None)
            .await
            .is_err()
    );
    let fake = SigningAttempt::from_committed_store(
        issued.bound(&mut database).await,
        participant(),
        SigningAttemptId::from_bytes([77; 32]),
        TransactionId::from_bytes([78; 32]),
        UnixSeconds::new(100).unwrap(),
    )
    .unwrap();
    assert!(
        evidence_store::protect(
            &fake,
            &issued.evidence,
            TransactionId::from_bytes([79; 32]),
            &crypto
        )
        .await
        .is_err()
    );
    database.close().await.unwrap();
}

#[tokio::test]
async fn actual_capture_turn_refuses_uncertainty_foreign_scope_and_fabricated_claim_reports() {
    use crate::workspace::{admission::Journal, fs::Namespace, issuance::absent_claim};
    let issued = Issued::new().await;
    let foreign = Issued::new().await;
    let mut database = issued.database().await;
    let crypto = issued.fixture.crypto();
    let namespace = Namespace::acquire(&issued.path, false).unwrap();
    let bound = issued.bound(&mut database).await;
    let fake = SigningAttempt::from_committed_store(
        bound,
        participant(),
        SigningAttemptId::from_bytes([77; 32]),
        TransactionId::from_bytes([78; 32]),
        UnixSeconds::new(100).unwrap(),
    )
    .unwrap();
    let fabricated = ArtifactEvidence::verify(&fake, issued.evidence.raw_json(), None).unwrap();
    let mut uncertain = true;
    let clock = FixedClock(0); // Any accidental clock consultation must fail.
    let mut journal = Journal {
        connection: &mut database,
        namespace: &namespace,
        selected: &issued.selected,
        crypto: &crypto,
        clock: &clock,
        uncertain: &mut uncertain,
    };
    assert_eq!(
        journal
            .capture_access(&issued.selected, &issued.proof)
            .await
            .err()
            .unwrap()
            .code(),
        "recovery_required"
    );
    *journal.uncertain = false;
    assert_eq!(
        journal
            .capture_access(&foreign.selected, &issued.proof)
            .await
            .err()
            .unwrap()
            .code(),
        "unauthorized"
    );
    let copy = || {
        issued
            .proof
            .with_secret(|raw| AttachmentProof::from_protected_input(raw).unwrap())
    };
    assert_eq!(
        journal
            .capture_issuance(
                issued.selected.clone(),
                copy(),
                foreign.evidence.retained_copy()
            )
            .await
            .err()
            .unwrap()
            .code(),
        "invalid_evidence"
    );
    assert_eq!(
        journal
            .capture_issuance(issued.selected.clone(), copy(), fabricated)
            .await
            .err()
            .unwrap()
            .code(),
        "invalid_evidence"
    );
    let retained = journal
        .capture_issuance(
            issued.selected.clone(),
            copy(),
            issued.evidence.retained_copy(),
        )
        .await
        .unwrap();
    assert!(matches!(retained, EvidenceCaptureOutcome::Committed(_)));
    let operation = issued
        .attempt
        .bound_plan()
        .prepared()
        .identity()
        .operation_id();
    assert_eq!(
        absent_claim(
            journal.connection,
            &issued.selected,
            operation,
            participant()
        )
        .await
        .err()
        .unwrap()
        .code(),
        "recovery_required"
    );
    let retained = evidence_store::read(journal.connection, &issued.attempt, &crypto)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.0.raw_json(), issued.evidence.raw_json());
    database.close().await.unwrap();
}

async fn installed_descendant_case(mode: u8) {
    use radroots_storage::command::{InstallationOutcome, InstallationRequest};
    let issued = Issued::new().await;
    let store = NativeWorkspace::open_existing(
        &issued.path,
        issued.selected.clone(),
        issued.fixture.crypto(),
        Arc::new(FixedClock(100)),
    )
    .await
    .unwrap();
    let binding = store
        .authenticate(issued.selected.clone(), &issued.proof)
        .await
        .unwrap();
    let original = issued.attempt.bound_plan().prepared().identity();
    let plan = *issued.attempt.bound_plan().plan().digest();
    let copy = || {
        issued
            .proof
            .with_secret(|raw| AttachmentProof::from_protected_input(raw).unwrap())
    };
    assert!(matches!(
        store
            .install_transition_and_outbox(
                InstallationRequest::new(
                    binding.clone(),
                    original.operation_id(),
                    original.command_digest(),
                    plan
                ),
                copy()
            )
            .await
            .unwrap(),
        InstallationOutcome::Committed(_)
    ));
    let mut database = issued.database().await;
    for statement in [
        "DELETE FROM artifact_evidence_parts",
        "DELETE FROM artifact_evidence",
        "DELETE FROM signing_attempts",
    ] {
        if mode == 3 && statement == "DELETE FROM signing_attempts" {
            continue;
        }
        sqlx::query(statement).execute(&mut database).await.unwrap();
    }
    let messages: i64 = sqlx::query_scalar("SELECT count(*) FROM chat_messages")
        .fetch_one(&mut database)
        .await
        .unwrap();
    let work: i64 = sqlx::query_scalar("SELECT count(*) FROM chat_publication_work")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!((messages, work), (1, 1));
    database.close().await.unwrap();
    let request = ClaimAttemptRequest::new(
        binding,
        original.operation_id(),
        original.command_digest(),
        plan,
        participant(),
    );
    let outcome = if mode == 0 {
        match store.claim_attempt(request, copy()).await {
            Ok(ClaimAttemptOutcome::Claimed(_)) => "claimed",
            Ok(ClaimAttemptOutcome::Retained(_)) => "retained",
            Ok(ClaimAttemptOutcome::Unknown(_)) => "unknown",
            Err(error) => error.code(),
        }
    } else if mode == 1 {
        match store.signing_attempt(request, copy()).await {
            Ok(Some(_)) => "found",
            Ok(None) => "absent",
            Err(error) => error.code(),
        }
    } else {
        match store.obtained_evidence(request, copy()).await {
            Ok(Some(_)) => "found",
            Ok(None) => "absent",
            Err(error) => error.code(),
        }
    };
    store.close().await.unwrap();
    assert_eq!(
        outcome, "recovery_required",
        "retained T2 descendants must refuse false absence or a new claim after complete S loss"
    );
}
#[tokio::test]
async fn installed_descendants_prevent_fresh_claim_after_complete_s_loss() {
    installed_descendant_case(0).await;
}
#[tokio::test]
async fn installed_descendants_prevent_false_claim_absence_after_complete_s_loss() {
    installed_descendant_case(1).await;
}
#[tokio::test]
async fn installed_descendants_prevent_false_obtained_absence_after_complete_s_loss() {
    installed_descendant_case(2).await;
}
#[tokio::test]
async fn installed_descendants_prevent_false_obtained_absence_with_retained_claim() {
    installed_descendant_case(3).await;
}
