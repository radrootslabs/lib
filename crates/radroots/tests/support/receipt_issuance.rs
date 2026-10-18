use super::*;
#[path = "receipt_recovery.rs"]
mod recovery;
use core::cell::Cell;
use radroots_nostr::{
    key::parse_secret_key,
    signing::{LocalSigner, OsNonceSource},
};
use radroots_signing::{
    issuance::{OutcomeKind, SigningIntent},
    provider::{ParticipantProvider, ProviderCall},
    request::CancellationSignal,
};
use radroots_storage::command::{
    ClaimAttemptOutcome, ClaimAttemptRequest, SealEvidence, SigningAttempt,
};
thread_local! {static CALLS:Cell<usize>=const {Cell::new(0)};static MISSING:Cell<bool>=const {Cell::new(false)};}
fn signer_clock() -> Result<u64, radroots_signing::Error> {
    let count = CALLS.with(|calls| {
        let value = calls.get() + 1;
        calls.set(value);
        value
    });
    if count > 1 && MISSING.with(Cell::get) {
        Err(radroots_signing::Error::new(
            radroots_signing::error::Kind::InternalError,
        ))
    } else {
        Ok(100_000)
    }
}
pub(super) fn selection(
    binding: &WorkspaceBinding,
    prepared: &PreparedOperation,
    plan_digest: [u8; 32],
    recipient: PublicKey,
) -> ClaimAttemptRequest {
    ClaimAttemptRequest::new(
        binding.clone(),
        prepared.identity().operation_id(),
        prepared.identity().command_digest(),
        plan_digest,
        recipient,
    )
}
pub(super) fn claim(outcome: ClaimAttemptOutcome) -> SigningAttempt {
    match outcome {
        ClaimAttemptOutcome::Claimed(value) => value,
        _ => panic!("first actual claim must confirm its commit"),
    }
}
pub(super) async fn prepare(
    fixture: &Fixture,
    store: &NativeWorkspace,
    binding: &WorkspaceBinding,
    proof: &AttachmentProof,
    id: u8,
    large: bool,
) -> (PreparedOperation, [u8; 32], Vec<PublicKey>) {
    let mut captured = peer_request(binding, id);
    if large {
        let mut value: serde_json::Value =
            serde_json::from_slice(captured.command().protected_wire()).unwrap();
        value["input"]["text"] = "\0".repeat(3500).into();
        captured = AdmissionRequest::from_protected_wire(
            binding.clone(),
            &serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    let prepared = committed(
        store
            .admit_command_and_hold(captured, copy(proof))
            .await
            .unwrap(),
    );
    let plan = plan(prepared.request().command(), 100, "native-issuance");
    let digest = *plan.digest();
    let recipients = plan.seals().iter().map(|seal| seal.recipient()).collect();
    bound(
        store
            .bind_signing_intents(binding.clone(), plan, copy(proof))
            .await
            .unwrap(),
    );
    let _ = fixture;
    (prepared, digest, recipients)
}
pub(super) async fn obtain(attempt: &SigningAttempt, missing: bool) -> SealEvidence {
    CALLS.with(|calls| calls.set(0));
    MISSING.with(|value| value.set(missing));
    // Published synthetic key in a qualified local provider, isolated from all host custody.
    let signer = LocalSigner::with_clock(
        parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
            .unwrap(),
        signer_clock,
    )
    .unwrap()
    .with_nonce_source(Arc::new(OsNonceSource));
    let observed = signer.provider_state().await.unwrap();
    assert_eq!(observed.identity(), key());
    assert_eq!(
        observed.exact_issuance(),
        Some(radroots_signing::recovery::ReplayCapability::NonReplayable)
    );
    let intent = SigningIntent::from_chat_seal(
        attempt.binding().operation_id(),
        attempt.bound_plan().prepared().identity().command_digest(),
        attempt.seal(),
        ProviderCall::new(key(), attempt.deadline_unix_ms(), CancellationSignal::new()),
    )
    .unwrap();
    assert_eq!(intent.binding(), attempt.binding());
    let outcome = signer.sign_exact(intent).await;
    assert_eq!(outcome.binding(), attempt.binding());
    let (_, kind) = outcome.into_parts();
    let OutcomeKind::Obtained(artifact) = kind else {
        panic!("actual local exact signing must obtain")
    };
    assert_eq!(artifact.observed_at_unix_ms().is_none(), missing);
    artifact.with_json(|raw| {
        SealEvidence::verify(attempt, raw, artifact.observed_at_unix_ms()).unwrap()
    })
}
