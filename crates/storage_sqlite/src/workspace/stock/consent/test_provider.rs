use super::*;
use radroots_secrets::{
    Error,
    file::FileProvider,
    id::BackendKind,
    provider::{SecretCapabilities, SecretProvider},
    wrapping::{KeyWrapping, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret},
};
use radroots_transport::BoxFuture;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

struct Gate {
    state: Mutex<(bool, bool, bool)>,
    changed: Condvar,
}
impl Gate {
    fn new() -> Self {
        Self {
            state: Mutex::new((false, false, false)),
            changed: Condvar::new(),
        }
    }
    fn arm(&self) {
        *self.state.lock().unwrap() = (true, false, false);
    }
    fn pause(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.0 {
            return;
        }
        state.0 = false;
        state.1 = true;
        self.changed.notify_all();
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(20), |s| !s.2)
            .unwrap();
        assert!(
            !timeout.timed_out() && state.2,
            "test must release provider"
        );
    }
    fn entered(&self) {
        let (state, timeout) = self
            .changed
            .wait_timeout_while(self.state.lock().unwrap(), Duration::from_secs(20), |s| {
                !s.1
            })
            .unwrap();
        assert!(
            !timeout.timed_out() && state.1,
            "worker must enter actual provider IO"
        );
    }
    fn release(&self) {
        self.state.lock().unwrap().2 = true;
        self.changed.notify_all();
    }
}
struct Provider {
    inner: Arc<FileProvider>,
    gate: Gate,
}
impl SecretProvider for Provider {
    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
    fn capabilities(&self) -> SecretCapabilities {
        self.inner.capabilities()
    }
}
impl KeyWrapping for Provider {
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>> {
        Box::pin(async move {
            let result = self.inner.wrap(request).await;
            if request.context().payload_schema().as_str() == "radroots.seller_stock.authority.v10"
            {
                self.gate.pause();
            }
            result
        })
    }
    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        self.inner.unwrap(request)
    }
}
async fn reopen(r: &mut Rig) -> Arc<Provider> {
    r.store.close().await.unwrap();
    let provider = Arc::new(Provider {
        inner: r.fixture.provider.clone(),
        gate: Gate::new(),
    });
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.input.clone(),
        r.fixture.crypto_with_provider(provider.clone()),
        r.clock.clone(),
    )
    .await
    .unwrap();
    provider
}

#[tokio::test]
async fn current_time_and_target_are_rechecked_after_actual_provider_io() {
    for changed_target in [false, true] {
        let mut r = Rig::new().await;
        let provider = reopen(&mut r).await;
        let capture = r.capture(&r.binding, 0x31);
        let mut raw = SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(r.path.join("stock_execution_v1.sqlite")),
        )
        .await
        .unwrap();
        provider.gate.arm();
        let mut future =
            Box::pin(
                r.store
                    .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof),
            );
        assert!(futures::poll!(future.as_mut()).is_pending());
        provider.gate.entered();
        if changed_target {
            // This write succeeds while the real wrapping provider is paused:
            // the worker must not hold a SQL transaction across provider IO.
            sqlx::query(
                "UPDATE stock_grants SET protected=randomblob(length(protected)) WHERE grant_id=?",
            )
            .bind(r.input.grant_id().as_bytes().as_slice())
            .execute(&mut raw)
            .await
            .unwrap();
        } else {
            r.clock.0.store(151, Ordering::SeqCst);
        }
        provider.gate.release();
        let result = future.await;
        if changed_target {
            assert!(result.is_err());
        } else {
            assert_eq!(result, Err(OperationError::DeadlineExceeded));
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_operations")
            .fetch_one(&mut raw)
            .await
            .unwrap();
        assert_eq!(count, 0);
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn cancelled_issuance_observer_cannot_release_owner_before_drain_and_original_recovery() {
    let mut r = Rig::new().await;
    let provider = reopen(&mut r).await;
    let capture = r.capture(&r.binding, 0x31);
    provider.gate.arm();
    let mut future =
        Box::pin(
            r.store
                .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof),
        );
    assert!(futures::poll!(future.as_mut()).is_pending());
    provider.gate.entered();
    drop(future);
    assert!(matches!(
        NativeWorkspace::open_seller_stock(
            &r.path,
            r.input.clone(),
            r.fixture.crypto(),
            r.clock.clone()
        )
        .await,
        Err(WorkspaceError::WriterActive)
    ));
    provider.gate.release();
    r.store.close().await.unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.input.clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let receipt = store
        .stock_consent(
            r.binding.clone(),
            r.minimum,
            capture.command().context().operation_id(),
            r.input.grant_id(),
            &r.proof,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        receipt.control_operation_id(),
        capture.control_operation_id()
    );
    assert_eq!(receipt.transaction_id(), capture.transaction_id());
    assert_eq!(
        committed(
            store
                .issue_stock_consent(r.binding.clone(), receipt.checkpoint(), &capture, &r.proof)
                .await
                .unwrap()
        ),
        receipt
    );
    store.close().await.unwrap();
}
