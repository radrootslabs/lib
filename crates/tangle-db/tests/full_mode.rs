use radroots_sql_core::{SqlError, SqliteExecutor};
use radroots_tangle_db::{TangleSql, export_manifest};
use radroots_tangle_db_schema::farm::{
    IFarmCreate, IFarmDelete, IFarmFindMany, IFarmFindOne, IFarmUpdate,
};
use radroots_tangle_db_schema::farm_gcs_location::{
    IFarmGcsLocationCreate, IFarmGcsLocationDelete, IFarmGcsLocationFindMany,
    IFarmGcsLocationFindOne, IFarmGcsLocationUpdate,
};
use radroots_tangle_db_schema::farm_member::{
    IFarmMemberCreate, IFarmMemberDelete, IFarmMemberFindMany, IFarmMemberFindOne,
    IFarmMemberUpdate,
};
use radroots_tangle_db_schema::farm_member_claim::{
    IFarmMemberClaimCreate, IFarmMemberClaimDelete, IFarmMemberClaimFindMany,
    IFarmMemberClaimFindOne, IFarmMemberClaimUpdate,
};
use radroots_tangle_db_schema::farm_tag::{
    IFarmTagCreate, IFarmTagDelete, IFarmTagFindMany, IFarmTagFindOne, IFarmTagUpdate,
};
use radroots_tangle_db_schema::gcs_location::{
    GcsLocationFarmArgs, GcsLocationFindManyRel, GcsLocationPlotArgs, GcsLocationTradeProductArgs,
    IGcsLocationCreate, IGcsLocationDelete, IGcsLocationFindMany, IGcsLocationFindOne,
    IGcsLocationUpdate,
};
use radroots_tangle_db_schema::log_error::{
    ILogErrorCreate, ILogErrorDelete, ILogErrorFindMany, ILogErrorFindOne, ILogErrorUpdate,
};
use radroots_tangle_db_schema::media_image::{
    IMediaImageCreate, IMediaImageDelete, IMediaImageFindMany, IMediaImageFindOne,
    IMediaImageUpdate, MediaImageFindManyRel, MediaImageTradeProductArgs,
};
use radroots_tangle_db_schema::nostr_event_state::{
    INostrEventStateCreate, INostrEventStateDelete, INostrEventStateFindMany,
    INostrEventStateFindOne, INostrEventStateUpdate,
};
use radroots_tangle_db_schema::nostr_profile::{
    INostrProfileCreate, INostrProfileDelete, INostrProfileFindMany, INostrProfileFindOne,
    INostrProfileUpdate, NostrProfileFindManyRel, NostrProfileRelayArgs,
};
use radroots_tangle_db_schema::nostr_profile_relay::INostrProfileRelayRelation;
use radroots_tangle_db_schema::nostr_relay::{
    INostrRelayCreate, INostrRelayDelete, INostrRelayFindMany, INostrRelayFindOne,
    INostrRelayUpdate, NostrRelayFindManyRel, NostrRelayProfileArgs,
};
use radroots_tangle_db_schema::plot::{
    IPlotCreate, IPlotDelete, IPlotFindMany, IPlotFindOne, IPlotUpdate,
};
use radroots_tangle_db_schema::plot_gcs_location::{
    IPlotGcsLocationCreate, IPlotGcsLocationDelete, IPlotGcsLocationFindMany,
    IPlotGcsLocationFindOne, IPlotGcsLocationUpdate,
};
use radroots_tangle_db_schema::plot_tag::{
    IPlotTagCreate, IPlotTagDelete, IPlotTagFindMany, IPlotTagFindOne, IPlotTagUpdate,
};
use radroots_tangle_db_schema::trade_product::{
    ITradeProductCreate, ITradeProductDelete, ITradeProductFindMany, ITradeProductFindOne,
    ITradeProductUpdate,
};
use radroots_tangle_db_schema::trade_product_location::ITradeProductLocationRelation;
use radroots_tangle_db_schema::trade_product_media::ITradeProductMediaRelation;
use radroots_types::types::IError;
use serde::de::DeserializeOwned;
use serde_json::json;

fn parse_json<T: DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("valid test payload")
}

fn hex64(ch: char) -> String {
    std::iter::repeat_n(ch, 64).collect()
}

fn assert_invalid_argument<T>(result: Result<T, IError<SqlError>>) {
    let err = match result {
        Ok(_) => panic!("invalid argument expected"),
        Err(err) => err,
    };
    assert!(matches!(err.err, SqlError::InvalidArgument(_)));
}

fn assert_not_found<T>(result: Result<T, IError<SqlError>>) {
    let err = match result {
        Ok(_) => panic!("not found expected"),
        Err(err) => err,
    };
    assert!(matches!(err.err, SqlError::NotFound(_)));
}

fn open_db() -> TangleSql<SqliteExecutor> {
    let exec = SqliteExecutor::open_memory().expect("open sqlite memory");
    let db = TangleSql::new(exec);
    db.migrate_up().expect("migrate up");
    db
}

#[test]
fn full_mode_crud_and_relation_paths() {
    let db = open_db();

    db.migrate_down().expect("migrate down");
    db.migrate_up().expect("migrate up again");

    let farm: IFarmCreate = parse_json(json!({
        "d_tag": "farm-a",
        "pubkey": hex64('a'),
        "name": "farm a"
    }));
    let farm_created = db.farm_create(&farm).expect("farm create").result;

    let gcs_location: IGcsLocationCreate = parse_json(json!({
        "d_tag": "gcs-a",
        "lat": 59.33,
        "lng": 18.06,
        "geohash": "u6sce4f",
        "point": "POINT(18.06 59.33)",
        "polygon": "POLYGON((18.06 59.33,18.07 59.33,18.07 59.34,18.06 59.34,18.06 59.33))"
    }));
    let gcs_created = db
        .gcs_location_create(&gcs_location)
        .expect("gcs create")
        .result;

    let plot: IPlotCreate = parse_json(json!({
        "d_tag": "plot-a",
        "farm_id": farm_created.id,
        "name": "plot a"
    }));
    let plot_created = db.plot_create(&plot).expect("plot create").result;

    let farm_gcs: IFarmGcsLocationCreate = parse_json(json!({
        "farm_id": farm_created.id,
        "gcs_location_id": gcs_created.id,
        "role": "primary"
    }));
    let farm_gcs_created = db
        .farm_gcs_location_create(&farm_gcs)
        .expect("farm gcs create")
        .result;

    let plot_gcs: IPlotGcsLocationCreate = parse_json(json!({
        "plot_id": plot_created.id,
        "gcs_location_id": gcs_created.id,
        "role": "primary"
    }));
    let plot_gcs_created = db
        .plot_gcs_location_create(&plot_gcs)
        .expect("plot gcs create")
        .result;

    let farm_tag: IFarmTagCreate = parse_json(json!({
        "farm_id": farm_created.id,
        "tag": "organic"
    }));
    let farm_tag_created = db
        .farm_tag_create(&farm_tag)
        .expect("farm tag create")
        .result;

    let plot_tag: IPlotTagCreate = parse_json(json!({
        "plot_id": plot_created.id,
        "tag": "north"
    }));
    let plot_tag_created = db
        .plot_tag_create(&plot_tag)
        .expect("plot tag create")
        .result;

    let farm_member: IFarmMemberCreate = parse_json(json!({
        "farm_id": farm_created.id,
        "member_pubkey": hex64('b'),
        "role": "owner"
    }));
    let farm_member_created = db
        .farm_member_create(&farm_member)
        .expect("farm member create")
        .result;

    let farm_member_claim: IFarmMemberClaimCreate = parse_json(json!({
        "member_pubkey": hex64('b'),
        "farm_pubkey": hex64('a')
    }));
    let farm_member_claim_created = db
        .farm_member_claim_create(&farm_member_claim)
        .expect("farm member claim create")
        .result;

    let log_error: ILogErrorCreate = parse_json(json!({
        "error": "panic",
        "message": "boom",
        "app_system": "studio",
        "app_version": "1.0.0",
        "nostr_pubkey": hex64('c')
    }));
    let log_error_created = db
        .log_error_create(&log_error)
        .expect("log error create")
        .result;

    let media_image: IMediaImageCreate = parse_json(json!({
        "file_path": "/img/a.jpg",
        "mime_type": "image/jpeg",
        "res_base": "https://cdn.example.com",
        "res_path": "img/a.jpg"
    }));
    let media_image_created = db
        .media_image_create(&media_image)
        .expect("media image create")
        .result;

    let nostr_profile: INostrProfileCreate = parse_json(json!({
        "public_key": hex64('d'),
        "profile_type": "farm",
        "name": "profile a"
    }));
    let nostr_profile_created = db
        .nostr_profile_create(&nostr_profile)
        .expect("nostr profile create")
        .result;

    let nostr_relay: INostrRelayCreate = parse_json(json!({
        "url": "wss://relay.example.com"
    }));
    let nostr_relay_created = db
        .nostr_relay_create(&nostr_relay)
        .expect("nostr relay create")
        .result;

    let nostr_event_state: INostrEventStateCreate = parse_json(json!({
        "key": "state-a",
        "kind": 30023,
        "pubkey": hex64('d'),
        "d_tag": "listing-a",
        "last_event_id": hex64('e'),
        "last_created_at": 1,
        "content_hash": "hash-a"
    }));
    let nostr_event_state_created = db
        .nostr_event_state_create(&nostr_event_state)
        .expect("nostr event state create")
        .result;

    let trade_product: ITradeProductCreate = parse_json(json!({
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
    let trade_product_created = db
        .trade_product_create(&trade_product)
        .expect("trade product create")
        .result;

    let gcs_extra: IGcsLocationCreate = parse_json(json!({
        "d_tag": "gcs-b",
        "lat": 59.34,
        "lng": 18.07,
        "geohash": "u6sce4g",
        "point": "POINT(18.07 59.34)",
        "polygon": "POLYGON((18.07 59.34,18.08 59.34,18.08 59.35,18.07 59.35,18.07 59.34))"
    }));
    let _gcs_extra_created = db
        .gcs_location_create(&gcs_extra)
        .expect("gcs extra create")
        .result;

    let media_image_extra: IMediaImageCreate = parse_json(json!({
        "file_path": "/img/b.jpg",
        "mime_type": "image/jpeg",
        "res_base": "https://cdn.example.com",
        "res_path": "img/b.jpg"
    }));
    let _media_image_extra_created = db
        .media_image_create(&media_image_extra)
        .expect("media image extra create")
        .result;

    let nostr_profile_extra: INostrProfileCreate = parse_json(json!({
        "public_key": hex64('f'),
        "profile_type": "farm",
        "name": "profile c"
    }));
    let nostr_profile_extra_created = db
        .nostr_profile_create(&nostr_profile_extra)
        .expect("nostr profile extra create")
        .result;

    let nostr_relay_extra: INostrRelayCreate = parse_json(json!({
        "url": "wss://relay2.example.com"
    }));
    let nostr_relay_extra_created = db
        .nostr_relay_create(&nostr_relay_extra)
        .expect("nostr relay extra create")
        .result;

    let profile_relay_rel: INostrProfileRelayRelation = parse_json(json!({
        "nostr_profile": { "id": nostr_profile_created.id },
        "nostr_relay": { "id": nostr_relay_created.id }
    }));
    db.nostr_profile_relay_set(&profile_relay_rel)
        .expect("profile relay set");

    let profile_relay_rel_extra: INostrProfileRelayRelation = parse_json(json!({
        "nostr_profile": { "id": nostr_profile_extra_created.id },
        "nostr_relay": { "id": nostr_relay_extra_created.id }
    }));
    db.nostr_profile_relay_set(&profile_relay_rel_extra)
        .expect("profile relay extra set");

    let product_location_rel: ITradeProductLocationRelation = parse_json(json!({
        "trade_product": { "id": trade_product_created.id },
        "gcs_location": { "id": gcs_created.id }
    }));
    db.trade_product_location_set(&product_location_rel)
        .expect("product location set");

    let product_media_rel: ITradeProductMediaRelation = parse_json(json!({
        "trade_product": { "id": trade_product_created.id },
        "media_image": { "id": media_image_created.id }
    }));
    db.trade_product_media_set(&product_media_rel)
        .expect("product media set");

    let _: IFarmFindMany = parse_json(json!({ "filter": { "id": farm_created.id } }));
    let farm_find_many: IFarmFindMany = parse_json(json!({ "filter": { "id": farm_created.id } }));
    assert_eq!(
        db.farm_find_many(&farm_find_many)
            .expect("farm find many")
            .results
            .len(),
        1
    );

    let farm_find_one: IFarmFindOne = parse_json(json!({ "on": { "id": farm_created.id } }));
    assert!(
        db.farm_find_one(&farm_find_one)
            .expect("farm find one")
            .result
            .is_some()
    );

    let farm_update_on_alt: IFarmUpdate =
        parse_json(json!({ "on": { "d_tag": "farm-a" }, "fields": { "name": "farm a+" } }));
    assert_eq!(
        db.farm_update(&farm_update_on_alt)
            .expect("farm update alt")
            .result
            .name,
        "farm a+"
    );
    let farm_update_on_id: IFarmUpdate =
        parse_json(json!({ "on": { "id": farm_created.id }, "fields": { "name": "farm a++" } }));
    assert_eq!(
        db.farm_update(&farm_update_on_id)
            .expect("farm update id")
            .result
            .name,
        "farm a++"
    );
    let farm_update_empty: IFarmUpdate =
        parse_json(json!({ "on": { "id": farm_created.id }, "fields": {} }));
    assert_invalid_argument(db.farm_update(&farm_update_empty));

    let plot_find_many: IPlotFindMany = parse_json(json!({ "filter": { "id": plot_created.id } }));
    assert_eq!(
        db.plot_find_many(&plot_find_many)
            .expect("plot find many")
            .results
            .len(),
        1
    );
    let plot_find_one: IPlotFindOne = parse_json(json!({ "on": { "id": plot_created.id } }));
    assert!(
        db.plot_find_one(&plot_find_one)
            .expect("plot find one")
            .result
            .is_some()
    );
    let plot_update_alt: IPlotUpdate =
        parse_json(json!({ "on": { "d_tag": "plot-a" }, "fields": { "name": "plot a+" } }));
    assert_eq!(
        db.plot_update(&plot_update_alt)
            .expect("plot update alt")
            .result
            .name,
        "plot a+"
    );
    let plot_update_id: IPlotUpdate =
        parse_json(json!({ "on": { "id": plot_created.id }, "fields": { "name": "plot a++" } }));
    assert_eq!(
        db.plot_update(&plot_update_id)
            .expect("plot update id")
            .result
            .name,
        "plot a++"
    );
    let plot_update_empty: IPlotUpdate =
        parse_json(json!({ "on": { "id": plot_created.id }, "fields": {} }));
    assert_invalid_argument(db.plot_update(&plot_update_empty));

    for opts in [
        IGcsLocationFindMany::Rel {
            rel: GcsLocationFindManyRel::OnTradeProduct(GcsLocationTradeProductArgs {
                id: trade_product_created.id.clone(),
            }),
        },
        IGcsLocationFindMany::Rel {
            rel: GcsLocationFindManyRel::OffTradeProduct(GcsLocationTradeProductArgs {
                id: trade_product_created.id.clone(),
            }),
        },
        IGcsLocationFindMany::Rel {
            rel: GcsLocationFindManyRel::OnFarm(GcsLocationFarmArgs {
                id: farm_created.id.clone(),
            }),
        },
        IGcsLocationFindMany::Rel {
            rel: GcsLocationFindManyRel::OffFarm(GcsLocationFarmArgs {
                id: farm_created.id.clone(),
            }),
        },
        IGcsLocationFindMany::Rel {
            rel: GcsLocationFindManyRel::OnPlot(GcsLocationPlotArgs {
                id: plot_created.id.clone(),
            }),
        },
        IGcsLocationFindMany::Rel {
            rel: GcsLocationFindManyRel::OffPlot(GcsLocationPlotArgs {
                id: plot_created.id.clone(),
            }),
        },
    ] {
        let _ = db.gcs_location_find_many(&opts).expect("gcs rel find many");
    }
    let gcs_find_many_filter: IGcsLocationFindMany =
        parse_json(json!({ "filter": { "id": gcs_created.id } }));
    assert_eq!(
        db.gcs_location_find_many(&gcs_find_many_filter)
            .expect("gcs find many filter")
            .results
            .len(),
        1
    );
    let gcs_find_one_on: IGcsLocationFindOne =
        parse_json(json!({ "on": { "id": gcs_created.id } }));
    assert!(
        db.gcs_location_find_one(&gcs_find_one_on)
            .expect("gcs find one on")
            .result
            .is_some()
    );
    let gcs_find_one_rel: IGcsLocationFindOne =
        parse_json(json!({ "rel": { "on_farm": { "id": farm_created.id } } }));
    assert!(
        db.gcs_location_find_one(&gcs_find_one_rel)
            .expect("gcs find one rel")
            .result
            .is_some()
    );
    let gcs_update_alt: IGcsLocationUpdate =
        parse_json(json!({ "on": { "d_tag": "gcs-a" }, "fields": { "label": "gcs a+" } }));
    assert_eq!(
        db.gcs_location_update(&gcs_update_alt)
            .expect("gcs update alt")
            .result
            .label
            .as_deref(),
        Some("gcs a+")
    );
    let gcs_update_id: IGcsLocationUpdate =
        parse_json(json!({ "on": { "id": gcs_created.id }, "fields": { "label": "gcs a++" } }));
    assert_eq!(
        db.gcs_location_update(&gcs_update_id)
            .expect("gcs update id")
            .result
            .label
            .as_deref(),
        Some("gcs a++")
    );
    let gcs_update_empty: IGcsLocationUpdate =
        parse_json(json!({ "on": { "id": gcs_created.id }, "fields": {} }));
    assert_invalid_argument(db.gcs_location_update(&gcs_update_empty));

    let farm_gcs_find_many: IFarmGcsLocationFindMany =
        parse_json(json!({ "filter": { "id": farm_gcs_created.id } }));
    assert_eq!(
        db.farm_gcs_location_find_many(&farm_gcs_find_many)
            .expect("farm gcs find many")
            .results
            .len(),
        1
    );
    let farm_gcs_find_one: IFarmGcsLocationFindOne =
        parse_json(json!({ "on": { "id": farm_gcs_created.id } }));
    assert!(
        db.farm_gcs_location_find_one(&farm_gcs_find_one)
            .expect("farm gcs find one")
            .result
            .is_some()
    );
    let farm_gcs_update_alt: IFarmGcsLocationUpdate = parse_json(json!({
        "on": { "farm_id": farm_created.id },
        "fields": { "role": "secondary" }
    }));
    assert_eq!(
        db.farm_gcs_location_update(&farm_gcs_update_alt)
            .expect("farm gcs update")
            .result
            .role,
        "secondary"
    );
    let farm_gcs_update_id: IFarmGcsLocationUpdate = parse_json(
        json!({ "on": { "id": farm_gcs_created.id }, "fields": { "role": "tertiary" } }),
    );
    assert_eq!(
        db.farm_gcs_location_update(&farm_gcs_update_id)
            .expect("farm gcs update id")
            .result
            .role,
        "tertiary"
    );
    let farm_gcs_update_empty: IFarmGcsLocationUpdate =
        parse_json(json!({ "on": { "id": farm_gcs_created.id }, "fields": {} }));
    assert_invalid_argument(db.farm_gcs_location_update(&farm_gcs_update_empty));

    let plot_gcs_find_many: IPlotGcsLocationFindMany =
        parse_json(json!({ "filter": { "id": plot_gcs_created.id } }));
    assert_eq!(
        db.plot_gcs_location_find_many(&plot_gcs_find_many)
            .expect("plot gcs find many")
            .results
            .len(),
        1
    );
    let plot_gcs_find_one: IPlotGcsLocationFindOne =
        parse_json(json!({ "on": { "id": plot_gcs_created.id } }));
    assert!(
        db.plot_gcs_location_find_one(&plot_gcs_find_one)
            .expect("plot gcs find one")
            .result
            .is_some()
    );
    let plot_gcs_update_alt: IPlotGcsLocationUpdate = parse_json(json!({
        "on": { "plot_id": plot_created.id },
        "fields": { "role": "secondary" }
    }));
    assert_eq!(
        db.plot_gcs_location_update(&plot_gcs_update_alt)
            .expect("plot gcs update")
            .result
            .role,
        "secondary"
    );
    let plot_gcs_update_id: IPlotGcsLocationUpdate = parse_json(
        json!({ "on": { "id": plot_gcs_created.id }, "fields": { "role": "tertiary" } }),
    );
    assert_eq!(
        db.plot_gcs_location_update(&plot_gcs_update_id)
            .expect("plot gcs update id")
            .result
            .role,
        "tertiary"
    );
    let plot_gcs_update_empty: IPlotGcsLocationUpdate =
        parse_json(json!({ "on": { "id": plot_gcs_created.id }, "fields": {} }));
    assert_invalid_argument(db.plot_gcs_location_update(&plot_gcs_update_empty));

    let farm_tag_find_many: IFarmTagFindMany =
        parse_json(json!({ "filter": { "id": farm_tag_created.id } }));
    assert_eq!(
        db.farm_tag_find_many(&farm_tag_find_many)
            .expect("farm tag find many")
            .results
            .len(),
        1
    );
    let farm_tag_find_one: IFarmTagFindOne =
        parse_json(json!({ "on": { "id": farm_tag_created.id } }));
    assert!(
        db.farm_tag_find_one(&farm_tag_find_one)
            .expect("farm tag find one")
            .result
            .is_some()
    );
    let farm_tag_update_alt: IFarmTagUpdate = parse_json(
        json!({ "on": { "farm_id": farm_created.id }, "fields": { "tag": "biodynamic" } }),
    );
    assert_eq!(
        db.farm_tag_update(&farm_tag_update_alt)
            .expect("farm tag update")
            .result
            .tag,
        "biodynamic"
    );
    let farm_tag_update_id: IFarmTagUpdate = parse_json(
        json!({ "on": { "id": farm_tag_created.id }, "fields": { "tag": "regenerative" } }),
    );
    assert_eq!(
        db.farm_tag_update(&farm_tag_update_id)
            .expect("farm tag update id")
            .result
            .tag,
        "regenerative"
    );
    let farm_tag_update_empty: IFarmTagUpdate =
        parse_json(json!({ "on": { "id": farm_tag_created.id }, "fields": {} }));
    assert_invalid_argument(db.farm_tag_update(&farm_tag_update_empty));

    let plot_tag_find_many: IPlotTagFindMany =
        parse_json(json!({ "filter": { "id": plot_tag_created.id } }));
    assert_eq!(
        db.plot_tag_find_many(&plot_tag_find_many)
            .expect("plot tag find many")
            .results
            .len(),
        1
    );
    let plot_tag_find_one: IPlotTagFindOne =
        parse_json(json!({ "on": { "id": plot_tag_created.id } }));
    assert!(
        db.plot_tag_find_one(&plot_tag_find_one)
            .expect("plot tag find one")
            .result
            .is_some()
    );
    let plot_tag_update_alt: IPlotTagUpdate =
        parse_json(json!({ "on": { "plot_id": plot_created.id }, "fields": { "tag": "south" } }));
    assert_eq!(
        db.plot_tag_update(&plot_tag_update_alt)
            .expect("plot tag update")
            .result
            .tag,
        "south"
    );
    let plot_tag_update_id: IPlotTagUpdate =
        parse_json(json!({ "on": { "id": plot_tag_created.id }, "fields": { "tag": "east" } }));
    assert_eq!(
        db.plot_tag_update(&plot_tag_update_id)
            .expect("plot tag update id")
            .result
            .tag,
        "east"
    );
    let plot_tag_update_empty: IPlotTagUpdate =
        parse_json(json!({ "on": { "id": plot_tag_created.id }, "fields": {} }));
    assert_invalid_argument(db.plot_tag_update(&plot_tag_update_empty));

    let farm_member_find_many: IFarmMemberFindMany =
        parse_json(json!({ "filter": { "id": farm_member_created.id } }));
    assert_eq!(
        db.farm_member_find_many(&farm_member_find_many)
            .expect("farm member find many")
            .results
            .len(),
        1
    );
    let farm_member_find_one: IFarmMemberFindOne =
        parse_json(json!({ "on": { "id": farm_member_created.id } }));
    assert!(
        db.farm_member_find_one(&farm_member_find_one)
            .expect("farm member find one")
            .result
            .is_some()
    );
    let farm_member_update_alt: IFarmMemberUpdate = parse_json(json!({
        "on": { "member_pubkey": hex64('b') },
        "fields": { "role": "editor" }
    }));
    assert_eq!(
        db.farm_member_update(&farm_member_update_alt)
            .expect("farm member update")
            .result
            .role,
        "editor"
    );
    let farm_member_update_id: IFarmMemberUpdate = parse_json(
        json!({ "on": { "id": farm_member_created.id }, "fields": { "role": "admin" } }),
    );
    assert_eq!(
        db.farm_member_update(&farm_member_update_id)
            .expect("farm member update id")
            .result
            .role,
        "admin"
    );
    let farm_member_update_empty: IFarmMemberUpdate =
        parse_json(json!({ "on": { "id": farm_member_created.id }, "fields": {} }));
    assert_invalid_argument(db.farm_member_update(&farm_member_update_empty));

    let farm_member_claim_find_many: IFarmMemberClaimFindMany =
        parse_json(json!({ "filter": { "id": farm_member_claim_created.id } }));
    assert_eq!(
        db.farm_member_claim_find_many(&farm_member_claim_find_many)
            .expect("farm member claim find many")
            .results
            .len(),
        1
    );
    let farm_member_claim_find_one: IFarmMemberClaimFindOne =
        parse_json(json!({ "on": { "id": farm_member_claim_created.id } }));
    assert!(
        db.farm_member_claim_find_one(&farm_member_claim_find_one)
            .expect("farm member claim find one")
            .result
            .is_some()
    );
    let farm_member_claim_update_alt: IFarmMemberClaimUpdate = parse_json(json!({
        "on": { "member_pubkey": hex64('b') },
        "fields": { "farm_pubkey": hex64('f') }
    }));
    assert_eq!(
        db.farm_member_claim_update(&farm_member_claim_update_alt)
            .expect("farm member claim update")
            .result
            .farm_pubkey,
        hex64('f')
    );
    let farm_member_claim_update_id: IFarmMemberClaimUpdate = parse_json(json!({
        "on": { "id": farm_member_claim_created.id },
        "fields": { "farm_pubkey": hex64('g') }
    }));
    assert_eq!(
        db.farm_member_claim_update(&farm_member_claim_update_id)
            .expect("farm member claim update id")
            .result
            .farm_pubkey,
        hex64('g')
    );
    let farm_member_claim_update_empty: IFarmMemberClaimUpdate =
        parse_json(json!({ "on": { "id": farm_member_claim_created.id }, "fields": {} }));
    assert_invalid_argument(db.farm_member_claim_update(&farm_member_claim_update_empty));

    let log_error_find_many: ILogErrorFindMany =
        parse_json(json!({ "filter": { "id": log_error_created.id } }));
    assert_eq!(
        db.log_error_find_many(&log_error_find_many)
            .expect("log error find many")
            .results
            .len(),
        1
    );
    let log_error_find_one: ILogErrorFindOne =
        parse_json(json!({ "on": { "id": log_error_created.id } }));
    assert!(
        db.log_error_find_one(&log_error_find_one)
            .expect("log error find one")
            .result
            .is_some()
    );
    let log_error_update_alt: ILogErrorUpdate = parse_json(json!({
        "on": { "nostr_pubkey": hex64('c') },
        "fields": { "message": "boom+" }
    }));
    assert_eq!(
        db.log_error_update(&log_error_update_alt)
            .expect("log error update")
            .result
            .message,
        "boom+"
    );
    let log_error_update_id: ILogErrorUpdate = parse_json(
        json!({ "on": { "id": log_error_created.id }, "fields": { "message": "boom++" } }),
    );
    assert_eq!(
        db.log_error_update(&log_error_update_id)
            .expect("log error update id")
            .result
            .message,
        "boom++"
    );
    let log_error_update_empty: ILogErrorUpdate =
        parse_json(json!({ "on": { "id": log_error_created.id }, "fields": {} }));
    assert_invalid_argument(db.log_error_update(&log_error_update_empty));

    for opts in [
        IMediaImageFindMany::Rel {
            rel: MediaImageFindManyRel::OnTradeProduct(MediaImageTradeProductArgs {
                id: trade_product_created.id.clone(),
            }),
        },
        IMediaImageFindMany::Rel {
            rel: MediaImageFindManyRel::OffTradeProduct(MediaImageTradeProductArgs {
                id: trade_product_created.id.clone(),
            }),
        },
    ] {
        let _ = db
            .media_image_find_many(&opts)
            .expect("media image rel find many");
    }
    let media_image_find_many_filter: IMediaImageFindMany =
        parse_json(json!({ "filter": { "id": media_image_created.id } }));
    assert_eq!(
        db.media_image_find_many(&media_image_find_many_filter)
            .expect("media image find many filter")
            .results
            .len(),
        1
    );
    let media_image_find_one_on: IMediaImageFindOne =
        parse_json(json!({ "on": { "id": media_image_created.id } }));
    assert!(
        db.media_image_find_one(&media_image_find_one_on)
            .expect("media image find one")
            .result
            .is_some()
    );
    let media_image_find_one_rel: IMediaImageFindOne =
        parse_json(json!({ "rel": { "on_trade_product": { "id": trade_product_created.id } } }));
    assert!(
        db.media_image_find_one(&media_image_find_one_rel)
            .expect("media image find one rel")
            .result
            .is_some()
    );
    let media_image_update_alt: IMediaImageUpdate =
        parse_json(json!({ "on": { "file_path": "/img/a.jpg" }, "fields": { "label": "hero" } }));
    assert_eq!(
        db.media_image_update(&media_image_update_alt)
            .expect("media image update")
            .result
            .label
            .as_deref(),
        Some("hero")
    );
    let media_image_update_id: IMediaImageUpdate = parse_json(
        json!({ "on": { "id": media_image_created.id }, "fields": { "label": "hero+" } }),
    );
    assert_eq!(
        db.media_image_update(&media_image_update_id)
            .expect("media image update id")
            .result
            .label
            .as_deref(),
        Some("hero+")
    );
    let media_image_update_empty: IMediaImageUpdate =
        parse_json(json!({ "on": { "id": media_image_created.id }, "fields": {} }));
    assert_invalid_argument(db.media_image_update(&media_image_update_empty));

    for opts in [
        INostrProfileFindMany::Rel {
            rel: NostrProfileFindManyRel::OnRelay(NostrProfileRelayArgs {
                id: nostr_relay_created.id.clone(),
            }),
        },
        INostrProfileFindMany::Rel {
            rel: NostrProfileFindManyRel::OffRelay(NostrProfileRelayArgs {
                id: nostr_relay_created.id.clone(),
            }),
        },
    ] {
        let _ = db
            .nostr_profile_find_many(&opts)
            .expect("nostr profile rel find many");
    }
    let nostr_profile_find_many_filter: INostrProfileFindMany =
        parse_json(json!({ "filter": { "id": nostr_profile_created.id } }));
    assert_eq!(
        db.nostr_profile_find_many(&nostr_profile_find_many_filter)
            .expect("nostr profile find many filter")
            .results
            .len(),
        1
    );
    let nostr_profile_find_one_on: INostrProfileFindOne =
        parse_json(json!({ "on": { "id": nostr_profile_created.id } }));
    assert!(
        db.nostr_profile_find_one(&nostr_profile_find_one_on)
            .expect("nostr profile find one")
            .result
            .is_some()
    );
    let nostr_profile_find_one_rel: INostrProfileFindOne =
        parse_json(json!({ "rel": { "on_relay": { "id": nostr_relay_created.id } } }));
    assert!(
        db.nostr_profile_find_one(&nostr_profile_find_one_rel)
            .expect("nostr profile find one rel")
            .result
            .is_some()
    );
    let nostr_profile_update_alt: INostrProfileUpdate = parse_json(
        json!({ "on": { "public_key": hex64('d') }, "fields": { "name": "profile b" } }),
    );
    assert_eq!(
        db.nostr_profile_update(&nostr_profile_update_alt)
            .expect("nostr profile update")
            .result
            .name,
        "profile b"
    );
    let nostr_profile_update_id: INostrProfileUpdate = parse_json(
        json!({ "on": { "id": nostr_profile_created.id }, "fields": { "name": "profile b+" } }),
    );
    assert_eq!(
        db.nostr_profile_update(&nostr_profile_update_id)
            .expect("nostr profile update id")
            .result
            .name,
        "profile b+"
    );
    let nostr_profile_update_empty: INostrProfileUpdate =
        parse_json(json!({ "on": { "id": nostr_profile_created.id }, "fields": {} }));
    assert_invalid_argument(db.nostr_profile_update(&nostr_profile_update_empty));

    let nostr_event_state_find_many: INostrEventStateFindMany =
        parse_json(json!({ "filter": { "id": nostr_event_state_created.id } }));
    assert_eq!(
        db.nostr_event_state_find_many(&nostr_event_state_find_many)
            .expect("nostr event state find many")
            .results
            .len(),
        1
    );
    let nostr_event_state_find_one: INostrEventStateFindOne =
        parse_json(json!({ "on": { "id": nostr_event_state_created.id } }));
    assert!(
        db.nostr_event_state_find_one(&nostr_event_state_find_one)
            .expect("nostr event state find one")
            .result
            .is_some()
    );
    let nostr_event_state_update_alt: INostrEventStateUpdate =
        parse_json(json!({ "on": { "key": "state-a" }, "fields": { "content_hash": "hash-b" } }));
    assert_eq!(
        db.nostr_event_state_update(&nostr_event_state_update_alt)
            .expect("nostr event state update")
            .result
            .content_hash,
        "hash-b"
    );
    let nostr_event_state_update_id: INostrEventStateUpdate = parse_json(
        json!({ "on": { "id": nostr_event_state_created.id }, "fields": { "content_hash": "hash-c" } }),
    );
    assert_eq!(
        db.nostr_event_state_update(&nostr_event_state_update_id)
            .expect("nostr event state update id")
            .result
            .content_hash,
        "hash-c"
    );
    let nostr_event_state_update_empty: INostrEventStateUpdate =
        parse_json(json!({ "on": { "id": nostr_event_state_created.id }, "fields": {} }));
    assert_invalid_argument(db.nostr_event_state_update(&nostr_event_state_update_empty));

    for opts in [
        INostrRelayFindMany::Rel {
            rel: NostrRelayFindManyRel::OnProfile(NostrRelayProfileArgs {
                public_key: hex64('d'),
            }),
        },
        INostrRelayFindMany::Rel {
            rel: NostrRelayFindManyRel::OffProfile(NostrRelayProfileArgs {
                public_key: hex64('d'),
            }),
        },
    ] {
        let _ = db
            .nostr_relay_find_many(&opts)
            .expect("nostr relay rel find many");
    }
    let nostr_relay_find_many_filter: INostrRelayFindMany =
        parse_json(json!({ "filter": { "id": nostr_relay_created.id } }));
    assert_eq!(
        db.nostr_relay_find_many(&nostr_relay_find_many_filter)
            .expect("nostr relay find many filter")
            .results
            .len(),
        1
    );
    let nostr_relay_find_one_on: INostrRelayFindOne =
        parse_json(json!({ "on": { "id": nostr_relay_created.id } }));
    assert!(
        db.nostr_relay_find_one(&nostr_relay_find_one_on)
            .expect("nostr relay find one")
            .result
            .is_some()
    );
    let nostr_relay_find_one_rel: INostrRelayFindOne =
        parse_json(json!({ "rel": { "on_profile": { "public_key": hex64('d') } } }));
    assert!(
        db.nostr_relay_find_one(&nostr_relay_find_one_rel)
            .expect("nostr relay find one rel")
            .result
            .is_some()
    );
    let nostr_relay_update_alt: INostrRelayUpdate = parse_json(json!({
        "on": { "url": "wss://relay.example.com" },
        "fields": { "name": "relay a" }
    }));
    assert_eq!(
        db.nostr_relay_update(&nostr_relay_update_alt)
            .expect("nostr relay update")
            .result
            .name
            .as_deref(),
        Some("relay a")
    );
    let nostr_relay_update_id: INostrRelayUpdate = parse_json(
        json!({ "on": { "id": nostr_relay_created.id }, "fields": { "name": "relay a+" } }),
    );
    assert_eq!(
        db.nostr_relay_update(&nostr_relay_update_id)
            .expect("nostr relay update id")
            .result
            .name
            .as_deref(),
        Some("relay a+")
    );
    let nostr_relay_update_empty: INostrRelayUpdate =
        parse_json(json!({ "on": { "id": nostr_relay_created.id }, "fields": {} }));
    assert_invalid_argument(db.nostr_relay_update(&nostr_relay_update_empty));

    let trade_product_find_many: ITradeProductFindMany =
        parse_json(json!({ "filter": { "id": trade_product_created.id } }));
    assert_eq!(
        db.trade_product_find_many(&trade_product_find_many)
            .expect("trade product find many")
            .results
            .len(),
        1
    );
    let trade_product_find_one: ITradeProductFindOne =
        parse_json(json!({ "on": { "id": trade_product_created.id } }));
    assert!(
        db.trade_product_find_one(&trade_product_find_one)
            .expect("trade product find one")
            .result
            .is_some()
    );
    let trade_product_update: ITradeProductUpdate = parse_json(
        json!({ "on": { "id": trade_product_created.id }, "fields": { "title": "coffee b" } }),
    );
    assert_eq!(
        db.trade_product_update(&trade_product_update)
            .expect("trade product update")
            .result
            .title,
        "coffee b"
    );
    let trade_product_update_empty: ITradeProductUpdate =
        parse_json(json!({ "on": { "id": trade_product_created.id }, "fields": {} }));
    assert_invalid_argument(db.trade_product_update(&trade_product_update_empty));

    let backup = db.backup_database().expect("backup");
    let backup_json = db.backup_database_json().expect("backup json");
    let _manifest = export_manifest(db.executor()).expect("export manifest");
    db.restore_database(&backup).expect("restore backup");
    db.restore_database_json(&backup_json)
        .expect("restore backup json");

    let gcs_delete_rel_found: IGcsLocationDelete =
        parse_json(json!({ "rel": { "off_trade_product": { "id": trade_product_created.id } } }));
    db.gcs_location_delete(&gcs_delete_rel_found)
        .expect("gcs rel delete found");

    let media_image_rel_delete_found: IMediaImageDelete =
        parse_json(json!({ "rel": { "off_trade_product": { "id": trade_product_created.id } } }));
    db.media_image_delete(&media_image_rel_delete_found)
        .expect("media image rel delete found");

    let nostr_relay_rel_delete_found: INostrRelayDelete =
        parse_json(json!({ "rel": { "on_profile": { "public_key": hex64('f') } } }));
    db.nostr_relay_delete(&nostr_relay_rel_delete_found)
        .expect("nostr relay rel delete found");

    let nostr_profile_rel_delete_found: INostrProfileDelete =
        parse_json(json!({ "rel": { "off_relay": { "id": nostr_relay_created.id } } }));
    db.nostr_profile_delete(&nostr_profile_rel_delete_found)
        .expect("nostr profile rel delete found");

    db.trade_product_media_unset(&product_media_rel)
        .expect("product media unset");
    db.trade_product_location_unset(&product_location_rel)
        .expect("product location unset");
    db.nostr_profile_relay_unset(&profile_relay_rel)
        .expect("profile relay unset");

    let trade_product_delete: ITradeProductDelete =
        parse_json(json!({ "on": { "id": trade_product_created.id } }));
    db.trade_product_delete(&trade_product_delete)
        .expect("trade product delete");
    let trade_product_delete_missing: ITradeProductDelete =
        parse_json(json!({ "on": { "id": trade_product_created.id } }));
    assert_not_found(db.trade_product_delete(&trade_product_delete_missing));

    let plot_gcs_delete: IPlotGcsLocationDelete =
        parse_json(json!({ "on": { "plot_id": plot_created.id } }));
    db.plot_gcs_location_delete(&plot_gcs_delete)
        .expect("plot gcs delete");
    let plot_gcs_delete_missing: IPlotGcsLocationDelete =
        parse_json(json!({ "on": { "id": plot_gcs_created.id } }));
    assert_not_found(db.plot_gcs_location_delete(&plot_gcs_delete_missing));

    let farm_gcs_delete: IFarmGcsLocationDelete =
        parse_json(json!({ "on": { "farm_id": farm_created.id } }));
    db.farm_gcs_location_delete(&farm_gcs_delete)
        .expect("farm gcs delete");
    let farm_gcs_delete_missing: IFarmGcsLocationDelete =
        parse_json(json!({ "on": { "id": farm_gcs_created.id } }));
    assert_not_found(db.farm_gcs_location_delete(&farm_gcs_delete_missing));

    let gcs_delete_on_non_primary: IGcsLocationDelete =
        parse_json(json!({ "on": { "d_tag": "gcs-a" } }));
    let _ = db.gcs_location_delete(&gcs_delete_on_non_primary);

    for payload in [
        json!({ "rel": { "on_trade_product": { "id": trade_product_created.id } } }),
        json!({ "rel": { "off_trade_product": { "id": trade_product_created.id } } }),
        json!({ "rel": { "on_farm": { "id": farm_created.id } } }),
        json!({ "rel": { "off_farm": { "id": farm_created.id } } }),
        json!({ "rel": { "on_plot": { "id": plot_created.id } } }),
        json!({ "rel": { "off_plot": { "id": plot_created.id } } }),
    ] {
        let opts: IGcsLocationDelete = parse_json(payload);
        let _ = db.gcs_location_delete(&opts);
    }
    let gcs_delete_missing: IGcsLocationDelete =
        parse_json(json!({ "on": { "id": gcs_created.id } }));
    assert_not_found(db.gcs_location_delete(&gcs_delete_missing));

    let media_image_delete: IMediaImageDelete =
        parse_json(json!({ "on": { "file_path": "/img/a.jpg" } }));
    db.media_image_delete(&media_image_delete)
        .expect("media image delete");
    let media_image_delete_missing: IMediaImageDelete =
        parse_json(json!({ "on": { "id": media_image_created.id } }));
    assert_not_found(db.media_image_delete(&media_image_delete_missing));
    for payload in [
        json!({ "rel": { "on_trade_product": { "id": trade_product_created.id } } }),
        json!({ "rel": { "off_trade_product": { "id": trade_product_created.id } } }),
    ] {
        let opts: IMediaImageDelete = parse_json(payload);
        let _ = db.media_image_delete(&opts);
    }

    let nostr_profile_delete: INostrProfileDelete =
        parse_json(json!({ "on": { "public_key": hex64('d') } }));
    db.nostr_profile_delete(&nostr_profile_delete)
        .expect("nostr profile delete");
    let nostr_profile_delete_missing: INostrProfileDelete =
        parse_json(json!({ "on": { "id": nostr_profile_created.id } }));
    assert_not_found(db.nostr_profile_delete(&nostr_profile_delete_missing));
    for payload in [
        json!({ "rel": { "on_relay": { "id": nostr_relay_created.id } } }),
        json!({ "rel": { "off_relay": { "id": nostr_relay_created.id } } }),
    ] {
        let opts: INostrProfileDelete = parse_json(payload);
        let _ = db.nostr_profile_delete(&opts);
    }

    let nostr_relay_delete: INostrRelayDelete =
        parse_json(json!({ "on": { "url": "wss://relay.example.com" } }));
    db.nostr_relay_delete(&nostr_relay_delete)
        .expect("nostr relay delete");
    let nostr_relay_delete_missing: INostrRelayDelete =
        parse_json(json!({ "on": { "id": nostr_relay_created.id } }));
    assert_not_found(db.nostr_relay_delete(&nostr_relay_delete_missing));
    for payload in [
        json!({ "rel": { "on_profile": { "public_key": hex64('d') } } }),
        json!({ "rel": { "off_profile": { "public_key": hex64('d') } } }),
    ] {
        let opts: INostrRelayDelete = parse_json(payload);
        let _ = db.nostr_relay_delete(&opts);
    }

    let nostr_event_state_delete: INostrEventStateDelete =
        parse_json(json!({ "on": { "key": "state-a" } }));
    db.nostr_event_state_delete(&nostr_event_state_delete)
        .expect("nostr event state delete");
    let nostr_event_state_delete_missing: INostrEventStateDelete =
        parse_json(json!({ "on": { "id": nostr_event_state_created.id } }));
    assert_not_found(db.nostr_event_state_delete(&nostr_event_state_delete_missing));

    let log_error_delete: ILogErrorDelete =
        parse_json(json!({ "on": { "nostr_pubkey": hex64('c') } }));
    db.log_error_delete(&log_error_delete)
        .expect("log error delete");
    let log_error_delete_missing: ILogErrorDelete =
        parse_json(json!({ "on": { "id": log_error_created.id } }));
    assert_not_found(db.log_error_delete(&log_error_delete_missing));

    let farm_member_claim_delete: IFarmMemberClaimDelete =
        parse_json(json!({ "on": { "member_pubkey": hex64('b') } }));
    db.farm_member_claim_delete(&farm_member_claim_delete)
        .expect("farm member claim delete");
    let farm_member_claim_delete_missing: IFarmMemberClaimDelete =
        parse_json(json!({ "on": { "id": farm_member_claim_created.id } }));
    assert_not_found(db.farm_member_claim_delete(&farm_member_claim_delete_missing));

    let farm_member_delete: IFarmMemberDelete =
        parse_json(json!({ "on": { "member_pubkey": hex64('b') } }));
    db.farm_member_delete(&farm_member_delete)
        .expect("farm member delete");
    let farm_member_delete_missing: IFarmMemberDelete =
        parse_json(json!({ "on": { "id": farm_member_created.id } }));
    assert_not_found(db.farm_member_delete(&farm_member_delete_missing));

    let plot_tag_delete: IPlotTagDelete = parse_json(json!({ "on": { "tag": "east" } }));
    db.plot_tag_delete(&plot_tag_delete)
        .expect("plot tag delete");
    let plot_tag_delete_missing: IPlotTagDelete =
        parse_json(json!({ "on": { "id": plot_tag_created.id } }));
    assert_not_found(db.plot_tag_delete(&plot_tag_delete_missing));

    let farm_tag_delete: IFarmTagDelete = parse_json(json!({ "on": { "tag": "regenerative" } }));
    db.farm_tag_delete(&farm_tag_delete)
        .expect("farm tag delete");
    let farm_tag_delete_missing: IFarmTagDelete =
        parse_json(json!({ "on": { "id": farm_tag_created.id } }));
    assert_not_found(db.farm_tag_delete(&farm_tag_delete_missing));

    let plot_delete: IPlotDelete = parse_json(json!({ "on": { "d_tag": "plot-a" } }));
    db.plot_delete(&plot_delete).expect("plot delete");
    let plot_delete_missing: IPlotDelete = parse_json(json!({ "on": { "id": plot_created.id } }));
    assert_not_found(db.plot_delete(&plot_delete_missing));

    let farm_delete: IFarmDelete = parse_json(json!({ "on": { "d_tag": "farm-a" } }));
    db.farm_delete(&farm_delete).expect("farm delete");
    let farm_delete_missing: IFarmDelete = parse_json(json!({ "on": { "id": farm_created.id } }));
    assert_not_found(db.farm_delete(&farm_delete_missing));
}
