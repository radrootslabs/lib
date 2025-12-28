pub use radroots_sql_core::error::SqlError;
pub use radroots_sql_core::{ExecOutcome, SqlExecutor};
use radroots_types::types::IError;

use radroots_tangle_db_schema::farm::{
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

use radroots_tangle_db_schema::farm_gcs_location::{
    IFarmGcsLocationCreate,
    IFarmGcsLocationCreateResolve,
    IFarmGcsLocationDelete,
    IFarmGcsLocationDeleteResolve,
    IFarmGcsLocationFindMany,
    IFarmGcsLocationFindManyResolve,
    IFarmGcsLocationFindOne,
    IFarmGcsLocationFindOneResolve,
    IFarmGcsLocationUpdate,
    IFarmGcsLocationUpdateResolve,
};

use radroots_tangle_db_schema::farm_member::{
    IFarmMemberCreate,
    IFarmMemberCreateResolve,
    IFarmMemberDelete,
    IFarmMemberDeleteResolve,
    IFarmMemberFindMany,
    IFarmMemberFindManyResolve,
    IFarmMemberFindOne,
    IFarmMemberFindOneResolve,
    IFarmMemberUpdate,
    IFarmMemberUpdateResolve,
};

use radroots_tangle_db_schema::farm_member_claim::{
    IFarmMemberClaimCreate,
    IFarmMemberClaimCreateResolve,
    IFarmMemberClaimDelete,
    IFarmMemberClaimDeleteResolve,
    IFarmMemberClaimFindMany,
    IFarmMemberClaimFindManyResolve,
    IFarmMemberClaimFindOne,
    IFarmMemberClaimFindOneResolve,
    IFarmMemberClaimUpdate,
    IFarmMemberClaimUpdateResolve,
};

use radroots_tangle_db_schema::farm_tag::{
    IFarmTagCreate,
    IFarmTagCreateResolve,
    IFarmTagDelete,
    IFarmTagDeleteResolve,
    IFarmTagFindMany,
    IFarmTagFindManyResolve,
    IFarmTagFindOne,
    IFarmTagFindOneResolve,
    IFarmTagUpdate,
    IFarmTagUpdateResolve,
};

use radroots_tangle_db_schema::gcs_location::{
    IGcsLocationCreate,
    IGcsLocationCreateResolve,
    IGcsLocationDelete,
    IGcsLocationDeleteResolve,
    IGcsLocationFindMany,
    IGcsLocationFindManyResolve,
    IGcsLocationFindOne,
    IGcsLocationFindOneResolve,
    IGcsLocationUpdate,
    IGcsLocationUpdateResolve,
};

use radroots_tangle_db_schema::log_error::{
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

use radroots_tangle_db_schema::media_image::{
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

use radroots_tangle_db_schema::nostr_profile::{
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

use radroots_tangle_db_schema::nostr_event_state::{
    INostrEventStateCreate,
    INostrEventStateCreateResolve,
    INostrEventStateDelete,
    INostrEventStateDeleteResolve,
    INostrEventStateFindMany,
    INostrEventStateFindManyResolve,
    INostrEventStateFindOne,
    INostrEventStateFindOneResolve,
    INostrEventStateUpdate,
    INostrEventStateUpdateResolve,
};

use radroots_tangle_db_schema::nostr_relay::{
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

use radroots_tangle_db_schema::trade_product::{
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

use radroots_tangle_db_schema::plot::{
    IPlotCreate,
    IPlotCreateResolve,
    IPlotDelete,
    IPlotDeleteResolve,
    IPlotFindMany,
    IPlotFindManyResolve,
    IPlotFindOne,
    IPlotFindOneResolve,
    IPlotUpdate,
    IPlotUpdateResolve,
};

use radroots_tangle_db_schema::plot_gcs_location::{
    IPlotGcsLocationCreate,
    IPlotGcsLocationCreateResolve,
    IPlotGcsLocationDelete,
    IPlotGcsLocationDeleteResolve,
    IPlotGcsLocationFindMany,
    IPlotGcsLocationFindManyResolve,
    IPlotGcsLocationFindOne,
    IPlotGcsLocationFindOneResolve,
    IPlotGcsLocationUpdate,
    IPlotGcsLocationUpdateResolve,
};

use radroots_tangle_db_schema::plot_tag::{
    IPlotTagCreate,
    IPlotTagCreateResolve,
    IPlotTagDelete,
    IPlotTagDeleteResolve,
    IPlotTagFindMany,
    IPlotTagFindManyResolve,
    IPlotTagFindOne,
    IPlotTagFindOneResolve,
    IPlotTagUpdate,
    IPlotTagUpdateResolve,
};

use radroots_tangle_db_schema::nostr_profile_relay::{
    INostrProfileRelayRelation,
    INostrProfileRelayResolve,
};

use radroots_tangle_db_schema::trade_product_location::{
    ITradeProductLocationRelation,
    ITradeProductLocationResolve,
};

use radroots_tangle_db_schema::trade_product_media::{
    ITradeProductMediaRelation,
    ITradeProductMediaResolve,
};

pub mod backup;
pub mod export;
pub mod migrations;
pub mod models;
pub use backup::{DatabaseBackup, MigrationBackup, SchemaEntry};
pub use export::{TANGLE_DB_EXPORT_VERSION, TableCount, TangleDbExportManifestRs, export_manifest};
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

    pub fn plot_create(
        &self,
        opts: &IPlotCreate,
    ) -> Result<IPlotCreateResolve, IError<SqlError>> {
        models::plot::create(self.executor(), opts)
    }

    pub fn plot_find_many(
        &self,
        opts: &IPlotFindMany,
    ) -> Result<IPlotFindManyResolve, IError<SqlError>> {
        models::plot::find_many(self.executor(), opts)
    }

    pub fn plot_find_one(
        &self,
        opts: &IPlotFindOne,
    ) -> Result<IPlotFindOneResolve, IError<SqlError>> {
        models::plot::find_one(self.executor(), opts)
    }

    pub fn plot_update(
        &self,
        opts: &IPlotUpdate,
    ) -> Result<IPlotUpdateResolve, IError<SqlError>> {
        models::plot::update(self.executor(), opts)
    }

    pub fn plot_delete(
        &self,
        opts: &IPlotDelete,
    ) -> Result<IPlotDeleteResolve, IError<SqlError>> {
        models::plot::delete(self.executor(), opts)
    }

    pub fn gcs_location_create(
        &self,
        opts: &IGcsLocationCreate,
    ) -> Result<IGcsLocationCreateResolve, IError<SqlError>> {
        models::gcs_location::create(self.executor(), opts)
    }

    pub fn gcs_location_find_many(
        &self,
        opts: &IGcsLocationFindMany,
    ) -> Result<IGcsLocationFindManyResolve, IError<SqlError>> {
        models::gcs_location::find_many(self.executor(), opts)
    }

    pub fn gcs_location_find_one(
        &self,
        opts: &IGcsLocationFindOne,
    ) -> Result<IGcsLocationFindOneResolve, IError<SqlError>> {
        models::gcs_location::find_one(self.executor(), opts)
    }

    pub fn gcs_location_update(
        &self,
        opts: &IGcsLocationUpdate,
    ) -> Result<IGcsLocationUpdateResolve, IError<SqlError>> {
        models::gcs_location::update(self.executor(), opts)
    }

    pub fn gcs_location_delete(
        &self,
        opts: &IGcsLocationDelete,
    ) -> Result<IGcsLocationDeleteResolve, IError<SqlError>> {
        models::gcs_location::delete(self.executor(), opts)
    }

    pub fn farm_gcs_location_create(
        &self,
        opts: &IFarmGcsLocationCreate,
    ) -> Result<IFarmGcsLocationCreateResolve, IError<SqlError>> {
        models::farm_gcs_location::create(self.executor(), opts)
    }

    pub fn farm_gcs_location_find_many(
        &self,
        opts: &IFarmGcsLocationFindMany,
    ) -> Result<IFarmGcsLocationFindManyResolve, IError<SqlError>> {
        models::farm_gcs_location::find_many(self.executor(), opts)
    }

    pub fn farm_gcs_location_find_one(
        &self,
        opts: &IFarmGcsLocationFindOne,
    ) -> Result<IFarmGcsLocationFindOneResolve, IError<SqlError>> {
        models::farm_gcs_location::find_one(self.executor(), opts)
    }

    pub fn farm_gcs_location_update(
        &self,
        opts: &IFarmGcsLocationUpdate,
    ) -> Result<IFarmGcsLocationUpdateResolve, IError<SqlError>> {
        models::farm_gcs_location::update(self.executor(), opts)
    }

    pub fn farm_gcs_location_delete(
        &self,
        opts: &IFarmGcsLocationDelete,
    ) -> Result<IFarmGcsLocationDeleteResolve, IError<SqlError>> {
        models::farm_gcs_location::delete(self.executor(), opts)
    }

    pub fn plot_gcs_location_create(
        &self,
        opts: &IPlotGcsLocationCreate,
    ) -> Result<IPlotGcsLocationCreateResolve, IError<SqlError>> {
        models::plot_gcs_location::create(self.executor(), opts)
    }

    pub fn plot_gcs_location_find_many(
        &self,
        opts: &IPlotGcsLocationFindMany,
    ) -> Result<IPlotGcsLocationFindManyResolve, IError<SqlError>> {
        models::plot_gcs_location::find_many(self.executor(), opts)
    }

    pub fn plot_gcs_location_find_one(
        &self,
        opts: &IPlotGcsLocationFindOne,
    ) -> Result<IPlotGcsLocationFindOneResolve, IError<SqlError>> {
        models::plot_gcs_location::find_one(self.executor(), opts)
    }

    pub fn plot_gcs_location_update(
        &self,
        opts: &IPlotGcsLocationUpdate,
    ) -> Result<IPlotGcsLocationUpdateResolve, IError<SqlError>> {
        models::plot_gcs_location::update(self.executor(), opts)
    }

    pub fn plot_gcs_location_delete(
        &self,
        opts: &IPlotGcsLocationDelete,
    ) -> Result<IPlotGcsLocationDeleteResolve, IError<SqlError>> {
        models::plot_gcs_location::delete(self.executor(), opts)
    }

    pub fn farm_tag_create(
        &self,
        opts: &IFarmTagCreate,
    ) -> Result<IFarmTagCreateResolve, IError<SqlError>> {
        models::farm_tag::create(self.executor(), opts)
    }

    pub fn farm_tag_find_many(
        &self,
        opts: &IFarmTagFindMany,
    ) -> Result<IFarmTagFindManyResolve, IError<SqlError>> {
        models::farm_tag::find_many(self.executor(), opts)
    }

    pub fn farm_tag_find_one(
        &self,
        opts: &IFarmTagFindOne,
    ) -> Result<IFarmTagFindOneResolve, IError<SqlError>> {
        models::farm_tag::find_one(self.executor(), opts)
    }

    pub fn farm_tag_update(
        &self,
        opts: &IFarmTagUpdate,
    ) -> Result<IFarmTagUpdateResolve, IError<SqlError>> {
        models::farm_tag::update(self.executor(), opts)
    }

    pub fn farm_tag_delete(
        &self,
        opts: &IFarmTagDelete,
    ) -> Result<IFarmTagDeleteResolve, IError<SqlError>> {
        models::farm_tag::delete(self.executor(), opts)
    }

    pub fn plot_tag_create(
        &self,
        opts: &IPlotTagCreate,
    ) -> Result<IPlotTagCreateResolve, IError<SqlError>> {
        models::plot_tag::create(self.executor(), opts)
    }

    pub fn plot_tag_find_many(
        &self,
        opts: &IPlotTagFindMany,
    ) -> Result<IPlotTagFindManyResolve, IError<SqlError>> {
        models::plot_tag::find_many(self.executor(), opts)
    }

    pub fn plot_tag_find_one(
        &self,
        opts: &IPlotTagFindOne,
    ) -> Result<IPlotTagFindOneResolve, IError<SqlError>> {
        models::plot_tag::find_one(self.executor(), opts)
    }

    pub fn plot_tag_update(
        &self,
        opts: &IPlotTagUpdate,
    ) -> Result<IPlotTagUpdateResolve, IError<SqlError>> {
        models::plot_tag::update(self.executor(), opts)
    }

    pub fn plot_tag_delete(
        &self,
        opts: &IPlotTagDelete,
    ) -> Result<IPlotTagDeleteResolve, IError<SqlError>> {
        models::plot_tag::delete(self.executor(), opts)
    }

    pub fn farm_member_create(
        &self,
        opts: &IFarmMemberCreate,
    ) -> Result<IFarmMemberCreateResolve, IError<SqlError>> {
        models::farm_member::create(self.executor(), opts)
    }

    pub fn farm_member_find_many(
        &self,
        opts: &IFarmMemberFindMany,
    ) -> Result<IFarmMemberFindManyResolve, IError<SqlError>> {
        models::farm_member::find_many(self.executor(), opts)
    }

    pub fn farm_member_find_one(
        &self,
        opts: &IFarmMemberFindOne,
    ) -> Result<IFarmMemberFindOneResolve, IError<SqlError>> {
        models::farm_member::find_one(self.executor(), opts)
    }

    pub fn farm_member_update(
        &self,
        opts: &IFarmMemberUpdate,
    ) -> Result<IFarmMemberUpdateResolve, IError<SqlError>> {
        models::farm_member::update(self.executor(), opts)
    }

    pub fn farm_member_delete(
        &self,
        opts: &IFarmMemberDelete,
    ) -> Result<IFarmMemberDeleteResolve, IError<SqlError>> {
        models::farm_member::delete(self.executor(), opts)
    }

    pub fn farm_member_claim_create(
        &self,
        opts: &IFarmMemberClaimCreate,
    ) -> Result<IFarmMemberClaimCreateResolve, IError<SqlError>> {
        models::farm_member_claim::create(self.executor(), opts)
    }

    pub fn farm_member_claim_find_many(
        &self,
        opts: &IFarmMemberClaimFindMany,
    ) -> Result<IFarmMemberClaimFindManyResolve, IError<SqlError>> {
        models::farm_member_claim::find_many(self.executor(), opts)
    }

    pub fn farm_member_claim_find_one(
        &self,
        opts: &IFarmMemberClaimFindOne,
    ) -> Result<IFarmMemberClaimFindOneResolve, IError<SqlError>> {
        models::farm_member_claim::find_one(self.executor(), opts)
    }

    pub fn farm_member_claim_update(
        &self,
        opts: &IFarmMemberClaimUpdate,
    ) -> Result<IFarmMemberClaimUpdateResolve, IError<SqlError>> {
        models::farm_member_claim::update(self.executor(), opts)
    }

    pub fn farm_member_claim_delete(
        &self,
        opts: &IFarmMemberClaimDelete,
    ) -> Result<IFarmMemberClaimDeleteResolve, IError<SqlError>> {
        models::farm_member_claim::delete(self.executor(), opts)
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

    pub fn nostr_event_state_create(
        &self,
        opts: &INostrEventStateCreate,
    ) -> Result<INostrEventStateCreateResolve, IError<SqlError>> {
        models::nostr_event_state::create(self.executor(), opts)
    }

    pub fn nostr_event_state_find_many(
        &self,
        opts: &INostrEventStateFindMany,
    ) -> Result<INostrEventStateFindManyResolve, IError<SqlError>> {
        models::nostr_event_state::find_many(self.executor(), opts)
    }

    pub fn nostr_event_state_find_one(
        &self,
        opts: &INostrEventStateFindOne,
    ) -> Result<INostrEventStateFindOneResolve, IError<SqlError>> {
        models::nostr_event_state::find_one(self.executor(), opts)
    }

    pub fn nostr_event_state_update(
        &self,
        opts: &INostrEventStateUpdate,
    ) -> Result<INostrEventStateUpdateResolve, IError<SqlError>> {
        models::nostr_event_state::update(self.executor(), opts)
    }

    pub fn nostr_event_state_delete(
        &self,
        opts: &INostrEventStateDelete,
    ) -> Result<INostrEventStateDeleteResolve, IError<SqlError>> {
        models::nostr_event_state::delete(self.executor(), opts)
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
