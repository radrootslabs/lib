use super::post_core_storage_v2::PostCoreStorageV2;
use crate::RadrootsEventStoreError;

pub(super) async fn apply_post_core_extensions_v2(
    storage: &mut PostCoreStorageV2<'_, '_>,
) -> Result<(), RadrootsEventStoreError> {
    storage.apply_pending_food_availability_transitions().await
}
