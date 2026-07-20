use super::post_core_extensions_v1::apply_post_core_extensions_v1;
use super::post_core_storage_v1::PostCoreStorageV1;
use super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult;
use crate::error::RadrootsEventStoreError;
use crate::model::RadrootsEventIngest;
use sqlx::{Sqlite, Transaction};

pub(super) struct PostCoreExtensionCapabilities<'borrow, 'db> {
    tx: &'borrow mut Transaction<'db, Sqlite>,
}

impl<'borrow, 'db> PostCoreExtensionCapabilities<'borrow, 'db> {
    pub(super) fn new(tx: &'borrow mut Transaction<'db, Sqlite>) -> Self {
        Self { tx }
    }

    pub(super) async fn apply_v1(
        &mut self,
        ingest: &RadrootsEventIngest,
        result: &ProtocolReconciliationV1IngestResult,
    ) -> Result<(), RadrootsEventStoreError> {
        let mut storage = PostCoreStorageV1::new(self.tx);
        apply_post_core_extensions_v1(&mut storage, ingest, result).await
    }
}
