use super::*;
use radroots_secrets::{
    Error,
    provider::{SecretCapabilities, SecretProvider},
    wrapping::{KeyWrapping, UnwrapRequest, WrapRequest, WrappedSecret},
};
use radroots_transport::source::BoxFuture;

/// Pause after real provider IO, before the worker can confirm the observed
/// rows. The other connection must remain able to write at this boundary.
struct PausedProvider {
    inner: Arc<FileProvider>,
    gate: PausedClock,
    schema: std::sync::Mutex<Option<&'static str>>,
}
impl PausedProvider {
    fn arm(&self, schema: &'static str) {
        *self.schema.lock().unwrap() = Some(schema);
        self.gate.arm();
    }
    fn pause(&self, schema: &str) {
        let selected = {
            let mut armed = self.schema.lock().unwrap();
            if *armed == Some(schema) {
                armed.take().is_some()
            } else {
                false
            }
        };
        if selected {
            self.gate.now().unwrap();
        }
    }
}
impl SecretProvider for PausedProvider {
    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
    fn capabilities(&self) -> SecretCapabilities {
        self.inner.capabilities()
    }
}
impl KeyWrapping for PausedProvider {
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>> {
        self.inner.wrap(request)
    }
    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        Box::pin(async move {
            let result = self.inner.unwrap(request).await;
            self.pause(request.context().payload_schema().as_str());
            result
        })
    }
}

#[tokio::test]
async fn source_reads_recheck_original_rows_changed_during_actual_provider_io() {
    for statement in [
        "UPDATE stock_source_parts SET protected=randomblob(length(protected)) WHERE role=0 AND part=0",
        "UPDATE stock_source_records SET protected=randomblob(length(protected))",
        "UPDATE stock_source_evidence SET transaction_id=zeroblob(32)",
    ] {
        let row = source_vector("request/peer");
        let f = Fixture::new();
        let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
        let min = source_minimum(&store, &e).await;
        let original = capture(&e, &row).await;
        let receipt = retained(
            store
                .retain_stock_source(binding.clone(), min, &original, e.proof())
                .await
                .unwrap(),
        );
        store.close().await.unwrap();
        let provider = Arc::new(PausedProvider {
            inner: f.provider.clone(),
            gate: PausedClock {
                state: std::sync::Mutex::new((false, false, false)),
                changed: std::sync::Condvar::new(),
            },
            schema: std::sync::Mutex::new(None),
        });
        let crypto = WorkspaceCrypto::new(
            provider.clone(),
            SecretRef::new(
                f.reference.id().clone(),
                f.reference.backend(),
                f.reference.key_version(),
            ),
            SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
            AccessPolicy::standard(),
        )
        .unwrap();
        let store =
            NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), crypto, f.clock.clone())
                .await
                .unwrap();
        let mut conn = database(&f).await;
        provider.arm("radroots.seller_stock.source_record.v1");
        let mut future = Box::pin(store.stock_source(
            binding,
            receipt.checkpoint(),
            original.envelope_id(),
            e.proof(),
        ));
        assert!(futures::poll!(future.as_mut()).is_pending());
        provider.gate.entered();
        // This actual write succeeds while the selected worker is awaiting
        // provider IO; no SQLite transaction may span that IO.
        sqlx::raw_sql(statement).execute(&mut conn).await.unwrap();
        provider.gate.release();
        assert!(matches!(
            future.await,
            Err(OperationError::RecoveryRequired)
        ));
        conn.close().await.unwrap();
        store.close().await.unwrap();
    }
}
