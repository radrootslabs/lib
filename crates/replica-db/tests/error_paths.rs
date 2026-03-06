use radroots_replica_db::ReplicaSql;
use radroots_replica_db_schema::gcs_location::{
    IGcsLocationCreate, IGcsLocationDelete, IGcsLocationFindMany, IGcsLocationFindOne,
    IGcsLocationUpdate,
};
use radroots_replica_db_schema::media_image::{
    IMediaImageCreate, IMediaImageDelete, IMediaImageFindMany, IMediaImageFindOne,
    IMediaImageUpdate,
};
use radroots_replica_db_schema::nostr_profile::{
    INostrProfileCreate, INostrProfileDelete, INostrProfileFindMany, INostrProfileFindOne,
    INostrProfileUpdate,
};
use radroots_replica_db_schema::nostr_relay::{
    INostrRelayCreate, INostrRelayDelete, INostrRelayFindMany, INostrRelayFindOne,
    INostrRelayUpdate,
};
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
