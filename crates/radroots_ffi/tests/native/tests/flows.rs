//! Real host/kernel effects through the same production objects used by languages.

use nostr::JsonUtil;
use radroots_ffi::{
    Admission, BusinessInstallation, CommandRequest, InquiryInput, Issuance, ListingInput,
    OperationKind, SessionState, StartMode, TargetObservation, Value,
};
use radroots_native_binding_fixture::{Host, Session};
use std::{sync::Arc, time::Duration};

fn request(host: &Host, id: u8) -> CommandRequest {
    CommandRequest {
        operation_id: format!("{id:02x}").repeat(32),
        first_issue_not_after: (host.unix_seconds().unwrap().parse::<u64>().unwrap() + 120)
            .to_string(),
        correlation_id: Some("native binding workflow".into()),
    }
}
fn listing(host: &Host, id: u8) -> Arc<ListingInput> {
    let input = serde_json::json!({"id":format!("{id:02x}").repeat(16),"title":"  Native carrots 🥕  ","summary":"Quote required","description_markdown":"Exact public terms","published_at":host.unix_seconds().unwrap(),
        "profile":{"profile":"radroots.listing","version":1,"state":"active","product":{"name":"Carrots","description":"Fresh carrots","producer":null,"origin_summary":null},"default_option":"02".repeat(32),
        "options":[{"id":"02".repeat(32),"label":"Bunch","unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},"increment":"1","minimum":"1","price":{"type":"quote_required"},"supply":{"type":"available","amount":"10"},"lead_seconds":"0"}],
        "fulfillment":{"modes":["pickup"],"area":"Example","lead_seconds":"0"},"authority":null,"media":[]}});
    ListingInput::parse(serde_json::to_vec(&input).unwrap()).unwrap()
}
async fn close(host: &Host, session: &Session) {
    let callbacks = host
        .measurements()
        .unwrap()
        .closed_callbacks
        .parse::<u64>()
        .unwrap();
    session.client.close().await.unwrap();
    assert_eq!(session.client.state(), SessionState::Closed);
    assert_eq!(
        host.measurements()
            .unwrap()
            .closed_callbacks
            .parse::<u64>()
            .unwrap(),
        callbacks + 1
    );
    host.cleanup().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_projection_reopen_and_explicit_retry_keep_actual_original_signature_and_bytes() {
    let host = Host::open(true, false, false).await.unwrap();
    let first = host.session();
    first.client.start(StartMode::Online).await.unwrap();
    host.set_accepting(false);
    let input = listing(&host, 1);
    let mut context = request(&host, 1);
    context.first_issue_not_after =
        (host.unix_seconds().unwrap().parse::<u64>().unwrap() + 10).to_string();
    let receipt = first
        .client
        .publish_listing(first.workspace.clone(), context.clone(), input.clone())
        .await
        .unwrap();
    assert_eq!(receipt.kind, OperationKind::Listing);
    assert_eq!(receipt.admission, Admission::Confirmed);
    assert_eq!(
        receipt.business_installation,
        BusinessInstallation::Installed
    );
    assert_eq!(receipt.issuance, Issuance::Obtained);
    assert!(receipt.message_id.is_none());
    assert!(receipt.artifacts.iter().all(|v| v.recipient.is_none()));
    assert_eq!(receipt.delivery_observations.len(), 1);
    assert_eq!(
        receipt.delivery_observations[0].outcome,
        TargetObservation::Rejected
    );
    let Value::Listing { listing } = receipt.business_result.as_ref().unwrap() else {
        panic!("actual public family")
    };
    assert_eq!(listing.title(), "  Native carrots 🥕  ");
    let original = host.measurements().unwrap();
    assert_eq!(original.signing_calls, "1");
    assert_eq!(original.legacy_signing_calls, "0");
    assert_eq!(original.relay_reads, "0");
    let event = nostr::Event::from_json(&original.original_signed_events[0]).unwrap();
    event.verify().unwrap();
    assert_eq!(event.id.to_hex(), listing.revision());
    assert_eq!(listing.id(), "01".repeat(16));
    assert_eq!(listing.author(), event.pubkey.to_hex());
    assert_eq!(listing.created_at(), event.created_at.as_secs().to_string());
    assert!(listing.published_at().parse::<u64>().unwrap() > 0);
    assert_eq!(listing.summary(), "Quote required");
    assert_eq!(listing.description_markdown(), "Exact public terms");
    let terms: serde_json::Value = serde_json::from_slice(&listing.terms_json().unwrap()).unwrap();
    assert_eq!(terms["product"]["name"], "Carrots");
    let replay = first
        .client
        .publish_listing(first.workspace.clone(), context, input)
        .await
        .unwrap();
    assert_eq!(replay.operation_checkpoint, receipt.operation_checkpoint);
    assert_eq!(
        host.measurements().unwrap().original_signed_events,
        original.original_signed_events
    );
    first.client.close().await.unwrap();
    let reopened = host.reopen().await.unwrap();
    reopened.client.start(StartMode::Online).await.unwrap();
    let queried = reopened
        .client
        .operation(
            reopened.workspace.clone(),
            receipt.operation_id.clone(),
            Some(receipt.command_digest.clone()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(queried.operation_checkpoint, receipt.operation_checkpoint);
    assert_eq!(
        host.measurements().unwrap().original_signed_events,
        original.original_signed_events
    );
    let expiry = receipt.first_issue_not_after.parse::<u64>().unwrap();
    let now = host.unix_seconds().unwrap().parse::<u64>().unwrap();
    if now <= expiry {
        tokio::time::sleep(Duration::from_secs(expiry - now + 1)).await;
    }
    host.set_accepting(true);
    let resumed = reopened
        .client
        .resume(
            reopened.workspace.clone(),
            receipt.operation_id,
            Some(receipt.command_digest),
        )
        .await
        .unwrap();
    assert_eq!(resumed.first_issue_not_after, receipt.first_issue_not_after);
    assert_eq!(resumed.delivery_attempts.len(), 2);
    assert_eq!(
        resumed.delivery_attempts[0].original_bytes_digest,
        resumed.delivery_attempts[1].original_bytes_digest
    );
    assert_eq!(
        resumed.delivery_attempts[0].publication_policy_digest,
        resumed.delivery_attempts[1].publication_policy_digest
    );
    let mut ordinals: Vec<_> = resumed
        .delivery_attempts
        .iter()
        .map(|attempt| attempt.target_ordinals.clone())
        .collect();
    ordinals.sort();
    assert_eq!(ordinals, [vec![1], vec![2]]);
    assert!(
        resumed
            .delivery_observations
            .iter()
            .any(|v| v.outcome == TargetObservation::Accepted)
    );
    let actual = host.measurements().unwrap();
    assert_eq!(actual.signing_calls, "1");
    assert_eq!(
        actual.original_signed_events,
        [
            original.original_signed_events[0].clone(),
            original.original_signed_events[0].clone()
        ]
    );
    close(&host, &reopened).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_value_and_foreign_scope_refusal_use_real_grants_and_no_additional_effects() {
    let host = Host::open(false, false, false).await.unwrap();
    let session = host.session();
    session.client.start(StartMode::Online).await.unwrap();
    let mut scalar = [0; 32];
    scalar[31] = 2;
    let peer = nostr::Keys::new(nostr::SecretKey::from_slice(&scalar).unwrap())
        .public_key()
        .to_hex();
    let text = "\u{feff}  Cafe\u{301} / Café Binding inquiry 🥕\nExact text\0  ";
    let input = InquiryInput::new("22".repeat(32), peer.clone(), None, text.into()).unwrap();
    let receipt = session
        .client
        .inquire(session.workspace.clone(), request(&host, 2), input.clone())
        .await
        .unwrap();
    assert_eq!(receipt.kind, OperationKind::Chat);
    assert_eq!(
        receipt.business_installation,
        BusinessInstallation::Installed
    );
    let Value::Chat { message } = receipt.business_result.unwrap() else {
        panic!("actual private family")
    };
    assert_eq!(message.peer(), peer);
    assert_eq!(message.text(), text);
    assert_eq!(Some(message.id()), receipt.message_id);
    assert_eq!(message.thread_id(), "22".repeat(32));
    assert_eq!(
        message.author(),
        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    );
    assert!(message.created_at().parse::<u64>().unwrap() > 0);
    assert!(matches!(
        message.relation().unwrap(),
        radroots_ffi::ChatRelation::Inquire { source: None }
    ));
    let queried = session
        .client
        .operation(
            session.workspace.clone(),
            receipt.operation_id.clone(),
            Some(receipt.command_digest.clone()),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(queried.business_result, Some(Value::Chat { .. })));
    assert_eq!(receipt.artifacts.len(), 2);
    assert!(receipt.artifacts.iter().all(|v| v.recipient.is_some()));
    let before = host.measurements().unwrap();
    assert_eq!(before.signing_calls, "2");
    assert_ne!(before.relay_reads, "0");
    let foreign = Host::open(false, false, false).await.unwrap();
    let other = foreign.session();
    other.client.start(StartMode::Online).await.unwrap();
    let error = session
        .client
        .inquire(other.workspace.clone(), request(&host, 3), input)
        .await
        .err()
        .expect("foreign scope must refuse");
    assert!(matches!(
        error,
        radroots_ffi::Error::Operation {
            admission: Admission::NotAdmitted,
            ..
        }
    ));
    assert_eq!(
        host.measurements().unwrap().signing_calls,
        before.signing_calls
    );
    assert_eq!(
        host.measurements().unwrap().original_signed_events,
        before.original_signed_events
    );
    assert_eq!(foreign.measurements().unwrap().signing_calls, "0");
    close(&foreign, &other).await;
    close(&host, &session).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_provider_outcome_survives_reopen_without_reissuance() {
    let host = Host::open(true, true, false).await.unwrap();
    let session = host.session();
    session.client.start(StartMode::Online).await.unwrap();
    let receipt = session
        .client
        .publish_listing(
            session.workspace.clone(),
            request(&host, 4),
            listing(&host, 4),
        )
        .await
        .unwrap();
    assert_eq!(receipt.issuance, Issuance::Unknown);
    assert!(receipt.business_result.is_none());
    session.client.close().await.unwrap();
    host.set_unknown_signing(false);
    let reopened = host.reopen().await.unwrap();
    reopened.client.start(StartMode::Online).await.unwrap();
    let resumed = reopened
        .client
        .resume(
            reopened.workspace.clone(),
            receipt.operation_id,
            Some(receipt.command_digest),
        )
        .await
        .unwrap();
    assert_eq!(resumed.issuance, Issuance::Unknown);
    assert!(resumed.business_result.is_none());
    let measured = host.measurements().unwrap();
    assert_eq!(measured.signing_calls, "1");
    assert!(measured.original_signed_events.is_empty());
    close(&host, &reopened).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_context_and_selectors_and_closed_attachment_have_no_signing_effects() {
    let host = Host::open(true, false, false).await.unwrap();
    let session = host.session();
    assert_eq!(
        session.client.start(StartMode::LocalOnly).await.unwrap(),
        SessionState::Running
    );
    let input = listing(&host, 7);
    for (field, value) in [
        ("operation_id", "bad"),
        ("first_issue_not_after", "not-time"),
        ("correlation_id", "not\nallowed"),
    ] {
        let mut context = request(&host, 7);
        match field {
            "operation_id" => context.operation_id = value.into(),
            "first_issue_not_after" => context.first_issue_not_after = value.into(),
            _ => context.correlation_id = Some(value.into()),
        }
        let error = session
            .client
            .publish_listing(session.workspace.clone(), context, input.clone())
            .await
            .err()
            .expect("invalid context");
        assert!(
            matches!(error, radroots_ffi::Error::InvalidInput { field: actual } if actual == field)
        );
    }
    for (id, digest, field) in [
        ("bad".into(), None, "operation_id"),
        ("77".repeat(32), Some("bad".into()), "command_digest"),
    ] {
        let error = session
            .client
            .operation(session.workspace.clone(), id, digest)
            .await
            .err()
            .expect("invalid selector");
        assert!(
            matches!(error, radroots_ffi::Error::InvalidInput { field: actual } if actual == field)
        );
    }
    assert!(
        session
            .client
            .operation(session.workspace.clone(), "77".repeat(32), None)
            .await
            .unwrap()
            .is_none()
    );
    session.workspace.close();
    assert!(matches!(
        session
            .client
            .publish_listing(session.workspace.clone(), request(&host, 7), input)
            .await,
        Err(radroots_ffi::Error::Operation {
            admission: Admission::NotAdmitted,
            ..
        })
    ));
    let measured = host.measurements().unwrap();
    assert_eq!(measured.signing_calls, "0");
    assert_eq!(measured.relay_reads, "0");
    assert!(measured.original_signed_events.is_empty());
    close(&host, &session).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_public_deliveries_retain_each_configured_shared_satisfaction_policy() {
    use radroots_native_binding_fixture::FixturePolicy;
    for (index, policy) in [
        FixturePolicy::All,
        FixturePolicy::Any,
        FixturePolicy::Quorum,
        FixturePolicy::Required,
    ]
    .into_iter()
    .enumerate()
    {
        let host = Host::open_with_policy(true, false, false, policy)
            .await
            .unwrap();
        let session = host.session();
        session.client.start(StartMode::Online).await.unwrap();
        let receipt = session
            .client
            .publish_listing(
                session.workspace.clone(),
                request(&host, 10 + index as u8),
                listing(&host, 10 + index as u8),
            )
            .await
            .unwrap();
        let attempt = &receipt.delivery_attempts[0];
        match (policy, &attempt.policy) {
            (FixturePolicy::All, radroots_ffi::TargetPolicy::All)
            | (FixturePolicy::Any, radroots_ffi::TargetPolicy::Any) => {}
            (FixturePolicy::Quorum, radroots_ffi::TargetPolicy::Quorum { threshold }) => {
                assert_eq!(*threshold, 1)
            }
            (FixturePolicy::Required, radroots_ffi::TargetPolicy::Required { fingerprints }) => {
                assert_eq!(fingerprints, &attempt.selected_fingerprints)
            }
            _ => panic!("source-owned policy changed at projection"),
        }
        assert_eq!(host.measurements().unwrap().signing_calls, "1");
        assert!(
            receipt
                .delivery_observations
                .iter()
                .any(|value| value.outcome == TargetObservation::Accepted)
        );
        close(&host, &session).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_observers_preserve_actual_obtained_signature_through_close_and_reopen() {
    let host = Host::open(true, false, true).await.unwrap();
    let session = host.session();
    session.client.start(StartMode::Online).await.unwrap();
    let request = request(&host, 5);
    let operation = request.operation_id.clone();
    let input = listing(&host, 5);
    let client = session.client.clone();
    let workspace = session.workspace.clone();
    let observer =
        tokio::spawn(async move { client.publish_listing(workspace, request, input).await });
    host.wait_until_signer_paused().await.unwrap();
    observer.abort();
    let _ = observer.await;
    let client = session.client.clone();
    let closing = tokio::spawn(async move { client.close().await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while session.client.state() != SessionState::Closing {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    closing.abort();
    let _ = closing.await;
    host.release_signer();
    session.client.close().await.unwrap();
    let reopened = host.reopen().await.unwrap();
    reopened.client.start(StartMode::Online).await.unwrap();
    let receipt = reopened
        .client
        .resume(reopened.workspace.clone(), operation, None)
        .await
        .unwrap();
    assert_eq!(receipt.issuance, Issuance::Obtained);
    assert_eq!(
        receipt.business_installation,
        BusinessInstallation::Installed
    );
    assert_eq!(host.measurements().unwrap().signing_calls, "1");
    close(&host, &reopened).await;
}

#[tokio::test]
async fn rejected_command_metadata_crosses_the_actual_native_boundary_without_raw_payloads() {
    use radroots_ffi::{Error, SignerRejectionReason};
    use radroots_native_binding_fixture::FixtureRejection;
    for (mode, reason, expected) in [
        (
            FixtureRejection::Malformed,
            SignerRejectionReason::InvalidArtifact,
            Some((
                "synthetic private rejection canary".len().to_string(),
                "synthetic private rejection canary".len().to_string(),
                "ba883ea08dff4c078672a055dd2be042d9debb5b22c66527e4499c068011879d",
            )),
        ),
        (
            FixtureRejection::Oversized,
            SignerRejectionReason::TooLarge,
            Some((
                "65536".into(),
                "65537".into(),
                "d2d5af5c0316f5c2b119635386239edc87abce035f851c3f6ee127e6c3cbf435",
            )),
        ),
        (
            FixtureRejection::Foreign,
            SignerRejectionReason::BindingMismatch,
            None,
        ),
    ] {
        let host = Host::open(true, false, false).await.unwrap();
        host.set_rejected_signing(mode);
        let session = host.session();
        session.client.start(StartMode::Online).await.unwrap();
        let context = request(&host, 70);
        let operation = context.operation_id.clone();
        let error = session
            .client
            .publish_listing(session.workspace.clone(), context, listing(&host, 70))
            .await
            .err()
            .expect("actual rejected callback");
        assert!(!format!("{error:?} {error}").contains("synthetic private rejection canary"));
        let Error::Operation {
            code,
            retry,
            operation_id,
            admission,
            signer_rejection: Some(rejected),
        } = error
        else {
            panic!("bounded original command diagnostic must cross the generated component")
        };
        assert_eq!(code, "signer_tampering");
        assert_eq!(retry, "manual_reconciliation");
        assert_eq!(operation_id.as_deref(), Some(operation.as_str()));
        assert_eq!(admission, Admission::Confirmed);
        assert_eq!(rejected.reason, reason);
        match (rejected.fingerprint, expected) {
            (None, None) => {}
            (Some(actual), Some((covered, total, digest))) => {
                assert_eq!(actual.covered_bytes, covered);
                assert_eq!(actual.total_bytes, total);
                assert_eq!(actual.sha256.len(), 32);
                // Independent Python hashlib vectors, not the producing Rust hash.
                assert_eq!(
                    actual
                        .sha256
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                    digest
                );
            }
            _ => panic!("fingerprint presence must survive without fabrication"),
        }
        let facts = session
            .client
            .operation(session.workspace.clone(), operation, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            facts.business_installation,
            BusinessInstallation::NotInstalled
        );
        assert_eq!(host.measurements().unwrap().signing_calls, "1");
        assert_eq!(host.measurements().unwrap().legacy_signing_calls, "0");
        assert!(
            host.measurements()
                .unwrap()
                .original_signed_events
                .is_empty()
        );
        close(&host, &session).await;
    }
}
