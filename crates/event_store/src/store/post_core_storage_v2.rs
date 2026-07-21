use super::food_availability_projection_v1::apply_pending_food_availability_transitions_v1;
use crate::RadrootsEventStoreError;
use sqlx::{Sqlite, Transaction};

pub(super) struct PostCoreStorageV2<'borrow, 'db> {
    tx: &'borrow mut Transaction<'db, Sqlite>,
}

impl<'borrow, 'db> PostCoreStorageV2<'borrow, 'db> {
    pub(super) fn new(tx: &'borrow mut Transaction<'db, Sqlite>) -> Self {
        Self { tx }
    }

    pub(super) async fn apply_pending_food_availability_transitions(
        &mut self,
    ) -> Result<(), RadrootsEventStoreError> {
        apply_pending_food_availability_transitions_v1(self.tx).await
    }
}
