use super::*;
use radroots_storage::authored_atomic::AuthoredAtomicCommand;

#[derive(Clone, Copy)]
pub(super) enum Phase {
    Prepare,
    Claim,
    Signed,
    DeliveryFact,
}

pub(super) struct Fault {
    pub(super) phase: Phase,
    pub(super) after: bool,
}

pub(super) fn inject(
    storage: &FaultStorage,
    command: &AuthoredAtomicCommand,
    after: bool,
) -> Result<(), radroots_storage::Error> {
    let mut fault = storage.capacity.lock().unwrap();
    if fault.as_ref().is_some_and(|fault| {
        fault.after == after
            && matches!(
                (fault.phase, command),
                (Phase::Prepare, AuthoredAtomicCommand::Prepare(_))
                    | (Phase::Claim, AuthoredAtomicCommand::Claim(_))
                    | (Phase::Signed, AuthoredAtomicCommand::RecordSigned(_))
                    | (
                        Phase::DeliveryFact,
                        AuthoredAtomicCommand::RecordDelivery(_)
                    )
            )
    }) {
        *fault = None;
        Err(radroots_storage::Error::SpaceInsufficient)
    } else {
        Ok(())
    }
}

fn setup(byte: u8) -> (Engine, Arc<FaultStorage>, Arc<MockSigner>, PushRequest) {
    let storage = Arc::new(FaultStorage::new(byte));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let engine = fault_engine(storage.clone(), signer.clone(), Arc::new(MockSink));
    (
        engine,
        storage,
        signer,
        request(byte, "wss://capacity.example"),
    )
}

#[test]
fn capacity_prepare_reports_failure_and_reconciles_the_original_operation() {
    for after in [false, true] {
        let (engine, storage, signer, push) = setup(181);
        *storage.capacity.lock().unwrap() = Some(Fault {
            phase: Phase::Prepare,
            after,
        });
        assert_eq!(
            block_on(engine.prepare_push(push.clone())),
            Err(Error::StorageSpaceInsufficient)
        );
        let observed = block_on(engine.push_status(push.operation_id())).unwrap();
        assert_eq!(observed.is_some(), after);
        let original = block_on(engine.prepare_push(push.clone())).unwrap();
        let replay = block_on(engine.prepare_push(push)).unwrap();
        assert_eq!(original.operation(), replay.operation());
        assert_eq!(original.artifact(), replay.artifact());
        assert_eq!(signer.calls.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn capacity_after_signed_receipt_preserves_exact_bytes_without_resigning() {
    let (engine, storage, signer, push) = setup(182);
    *storage.capacity.lock().unwrap() = Some(Fault {
        phase: Phase::Signed,
        after: true,
    });
    assert_eq!(
        block_on(engine.sign_prepared(push.clone())),
        Err(Error::StorageSpaceInsufficient)
    );
    let original = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    let signed = original
        .artifact()
        .signed()
        .expect("durable original signed bytes");
    let replay = block_on(engine.sign_prepared(push)).unwrap();
    assert_eq!(replay.artifact().signed().unwrap(), signed);
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn capacity_claim_failure_never_calls_the_signer_or_discards_prepared_work() {
    for after in [false, true] {
        let (engine, storage, signer, push) = setup(183);
        block_on(engine.prepare_push(push.clone())).unwrap();
        *storage.capacity.lock().unwrap() = Some(Fault {
            phase: Phase::Claim,
            after,
        });
        assert_eq!(
            block_on(engine.sign_prepared(push.clone())),
            Err(Error::StorageSpaceInsufficient)
        );
        let status = block_on(engine.push_status(push.operation_id()))
            .unwrap()
            .unwrap();
        assert_eq!(status.artifact().signing_claim().is_some(), after);
        assert!(status.artifact().signed().is_none());
        assert_eq!(signer.calls.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn capacity_local_admission_retains_signed_bytes_and_reports_no_success() {
    let (engine, storage, signer, push) = setup(184);
    block_on(engine.sign_prepared(push.clone())).unwrap();
    let original = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    storage.fail_admission_with(3);
    assert_eq!(
        block_on(engine.admit_signed(push.operation_id())),
        Err(Error::StorageSpaceInsufficient)
    );
    let status = block_on(engine.push_status(push.operation_id()))
        .unwrap()
        .unwrap();
    assert_eq!(status.artifact().signed(), original.artifact().signed());
    assert!(!status.artifact().admission_state().is_admitted());
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
}
