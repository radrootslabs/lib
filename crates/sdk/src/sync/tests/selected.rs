use super::*;
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, EventSink, SinkFailure,
    outcome::DeliveryOutcome,
    sink::{DeliveryTargetReceipt, SinkStatus},
};

#[derive(Default)]
struct SelectedSink(std::sync::Mutex<Vec<(DeliveryRequest, TargetSet)>>);

impl EventSink for SelectedSink {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
        Box::pin(async { panic!("selected delivery does not probe status") })
    }

    fn deliver(
        &self,
        _: DeliveryRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        Box::pin(async { panic!("selected delivery must not widen to ordinary delivery") })
    }

    fn deliver_selected(
        &self,
        request: DeliveryRequest,
        selected: TargetSet,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        Box::pin(async move {
            request.validate_target_selection(&selected).unwrap();
            self.0
                .lock()
                .unwrap()
                .push((request.clone(), selected.clone()));
            Ok(DeliveryReceipt::for_request(
                &request,
                request
                    .target_set()
                    .targets()
                    .iter()
                    .map(|target| {
                        if selected.targets().contains(target) {
                            DeliveryTargetReceipt::attempted(
                                target.clone(),
                                DeliveryOutcome::accepted(),
                            )
                        } else {
                            DeliveryTargetReceipt::skipped(
                                target.clone(),
                                DeliveryOutcome::unavailable(),
                            )
                            .unwrap()
                        }
                    })
                    .collect(),
            )
            .unwrap())
        })
    }
}

#[tokio::test]
async fn sdk_selected_delivery_preserves_subset_and_full_durable_request() {
    let storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([211; 32]).unwrap(),
    ));
    let signer = radroots_nostr::signing::LocalSigner::new(
        radroots_nostr::key::SecretKey::parse(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap(),
    )
    .unwrap();
    let author = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    let (clock, ids, deadlines) = HostPolicy::default().composition();
    let now = clock.now_unix_ms().unwrap();
    let sink = Arc::new(SelectedSink::default());
    let engine = Engine::builder(storage.clone(), clock, ids, deadlines)
        .signer(Arc::new(signer))
        .sink(sink.clone())
        .build()
        .unwrap();
    let client = ClientBuilder::new()
        .storage(storage)
        .sync_engine(engine)
        .build()
        .unwrap();
    let operations = client.sync().unwrap().unwrap();
    let targets = TargetSet::new(vec![
        target(),
        Target::nostr_relay("wss://held.example").unwrap(),
    ])
    .unwrap();
    let selected = TargetSet::new(vec![targets.targets()[0].clone()]).unwrap();
    let request = PushRequest::new(
        SyncId::new([211; 16]).unwrap(),
        IdempotencyKey::parse("sdk-selected-delivery").unwrap(),
        Actor::new(
            PublicKey::from_hex(author).unwrap(),
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .unwrap(),
        AuthoredEventPlan::from_generic(
            GenericEventDraft::new(
                "radroots.social.geochat.v1",
                20_000,
                now / 1_000,
                Vec::new(),
                "selected",
                author,
            )
            .unwrap(),
        )
        .unwrap(),
        targets.clone(),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        now + 60_000,
        CancellationPolicy::LocalCooperative,
    )
    .unwrap();
    let id = request.operation_id();
    operations.submit_push(request).await.unwrap();
    let original = operations.push_status(id).await.unwrap().unwrap();
    let original_request = original.delivery_plan().request().unwrap();
    assert_eq!(
        operations
            .deliver_push_selected(
                id,
                TargetSet::new(vec![Target::nostr_relay("wss://foreign.example").unwrap()])
                    .unwrap(),
            )
            .await
            .unwrap_err(),
        Error::InvalidDeliveryRequest
    );
    assert!(sink.0.lock().unwrap().is_empty());
    operations
        .deliver_push_selected(id, selected.clone())
        .await
        .unwrap();
    let after = operations.push_status(id).await.unwrap().unwrap();
    assert_eq!(after.delivery_plan().request(), Some(original_request));
    assert_eq!(
        sink.0.lock().unwrap().as_slice(),
        &[(original_request.clone(), selected)]
    );
    assert!(!after.delivery_plan().state().is_terminal());
    assert_eq!(after.delivery_plan().delivery_facts().len(), 1);
    client.close().await.unwrap();
}
