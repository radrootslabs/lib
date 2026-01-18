use radroots_events::farm::{RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon};
use radroots_events::kinds::{KIND_FARM, KIND_LIST_SET_GENERIC, KIND_PLOT, KIND_PROFILE};
use radroots_sql_core::SqliteExecutor;
use radroots_sql_core::error::SqlError;
use crate::{
    radroots_tangle_sync_all,
    RadrootsTangleFarmSelector,
    RadrootsTangleSyncRequest,
    RADROOTS_TANGLE_TRANSFER_VERSION,
};
use radroots_types::types::IError;
use radroots_tangle_db_schema::farm::IFarmFields;
use radroots_tangle_db_schema::farm_gcs_location::IFarmGcsLocationFields;
use radroots_tangle_db_schema::farm_member::IFarmMemberFields;
use radroots_tangle_db_schema::farm_member_claim::IFarmMemberClaimFields;
use radroots_tangle_db_schema::farm_tag::IFarmTagFields;
use radroots_tangle_db_schema::gcs_location::IGcsLocationFields;
use radroots_tangle_db_schema::nostr_profile::INostrProfileFields;
use radroots_tangle_db_schema::plot::IPlotFields;
use radroots_tangle_db_schema::plot_gcs_location::IPlotGcsLocationFields;
use radroots_tangle_db_schema::plot_tag::IPlotTagFields;
use radroots_tangle_db::{
    farm,
    farm_gcs_location,
    farm_member,
    farm_member_claim,
    farm_tag,
    gcs_location,
    migrations,
    nostr_profile,
    plot,
    plot_gcs_location,
    plot_tag,
};

fn unwrap_sql<T>(result: Result<T, IError<SqlError>>, label: &str) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("{label}: {}", err.err),
    }
}

#[test]
fn sync_all_emits_expected_order() {
    let exec = SqliteExecutor::open_memory().expect("exec");
    migrations::run_all_up(&exec).expect("migrations");

    let farm_pubkey = "f".repeat(64);
    let farm_fields = IFarmFields {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        pubkey: farm_pubkey.clone(),
        name: "Green Farm".to_string(),
        about: Some("About".to_string()),
        website: None,
        picture: None,
        banner: None,
        location_primary: None,
        location_city: None,
        location_region: None,
        location_country: None,
    };
    let farm_row = unwrap_sql(farm::create(&exec, &farm_fields), "farm").result;

    let gcs_point = RadrootsGeoJsonPoint {
        r#type: "Point".to_string(),
        coordinates: [-122.4, 37.7],
    };
    let gcs_polygon = RadrootsGeoJsonPolygon {
        r#type: "Polygon".to_string(),
        coordinates: vec![vec![
            [-122.4, 37.7],
            [-122.4, 37.701],
            [-122.401, 37.701],
            [-122.4, 37.7],
        ]],
    };
    let gcs_fields = IGcsLocationFields {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
        lat: 37.7,
        lng: -122.4,
        geohash: "9q8yy".to_string(),
        point: serde_json::to_string(&gcs_point).expect("point"),
        polygon: serde_json::to_string(&gcs_polygon).expect("polygon"),
        accuracy: None,
        altitude: None,
        tag_0: None,
        label: None,
        area: None,
        elevation: None,
        soil: None,
        climate: None,
        gc_id: None,
        gc_name: None,
        gc_admin1_id: None,
        gc_admin1_name: None,
        gc_country_id: None,
        gc_country_name: None,
    };
    let gcs_row = unwrap_sql(gcs_location::create(&exec, &gcs_fields), "gcs").result;

    let farm_gcs_fields = IFarmGcsLocationFields {
        farm_id: farm_row.id.clone(),
        gcs_location_id: gcs_row.id.clone(),
        role: "primary".to_string(),
    };
    let _ = unwrap_sql(
        farm_gcs_location::create(&exec, &farm_gcs_fields),
        "farm_gcs",
    );

    let plot_fields = IPlotFields {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        farm_id: farm_row.id.clone(),
        name: "Plot A".to_string(),
        about: None,
        location_primary: None,
        location_city: None,
        location_region: None,
        location_country: None,
    };
    let plot_row = unwrap_sql(plot::create(&exec, &plot_fields), "plot").result;

    let plot_gcs_fields = IPlotGcsLocationFields {
        plot_id: plot_row.id.clone(),
        gcs_location_id: gcs_row.id.clone(),
        role: "primary".to_string(),
    };
    let _ = unwrap_sql(
        plot_gcs_location::create(&exec, &plot_gcs_fields),
        "plot_gcs",
    );

    let _ = unwrap_sql(
        farm_tag::create(
            &exec,
            &IFarmTagFields {
                farm_id: farm_row.id.clone(),
                tag: "coffee".to_string(),
            },
        ),
        "farm_tag",
    );

    let _ = unwrap_sql(
        plot_tag::create(
            &exec,
            &IPlotTagFields {
                plot_id: plot_row.id.clone(),
                tag: "orchard".to_string(),
            },
        ),
        "plot_tag",
    );

    let owner_pubkey = "o".repeat(64);
    let _ = unwrap_sql(
        farm_member::create(
            &exec,
            &IFarmMemberFields {
                farm_id: farm_row.id.clone(),
                member_pubkey: owner_pubkey.clone(),
                role: "owner".to_string(),
            },
        ),
        "farm_member",
    );

    let _ = unwrap_sql(
        farm_member_claim::create(
            &exec,
            &IFarmMemberClaimFields {
                member_pubkey: owner_pubkey.clone(),
                farm_pubkey: farm_pubkey.clone(),
            },
        ),
        "farm_member_claim",
    );

    let _ = unwrap_sql(
        nostr_profile::create(
            &exec,
            &INostrProfileFields {
                public_key: farm_pubkey.clone(),
                profile_type: "farm".to_string(),
                name: "Farm Profile".to_string(),
                display_name: None,
                about: None,
                website: None,
                picture: None,
                banner: None,
                nip05: None,
                lud06: None,
                lud16: None,
            },
        ),
        "farm_profile",
    );

    let _ = unwrap_sql(
        nostr_profile::create(
            &exec,
            &INostrProfileFields {
                public_key: owner_pubkey.clone(),
                profile_type: "individual".to_string(),
                name: "Owner".to_string(),
                display_name: None,
                about: None,
                website: None,
                picture: None,
                banner: None,
                nip05: None,
                lud06: None,
                lud16: None,
            },
        ),
        "owner_profile",
    );

    let request = RadrootsTangleSyncRequest {
        farm: RadrootsTangleFarmSelector {
            id: Some(farm_row.id.clone()),
            d_tag: None,
            pubkey: None,
        },
        options: None,
    };
    let bundle = radroots_tangle_sync_all(&exec, &request).expect("sync");

    assert_eq!(bundle.version, RADROOTS_TANGLE_TRANSFER_VERSION);
    assert_eq!(bundle.events.len(), 9);
    let kinds = bundle.events.iter().map(|event| event.kind).collect::<Vec<_>>();
    assert_eq!(kinds[0], KIND_PROFILE);
    assert_eq!(kinds[1], KIND_PROFILE);
    assert_eq!(kinds[2], KIND_FARM);
    assert_eq!(kinds[3], KIND_PLOT);
    assert!(kinds[4..8].iter().all(|kind| *kind == KIND_LIST_SET_GENERIC));
    assert_eq!(kinds[8], KIND_LIST_SET_GENERIC);
}
