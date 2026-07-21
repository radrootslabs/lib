use super::post_core_extension_capabilities::PostCoreExtensionCapabilities;
use super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult;
use crate::error::RadrootsEventStoreError;
use crate::model::RadrootsEventIngest;

pub(super) async fn dispatch_post_core_extensions(
    capabilities: &mut PostCoreExtensionCapabilities<'_, '_>,
    ingest: &RadrootsEventIngest,
    result: &ProtocolReconciliationV1IngestResult,
) -> Result<(), RadrootsEventStoreError> {
    capabilities.apply_v1(ingest, result).await?;
    capabilities.apply_v2().await?;
    Ok(())
}
