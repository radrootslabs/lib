//! Compile the complete port thread bounds, with representative local adapters.
//! These unavailable adapters prove host composition, not durable execution.

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn every_native_port_retains_its_thread_guarantees() {
    use radroots_storage as store;
    fn send_sync<T: Send + Sync + ?Sized>() {}
    fn send<T: Send + ?Sized>() {}
    send_sync::<dyn radroots_signing::Signer>();
    send_sync::<dyn radroots_signing::CurrentAuthoringAuthority>();
    send_sync::<dyn radroots_signing::request::ProgressObserver>();
    send_sync::<dyn radroots_transport::EventSource>();
    send_sync::<dyn radroots_transport::EventSink>();
    send_sync::<dyn radroots_transport::EventSubscriber>();
    send::<dyn radroots_transport::EventSubscription>();
    send_sync::<dyn store::EventStore>();
    send_sync::<dyn store::Journal>();
    send_sync::<dyn store::Outbox>();
    send_sync::<dyn store::ProjectionStore>();
    send_sync::<dyn store::private_artifact::PrivateArtifactStore>();
    send_sync::<dyn store::backup::StorageReliability>();
    send_sync::<dyn store::authored_draft::AuthoredDraftStore>();
    send_sync::<dyn store::atomic::AtomicStorage>();
    send_sync::<dyn store::authored_atomic::AuthoredAtomicStorage>();
    send_sync::<dyn store::status::StorageStatusProvider>();
    send_sync::<dyn radroots_sync::policy::Clock>();
    send_sync::<dyn radroots_sync::policy::IdSource>();
    send_sync::<radroots_sync::Engine>();
    send::<radroots_signing::signer::BoxFuture<'static, ()>>();
    send::<radroots_transport::BoxFuture<'static, ()>>();
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use radroots_signing::{self as signing, host::Shared as SignerHandle};
    use radroots_storage::{self as store, event::*};
    use radroots_sync::policy::{Clock, IdSource, OperationKind, SyncId};
    use radroots_transport::{
        self as transport, BoxFuture, EventSink, EventSource, EventSubscriber,
    };
    use std::{cell::Cell, rc::Rc};

    struct Local(Rc<Cell<usize>>);

    impl Local {
        fn failed<T: 'static, E: 'static>(&self, error: E) -> BoxFuture<'_, Result<T, E>> {
            Box::pin(async move {
                self.0.set(self.0.get() + 1);
                Err(error)
            })
        }
    }

    impl signing::Signer for Local {
        fn status(
            &self,
        ) -> signing::signer::BoxFuture<'_, Result<signing::SignerStatus, signing::Error>> {
            self.failed(signing::Error::new(signing::error::Kind::SignerUnavailable))
        }
        fn sign(
            &self,
            _: signing::SignRequest,
        ) -> signing::signer::BoxFuture<'_, Result<signing::SignReceipt, signing::Error>> {
            self.failed(signing::Error::new(signing::error::Kind::SignerUnavailable))
        }
    }
    impl signing::request::ProgressObserver for Local {
        fn on_progress(&self, _: &signing::status::SignProgress) {
            self.0.set(self.0.get() + 1);
        }
    }
    impl signing::CurrentAuthoringAuthority for Local {
        fn evaluate(
            &self,
            _: &radroots_event_codec::authoring::AuthoredEventPlan,
        ) -> signing::CurrentAuthoringDecision {
            signing::CurrentAuthoringDecision::Blocked {
                code: "unavailable",
            }
        }
    }
    impl EventSource for Local {
        fn status(&self) -> BoxFuture<'_, Result<transport::SourceStatus, transport::Error>> {
            self.failed(transport::Error::TransportUnavailable)
        }
        fn fetch(
            &self,
            _: transport::FetchRequest,
        ) -> BoxFuture<'_, Result<transport::FetchPage, transport::Error>> {
            self.failed(transport::Error::TransportUnavailable)
        }
    }
    impl EventSink for Local {
        fn status(&self) -> BoxFuture<'_, Result<transport::SinkStatus, transport::Error>> {
            self.failed(transport::Error::TransportUnavailable)
        }
        fn deliver(
            &self,
            request: transport::DeliveryRequest,
        ) -> BoxFuture<'_, Result<transport::DeliveryReceipt, transport::SinkFailure>> {
            self.failed(transport::SinkFailure::transport_unavailable(&request))
        }
    }
    impl EventSubscriber for Local {
        fn subscribe(
            &self,
            _: transport::SubscriptionRequest,
        ) -> BoxFuture<'_, Result<transport::BoxSubscription, transport::Error>> {
            self.failed(transport::Error::SubscriptionUnavailable)
        }
    }
    struct Subscription {
        local: Local,
        request: transport::SubscriptionRequest,
    }
    impl transport::EventSubscription for Subscription {
        fn request(&self) -> &transport::SubscriptionRequest {
            &self.request
        }
        fn next(&mut self) -> BoxFuture<'_, Result<transport::SubscriptionNext, transport::Error>> {
            self.local.failed(transport::Error::TransportUnavailable)
        }
        fn cancel(
            &mut self,
        ) -> BoxFuture<'_, Result<transport::SubscriptionEnd, transport::Error>> {
            self.local.failed(transport::Error::TransportUnavailable)
        }
    }
    impl Clock for Local {
        fn now_unix_ms(&self) -> Result<u64, radroots_sync::Error> {
            self.0.set(self.0.get() + 1);
            Err(radroots_sync::Error::ClockUnavailable)
        }
    }
    impl IdSource for Local {
        fn next_id(&self, _: OperationKind) -> Result<SyncId, radroots_sync::Error> {
            self.0.set(self.0.get() + 1);
            Err(radroots_sync::Error::InvalidSyncId)
        }
    }
    impl store::EventStore for Local {
        fn status(&self) -> BoxFuture<'_, Result<store::status::EventStoreStatus, store::Error>> {
            self.failed(store::Error::BackendUnavailable)
        }
        fn admit(
            &self,
            _: EventAdmission,
        ) -> BoxFuture<'_, Result<AdmissionReceipt, store::Error>> {
            self.failed(store::Error::BackendUnavailable)
        }
        fn query_raw(
            &self,
            _: EventQuery,
        ) -> BoxFuture<'_, Result<EventPage<StoredRawEvent>, store::Error>> {
            self.failed(store::Error::BackendUnavailable)
        }
        fn query_verified(
            &self,
            _: EventQuery,
        ) -> BoxFuture<'_, Result<EventPage<StoredVerifiedEvent>, store::Error>> {
            self.failed(store::Error::BackendUnavailable)
        }
        fn query_visible(
            &self,
            _: EventQuery,
        ) -> BoxFuture<'_, Result<EventPage<StoredVisibleEvent>, store::Error>> {
            self.failed(store::Error::BackendUnavailable)
        }
        fn rebuild_visibility(&self) -> BoxFuture<'_, Result<VisibilitySnapshot, store::Error>> {
            self.failed(store::Error::BackendUnavailable)
        }
        fn query_provenance(
            &self,
            _: radroots_event::EventId,
            _: EventQueryBounds,
        ) -> BoxFuture<'_, Result<EventPage<StoredEventProvenance>, store::Error>> {
            self.failed(store::Error::BackendUnavailable)
        }
    }
    impl store::status::StorageStatusProvider for Local {
        fn storage_status(
            &self,
        ) -> BoxFuture<'_, Result<store::status::StorageStatus, store::Error>> {
            self.failed(store::Error::BackendUnavailable)
        }
    }

    #[test]
    fn browser_ports_accept_real_local_callback_and_future_state() {
        let state = Rc::new(Cell::new(0));
        let signer: signing::signer::DynSigner = SignerHandle::new(Local(state.clone()));
        let source: transport::host::Shared<dyn EventSource> = Rc::new(Local(state.clone()));
        let sink: transport::host::Shared<dyn EventSink> = Rc::new(Local(state.clone()));
        let subscriber: transport::host::Shared<dyn EventSubscriber> =
            Rc::new(Local(state.clone()));
        let store: Rc<dyn store::EventStore> = Rc::new(Local(state.clone()));
        let observer: SignerHandle<dyn signing::request::ProgressObserver> =
            Rc::new(Local(state.clone()));
        let authority: Rc<dyn signing::CurrentAuthoringAuthority> = Rc::new(Local(state.clone()));
        let clock: transport::host::Shared<dyn Clock> = Rc::new(Local(state.clone()));
        let ids: transport::host::Shared<dyn IdSource> = Rc::new(Local(state.clone()));
        drop((sink, subscriber, observer, authority, clock, ids));
        futures::executor::block_on(async {
            assert!(signer.status().await.is_err());
            assert!(source.status().await.is_err());
            assert!(store.status().await.is_err());
        });
        assert_eq!(state.get(), 3);
        fn owned_subscription(
            request: transport::SubscriptionRequest,
            state: Rc<Cell<usize>>,
        ) -> transport::BoxSubscription {
            Box::new(Subscription {
                local: Local(state),
                request,
            })
        }
        let _constructor = owned_subscription;
    }
}
