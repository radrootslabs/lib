use radroots_replica_db::ReplicaSql;
use radroots_replica_db_schema::farm::{
    IFarmCreate, IFarmDelete, IFarmFindMany, IFarmFindOne, IFarmUpdate,
};
use radroots_replica_db_schema::farm_gcs_location::{
    IFarmGcsLocationCreate, IFarmGcsLocationDelete, IFarmGcsLocationFindMany,
    IFarmGcsLocationFindOne, IFarmGcsLocationUpdate,
};
use radroots_replica_db_schema::farm_member::{
    IFarmMemberCreate, IFarmMemberDelete, IFarmMemberFindMany, IFarmMemberFindOne,
    IFarmMemberUpdate,
};
use radroots_replica_db_schema::farm_member_claim::{
    IFarmMemberClaimCreate, IFarmMemberClaimDelete, IFarmMemberClaimFindMany,
    IFarmMemberClaimFindOne, IFarmMemberClaimUpdate,
};
use radroots_replica_db_schema::farm_tag::{
    IFarmTagCreate, IFarmTagDelete, IFarmTagFindMany, IFarmTagFindOne, IFarmTagUpdate,
};
use radroots_replica_db_schema::gcs_location::{
    IGcsLocationCreate, IGcsLocationDelete, IGcsLocationFindMany, IGcsLocationFindOne,
    IGcsLocationUpdate,
};
use radroots_replica_db_schema::log_error::{
    ILogErrorCreate, ILogErrorDelete, ILogErrorFindMany, ILogErrorFindOne, ILogErrorUpdate,
};
use radroots_replica_db_schema::media_image::{
    IMediaImageCreate, IMediaImageDelete, IMediaImageFindMany, IMediaImageFindOne,
    IMediaImageUpdate,
};
use radroots_replica_db_schema::nostr_event_state::{
    INostrEventStateCreate, INostrEventStateDelete, INostrEventStateFindMany,
    INostrEventStateFindOne, INostrEventStateUpdate,
};
use radroots_replica_db_schema::nostr_profile_relay::INostrProfileRelayRelation;
use radroots_replica_db_schema::nostr_profile::{
    INostrProfileCreate, INostrProfileDelete, INostrProfileFindMany, INostrProfileFindOne,
    INostrProfileUpdate,
};
use radroots_replica_db_schema::nostr_relay::{
    INostrRelayCreate, INostrRelayDelete, INostrRelayFindMany, INostrRelayFindOne,
    INostrRelayUpdate,
};
use radroots_replica_db_schema::plot::{
    IPlotCreate, IPlotDelete, IPlotFindMany, IPlotFindOne, IPlotUpdate,
};
use radroots_replica_db_schema::plot_gcs_location::{
    IPlotGcsLocationCreate, IPlotGcsLocationDelete, IPlotGcsLocationFindMany,
    IPlotGcsLocationFindOne, IPlotGcsLocationUpdate,
};
use radroots_replica_db_schema::plot_tag::{
    IPlotTagCreate, IPlotTagDelete, IPlotTagFindMany, IPlotTagFindOne, IPlotTagUpdate,
};
use radroots_replica_db_schema::trade_product::{
    ITradeProductCreate, ITradeProductDelete, ITradeProductFindMany, ITradeProductFindOne,
    ITradeProductUpdate,
};
use radroots_replica_db_schema::trade_product_location::ITradeProductLocationRelation;
use radroots_replica_db_schema::trade_product_media::ITradeProductMediaRelation;
use radroots_sql_core::{SqlError, SqlExecutor, SqliteExecutor};
use radroots_types::types::IError;
use serde::de::DeserializeOwned;
use serde_json::json;

fn parse_json<T: DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("valid test payload")
}

fn hex64(ch: char) -> String {
    std::iter::repeat_n(ch, 64).collect()
}

fn open_db() -> ReplicaSql<SqliteExecutor> {
    let exec = SqliteExecutor::open_memory().expect("open sqlite memory");
    let db = ReplicaSql::new(exec);
    db.migrate_up().expect("migrate up");
    db
}

fn drop_table(db: &ReplicaSql<SqliteExecutor>, table_name: &str) {
    let sql = format!("DROP TABLE {table_name};");
    db.executor().exec(&sql, "[]").expect("drop table");
}

fn assert_invalid_query<T>(result: Result<T, IError<SqlError>>) {
    let err = match result {
        Ok(_) => panic!("invalid query expected"),
        Err(err) => err,
    };
    assert!(matches!(err.err, SqlError::InvalidQuery(_)));
}

fn assert_not_found<T>(result: Result<T, IError<SqlError>>) {
    let err = match result {
        Ok(_) => panic!("not found expected"),
        Err(err) => err,
    };
    assert!(matches!(err.err, SqlError::NotFound(_)));
}

#[test]
fn gcs_location_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IGcsLocationUpdate = parse_json(json!({
        "on": { "d_tag": "missing-gcs" },
        "fields": { "label": "x" }
    }));
    assert_not_found(db.gcs_location_update(&update_missing));

    let delete_missing_on: IGcsLocationDelete = parse_json(json!({
        "on": { "d_tag": "missing-gcs" }
    }));
    assert_not_found(db.gcs_location_delete(&delete_missing_on));

    let delete_missing_rel: IGcsLocationDelete = parse_json(json!({
        "rel": { "off_plot": { "id": "missing-plot" } }
    }));
    assert_not_found(db.gcs_location_delete(&delete_missing_rel));

    drop_table(&db, "gcs_location");

    let create_opts: IGcsLocationCreate = parse_json(json!({
        "d_tag": "gcs-a",
        "lat": 59.33,
        "lng": 18.06,
        "geohash": "u6sce4f",
        "point": "POINT(18.06 59.33)",
        "polygon": "POLYGON((18.06 59.33,18.07 59.33,18.07 59.34,18.06 59.34,18.06 59.33))"
    }));
    assert_invalid_query(db.gcs_location_create(&create_opts));

    let find_many_filter: IGcsLocationFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.gcs_location_find_many(&find_many_filter));

    let find_many_rel: IGcsLocationFindMany = parse_json(json!({
        "rel": { "on_farm": { "id": "farm-1" } }
    }));
    assert_invalid_query(db.gcs_location_find_many(&find_many_rel));

    let find_one_on: IGcsLocationFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.gcs_location_find_one(&find_one_on));

    let find_one_rel: IGcsLocationFindOne = parse_json(json!({
        "rel": { "off_trade_product": { "id": "tp-1" } }
    }));
    assert_invalid_query(db.gcs_location_find_one(&find_one_rel));

    let update_id: IGcsLocationUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "label": "x" }
    }));
    assert_invalid_query(db.gcs_location_update(&update_id));

    let delete_id: IGcsLocationDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.gcs_location_delete(&delete_id));

    let delete_rel: IGcsLocationDelete = parse_json(json!({
        "rel": { "on_plot": { "id": "plot-1" } }
    }));
    assert_invalid_query(db.gcs_location_delete(&delete_rel));
}

#[test]
fn media_image_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IMediaImageUpdate = parse_json(json!({
        "on": { "file_path": "/missing.jpg" },
        "fields": { "label": "x" }
    }));
    assert_not_found(db.media_image_update(&update_missing));

    let delete_missing_on: IMediaImageDelete = parse_json(json!({
        "on": { "file_path": "/missing.jpg" }
    }));
    assert_not_found(db.media_image_delete(&delete_missing_on));

    let delete_missing_rel: IMediaImageDelete = parse_json(json!({
        "rel": { "off_trade_product": { "id": "missing-tp" } }
    }));
    assert_not_found(db.media_image_delete(&delete_missing_rel));

    drop_table(&db, "media_image");

    let create_opts: IMediaImageCreate = parse_json(json!({
        "file_path": "/img/a.jpg",
        "mime_type": "image/jpeg",
        "res_base": "https://cdn.example.com",
        "res_path": "img/a.jpg"
    }));
    assert_invalid_query(db.media_image_create(&create_opts));

    let find_many_filter: IMediaImageFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.media_image_find_many(&find_many_filter));

    let find_many_rel: IMediaImageFindMany = parse_json(json!({
        "rel": { "on_trade_product": { "id": "tp-1" } }
    }));
    assert_invalid_query(db.media_image_find_many(&find_many_rel));

    let find_one_on: IMediaImageFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.media_image_find_one(&find_one_on));

    let find_one_rel: IMediaImageFindOne = parse_json(json!({
        "rel": { "off_trade_product": { "id": "tp-1" } }
    }));
    assert_invalid_query(db.media_image_find_one(&find_one_rel));

    let update_id: IMediaImageUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "label": "x" }
    }));
    assert_invalid_query(db.media_image_update(&update_id));

    let delete_id: IMediaImageDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.media_image_delete(&delete_id));

    let delete_rel: IMediaImageDelete = parse_json(json!({
        "rel": { "on_trade_product": { "id": "tp-1" } }
    }));
    assert_invalid_query(db.media_image_delete(&delete_rel));
}

#[test]
fn nostr_profile_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: INostrProfileUpdate = parse_json(json!({
        "on": { "public_key": hex64('a') },
        "fields": { "name": "x" }
    }));
    assert_not_found(db.nostr_profile_update(&update_missing));

    let delete_missing_on: INostrProfileDelete = parse_json(json!({
        "on": { "public_key": hex64('a') }
    }));
    assert_not_found(db.nostr_profile_delete(&delete_missing_on));

    let delete_missing_rel: INostrProfileDelete = parse_json(json!({
        "rel": { "off_relay": { "id": "missing-relay" } }
    }));
    assert_not_found(db.nostr_profile_delete(&delete_missing_rel));

    drop_table(&db, "nostr_profile");

    let create_opts: INostrProfileCreate = parse_json(json!({
        "public_key": hex64('d'),
        "profile_type": "farm",
        "name": "profile a"
    }));
    assert_invalid_query(db.nostr_profile_create(&create_opts));

    let find_many_filter: INostrProfileFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.nostr_profile_find_many(&find_many_filter));

    let find_many_rel: INostrProfileFindMany = parse_json(json!({
        "rel": { "on_relay": { "id": "relay-1" } }
    }));
    assert_invalid_query(db.nostr_profile_find_many(&find_many_rel));

    let find_one_on: INostrProfileFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.nostr_profile_find_one(&find_one_on));

    let find_one_rel: INostrProfileFindOne = parse_json(json!({
        "rel": { "off_relay": { "id": "relay-1" } }
    }));
    assert_invalid_query(db.nostr_profile_find_one(&find_one_rel));

    let update_id: INostrProfileUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "name": "x" }
    }));
    assert_invalid_query(db.nostr_profile_update(&update_id));

    let delete_id: INostrProfileDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.nostr_profile_delete(&delete_id));

    let delete_rel: INostrProfileDelete = parse_json(json!({
        "rel": { "on_relay": { "id": "relay-1" } }
    }));
    assert_invalid_query(db.nostr_profile_delete(&delete_rel));
}

#[test]
fn nostr_relay_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: INostrRelayUpdate = parse_json(json!({
        "on": { "url": "wss://missing.example.com" },
        "fields": { "name": "x" }
    }));
    assert_not_found(db.nostr_relay_update(&update_missing));

    let delete_missing_on: INostrRelayDelete = parse_json(json!({
        "on": { "url": "wss://missing.example.com" }
    }));
    assert_not_found(db.nostr_relay_delete(&delete_missing_on));

    let delete_missing_rel: INostrRelayDelete = parse_json(json!({
        "rel": { "off_profile": { "public_key": hex64('b') } }
    }));
    assert_not_found(db.nostr_relay_delete(&delete_missing_rel));

    drop_table(&db, "nostr_relay");

    let create_opts: INostrRelayCreate = parse_json(json!({
        "url": "wss://relay.example.com"
    }));
    assert_invalid_query(db.nostr_relay_create(&create_opts));

    let find_many_filter: INostrRelayFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.nostr_relay_find_many(&find_many_filter));

    let find_many_rel: INostrRelayFindMany = parse_json(json!({
        "rel": { "on_profile": { "public_key": hex64('d') } }
    }));
    assert_invalid_query(db.nostr_relay_find_many(&find_many_rel));

    let find_one_on: INostrRelayFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.nostr_relay_find_one(&find_one_on));

    let find_one_rel: INostrRelayFindOne = parse_json(json!({
        "rel": { "off_profile": { "public_key": hex64('d') } }
    }));
    assert_invalid_query(db.nostr_relay_find_one(&find_one_rel));

    let update_id: INostrRelayUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "name": "x" }
    }));
    assert_invalid_query(db.nostr_relay_update(&update_id));

    let delete_id: INostrRelayDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.nostr_relay_delete(&delete_id));

    let delete_rel: INostrRelayDelete = parse_json(json!({
        "rel": { "on_profile": { "public_key": hex64('d') } }
    }));
    assert_invalid_query(db.nostr_relay_delete(&delete_rel));
}

#[test]
fn farm_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IFarmUpdate = parse_json(json!({
        "on": { "d_tag": "missing-farm" },
        "fields": { "name": "farm x" }
    }));
    assert_not_found(db.farm_update(&update_missing));

    let update_missing_id: IFarmUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "name": "farm y" }
    }));
    assert_not_found(db.farm_update(&update_missing_id));

    let delete_missing_on: IFarmDelete = parse_json(json!({
        "on": { "d_tag": "missing-farm" }
    }));
    assert_not_found(db.farm_delete(&delete_missing_on));

    drop_table(&db, "farm");

    let create_opts: IFarmCreate = parse_json(json!({
        "d_tag": "farm-a",
        "pubkey": hex64('a'),
        "name": "farm a"
    }));
    assert_invalid_query(db.farm_create(&create_opts));

    let find_many_filter: IFarmFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_find_many(&find_many_filter));

    let find_one_on: IFarmFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_find_one(&find_one_on));

    let update_id: IFarmUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "name": "farm z" }
    }));
    assert_invalid_query(db.farm_update(&update_id));

    let delete_id: IFarmDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_delete(&delete_id));
}

#[test]
fn plot_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IPlotUpdate = parse_json(json!({
        "on": { "d_tag": "missing-plot" },
        "fields": { "name": "plot x" }
    }));
    assert_not_found(db.plot_update(&update_missing));

    let update_missing_id: IPlotUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "name": "plot y" }
    }));
    assert_not_found(db.plot_update(&update_missing_id));

    let delete_missing_on: IPlotDelete = parse_json(json!({
        "on": { "d_tag": "missing-plot" }
    }));
    assert_not_found(db.plot_delete(&delete_missing_on));

    drop_table(&db, "plot");

    let create_opts: IPlotCreate = parse_json(json!({
        "d_tag": "plot-a",
        "farm_id": "farm-1",
        "name": "plot a"
    }));
    assert_invalid_query(db.plot_create(&create_opts));

    let find_many_filter: IPlotFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.plot_find_many(&find_many_filter));

    let find_one_on: IPlotFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.plot_find_one(&find_one_on));

    let update_id: IPlotUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "name": "plot z" }
    }));
    assert_invalid_query(db.plot_update(&update_id));

    let delete_id: IPlotDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.plot_delete(&delete_id));
}

#[test]
fn farm_gcs_location_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IFarmGcsLocationUpdate = parse_json(json!({
        "on": { "farm_id": "farm-1" },
        "fields": { "role": "x" }
    }));
    assert_not_found(db.farm_gcs_location_update(&update_missing));

    let update_missing_id: IFarmGcsLocationUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "role": "y" }
    }));
    assert_not_found(db.farm_gcs_location_update(&update_missing_id));

    let delete_missing_on: IFarmGcsLocationDelete = parse_json(json!({
        "on": { "farm_id": "farm-1" }
    }));
    assert_not_found(db.farm_gcs_location_delete(&delete_missing_on));

    drop_table(&db, "farm_gcs_location");

    let create_opts: IFarmGcsLocationCreate = parse_json(json!({
        "farm_id": "farm-1",
        "gcs_location_id": "gcs-1",
        "role": "primary"
    }));
    assert_invalid_query(db.farm_gcs_location_create(&create_opts));

    let find_many_filter: IFarmGcsLocationFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_gcs_location_find_many(&find_many_filter));

    let find_one_on: IFarmGcsLocationFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_gcs_location_find_one(&find_one_on));

    let update_id: IFarmGcsLocationUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "role": "z" }
    }));
    assert_invalid_query(db.farm_gcs_location_update(&update_id));

    let delete_id: IFarmGcsLocationDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_gcs_location_delete(&delete_id));
}

#[test]
fn plot_gcs_location_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IPlotGcsLocationUpdate = parse_json(json!({
        "on": { "plot_id": "plot-1" },
        "fields": { "role": "x" }
    }));
    assert_not_found(db.plot_gcs_location_update(&update_missing));

    let update_missing_id: IPlotGcsLocationUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "role": "y" }
    }));
    assert_not_found(db.plot_gcs_location_update(&update_missing_id));

    let delete_missing_on: IPlotGcsLocationDelete = parse_json(json!({
        "on": { "plot_id": "plot-1" }
    }));
    assert_not_found(db.plot_gcs_location_delete(&delete_missing_on));

    drop_table(&db, "plot_gcs_location");

    let create_opts: IPlotGcsLocationCreate = parse_json(json!({
        "plot_id": "plot-1",
        "gcs_location_id": "gcs-1",
        "role": "primary"
    }));
    assert_invalid_query(db.plot_gcs_location_create(&create_opts));

    let find_many_filter: IPlotGcsLocationFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.plot_gcs_location_find_many(&find_many_filter));

    let find_one_on: IPlotGcsLocationFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.plot_gcs_location_find_one(&find_one_on));

    let update_id: IPlotGcsLocationUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "role": "z" }
    }));
    assert_invalid_query(db.plot_gcs_location_update(&update_id));

    let delete_id: IPlotGcsLocationDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.plot_gcs_location_delete(&delete_id));
}

#[test]
fn farm_tag_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IFarmTagUpdate = parse_json(json!({
        "on": { "farm_id": "farm-1" },
        "fields": { "tag": "x" }
    }));
    assert_not_found(db.farm_tag_update(&update_missing));

    let update_missing_id: IFarmTagUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "tag": "y" }
    }));
    assert_not_found(db.farm_tag_update(&update_missing_id));

    let delete_missing_on: IFarmTagDelete = parse_json(json!({
        "on": { "farm_id": "farm-1" }
    }));
    assert_not_found(db.farm_tag_delete(&delete_missing_on));

    drop_table(&db, "farm_tag");

    let create_opts: IFarmTagCreate = parse_json(json!({
        "farm_id": "farm-1",
        "tag": "organic"
    }));
    assert_invalid_query(db.farm_tag_create(&create_opts));

    let find_many_filter: IFarmTagFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_tag_find_many(&find_many_filter));

    let find_one_on: IFarmTagFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_tag_find_one(&find_one_on));

    let update_id: IFarmTagUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "tag": "z" }
    }));
    assert_invalid_query(db.farm_tag_update(&update_id));

    let delete_id: IFarmTagDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_tag_delete(&delete_id));
}

#[test]
fn plot_tag_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IPlotTagUpdate = parse_json(json!({
        "on": { "plot_id": "plot-1" },
        "fields": { "tag": "x" }
    }));
    assert_not_found(db.plot_tag_update(&update_missing));

    let update_missing_id: IPlotTagUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "tag": "y" }
    }));
    assert_not_found(db.plot_tag_update(&update_missing_id));

    let delete_missing_on: IPlotTagDelete = parse_json(json!({
        "on": { "plot_id": "plot-1" }
    }));
    assert_not_found(db.plot_tag_delete(&delete_missing_on));

    drop_table(&db, "plot_tag");

    let create_opts: IPlotTagCreate = parse_json(json!({
        "plot_id": "plot-1",
        "tag": "north"
    }));
    assert_invalid_query(db.plot_tag_create(&create_opts));

    let find_many_filter: IPlotTagFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.plot_tag_find_many(&find_many_filter));

    let find_one_on: IPlotTagFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.plot_tag_find_one(&find_one_on));

    let update_id: IPlotTagUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "tag": "z" }
    }));
    assert_invalid_query(db.plot_tag_update(&update_id));

    let delete_id: IPlotTagDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.plot_tag_delete(&delete_id));
}

#[test]
fn farm_member_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IFarmMemberUpdate = parse_json(json!({
        "on": { "member_pubkey": hex64('a') },
        "fields": { "role": "x" }
    }));
    assert_not_found(db.farm_member_update(&update_missing));

    let update_missing_id: IFarmMemberUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "role": "y" }
    }));
    assert_not_found(db.farm_member_update(&update_missing_id));

    let delete_missing_on: IFarmMemberDelete = parse_json(json!({
        "on": { "member_pubkey": hex64('a') }
    }));
    assert_not_found(db.farm_member_delete(&delete_missing_on));

    drop_table(&db, "farm_member");

    let create_opts: IFarmMemberCreate = parse_json(json!({
        "farm_id": "farm-1",
        "member_pubkey": hex64('a'),
        "role": "owner"
    }));
    assert_invalid_query(db.farm_member_create(&create_opts));

    let find_many_filter: IFarmMemberFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_member_find_many(&find_many_filter));

    let find_one_on: IFarmMemberFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_member_find_one(&find_one_on));

    let update_id: IFarmMemberUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "role": "z" }
    }));
    assert_invalid_query(db.farm_member_update(&update_id));

    let delete_id: IFarmMemberDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_member_delete(&delete_id));
}

#[test]
fn farm_member_claim_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: IFarmMemberClaimUpdate = parse_json(json!({
        "on": { "member_pubkey": hex64('a') },
        "fields": { "farm_pubkey": hex64('b') }
    }));
    assert_not_found(db.farm_member_claim_update(&update_missing));

    let update_missing_id: IFarmMemberClaimUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "farm_pubkey": hex64('c') }
    }));
    assert_not_found(db.farm_member_claim_update(&update_missing_id));

    let delete_missing_on: IFarmMemberClaimDelete = parse_json(json!({
        "on": { "member_pubkey": hex64('a') }
    }));
    assert_not_found(db.farm_member_claim_delete(&delete_missing_on));

    drop_table(&db, "farm_member_claim");

    let create_opts: IFarmMemberClaimCreate = parse_json(json!({
        "member_pubkey": hex64('a'),
        "farm_pubkey": hex64('b')
    }));
    assert_invalid_query(db.farm_member_claim_create(&create_opts));

    let find_many_filter: IFarmMemberClaimFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_member_claim_find_many(&find_many_filter));

    let find_one_on: IFarmMemberClaimFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_member_claim_find_one(&find_one_on));

    let update_id: IFarmMemberClaimUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "farm_pubkey": hex64('c') }
    }));
    assert_invalid_query(db.farm_member_claim_update(&update_id));

    let delete_id: IFarmMemberClaimDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.farm_member_claim_delete(&delete_id));
}

#[test]
fn log_error_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: ILogErrorUpdate = parse_json(json!({
        "on": { "nostr_pubkey": hex64('a') },
        "fields": { "message": "x" }
    }));
    assert_not_found(db.log_error_update(&update_missing));

    let update_missing_id: ILogErrorUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "message": "y" }
    }));
    assert_not_found(db.log_error_update(&update_missing_id));

    let delete_missing_on: ILogErrorDelete = parse_json(json!({
        "on": { "nostr_pubkey": hex64('a') }
    }));
    assert_not_found(db.log_error_delete(&delete_missing_on));

    drop_table(&db, "log_error");

    let create_opts: ILogErrorCreate = parse_json(json!({
        "error": "panic",
        "message": "boom",
        "app_system": "studio",
        "app_version": "1.0.0",
        "nostr_pubkey": hex64('a')
    }));
    assert_invalid_query(db.log_error_create(&create_opts));

    let find_many_filter: ILogErrorFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.log_error_find_many(&find_many_filter));

    let find_one_on: ILogErrorFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.log_error_find_one(&find_one_on));

    let update_id: ILogErrorUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "message": "z" }
    }));
    assert_invalid_query(db.log_error_update(&update_id));

    let delete_id: ILogErrorDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.log_error_delete(&delete_id));
}

#[test]
fn nostr_event_state_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: INostrEventStateUpdate = parse_json(json!({
        "on": { "key": "state-a" },
        "fields": { "content_hash": "hash-x" }
    }));
    assert_not_found(db.nostr_event_state_update(&update_missing));

    let update_missing_id: INostrEventStateUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "content_hash": "hash-y" }
    }));
    assert_not_found(db.nostr_event_state_update(&update_missing_id));

    let delete_missing_on: INostrEventStateDelete = parse_json(json!({
        "on": { "key": "state-a" }
    }));
    assert_not_found(db.nostr_event_state_delete(&delete_missing_on));

    drop_table(&db, "nostr_event_state");

    let create_opts: INostrEventStateCreate = parse_json(json!({
        "key": "state-a",
        "kind": 30023,
        "pubkey": hex64('a'),
        "d_tag": "listing-a",
        "last_event_id": hex64('b'),
        "last_created_at": 1,
        "content_hash": "hash-a"
    }));
    assert_invalid_query(db.nostr_event_state_create(&create_opts));

    let find_many_filter: INostrEventStateFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.nostr_event_state_find_many(&find_many_filter));

    let find_one_on: INostrEventStateFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.nostr_event_state_find_one(&find_one_on));

    let update_id: INostrEventStateUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "content_hash": "hash-z" }
    }));
    assert_invalid_query(db.nostr_event_state_update(&update_id));

    let delete_id: INostrEventStateDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.nostr_event_state_delete(&delete_id));
}

#[test]
fn trade_product_error_paths_cover_regions() {
    let db = open_db();

    let update_missing: ITradeProductUpdate = parse_json(json!({
        "on": { "id": "missing-id" },
        "fields": { "title": "x" }
    }));
    assert_not_found(db.trade_product_update(&update_missing));

    let delete_missing_on: ITradeProductDelete = parse_json(json!({
        "on": { "id": "missing-id" }
    }));
    assert_not_found(db.trade_product_delete(&delete_missing_on));

    drop_table(&db, "trade_product");

    let create_opts: ITradeProductCreate = parse_json(json!({
        "key": "product-a",
        "category": "coffee",
        "title": "coffee a",
        "summary": "summary",
        "process": "washed",
        "lot": "lot-a",
        "profile": "floral",
        "year": 2024,
        "qty_amt": 100,
        "qty_unit": "kg",
        "price_amt": 7.5,
        "price_currency": "USD",
        "price_qty_amt": 1,
        "price_qty_unit": "kg"
    }));
    assert_invalid_query(db.trade_product_create(&create_opts));

    let find_many_filter: ITradeProductFindMany = parse_json(json!({
        "filter": { "id": "id-1" }
    }));
    assert_invalid_query(db.trade_product_find_many(&find_many_filter));

    let find_one_on: ITradeProductFindOne = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.trade_product_find_one(&find_one_on));

    let update_id: ITradeProductUpdate = parse_json(json!({
        "on": { "id": "id-1" },
        "fields": { "title": "z" }
    }));
    assert_invalid_query(db.trade_product_update(&update_id));

    let delete_id: ITradeProductDelete = parse_json(json!({
        "on": { "id": "id-1" }
    }));
    assert_invalid_query(db.trade_product_delete(&delete_id));
}

#[test]
fn relation_set_unset_error_paths_cover_regions() {
    let db = open_db();

    drop_table(&db, "nostr_profile_relay");
    let profile_relay_rel: INostrProfileRelayRelation = parse_json(json!({
        "nostr_profile": { "id": "profile-1" },
        "nostr_relay": { "id": "relay-1" }
    }));
    assert_invalid_query(db.nostr_profile_relay_set(&profile_relay_rel));
    assert_invalid_query(db.nostr_profile_relay_unset(&profile_relay_rel));

    drop_table(&db, "trade_product_location");
    let product_location_rel: ITradeProductLocationRelation = parse_json(json!({
        "trade_product": { "id": "product-1" },
        "gcs_location": { "id": "gcs-1" }
    }));
    assert_invalid_query(db.trade_product_location_set(&product_location_rel));
    assert_invalid_query(db.trade_product_location_unset(&product_location_rel));

    drop_table(&db, "trade_product_media");
    let product_media_rel: ITradeProductMediaRelation = parse_json(json!({
        "trade_product": { "id": "product-1" },
        "media_image": { "id": "media-1" }
    }));
    assert_invalid_query(db.trade_product_media_set(&product_media_rel));
    assert_invalid_query(db.trade_product_media_unset(&product_media_rel));
}
