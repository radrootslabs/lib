//! Structural report validation is separate from the actual native T2 tests.
use super::*;
use radroots_domain::TransactionId;
use radroots_storage::command::PublicationWork;

fn restamp_plan(source: &BoundSigningPlan, transaction: TransactionId) -> BoundSigningPlan {
    let original = source.prepared();
    let request = AdmissionRequest::from_protected_wire(
        original.request().binding().clone(),
        original.request().command().protected_wire(),
    )
    .unwrap();
    let prepared = PreparedOperation::from_committed_store(
        request,
        original.transaction_id(),
        original.revision(),
        original.issuance_window(),
    )
    .unwrap();
    let plan = ChatSigningPlan::parse(
        prepared.request().chat_command().unwrap(),
        source.plan().protected_wire(),
    )
    .unwrap();
    BoundSigningPlan::from_committed_store(prepared, plan, transaction, source.bound_at()).unwrap()
}

#[tokio::test]
async fn structural_installation_reports_bind_original_plan_capture_and_complete_sorted_work_scope()
{
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 227, false).await;
    let mut captures = Vec::new();
    for recipient in recipients {
        let attempt = claim(
            store
                .claim_attempt(
                    selection(&binding, &prepared, digest, recipient),
                    copy(&proof),
                )
                .await
                .unwrap(),
        );
        captures.push(capture(&store, &input, &proof, &attempt).await);
    }
    let state = snapshot(&store, &binding, &prepared, &proof).await;
    let bound = state.bound_plan().unwrap();
    let transaction = TransactionId::from_bytes([0xa1; 32]);
    let time = UnixSeconds::new(110).unwrap();
    let installation =
        ChatInstallation::from_committed_store(bound, captures[0], transaction, time).unwrap();
    let first =
        PublicationWork::from_committed_store(bound, installation, captures[0], transaction, time)
            .unwrap();
    let second =
        PublicationWork::from_committed_store(bound, installation, captures[1], transaction, time)
            .unwrap();
    assert!(matches!(
        ChatInstallation::from_committed_store(
            bound,
            captures[0],
            transaction,
            UnixSeconds::new(99).unwrap()
        ),
        Err(OperationError::RecoveryRequired)
    ));
    assert!(matches!(
        PublicationWork::from_committed_store(
            bound,
            installation,
            captures[0],
            transaction,
            UnixSeconds::new(109).unwrap()
        ),
        Err(OperationError::RecoveryRequired)
    ));
    for (installed, work) in [
        (None, vec![first]),
        (Some(installation), vec![]),
        (Some(installation), vec![first, second, first]),
        (Some(installation), vec![second, first]),
        (Some(installation), vec![first, first]),
        (Some(installation), vec![second]),
    ] {
        assert!(matches!(
            snapshot(&store, &binding, &prepared, &proof)
                .await
                .with_installation(installed, work),
            Err(OperationError::RecoveryRequired)
        ));
    }
    let reported = snapshot(&store, &binding, &prepared, &proof)
        .await
        .with_installation(Some(installation), vec![first, second])
        .unwrap();
    assert_eq!(reported.installation(), Some(installation));
    assert_eq!(
        reported.installed_message().unwrap().text(),
        prepared.request().chat_command().unwrap().text()
    );
    // No actual installation was performed by constructing any of these values.
    assert!(
        snapshot(&store, &binding, &prepared, &proof)
            .await
            .installation()
            .is_none()
    );

    let restamped = restamp_plan(bound, TransactionId::from_bytes([0xa2; 32]));
    let changed =
        ChatInstallation::from_committed_store(&restamped, captures[0], transaction, time).unwrap();
    let changed_work =
        PublicationWork::from_committed_store(&restamped, changed, captures[0], transaction, time)
            .unwrap();
    assert!(matches!(
        snapshot(&store, &binding, &prepared, &proof)
            .await
            .with_installation(Some(changed), vec![changed_work]),
        Err(OperationError::RecoveryRequired)
    ));
    assert!(matches!(
        PublicationWork::from_committed_store(
            &restamped,
            installation,
            captures[0],
            transaction,
            time
        ),
        Err(OperationError::RecoveryRequired)
    ));
    let wrong_installation = ChatInstallation::from_committed_store(
        bound,
        captures[0],
        TransactionId::from_bytes([0xa3; 32]),
        time,
    )
    .unwrap();
    assert!(matches!(
        snapshot(&store, &binding, &prepared, &proof)
            .await
            .with_installation(Some(wrong_installation), vec![first]),
        Err(OperationError::RecoveryRequired)
    ));

    let (foreign, foreign_digest, foreign_recipients) =
        prepare(&fixture, &store, &binding, &proof, 228, false).await;
    let attempt = claim(
        store
            .claim_attempt(
                selection(&binding, &foreign, foreign_digest, foreign_recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let foreign_capture = capture(&store, &input, &proof, &attempt).await;
    let foreign_state = snapshot(&store, &binding, &foreign, &proof).await;
    let foreign_installation = ChatInstallation::from_committed_store(
        foreign_state.bound_plan().unwrap(),
        foreign_capture,
        transaction,
        time,
    )
    .unwrap();
    assert!(matches!(
        ChatInstallation::from_committed_store(bound, foreign_capture, transaction, time),
        Err(OperationError::RecoveryRequired)
    ));
    assert!(matches!(
        PublicationWork::from_committed_store(
            bound,
            installation,
            foreign_capture,
            transaction,
            time
        ),
        Err(OperationError::RecoveryRequired)
    ));
    assert!(matches!(
        PublicationWork::from_committed_store(
            bound,
            foreign_installation,
            captures[0],
            transaction,
            time
        ),
        Err(OperationError::RecoveryRequired)
    ));
    assert!(matches!(
        snapshot(&store, &binding, &prepared, &proof)
            .await
            .with_installation(Some(foreign_installation), vec![first]),
        Err(OperationError::RecoveryRequired)
    ));

    let unbound = committed(
        store
            .admit_command_and_hold(
                request(&binding, 229, "original unbound", "trace"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert!(matches!(
        snapshot(&store, &binding, &unbound, &proof)
            .await
            .with_installation(Some(installation), vec![first]),
        Err(OperationError::RecoveryRequired)
    ));
    store.close().await.unwrap();
}
