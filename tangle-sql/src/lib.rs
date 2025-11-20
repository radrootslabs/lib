pub use radroots_sql_core::error::SqlError;
pub use radroots_sql_core::{ExecOutcome, SqlExecutor};
use radroots_types::types::IError;

use radroots_tangle_schema::farm::{
    IFarmCreate,
    IFarmCreateResolve,
    IFarmDelete,
    IFarmDeleteResolve,
    IFarmFindMany,
    IFarmFindManyResolve,
    IFarmFindOne,
    IFarmFindOneResolve,
    IFarmUpdate,
    IFarmUpdateResolve,
};

use radroots_tangle_schema::location_gcs::{
    ILocationGcsCreate,
    ILocationGcsCreateResolve,
    ILocationGcsDelete,
    ILocationGcsDeleteResolve,
    ILocationGcsFindMany,
    ILocationGcsFindManyResolve,
    ILocationGcsFindOne,
    ILocationGcsFindOneResolve,
    ILocationGcsUpdate,
    ILocationGcsUpdateResolve,
};

use radroots_tangle_schema::log_error::{
    ILogErrorCreate,
    ILogErrorCreateResolve,
    ILogErrorDelete,
    ILogErrorDeleteResolve,
    ILogErrorFindMany,
    ILogErrorFindManyResolve,
    ILogErrorFindOne,
    ILogErrorFindOneResolve,
    ILogErrorUpdate,
    ILogErrorUpdateResolve,
};

use radroots_tangle_schema::media_image::{
    IMediaImageCreate,
    IMediaImageCreateResolve,
    IMediaImageDelete,
    IMediaImageDeleteResolve,
    IMediaImageFindMany,
    IMediaImageFindManyResolve,
    IMediaImageFindOne,
    IMediaImageFindOneResolve,
    IMediaImageUpdate,
    IMediaImageUpdateResolve,
};

use radroots_tangle_schema::nostr_profile::{
    INostrProfileCreate,
    INostrProfileCreateResolve,
    INostrProfileDelete,
    INostrProfileDeleteResolve,
    INostrProfileFindMany,
    INostrProfileFindManyResolve,
    INostrProfileFindOne,
    INostrProfileFindOneResolve,
    INostrProfileUpdate,
    INostrProfileUpdateResolve,
};

use radroots_tangle_schema::nostr_relay::{
    INostrRelayCreate,
    INostrRelayCreateResolve,
    INostrRelayDelete,
    INostrRelayDeleteResolve,
    INostrRelayFindMany,
    INostrRelayFindManyResolve,
    INostrRelayFindOne,
    INostrRelayFindOneResolve,
    INostrRelayUpdate,
    INostrRelayUpdateResolve,
};

use radroots_tangle_schema::trade_product::{
    ITradeProductCreate,
    ITradeProductCreateResolve,
    ITradeProductDelete,
    ITradeProductDeleteResolve,
    ITradeProductFindMany,
    ITradeProductFindManyResolve,
    ITradeProductFindOne,
    ITradeProductFindOneResolve,
    ITradeProductUpdate,
    ITradeProductUpdateResolve,
};

use radroots_tangle_schema::farm_location::{
    IFarmLocationRelation,
    IFarmLocationResolve,
};

use radroots_tangle_schema::nostr_profile_relay::{
    INostrProfileRelayRelation,
    INostrProfileRelayResolve,
};

use radroots_tangle_schema::trade_product_location::{
    ITradeProductLocationRelation,
    ITradeProductLocationResolve,
};

use radroots_tangle_schema::trade_product_media::{
    ITradeProductMediaRelation,
    ITradeProductMediaResolve,
};

pub mod backup;
pub mod migrations;
pub mod models;
pub use backup::{DatabaseBackup, MigrationBackup, SchemaEntry};
pub use models::*;

pub struct TangleSql<E: SqlExecutor> {
    executor: E,
}

impl<E: SqlExecutor> TangleSql<E> {
    pub fn new(executor: E) -> Self {
        Self { executor }
    }

    pub fn executor(&self) -> &E {
        &self.executor
    }

    pub fn migrate_up(&self) -> Result<(), SqlError> {
        crate::migrations::run_all_up(self.executor())
    }

    pub fn migrate_down(&self) -> Result<(), SqlError> {
        crate::migrations::run_all_down(self.executor())
    }

    pub fn backup_database(&self) -> Result<DatabaseBackup, SqlError> {
        crate::backup::export_database_backup(self.executor())
    }

    pub fn backup_database_json(&self) -> Result<String, SqlError> {
        crate::backup::export_database_backup_json(self.executor())
    }

    pub fn restore_database(&self, backup: &DatabaseBackup) -> Result<(), SqlError> {
        crate::backup::restore_database_backup(self.executor(), backup)
    }

    pub fn restore_database_json(&self, backup_json: &str) -> Result<(), SqlError> {
        crate::backup::restore_database_backup_json(self.executor(), backup_json)
    }

    pub fn farm_create(
        &self,
        opts: &IFarmCreate,
    ) -> Result<IFarmCreateResolve, IError<SqlError>> {
        models::farm::create(self.executor(), opts)
    }

    pub fn farm_find_many(
        &self,
        opts: &IFarmFindMany,
    ) -> Result<IFarmFindManyResolve, IError<SqlError>> {
        models::farm::find_many(self.executor(), opts)
    }

    pub fn farm_find_one(
        &self,
        opts: &IFarmFindOne,
    ) -> Result<IFarmFindOneResolve, IError<SqlError>> {
        models::farm::find_one(self.executor(), opts)
    }

    pub fn farm_update(
        &self,
        opts: &IFarmUpdate,
    ) -> Result<IFarmUpdateResolve, IError<SqlError>> {
        models::farm::update(self.executor(), opts)
    }

    pub fn farm_delete(
        &self,
        opts: &IFarmDelete,
    ) -> Result<IFarmDeleteResolve, IError<SqlError>> {
        models::farm::delete(self.executor(), opts)
    }

    pub fn location_gcs_create(
        &self,
        opts: &ILocationGcsCreate,
    ) -> Result<ILocationGcsCreateResolve, IError<SqlError>> {
        models::location_gcs::create(self.executor(), opts)
    }

    pub fn location_gcs_find_many(
        &self,
        opts: &ILocationGcsFindMany,
    ) -> Result<ILocationGcsFindManyResolve, IError<SqlError>> {
        models::location_gcs::find_many(self.executor(), opts)
    }

    pub fn location_gcs_find_one(
        &self,
        opts: &ILocationGcsFindOne,
    ) -> Result<ILocationGcsFindOneResolve, IError<SqlError>> {
        models::location_gcs::find_one(self.executor(), opts)
    }

    pub fn location_gcs_update(
        &self,
        opts: &ILocationGcsUpdate,
    ) -> Result<ILocationGcsUpdateResolve, IError<SqlError>> {
        models::location_gcs::update(self.executor(), opts)
    }

    pub fn location_gcs_delete(
        &self,
        opts: &ILocationGcsDelete,
    ) -> Result<ILocationGcsDeleteResolve, IError<SqlError>> {
        models::location_gcs::delete(self.executor(), opts)
    }

    pub fn log_error_create(
        &self,
        opts: &ILogErrorCreate,
    ) -> Result<ILogErrorCreateResolve, IError<SqlError>> {
        models::log_error::create(self.executor(), opts)
    }

    pub fn log_error_find_many(
        &self,
        opts: &ILogErrorFindMany,
    ) -> Result<ILogErrorFindManyResolve, IError<SqlError>> {
        models::log_error::find_many(self.executor(), opts)
    }

    pub fn log_error_find_one(
        &self,
        opts: &ILogErrorFindOne,
    ) -> Result<ILogErrorFindOneResolve, IError<SqlError>> {
        models::log_error::find_one(self.executor(), opts)
    }

    pub fn log_error_update(
        &self,
        opts: &ILogErrorUpdate,
    ) -> Result<ILogErrorUpdateResolve, IError<SqlError>> {
        models::log_error::update(self.executor(), opts)
    }

    pub fn log_error_delete(
        &self,
        opts: &ILogErrorDelete,
    ) -> Result<ILogErrorDeleteResolve, IError<SqlError>> {
        models::log_error::delete(self.executor(), opts)
    }

    pub fn media_image_create(
        &self,
        opts: &IMediaImageCreate,
    ) -> Result<IMediaImageCreateResolve, IError<SqlError>> {
        models::media_image::create(self.executor(), opts)
    }

    pub fn media_image_find_many(
        &self,
        opts: &IMediaImageFindMany,
    ) -> Result<IMediaImageFindManyResolve, IError<SqlError>> {
        models::media_image::find_many(self.executor(), opts)
    }

    pub fn media_image_find_one(
        &self,
        opts: &IMediaImageFindOne,
    ) -> Result<IMediaImageFindOneResolve, IError<SqlError>> {
        models::media_image::find_one(self.executor(), opts)
    }

    pub fn media_image_update(
        &self,
        opts: &IMediaImageUpdate,
    ) -> Result<IMediaImageUpdateResolve, IError<SqlError>> {
        models::media_image::update(self.executor(), opts)
    }

    pub fn media_image_delete(
        &self,
        opts: &IMediaImageDelete,
    ) -> Result<IMediaImageDeleteResolve, IError<SqlError>> {
        models::media_image::delete(self.executor(), opts)
    }

    pub fn nostr_profile_create(
        &self,
        opts: &INostrProfileCreate,
    ) -> Result<INostrProfileCreateResolve, IError<SqlError>> {
        models::nostr_profile::create(self.executor(), opts)
    }

    pub fn nostr_profile_find_many(
        &self,
        opts: &INostrProfileFindMany,
    ) -> Result<INostrProfileFindManyResolve, IError<SqlError>> {
        models::nostr_profile::find_many(self.executor(), opts)
    }

    pub fn nostr_profile_find_one(
        &self,
        opts: &INostrProfileFindOne,
    ) -> Result<INostrProfileFindOneResolve, IError<SqlError>> {
        models::nostr_profile::find_one(self.executor(), opts)
    }

    pub fn nostr_profile_update(
        &self,
        opts: &INostrProfileUpdate,
    ) -> Result<INostrProfileUpdateResolve, IError<SqlError>> {
        models::nostr_profile::update(self.executor(), opts)
    }

    pub fn nostr_profile_delete(
        &self,
        opts: &INostrProfileDelete,
    ) -> Result<INostrProfileDeleteResolve, IError<SqlError>> {
        models::nostr_profile::delete(self.executor(), opts)
    }

    pub fn nostr_relay_create(
        &self,
        opts: &INostrRelayCreate,
    ) -> Result<INostrRelayCreateResolve, IError<SqlError>> {
        models::nostr_relay::create(self.executor(), opts)
    }

    pub fn nostr_relay_find_many(
        &self,
        opts: &INostrRelayFindMany,
    ) -> Result<INostrRelayFindManyResolve, IError<SqlError>> {
        models::nostr_relay::find_many(self.executor(), opts)
    }

    pub fn nostr_relay_find_one(
        &self,
        opts: &INostrRelayFindOne,
    ) -> Result<INostrRelayFindOneResolve, IError<SqlError>> {
        models::nostr_relay::find_one(self.executor(), opts)
    }

    pub fn nostr_relay_update(
        &self,
        opts: &INostrRelayUpdate,
    ) -> Result<INostrRelayUpdateResolve, IError<SqlError>> {
        models::nostr_relay::update(self.executor(), opts)
    }

    pub fn nostr_relay_delete(
        &self,
        opts: &INostrRelayDelete,
    ) -> Result<INostrRelayDeleteResolve, IError<SqlError>> {
        models::nostr_relay::delete(self.executor(), opts)
    }

    pub fn trade_product_create(
        &self,
        opts: &ITradeProductCreate,
    ) -> Result<ITradeProductCreateResolve, IError<SqlError>> {
        models::trade_product::create(self.executor(), opts)
    }

    pub fn trade_product_find_many(
        &self,
        opts: &ITradeProductFindMany,
    ) -> Result<ITradeProductFindManyResolve, IError<SqlError>> {
        models::trade_product::find_many(self.executor(), opts)
    }

    pub fn trade_product_find_one(
        &self,
        opts: &ITradeProductFindOne,
    ) -> Result<ITradeProductFindOneResolve, IError<SqlError>> {
        models::trade_product::find_one(self.executor(), opts)
    }

    pub fn trade_product_update(
        &self,
        opts: &ITradeProductUpdate,
    ) -> Result<ITradeProductUpdateResolve, IError<SqlError>> {
        models::trade_product::update(self.executor(), opts)
    }

    pub fn trade_product_delete(
        &self,
        opts: &ITradeProductDelete,
    ) -> Result<ITradeProductDeleteResolve, IError<SqlError>> {
        models::trade_product::delete(self.executor(), opts)
    }

    pub fn farm_location_set(
        &self,
        opts: &IFarmLocationRelation,
    ) -> Result<IFarmLocationResolve, IError<SqlError>> {
        models::farm_location::set(self.executor(), opts)
    }

    pub fn farm_location_unset(
        &self,
        opts: &IFarmLocationRelation,
    ) -> Result<IFarmLocationResolve, IError<SqlError>> {
        models::farm_location::unset(self.executor(), opts)
    }

    pub fn nostr_profile_relay_set(
        &self,
        opts: &INostrProfileRelayRelation,
    ) -> Result<INostrProfileRelayResolve, IError<SqlError>> {
        models::nostr_profile_relay::set(self.executor(), opts)
    }

    pub fn nostr_profile_relay_unset(
        &self,
        opts: &INostrProfileRelayRelation,
    ) -> Result<INostrProfileRelayResolve, IError<SqlError>> {
        models::nostr_profile_relay::unset(self.executor(), opts)
    }

    pub fn trade_product_location_set(
        &self,
        opts: &ITradeProductLocationRelation,
    ) -> Result<ITradeProductLocationResolve, IError<SqlError>> {
        models::trade_product_location::set(self.executor(), opts)
    }

    pub fn trade_product_location_unset(
        &self,
        opts: &ITradeProductLocationRelation,
    ) -> Result<ITradeProductLocationResolve, IError<SqlError>> {
        models::trade_product_location::unset(self.executor(), opts)
    }

    pub fn trade_product_media_set(
        &self,
        opts: &ITradeProductMediaRelation,
    ) -> Result<ITradeProductMediaResolve, IError<SqlError>> {
        models::trade_product_media::set(self.executor(), opts)
    }

    pub fn trade_product_media_unset(
        &self,
        opts: &ITradeProductMediaRelation,
    ) -> Result<ITradeProductMediaResolve, IError<SqlError>> {
        models::trade_product_media::unset(self.executor(), opts)
    }

}
