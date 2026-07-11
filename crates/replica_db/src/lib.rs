use radroots_replica_db_schema::ReplicaSchemaError;
pub use radroots_sql_core::error::SqlError;
pub use radroots_sql_core::{ExecOutcome, SqlExecutor};

use radroots_replica_db_schema::farm::{
    IFarmCreate, IFarmCreateResolve, IFarmDelete, IFarmDeleteResolve, IFarmFindMany,
    IFarmFindManyResolve, IFarmFindOne, IFarmFindOneResolve, IFarmUpdate, IFarmUpdateResolve,
};

use radroots_replica_db_schema::farm_gcs_location::{
    IFarmGcsLocationCreate, IFarmGcsLocationCreateResolve, IFarmGcsLocationDelete,
    IFarmGcsLocationDeleteResolve, IFarmGcsLocationFindMany, IFarmGcsLocationFindManyResolve,
    IFarmGcsLocationFindOne, IFarmGcsLocationFindOneResolve, IFarmGcsLocationUpdate,
    IFarmGcsLocationUpdateResolve,
};

use radroots_replica_db_schema::farm_member::{
    IFarmMemberCreate, IFarmMemberCreateResolve, IFarmMemberDelete, IFarmMemberDeleteResolve,
    IFarmMemberFindMany, IFarmMemberFindManyResolve, IFarmMemberFindOne, IFarmMemberFindOneResolve,
    IFarmMemberUpdate, IFarmMemberUpdateResolve,
};

use radroots_replica_db_schema::farm_member_claim::{
    IFarmMemberClaimCreate, IFarmMemberClaimCreateResolve, IFarmMemberClaimDelete,
    IFarmMemberClaimDeleteResolve, IFarmMemberClaimFindMany, IFarmMemberClaimFindManyResolve,
    IFarmMemberClaimFindOne, IFarmMemberClaimFindOneResolve, IFarmMemberClaimUpdate,
    IFarmMemberClaimUpdateResolve,
};

use radroots_replica_db_schema::farm_tag::{
    IFarmTagCreate, IFarmTagCreateResolve, IFarmTagDelete, IFarmTagDeleteResolve, IFarmTagFindMany,
    IFarmTagFindManyResolve, IFarmTagFindOne, IFarmTagFindOneResolve, IFarmTagUpdate,
    IFarmTagUpdateResolve,
};

use radroots_replica_db_schema::gcs_location::{
    IGcsLocationCreate, IGcsLocationCreateResolve, IGcsLocationDelete, IGcsLocationDeleteResolve,
    IGcsLocationFindMany, IGcsLocationFindManyResolve, IGcsLocationFindOne,
    IGcsLocationFindOneResolve, IGcsLocationUpdate, IGcsLocationUpdateResolve,
};

use radroots_replica_db_schema::log_error::{
    ILogErrorCreate, ILogErrorCreateResolve, ILogErrorDelete, ILogErrorDeleteResolve,
    ILogErrorFindMany, ILogErrorFindManyResolve, ILogErrorFindOne, ILogErrorFindOneResolve,
    ILogErrorUpdate, ILogErrorUpdateResolve,
};

use radroots_replica_db_schema::media_image::{
    IMediaImageCreate, IMediaImageCreateResolve, IMediaImageDelete, IMediaImageDeleteResolve,
    IMediaImageFindMany, IMediaImageFindManyResolve, IMediaImageFindOne, IMediaImageFindOneResolve,
    IMediaImageUpdate, IMediaImageUpdateResolve,
};

use radroots_replica_db_schema::nostr_profile::{
    INostrProfileCreate, INostrProfileCreateResolve, INostrProfileDelete,
    INostrProfileDeleteResolve, INostrProfileFindMany, INostrProfileFindManyResolve,
    INostrProfileFindOne, INostrProfileFindOneResolve, INostrProfileUpdate,
    INostrProfileUpdateResolve,
};

use radroots_replica_db_schema::nostr_event_head::{
    INostrEventHeadCreate, INostrEventHeadCreateResolve, INostrEventHeadDelete,
    INostrEventHeadDeleteResolve, INostrEventHeadFindMany, INostrEventHeadFindManyResolve,
    INostrEventHeadFindOne, INostrEventHeadFindOneResolve, INostrEventHeadUpdate,
    INostrEventHeadUpdateResolve,
};

use radroots_replica_db_schema::nostr_relay::{
    INostrRelayCreate, INostrRelayCreateResolve, INostrRelayDelete, INostrRelayDeleteResolve,
    INostrRelayFindMany, INostrRelayFindManyResolve, INostrRelayFindOne, INostrRelayFindOneResolve,
    INostrRelayUpdate, INostrRelayUpdateResolve,
};

use radroots_replica_db_schema::trade_product::{
    ITradeProductCreate, ITradeProductCreateResolve, ITradeProductDelete,
    ITradeProductDeleteResolve, ITradeProductFindMany, ITradeProductFindManyResolve,
    ITradeProductFindOne, ITradeProductFindOneResolve, ITradeProductUpdate,
    ITradeProductUpdateResolve,
};

use radroots_replica_db_schema::plot::{
    IPlotCreate, IPlotCreateResolve, IPlotDelete, IPlotDeleteResolve, IPlotFindMany,
    IPlotFindManyResolve, IPlotFindOne, IPlotFindOneResolve, IPlotUpdate, IPlotUpdateResolve,
};

use radroots_replica_db_schema::plot_gcs_location::{
    IPlotGcsLocationCreate, IPlotGcsLocationCreateResolve, IPlotGcsLocationDelete,
    IPlotGcsLocationDeleteResolve, IPlotGcsLocationFindMany, IPlotGcsLocationFindManyResolve,
    IPlotGcsLocationFindOne, IPlotGcsLocationFindOneResolve, IPlotGcsLocationUpdate,
    IPlotGcsLocationUpdateResolve,
};

use radroots_replica_db_schema::plot_tag::{
    IPlotTagCreate, IPlotTagCreateResolve, IPlotTagDelete, IPlotTagDeleteResolve, IPlotTagFindMany,
    IPlotTagFindManyResolve, IPlotTagFindOne, IPlotTagFindOneResolve, IPlotTagUpdate,
    IPlotTagUpdateResolve,
};

use radroots_replica_db_schema::nostr_profile_relay::{
    INostrProfileRelayRelation, INostrProfileRelayResolve,
};

use radroots_replica_db_schema::trade_product_location::{
    ITradeProductLocationRelation, ITradeProductLocationResolve,
};

use radroots_replica_db_schema::trade_product_media::{
    ITradeProductMediaRelation, ITradeProductMediaResolve,
};

pub mod backup;
pub mod export;
pub mod migrations;
pub mod models;
pub mod query;
pub use backup::{DatabaseBackup, MigrationBackup, SchemaEntry};
pub use export::{
    REPLICA_DB_EXPORT_VERSION, ReplicaDbExportManifestRs, TableCount, export_manifest,
};
pub use models::*;
pub use query::ReplicaTradeProductSummaryRow;

pub struct ReplicaSql<E: SqlExecutor> {
    executor: E,
}

impl<E: SqlExecutor> ReplicaSql<E> {
    pub fn coverage_branch_probe(enabled: bool) -> &'static str {
        if enabled { "enabled" } else { "disabled" }
    }
}

impl<E: SqlExecutor> ReplicaSql<E> {
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
    ) -> Result<IFarmCreateResolve, ReplicaSchemaError<SqlError>> {
        models::farm::create(self.executor(), opts)
    }

    pub fn farm_find_many(
        &self,
        opts: &IFarmFindMany,
    ) -> Result<IFarmFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::farm::find_many(self.executor(), opts)
    }

    pub fn farm_find_one(
        &self,
        opts: &IFarmFindOne,
    ) -> Result<IFarmFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::farm::find_one(self.executor(), opts)
    }

    pub fn farm_update(
        &self,
        opts: &IFarmUpdate,
    ) -> Result<IFarmUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::farm::update(self.executor(), opts)
    }

    pub fn farm_delete(
        &self,
        opts: &IFarmDelete,
    ) -> Result<IFarmDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::farm::delete(self.executor(), opts)
    }

    pub fn plot_create(
        &self,
        opts: &IPlotCreate,
    ) -> Result<IPlotCreateResolve, ReplicaSchemaError<SqlError>> {
        models::plot::create(self.executor(), opts)
    }

    pub fn plot_find_many(
        &self,
        opts: &IPlotFindMany,
    ) -> Result<IPlotFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::plot::find_many(self.executor(), opts)
    }

    pub fn plot_find_one(
        &self,
        opts: &IPlotFindOne,
    ) -> Result<IPlotFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::plot::find_one(self.executor(), opts)
    }

    pub fn plot_update(
        &self,
        opts: &IPlotUpdate,
    ) -> Result<IPlotUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::plot::update(self.executor(), opts)
    }

    pub fn plot_delete(
        &self,
        opts: &IPlotDelete,
    ) -> Result<IPlotDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::plot::delete(self.executor(), opts)
    }

    pub fn gcs_location_create(
        &self,
        opts: &IGcsLocationCreate,
    ) -> Result<IGcsLocationCreateResolve, ReplicaSchemaError<SqlError>> {
        models::gcs_location::create(self.executor(), opts)
    }

    pub fn gcs_location_find_many(
        &self,
        opts: &IGcsLocationFindMany,
    ) -> Result<IGcsLocationFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::gcs_location::find_many(self.executor(), opts)
    }

    pub fn gcs_location_find_one(
        &self,
        opts: &IGcsLocationFindOne,
    ) -> Result<IGcsLocationFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::gcs_location::find_one(self.executor(), opts)
    }

    pub fn gcs_location_update(
        &self,
        opts: &IGcsLocationUpdate,
    ) -> Result<IGcsLocationUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::gcs_location::update(self.executor(), opts)
    }

    pub fn gcs_location_delete(
        &self,
        opts: &IGcsLocationDelete,
    ) -> Result<IGcsLocationDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::gcs_location::delete(self.executor(), opts)
    }

    pub fn farm_gcs_location_create(
        &self,
        opts: &IFarmGcsLocationCreate,
    ) -> Result<IFarmGcsLocationCreateResolve, ReplicaSchemaError<SqlError>> {
        models::farm_gcs_location::create(self.executor(), opts)
    }

    pub fn farm_gcs_location_find_many(
        &self,
        opts: &IFarmGcsLocationFindMany,
    ) -> Result<IFarmGcsLocationFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::farm_gcs_location::find_many(self.executor(), opts)
    }

    pub fn farm_gcs_location_find_one(
        &self,
        opts: &IFarmGcsLocationFindOne,
    ) -> Result<IFarmGcsLocationFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::farm_gcs_location::find_one(self.executor(), opts)
    }

    pub fn farm_gcs_location_update(
        &self,
        opts: &IFarmGcsLocationUpdate,
    ) -> Result<IFarmGcsLocationUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::farm_gcs_location::update(self.executor(), opts)
    }

    pub fn farm_gcs_location_delete(
        &self,
        opts: &IFarmGcsLocationDelete,
    ) -> Result<IFarmGcsLocationDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::farm_gcs_location::delete(self.executor(), opts)
    }

    pub fn plot_gcs_location_create(
        &self,
        opts: &IPlotGcsLocationCreate,
    ) -> Result<IPlotGcsLocationCreateResolve, ReplicaSchemaError<SqlError>> {
        models::plot_gcs_location::create(self.executor(), opts)
    }

    pub fn plot_gcs_location_find_many(
        &self,
        opts: &IPlotGcsLocationFindMany,
    ) -> Result<IPlotGcsLocationFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::plot_gcs_location::find_many(self.executor(), opts)
    }

    pub fn plot_gcs_location_find_one(
        &self,
        opts: &IPlotGcsLocationFindOne,
    ) -> Result<IPlotGcsLocationFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::plot_gcs_location::find_one(self.executor(), opts)
    }

    pub fn plot_gcs_location_update(
        &self,
        opts: &IPlotGcsLocationUpdate,
    ) -> Result<IPlotGcsLocationUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::plot_gcs_location::update(self.executor(), opts)
    }

    pub fn plot_gcs_location_delete(
        &self,
        opts: &IPlotGcsLocationDelete,
    ) -> Result<IPlotGcsLocationDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::plot_gcs_location::delete(self.executor(), opts)
    }

    pub fn farm_tag_create(
        &self,
        opts: &IFarmTagCreate,
    ) -> Result<IFarmTagCreateResolve, ReplicaSchemaError<SqlError>> {
        models::farm_tag::create(self.executor(), opts)
    }

    pub fn farm_tag_find_many(
        &self,
        opts: &IFarmTagFindMany,
    ) -> Result<IFarmTagFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::farm_tag::find_many(self.executor(), opts)
    }

    pub fn farm_tag_find_one(
        &self,
        opts: &IFarmTagFindOne,
    ) -> Result<IFarmTagFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::farm_tag::find_one(self.executor(), opts)
    }

    pub fn farm_tag_update(
        &self,
        opts: &IFarmTagUpdate,
    ) -> Result<IFarmTagUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::farm_tag::update(self.executor(), opts)
    }

    pub fn farm_tag_delete(
        &self,
        opts: &IFarmTagDelete,
    ) -> Result<IFarmTagDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::farm_tag::delete(self.executor(), opts)
    }

    pub fn plot_tag_create(
        &self,
        opts: &IPlotTagCreate,
    ) -> Result<IPlotTagCreateResolve, ReplicaSchemaError<SqlError>> {
        models::plot_tag::create(self.executor(), opts)
    }

    pub fn plot_tag_find_many(
        &self,
        opts: &IPlotTagFindMany,
    ) -> Result<IPlotTagFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::plot_tag::find_many(self.executor(), opts)
    }

    pub fn plot_tag_find_one(
        &self,
        opts: &IPlotTagFindOne,
    ) -> Result<IPlotTagFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::plot_tag::find_one(self.executor(), opts)
    }

    pub fn plot_tag_update(
        &self,
        opts: &IPlotTagUpdate,
    ) -> Result<IPlotTagUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::plot_tag::update(self.executor(), opts)
    }

    pub fn plot_tag_delete(
        &self,
        opts: &IPlotTagDelete,
    ) -> Result<IPlotTagDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::plot_tag::delete(self.executor(), opts)
    }

    pub fn farm_member_create(
        &self,
        opts: &IFarmMemberCreate,
    ) -> Result<IFarmMemberCreateResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member::create(self.executor(), opts)
    }

    pub fn farm_member_find_many(
        &self,
        opts: &IFarmMemberFindMany,
    ) -> Result<IFarmMemberFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member::find_many(self.executor(), opts)
    }

    pub fn farm_member_find_one(
        &self,
        opts: &IFarmMemberFindOne,
    ) -> Result<IFarmMemberFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member::find_one(self.executor(), opts)
    }

    pub fn farm_member_update(
        &self,
        opts: &IFarmMemberUpdate,
    ) -> Result<IFarmMemberUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member::update(self.executor(), opts)
    }

    pub fn farm_member_delete(
        &self,
        opts: &IFarmMemberDelete,
    ) -> Result<IFarmMemberDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member::delete(self.executor(), opts)
    }

    pub fn farm_member_claim_create(
        &self,
        opts: &IFarmMemberClaimCreate,
    ) -> Result<IFarmMemberClaimCreateResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member_claim::create(self.executor(), opts)
    }

    pub fn farm_member_claim_find_many(
        &self,
        opts: &IFarmMemberClaimFindMany,
    ) -> Result<IFarmMemberClaimFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member_claim::find_many(self.executor(), opts)
    }

    pub fn farm_member_claim_find_one(
        &self,
        opts: &IFarmMemberClaimFindOne,
    ) -> Result<IFarmMemberClaimFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member_claim::find_one(self.executor(), opts)
    }

    pub fn farm_member_claim_update(
        &self,
        opts: &IFarmMemberClaimUpdate,
    ) -> Result<IFarmMemberClaimUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member_claim::update(self.executor(), opts)
    }

    pub fn farm_member_claim_delete(
        &self,
        opts: &IFarmMemberClaimDelete,
    ) -> Result<IFarmMemberClaimDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::farm_member_claim::delete(self.executor(), opts)
    }

    pub fn log_error_create(
        &self,
        opts: &ILogErrorCreate,
    ) -> Result<ILogErrorCreateResolve, ReplicaSchemaError<SqlError>> {
        models::log_error::create(self.executor(), opts)
    }

    pub fn log_error_find_many(
        &self,
        opts: &ILogErrorFindMany,
    ) -> Result<ILogErrorFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::log_error::find_many(self.executor(), opts)
    }

    pub fn log_error_find_one(
        &self,
        opts: &ILogErrorFindOne,
    ) -> Result<ILogErrorFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::log_error::find_one(self.executor(), opts)
    }

    pub fn log_error_update(
        &self,
        opts: &ILogErrorUpdate,
    ) -> Result<ILogErrorUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::log_error::update(self.executor(), opts)
    }

    pub fn log_error_delete(
        &self,
        opts: &ILogErrorDelete,
    ) -> Result<ILogErrorDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::log_error::delete(self.executor(), opts)
    }

    pub fn media_image_create(
        &self,
        opts: &IMediaImageCreate,
    ) -> Result<IMediaImageCreateResolve, ReplicaSchemaError<SqlError>> {
        models::media_image::create(self.executor(), opts)
    }

    pub fn media_image_find_many(
        &self,
        opts: &IMediaImageFindMany,
    ) -> Result<IMediaImageFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::media_image::find_many(self.executor(), opts)
    }

    pub fn media_image_find_one(
        &self,
        opts: &IMediaImageFindOne,
    ) -> Result<IMediaImageFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::media_image::find_one(self.executor(), opts)
    }

    pub fn media_image_update(
        &self,
        opts: &IMediaImageUpdate,
    ) -> Result<IMediaImageUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::media_image::update(self.executor(), opts)
    }

    pub fn media_image_delete(
        &self,
        opts: &IMediaImageDelete,
    ) -> Result<IMediaImageDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::media_image::delete(self.executor(), opts)
    }

    pub fn nostr_profile_create(
        &self,
        opts: &INostrProfileCreate,
    ) -> Result<INostrProfileCreateResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_profile::create(self.executor(), opts)
    }

    pub fn nostr_profile_find_many(
        &self,
        opts: &INostrProfileFindMany,
    ) -> Result<INostrProfileFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_profile::find_many(self.executor(), opts)
    }

    pub fn nostr_profile_find_one(
        &self,
        opts: &INostrProfileFindOne,
    ) -> Result<INostrProfileFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_profile::find_one(self.executor(), opts)
    }

    pub fn nostr_profile_update(
        &self,
        opts: &INostrProfileUpdate,
    ) -> Result<INostrProfileUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_profile::update(self.executor(), opts)
    }

    pub fn nostr_profile_delete(
        &self,
        opts: &INostrProfileDelete,
    ) -> Result<INostrProfileDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_profile::delete(self.executor(), opts)
    }

    pub fn nostr_event_head_create(
        &self,
        opts: &INostrEventHeadCreate,
    ) -> Result<INostrEventHeadCreateResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_event_head::create(self.executor(), opts)
    }

    pub fn nostr_event_head_find_many(
        &self,
        opts: &INostrEventHeadFindMany,
    ) -> Result<INostrEventHeadFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_event_head::find_many(self.executor(), opts)
    }

    pub fn nostr_event_head_find_one(
        &self,
        opts: &INostrEventHeadFindOne,
    ) -> Result<INostrEventHeadFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_event_head::find_one(self.executor(), opts)
    }

    pub fn nostr_event_head_update(
        &self,
        opts: &INostrEventHeadUpdate,
    ) -> Result<INostrEventHeadUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_event_head::update(self.executor(), opts)
    }

    pub fn nostr_event_head_delete(
        &self,
        opts: &INostrEventHeadDelete,
    ) -> Result<INostrEventHeadDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_event_head::delete(self.executor(), opts)
    }

    pub fn nostr_relay_create(
        &self,
        opts: &INostrRelayCreate,
    ) -> Result<INostrRelayCreateResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_relay::create(self.executor(), opts)
    }

    pub fn nostr_relay_find_many(
        &self,
        opts: &INostrRelayFindMany,
    ) -> Result<INostrRelayFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_relay::find_many(self.executor(), opts)
    }

    pub fn nostr_relay_find_one(
        &self,
        opts: &INostrRelayFindOne,
    ) -> Result<INostrRelayFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_relay::find_one(self.executor(), opts)
    }

    pub fn nostr_relay_update(
        &self,
        opts: &INostrRelayUpdate,
    ) -> Result<INostrRelayUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_relay::update(self.executor(), opts)
    }

    pub fn nostr_relay_delete(
        &self,
        opts: &INostrRelayDelete,
    ) -> Result<INostrRelayDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_relay::delete(self.executor(), opts)
    }

    pub fn trade_product_create(
        &self,
        opts: &ITradeProductCreate,
    ) -> Result<ITradeProductCreateResolve, ReplicaSchemaError<SqlError>> {
        models::trade_product::create(self.executor(), opts)
    }

    pub fn trade_product_find_many(
        &self,
        opts: &ITradeProductFindMany,
    ) -> Result<ITradeProductFindManyResolve, ReplicaSchemaError<SqlError>> {
        models::trade_product::find_many(self.executor(), opts)
    }

    pub fn trade_product_find_one(
        &self,
        opts: &ITradeProductFindOne,
    ) -> Result<ITradeProductFindOneResolve, ReplicaSchemaError<SqlError>> {
        models::trade_product::find_one(self.executor(), opts)
    }

    pub fn trade_product_update(
        &self,
        opts: &ITradeProductUpdate,
    ) -> Result<ITradeProductUpdateResolve, ReplicaSchemaError<SqlError>> {
        models::trade_product::update(self.executor(), opts)
    }

    pub fn trade_product_delete(
        &self,
        opts: &ITradeProductDelete,
    ) -> Result<ITradeProductDeleteResolve, ReplicaSchemaError<SqlError>> {
        models::trade_product::delete(self.executor(), opts)
    }

    pub fn nostr_profile_relay_set(
        &self,
        opts: &INostrProfileRelayRelation,
    ) -> Result<INostrProfileRelayResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_profile_relay::set(self.executor(), opts)
    }

    pub fn nostr_profile_relay_unset(
        &self,
        opts: &INostrProfileRelayRelation,
    ) -> Result<INostrProfileRelayResolve, ReplicaSchemaError<SqlError>> {
        models::nostr_profile_relay::unset(self.executor(), opts)
    }

    pub fn trade_product_location_set(
        &self,
        opts: &ITradeProductLocationRelation,
    ) -> Result<ITradeProductLocationResolve, ReplicaSchemaError<SqlError>> {
        models::trade_product_location::set(self.executor(), opts)
    }

    pub fn trade_product_location_unset(
        &self,
        opts: &ITradeProductLocationRelation,
    ) -> Result<ITradeProductLocationResolve, ReplicaSchemaError<SqlError>> {
        models::trade_product_location::unset(self.executor(), opts)
    }

    pub fn trade_product_media_set(
        &self,
        opts: &ITradeProductMediaRelation,
    ) -> Result<ITradeProductMediaResolve, ReplicaSchemaError<SqlError>> {
        models::trade_product_media::set(self.executor(), opts)
    }

    pub fn trade_product_media_unset(
        &self,
        opts: &ITradeProductMediaRelation,
    ) -> Result<ITradeProductMediaResolve, ReplicaSchemaError<SqlError>> {
        models::trade_product_media::unset(self.executor(), opts)
    }
}

#[cfg(test)]
mod tests {
    use super::ReplicaSql;
    use radroots_sql_core::{ExecOutcome, SqlError, SqlExecutor};

    struct ProbeExecutor;

    impl SqlExecutor for ProbeExecutor {
        fn exec(&self, _sql: &str, _params_json: &str) -> Result<ExecOutcome, SqlError> {
            Ok(ExecOutcome {
                changes: 0,
                last_insert_id: 0,
            })
        }

        fn query_raw(&self, _sql: &str, _params_json: &str) -> Result<String, SqlError> {
            Ok("[]".to_string())
        }

        fn begin(&self) -> Result<(), SqlError> {
            Ok(())
        }

        fn commit(&self) -> Result<(), SqlError> {
            Ok(())
        }

        fn rollback(&self) -> Result<(), SqlError> {
            Ok(())
        }
    }

    #[test]
    fn replica_sql_constructor_and_executor_access_are_supported() {
        let db = ReplicaSql::new(ProbeExecutor);
        let exec = db.executor();
        assert!(exec.exec("select 1", "[]").is_ok());
        assert!(exec.query_raw("select 1", "[]").is_ok());
        assert!(exec.begin().is_ok());
        assert!(exec.commit().is_ok());
        assert!(exec.rollback().is_ok());
        assert_eq!(
            ReplicaSql::<ProbeExecutor>::coverage_branch_probe(true),
            "enabled"
        );
        assert_eq!(
            ReplicaSql::<ProbeExecutor>::coverage_branch_probe(false),
            "disabled"
        );
    }
}
