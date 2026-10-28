use super::*;
#[path = "history_tests.rs"]
mod history;
use radroots_secrets::{
    Error,
    file::FileProvider,
    id::BackendKind,
    provider::{SecretCapabilities, SecretProvider},
    wrapping::{KeyWrapping, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret},
};
use radroots_transport::BoxFuture;
use std::{
    sync::{Condvar, Mutex},
    time::Duration,
};
#[derive(Clone, Copy, Eq, PartialEq)]
enum Direction {
    Wrap,
    Unwrap,
}
struct Selection {
    direction: Direction,
    schema: &'static str,
    skip: usize,
}
struct State {
    selection: Option<Selection>,
    entered: bool,
    released: bool,
}
struct Gate {
    state: Mutex<State>,
    changed: Condvar,
}
impl Gate {
    fn new() -> Self {
        Self {
            state: Mutex::new(State {
                selection: None,
                entered: false,
                released: false,
            }),
            changed: Condvar::new(),
        }
    }
    fn arm(&self, direction: Direction, schema: &'static str, skip: usize) {
        *self.state.lock().unwrap() = State {
            selection: Some(Selection {
                direction,
                schema,
                skip,
            }),
            entered: false,
            released: false,
        };
    }
    fn pause(&self, direction: Direction, schema: &str) {
        let mut state = self.state.lock().unwrap();
        let Some(selection) = state.selection.as_mut() else {
            return;
        };
        if selection.direction != direction || selection.schema != schema {
            return;
        }
        if selection.skip > 0 {
            selection.skip -= 1;
            return;
        }
        state.selection = None;
        state.entered = true;
        self.changed.notify_all();
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(20), |state| !state.released)
            .unwrap();
        assert!(
            !timeout.timed_out() && state.released,
            "test must release provider"
        );
    }
    fn entered(&self) {
        let (state, timeout) = self
            .changed
            .wait_timeout_while(
                self.state.lock().unwrap(),
                Duration::from_secs(20),
                |state| !state.entered,
            )
            .unwrap();
        assert!(
            !timeout.timed_out() && state.entered,
            "worker must reach actual provider IO"
        );
    }
    fn release(&self) {
        self.state.lock().unwrap().released = true;
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
            self.gate
                .pause(Direction::Wrap, request.context().payload_schema().as_str());
            result
        })
    }
    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        Box::pin(async move {
            let result = self.inner.unwrap(request).await;
            self.gate.pause(
                Direction::Unwrap,
                request.context().payload_schema().as_str(),
            );
            result
        })
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
        r.enrollment.input().clone(),
        r.fixture.crypto_with_provider(provider.clone()),
        r.clock.clone(),
    )
    .await
    .unwrap();
    provider
}

#[tokio::test]
async fn public_source_close_drains_admitted_work_after_observer_loss() {
    let mut r = Rig::new().await;
    let provider = reopen(&mut r).await;
    let capture = r.capture("listing_fixed");
    let minimum = r.minimum().await;
    provider
        .gate
        .arm(Direction::Wrap, "radroots.seller_stock.authority.v17", 0);
    let mut future = Box::pin(r.store.retain_stock_listing(
        r.binding.clone(),
        minimum,
        &capture,
        r.enrollment.proof(),
    ));
    assert!(futures::poll!(future.as_mut()).is_pending());
    provider.gate.entered();
    drop(future);
    assert!(matches!(
        NativeWorkspace::open_seller_stock(
            &r.path,
            r.enrollment.input().clone(),
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
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let source = store
        .stock_listing_source(
            r.binding.clone(),
            minimum,
            capture.listing_revision(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source.receipt().transaction_id(), capture.transaction_id());
    assert_eq!(source.raw_json(), vector("listing_fixed"));
    store.close().await.unwrap();
}

#[tokio::test]
async fn public_source_writes_recheck_expiry_and_head_rows_after_actual_protection_io() {
    for changed_head in [false, true] {
        let mut r = Rig::new().await;
        r.retain(&r.capture("listing_fixed")).await;
        let provider = reopen(&mut r).await;
        let capture = r.capture("listing_unsupported");
        let minimum = r.minimum().await;
        let mut raw = database(&r).await;
        provider
            .gate
            .arm(Direction::Wrap, "radroots.seller_stock.authority.v17", 0);
        let mut future = Box::pin(r.store.retain_stock_listing(
            r.binding.clone(),
            minimum,
            &capture,
            r.enrollment.proof(),
        ));
        assert!(futures::poll!(future.as_mut()).is_pending());
        provider.gate.entered();
        let original: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_public_heads")
            .fetch_one(&mut raw)
            .await
            .unwrap();
        if changed_head {
            sqlx::query("UPDATE stock_public_heads SET protected=randomblob(length(protected))")
                .execute(&mut raw)
                .await
                .unwrap();
        } else {
            r.clock.0.store(300, Ordering::SeqCst);
        }
        provider.gate.release();
        let result = future.await;
        if changed_head {
            assert_eq!(result.unwrap_err().code(), "recovery_required");
        } else {
            assert_eq!(
                result,
                Err(OperationError::Workspace(WorkspaceError::GrantExpired))
            );
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_public_events")
            .fetch_one(&mut raw)
            .await
            .unwrap();
        assert_eq!(count, 1);
        if changed_head {
            sqlx::query("UPDATE stock_public_heads SET protected=?")
                .bind(original)
                .execute(&mut raw)
                .await
                .unwrap();
        }
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn public_source_reads_recheck_parts_changed_after_their_successful_authenticated_opening() {
    let mut r = Rig::new().await;
    let capture = r.capture("listing_fixed");
    let receipt = r.retain(&capture).await;
    let provider = reopen(&mut r).await;
    let mut raw = database(&r).await;
    // Minimum, coordinate and original reads precede raw-part admission; pause
    // the fourth event-header opening in final positive identity verification.
    provider.gate.arm(
        Direction::Unwrap,
        "radroots.seller_stock.public_source_event.v1",
        3,
    );
    let mut future = Box::pin(r.store.stock_listing_head(
        r.binding.clone(),
        receipt.checkpoint(),
        capture.listing_revision().coordinate(),
        r.enrollment.proof(),
    ));
    assert!(futures::poll!(future.as_mut()).is_pending());
    provider.gate.entered();
    let original: Vec<u8> =
        sqlx::query_scalar("SELECT protected FROM stock_public_parts WHERE part=0")
            .fetch_one(&mut raw)
            .await
            .unwrap();
    sqlx::query(
        "UPDATE stock_public_parts SET protected=randomblob(length(protected)) WHERE part=0",
    )
    .execute(&mut raw)
    .await
    .unwrap();
    provider.gate.release();
    assert!(matches!(
        future.await,
        Err(OperationError::RecoveryRequired)
    ));
    sqlx::query("UPDATE stock_public_parts SET protected=? WHERE part=0")
        .bind(original)
        .execute(&mut raw)
        .await
        .unwrap();
    assert_eq!(
        r.store
            .stock_listing_head(
                r.binding.clone(),
                receipt.checkpoint(),
                capture.listing_revision().coordinate(),
                r.enrollment.proof()
            )
            .await
            .unwrap()
            .unwrap()
            .raw_json(),
        vector("listing_fixed")
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}
