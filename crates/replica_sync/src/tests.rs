use crate::{
    RADROOTS_REPLICA_TRANSFER_VERSION, RadrootsReplicaFarmSelector, RadrootsReplicaSyncRequest,
    radroots_replica_pending_publish_batch, radroots_replica_sync_all,
};
use radroots_event::gcs::{RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon};
use radroots_event::kinds::{KIND_FARM, KIND_LIST_SET_GENERIC, KIND_PLOT};
use radroots_replica_schema::ReplicaSchemaError;
use radroots_replica_schema::farm::IFarmFields;
use radroots_replica_schema::farm_gcs_location::IFarmGcsLocationFields;
use radroots_replica_schema::farm_member::IFarmMemberFields;
use radroots_replica_schema::farm_member_claim::IFarmMemberClaimFields;
use radroots_replica_schema::farm_tag::IFarmTagFields;
use radroots_replica_schema::gcs_location::IGcsLocationFields;
use radroots_replica_schema::nostr_profile::{INostrProfileFields, INostrProfileFindMany};
use radroots_replica_schema::plot::IPlotFields;
use radroots_replica_schema::plot_gcs_location::IPlotGcsLocationFields;
use radroots_replica_schema::plot_tag::IPlotTagFields;
use radroots_replica_store::{
    farm, farm_gcs_location, farm_member, farm_member_claim, farm_tag, gcs_location, migrations,
    nostr_profile, plot, plot_gcs_location, plot_tag,
};
use radroots_sql_core::SqlxSqliteExecutor;
use radroots_sql_core::error::SqlError;
use serde::Deserialize;
use std::panic;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileExclusionSuite {
    suite: String,
    contract_version: String,
    vectors: Vec<ProfileExclusionCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileExclusionCase {
    id: String,
    kind: String,
    input: ProfileExclusionInput,
    expected: ProfileExclusionExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileExclusionInput {
    farm_profile_name: String,
    owner_profile_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileExclusionExpected {
    stored_profile_rows: usize,
    transfer_event_count: usize,
    pending_event_count: usize,
    forbidden_kind: u32,
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn unwrap_sql<T>(result: Result<T, ReplicaSchemaError<SqlError>>, label: &str) -> T {
    result
        .map_err(|err| format!("{label}: {}", err.error))
        .unwrap()
}

#[test]
fn profile_exclusion_vector_executes_sync_and_pending_publication() {
    let suite = serde_json::from_str::<ProfileExclusionSuite>(include_str!(
        "../tests/fixtures/profile_exclusion.v1.json"
    ))
    .expect("profile exclusion vector");
    assert_eq!(suite.suite, "replica_profile_exclusion_v1");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(suite.vectors.len(), 1);
    let case = &suite.vectors[0];
    assert_eq!(case.id, "replica_profile_exclusion_stored_rows_001");
    assert_eq!(case.kind, "replica.profile_exclusion");

    let exec = SqlxSqliteExecutor::open_memory().expect("exec");
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

    let owner_pubkey = "8".repeat(64);
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
                name: case.input.farm_profile_name.clone(),
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
                name: case.input.owner_profile_name.clone(),
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
    let stored_profiles = unwrap_sql(
        nostr_profile::find_many(
            &exec,
            &INostrProfileFindMany::Filter {
                filter: Box::new(None),
            },
        ),
        "stored_profiles",
    );
    assert_eq!(
        stored_profiles.results.len(),
        case.expected.stored_profile_rows
    );

    let request = RadrootsReplicaSyncRequest {
        farm: RadrootsReplicaFarmSelector {
            id: Some(farm_row.id.clone()),
            d_tag: None,
            pubkey: None,
        },
        options: None,
    };
    let bundle = radroots_replica_sync_all(&exec, &request).expect("sync");

    assert_eq!(bundle.version, RADROOTS_REPLICA_TRANSFER_VERSION);
    assert_eq!(bundle.events.len(), case.expected.transfer_event_count);
    let kinds = bundle
        .events
        .iter()
        .map(|event| event.kind)
        .collect::<Vec<_>>();
    assert_eq!(kinds[0], KIND_FARM);
    assert_eq!(kinds[1], KIND_PLOT);
    assert!(kinds[2..].iter().all(|kind| *kind == KIND_LIST_SET_GENERIC));
    assert!(
        kinds
            .iter()
            .all(|kind| *kind != case.expected.forbidden_kind)
    );

    let pending = radroots_replica_pending_publish_batch(&exec).expect("pending publication");
    assert_eq!(
        pending.pending_events.len(),
        case.expected.pending_event_count
    );
    assert!(
        pending
            .pending_events
            .iter()
            .all(|event| event.kind != case.expected.forbidden_kind)
    );
}

#[test]
fn sync_request_json_rejects_removed_profile_option() {
    let encoded = serde_json::json!({
        "farm": {
            "id": "AAAAAAAAAAAAAAAAAAAAAA",
            "d_tag": null,
            "pubkey": null
        },
        "options": {
            "include_list_sets": true,
            "include_membership_claims": false
        }
    });
    let request: RadrootsReplicaSyncRequest =
        serde_json::from_value(encoded).expect("current sync request");
    let options = request.options.expect("sync options");
    assert_eq!(options.include_list_sets, Some(true));
    assert_eq!(options.include_membership_claims, Some(false));
    assert_eq!(RADROOTS_REPLICA_TRANSFER_VERSION, 2);

    let legacy = serde_json::json!({
        "farm": {
            "id": "AAAAAAAAAAAAAAAAAAAAAA",
            "d_tag": null,
            "pubkey": null
        },
        "options": {
            "include_profiles": true
        }
    });
    let error = serde_json::from_value::<RadrootsReplicaSyncRequest>(legacy)
        .expect_err("removed include_profiles option must fail closed");
    assert!(
        error
            .to_string()
            .contains("unknown field `include_profiles`")
    );
}

#[test]
fn unwrap_sql_panics_on_error() {
    let result = panic::catch_unwind(|| {
        let err = ReplicaSchemaError::from(SqlError::InvalidArgument("bad".to_string()));
        unwrap_sql::<()>(Err(err), "unwrap");
    });
    assert!(result.is_err());
}
