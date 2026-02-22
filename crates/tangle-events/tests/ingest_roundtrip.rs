use radroots_events::RadrootsNostrEvent;
use radroots_events::farm::{
    RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef, RadrootsGcsLocation, RadrootsGeoJsonPoint,
    RadrootsGeoJsonPolygon,
};
use radroots_events::kinds::{
    KIND_FARM, KIND_LIST_SET_FOLLOW, KIND_LIST_SET_GENERIC, KIND_PLOT, KIND_PROFILE,
};
use radroots_events::list::RadrootsListEntry;
use radroots_events::list_set::RadrootsListSet;
use radroots_events::plot::{RadrootsPlot, RadrootsPlotLocation};
use radroots_events::profile::{
    RADROOTS_PROFILE_TYPE_TAG_KEY, RadrootsProfile, RadrootsProfileType,
    radroots_profile_type_tag_value,
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::farm::encode as farm_encode;
use radroots_events_codec::farm::list_sets as farm_list_sets;
use radroots_events_codec::list_set::encode as list_set_encode;
use radroots_events_codec::plot::encode as plot_encode;
use radroots_sql_core::SqlExecutor;
use radroots_sql_core::SqliteExecutor;
use radroots_sql_core::error::SqlError;
use radroots_tangle_db::{
    farm, farm_gcs_location, farm_member, farm_member_claim, farm_tag, gcs_location, migrations,
    nostr_profile, plot, plot_gcs_location, plot_tag,
};
use radroots_tangle_db_schema::farm::{IFarmFields, IFarmFieldsFilter, IFarmFindMany};
use radroots_tangle_db_schema::farm_gcs_location::IFarmGcsLocationFields;
use radroots_tangle_db_schema::farm_member::{
    IFarmMemberFields, IFarmMemberFieldsFilter, IFarmMemberFindMany,
};
use radroots_tangle_db_schema::farm_member_claim::{
    IFarmMemberClaimFields, IFarmMemberClaimFieldsFilter, IFarmMemberClaimFindMany,
};
use radroots_tangle_db_schema::farm_tag::{IFarmTagFields, IFarmTagFieldsFilter, IFarmTagFindMany};
use radroots_tangle_db_schema::gcs_location::IGcsLocationFields;
use radroots_tangle_db_schema::nostr_profile::INostrProfileFields;
use radroots_tangle_db_schema::plot::IPlotFields;
use radroots_tangle_db_schema::plot_gcs_location::IPlotGcsLocationFields;
use radroots_tangle_db_schema::plot_tag::{IPlotTagFields, IPlotTagFieldsFilter, IPlotTagFindMany};
use radroots_tangle_events::{
    RADROOTS_TANGLE_TRANSFER_VERSION, RadrootsTangleEventDraft, RadrootsTangleEventsError,
    RadrootsTangleFarmSelector, RadrootsTangleIngestOutcome, RadrootsTangleSyncOptions,
    RadrootsTangleSyncRequest, radroots_tangle_ingest_event, radroots_tangle_sync_all,
    radroots_tangle_sync_status,
};
use radroots_types::types::IError;

fn unwrap_sql<T>(result: Result<T, IError<SqlError>>, label: &str) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("{label}: {}", err.err),
    }
}

fn draft_to_event(draft: &RadrootsTangleEventDraft, index: u32) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: format!("{:064x}", index as u64 + 1),
        author: draft.author.clone(),
        created_at: 1_720_000_000 + index,
        kind: draft.kind,
        tags: draft.tags.clone(),
        content: draft.content.clone(),
        sig: "f".repeat(128),
    }
}

fn seed_source(
    exec: &SqliteExecutor,
) -> (
    RadrootsTangleSyncRequest,
    String,
    String,
    Vec<RadrootsTangleEventDraft>,
) {
    migrations::run_all_up(exec).expect("migrations");

    let farm_pubkey = "f".repeat(64);
    let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA".to_string();
    let farm_fields = IFarmFields {
        d_tag: farm_d_tag.clone(),
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
    let farm_row = unwrap_sql(farm::create(exec, &farm_fields), "farm").result;

    let point = radroots_events::farm::RadrootsGeoJsonPoint {
        r#type: "Point".to_string(),
        coordinates: [-122.4, 37.7],
    };
    let polygon = radroots_events::farm::RadrootsGeoJsonPolygon {
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
        point: serde_json::to_string(&point).expect("point"),
        polygon: serde_json::to_string(&polygon).expect("polygon"),
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
    let gcs_row = unwrap_sql(gcs_location::create(exec, &gcs_fields), "gcs").result;

    let _ = unwrap_sql(
        farm_gcs_location::create(
            exec,
            &IFarmGcsLocationFields {
                farm_id: farm_row.id.clone(),
                gcs_location_id: gcs_row.id.clone(),
                role: "primary".to_string(),
            },
        ),
        "farm_gcs",
    );

    let plot_row = unwrap_sql(
        plot::create(
            exec,
            &IPlotFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
                farm_id: farm_row.id.clone(),
                name: "Plot A".to_string(),
                about: None,
                location_primary: None,
                location_city: None,
                location_region: None,
                location_country: None,
            },
        ),
        "plot",
    )
    .result;

    let _ = unwrap_sql(
        plot_gcs_location::create(
            exec,
            &IPlotGcsLocationFields {
                plot_id: plot_row.id.clone(),
                gcs_location_id: gcs_row.id.clone(),
                role: "primary".to_string(),
            },
        ),
        "plot_gcs",
    );

    let _ = unwrap_sql(
        farm_tag::create(
            exec,
            &IFarmTagFields {
                farm_id: farm_row.id.clone(),
                tag: "coffee".to_string(),
            },
        ),
        "farm_tag",
    );

    let _ = unwrap_sql(
        plot_tag::create(
            exec,
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
            exec,
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
            exec,
            &IFarmMemberClaimFields {
                member_pubkey: owner_pubkey.clone(),
                farm_pubkey: farm_pubkey.clone(),
            },
        ),
        "farm_member_claim",
    );

    let _ = unwrap_sql(
        nostr_profile::create(
            exec,
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
            exec,
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
            id: Some(farm_row.id),
            d_tag: None,
            pubkey: None,
        },
        options: None,
    };
    let bundle = radroots_tangle_sync_all(exec, &request).expect("sync");
    (request, farm_d_tag, farm_pubkey, bundle.events)
}

#[test]
fn ingest_roundtrip_yields_zero_pending_sync() {
    let source = SqliteExecutor::open_memory().expect("source db");
    let (_source_request, farm_d_tag, farm_pubkey, drafts) = seed_source(&source);
    assert_eq!(drafts.len(), 9);

    let target = SqliteExecutor::open_memory().expect("target db");
    migrations::run_all_up(&target).expect("target migrations");

    let mut skipped = 0usize;
    for (index, draft) in drafts.iter().enumerate() {
        let event = draft_to_event(draft, index as u32);
        let first = radroots_tangle_ingest_event(&target, &event).expect("first ingest");
        assert_eq!(first, RadrootsTangleIngestOutcome::Applied);
        let second = radroots_tangle_ingest_event(&target, &event).expect("second ingest");
        if second == RadrootsTangleIngestOutcome::Skipped {
            skipped += 1;
        }
    }
    assert!(skipped > 0);

    let status = radroots_tangle_sync_status(&target).expect("sync status");
    assert_eq!(status.expected_count, drafts.len());
    assert_eq!(status.pending_count, 0);

    let replay = radroots_tangle_sync_all(
        &target,
        &RadrootsTangleSyncRequest {
            farm: RadrootsTangleFarmSelector {
                id: None,
                d_tag: Some(farm_d_tag),
                pubkey: Some(farm_pubkey),
            },
            options: None,
        },
    )
    .expect("replay sync");
    assert_eq!(replay.version, RADROOTS_TANGLE_TRANSFER_VERSION);
    assert_eq!(replay.events.len(), drafts.len());
}

#[test]
fn sync_status_empty_db_is_zero() {
    let exec = SqliteExecutor::open_memory().expect("db");
    migrations::run_all_up(&exec).expect("migrations");
    let status = radroots_tangle_sync_status(&exec).expect("status");
    assert_eq!(status.expected_count, 0);
    assert_eq!(status.pending_count, 0);
}

#[test]
fn sync_all_selector_and_options_paths_are_supported() {
    let source = SqliteExecutor::open_memory().expect("source db");
    let (request, farm_d_tag, farm_pubkey, full_events) = seed_source(&source);

    let by_pair = radroots_tangle_sync_all(
        &source,
        &RadrootsTangleSyncRequest {
            farm: RadrootsTangleFarmSelector {
                id: None,
                d_tag: Some(farm_d_tag.clone()),
                pubkey: Some(farm_pubkey.clone()),
            },
            options: None,
        },
    )
    .expect("selector by d_tag + pubkey");
    assert_eq!(by_pair.events.len(), full_events.len());

    let reduced = radroots_tangle_sync_all(
        &source,
        &RadrootsTangleSyncRequest {
            farm: request.farm,
            options: Some(RadrootsTangleSyncOptions {
                include_profiles: Some(false),
                include_list_sets: Some(false),
                include_membership_claims: Some(false),
            }),
        },
    )
    .expect("reduced sync");
    assert_eq!(reduced.events.len(), 2);
}

#[test]
fn ingest_rejects_unsupported_kind() {
    let exec = SqliteExecutor::open_memory().expect("db");
    migrations::run_all_up(&exec).expect("migrations");
    let event = RadrootsNostrEvent {
        id: format!("{:064x}", 1u64),
        author: "a".repeat(64),
        created_at: 1_720_000_001,
        kind: 42,
        tags: Vec::new(),
        content: String::new(),
        sig: "f".repeat(128),
    };
    let err = radroots_tangle_ingest_event(&exec, &event).expect_err("unsupported kind");
    assert!(err.to_string().contains("unsupported kind"));
}

fn event_with_parts(
    id: u64,
    author: &str,
    created_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: format!("{id:064x}"),
        author: author.to_string(),
        created_at,
        kind,
        tags,
        content,
        sig: "f".repeat(128),
    }
}

fn sample_point(lat: f64, lng: f64) -> RadrootsGeoJsonPoint {
    RadrootsGeoJsonPoint {
        r#type: "Point".to_string(),
        coordinates: [lng, lat],
    }
}

fn sample_polygon(lat: f64, lng: f64) -> RadrootsGeoJsonPolygon {
    RadrootsGeoJsonPolygon {
        r#type: "Polygon".to_string(),
        coordinates: vec![vec![
            [lng, lat],
            [lng, lat + 0.001],
            [lng - 0.001, lat + 0.001],
            [lng, lat],
        ]],
    }
}

fn sample_gcs(lat: f64, lng: f64, geohash: &str) -> RadrootsGcsLocation {
    RadrootsGcsLocation {
        lat,
        lng,
        geohash: geohash.to_string(),
        point: sample_point(lat, lng),
        polygon: sample_polygon(lat, lng),
        accuracy: Some(2.0),
        altitude: Some(10.0),
        tag_0: Some("soil".to_string()),
        label: Some("north".to_string()),
        area: Some(1_000.0),
        elevation: Some(5),
        soil: Some("loam".to_string()),
        climate: Some("temperate".to_string()),
        gc_id: Some("gc".to_string()),
        gc_name: Some("name".to_string()),
        gc_admin1_id: Some("admin1".to_string()),
        gc_admin1_name: Some("admin1_name".to_string()),
        gc_country_id: Some("country".to_string()),
        gc_country_name: Some("country_name".to_string()),
    }
}

fn profile_event(
    id: u64,
    author: &str,
    created_at: u32,
    profile_type: Option<RadrootsProfileType>,
    name: &str,
) -> RadrootsNostrEvent {
    let profile = RadrootsProfile {
        name: name.to_string(),
        display_name: Some(format!("{name}_display")),
        nip05: Some(format!("{name}@example.com")),
        about: Some(format!("{name} about")),
        website: Some("https://example.com".to_string()),
        picture: Some("https://example.com/p.png".to_string()),
        banner: Some("https://example.com/b.png".to_string()),
        lud06: Some("lud06".to_string()),
        lud16: Some("lud16".to_string()),
        bot: None,
    };
    let mut tags = Vec::new();
    if let Some(kind) = profile_type {
        tags.push(vec![
            RADROOTS_PROFILE_TYPE_TAG_KEY.to_string(),
            radroots_profile_type_tag_value(kind).to_string(),
        ]);
    }
    event_with_parts(
        id,
        author,
        created_at,
        KIND_PROFILE,
        serde_json::to_string(&profile).expect("profile json"),
        tags,
    )
}

fn farm_event(
    id: u64,
    author: &str,
    created_at: u32,
    d_tag: &str,
    name: &str,
    location: Option<RadrootsFarmLocation>,
    tags: Option<Vec<String>>,
) -> RadrootsNostrEvent {
    let farm = RadrootsFarm {
        d_tag: d_tag.to_string(),
        name: name.to_string(),
        about: Some(format!("{name} about")),
        website: Some("https://farm.example.com".to_string()),
        picture: Some("https://farm.example.com/p.png".to_string()),
        banner: Some("https://farm.example.com/b.png".to_string()),
        location,
        tags,
    };
    let event_tags = farm_encode::farm_build_tags(&farm).expect("farm tags");
    event_with_parts(
        id,
        author,
        created_at,
        KIND_FARM,
        serde_json::to_string(&farm).expect("farm json"),
        event_tags,
    )
}

fn plot_event(
    id: u64,
    author: &str,
    created_at: u32,
    d_tag: &str,
    farm_ref: RadrootsFarmRef,
    name: &str,
    location: Option<RadrootsPlotLocation>,
    tags: Option<Vec<String>>,
) -> RadrootsNostrEvent {
    let plot = RadrootsPlot {
        d_tag: d_tag.to_string(),
        farm: farm_ref,
        name: name.to_string(),
        about: Some(format!("{name} about")),
        location,
        tags,
    };
    let event_tags = plot_encode::plot_build_tags(&plot).expect("plot tags");
    event_with_parts(
        id,
        author,
        created_at,
        KIND_PLOT,
        serde_json::to_string(&plot).expect("plot json"),
        event_tags,
    )
}

fn list_set_event(
    id: u64,
    author: &str,
    created_at: u32,
    kind: u32,
    list_set: &RadrootsListSet,
) -> RadrootsNostrEvent {
    let parts = list_set_encode::to_wire_parts_with_kind(list_set, kind).expect("list set parts");
    event_with_parts(id, author, created_at, kind, parts.content, parts.tags)
}

#[test]
fn ingest_event_paths_cover_profile_farm_plot_and_list_set_variants() {
    let exec = SqliteExecutor::open_memory().expect("db");
    migrations::run_all_up(&exec).expect("migrations");

    let profile_pubkey = "p".repeat(64);
    let profile_create = profile_event(
        101,
        &profile_pubkey,
        10,
        Some(RadrootsProfileType::Individual),
        "alice",
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &profile_create).expect("profile create"),
        RadrootsTangleIngestOutcome::Applied
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &profile_create).expect("profile skip same"),
        RadrootsTangleIngestOutcome::Skipped
    );
    let profile_older = profile_event(
        102,
        &profile_pubkey,
        9,
        Some(RadrootsProfileType::Individual),
        "alice-older",
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &profile_older).expect("profile skip older"),
        RadrootsTangleIngestOutcome::Skipped
    );
    let profile_same_time_new_hash = profile_event(
        103,
        &profile_pubkey,
        10,
        Some(RadrootsProfileType::Individual),
        "alice-updated",
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &profile_same_time_new_hash)
            .expect("profile apply same timestamp different hash"),
        RadrootsTangleIngestOutcome::Applied
    );
    let profile_missing_type = profile_event(104, &profile_pubkey, 11, None, "missing-type");
    let err = radroots_tangle_ingest_event(&exec, &profile_missing_type)
        .expect_err("profile type is required");
    assert!(err.to_string().contains("profile_type required"));

    let profile_types = [
        (RadrootsProfileType::Farm, "f".repeat(64), "farm-profile"),
        (RadrootsProfileType::Coop, "c".repeat(64), "coop-profile"),
        (RadrootsProfileType::Any, "a".repeat(64), "any-profile"),
        (
            RadrootsProfileType::Radrootsd,
            "d".repeat(64),
            "radrootsd-profile",
        ),
    ];
    for (index, (profile_type, pubkey, name)) in profile_types.iter().enumerate() {
        let event = profile_event(
            110 + index as u64,
            pubkey,
            20 + index as u32,
            Some(*profile_type),
            name,
        );
        assert_eq!(
            radroots_tangle_ingest_event(&exec, &event).expect("profile variant"),
            RadrootsTangleIngestOutcome::Applied
        );
    }

    let farm_pubkey = "e".repeat(64);
    let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
    let farm_location = RadrootsFarmLocation {
        primary: Some("farm-primary".to_string()),
        city: Some("city".to_string()),
        region: Some("region".to_string()),
        country: Some("country".to_string()),
        gcs: sample_gcs(37.7, -122.4, "9q8yy"),
    };
    let farm_create = farm_event(
        200,
        &farm_pubkey,
        100,
        farm_d_tag,
        "farm-a",
        Some(farm_location.clone()),
        Some(vec![
            "coffee".to_string(),
            " ".to_string(),
            "coffee".to_string(),
            "grain".to_string(),
        ]),
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &farm_create).expect("farm create"),
        RadrootsTangleIngestOutcome::Applied
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &farm_create).expect("farm skip same"),
        RadrootsTangleIngestOutcome::Skipped
    );
    let farm_older = farm_event(
        201,
        &farm_pubkey,
        99,
        farm_d_tag,
        "farm-older",
        Some(farm_location.clone()),
        None,
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &farm_older).expect("farm skip older"),
        RadrootsTangleIngestOutcome::Skipped
    );
    let farm_update_same_time = farm_event(
        202,
        &farm_pubkey,
        100,
        farm_d_tag,
        "farm-a-updated",
        None,
        Some(vec!["market".to_string()]),
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &farm_update_same_time).expect("farm update"),
        RadrootsTangleIngestOutcome::Applied
    );

    let farm_rows = unwrap_sql(
        farm::find_many(
            &exec,
            &IFarmFindMany {
                filter: Some(IFarmFieldsFilter {
                    id: None,
                    created_at: None,
                    updated_at: None,
                    d_tag: Some(farm_d_tag.to_string()),
                    pubkey: Some(farm_pubkey.clone()),
                    name: None,
                    about: None,
                    website: None,
                    picture: None,
                    banner: None,
                    location_primary: None,
                    location_city: None,
                    location_region: None,
                    location_country: None,
                }),
            },
        ),
        "farm find_many",
    )
    .results;
    assert_eq!(farm_rows.len(), 1);
    let farm_id = farm_rows[0].id.clone();

    let farm_tags = unwrap_sql(
        farm_tag::find_many(
            &exec,
            &IFarmTagFindMany {
                filter: Some(IFarmTagFieldsFilter {
                    id: None,
                    created_at: None,
                    updated_at: None,
                    farm_id: Some(farm_id.clone()),
                    tag: None,
                }),
            },
        ),
        "farm tags",
    )
    .results;
    assert_eq!(farm_tags.len(), 1);
    assert_eq!(farm_tags[0].tag, "market");

    let plot_d_tag = "AAAAAAAAAAAAAAAAAAAAAQ";
    let plot_location = RadrootsPlotLocation {
        primary: Some("plot-primary".to_string()),
        city: Some("plot-city".to_string()),
        region: Some("plot-region".to_string()),
        country: Some("plot-country".to_string()),
        gcs: sample_gcs(37.8, -122.5, "9q8yz"),
    };
    let plot_create = plot_event(
        300,
        &farm_pubkey,
        200,
        plot_d_tag,
        RadrootsFarmRef {
            pubkey: farm_pubkey.clone(),
            d_tag: farm_d_tag.to_string(),
        },
        "plot-a",
        Some(plot_location.clone()),
        Some(vec![
            "orchard".to_string(),
            " ".to_string(),
            "orchard".to_string(),
            "shade".to_string(),
        ]),
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &plot_create).expect("plot create"),
        RadrootsTangleIngestOutcome::Applied
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &plot_create).expect("plot skip same"),
        RadrootsTangleIngestOutcome::Skipped
    );
    let plot_older = plot_event(
        301,
        &farm_pubkey,
        199,
        plot_d_tag,
        RadrootsFarmRef {
            pubkey: farm_pubkey.clone(),
            d_tag: farm_d_tag.to_string(),
        },
        "plot-older",
        Some(plot_location.clone()),
        None,
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &plot_older).expect("plot skip older"),
        RadrootsTangleIngestOutcome::Skipped
    );
    let plot_update = plot_event(
        302,
        &farm_pubkey,
        200,
        plot_d_tag,
        RadrootsFarmRef {
            pubkey: farm_pubkey.clone(),
            d_tag: farm_d_tag.to_string(),
        },
        "plot-a-updated",
        None,
        Some(vec!["updated".to_string()]),
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &plot_update).expect("plot update"),
        RadrootsTangleIngestOutcome::Applied
    );
    let plot_missing_farm = plot_event(
        303,
        &farm_pubkey,
        201,
        "AAAAAAAAAAAAAAAAAAAAAg",
        RadrootsFarmRef {
            pubkey: "z".repeat(64),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        },
        "plot-missing-farm",
        None,
        None,
    );
    let missing_farm_err = radroots_tangle_ingest_event(&exec, &plot_missing_farm)
        .expect_err("plot requires existing farm");
    assert!(missing_farm_err.to_string().contains("farm not found"));

    let plot_rows = unwrap_sql(
        plot::find_many(
            &exec,
            &radroots_tangle_db_schema::plot::IPlotFindMany { filter: None },
        ),
        "plot rows",
    )
    .results;
    assert_eq!(plot_rows.len(), 1);
    let plot_id = plot_rows[0].id.clone();
    let plot_tags = unwrap_sql(
        plot_tag::find_many(
            &exec,
            &IPlotTagFindMany {
                filter: Some(IPlotTagFieldsFilter {
                    id: None,
                    created_at: None,
                    updated_at: None,
                    plot_id: Some(plot_id),
                    tag: None,
                }),
            },
        ),
        "plot tags",
    )
    .results;
    assert_eq!(plot_tags.len(), 1);
    assert_eq!(plot_tags[0].tag, "updated");

    let non_generic_list_set = RadrootsListSet {
        d_tag: "member_of.farms".to_string(),
        content: String::new(),
        entries: vec![RadrootsListEntry {
            tag: "p".to_string(),
            values: vec![farm_pubkey.clone()],
        }],
        title: None,
        description: None,
        image: None,
    };
    let non_generic_event = list_set_event(
        400,
        &profile_pubkey,
        300,
        KIND_LIST_SET_FOLLOW,
        &non_generic_list_set,
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &non_generic_event).expect("non-generic list set"),
        RadrootsTangleIngestOutcome::Skipped
    );

    let metadata_list_set = RadrootsListSet {
        d_tag: "member_of.farms".to_string(),
        content: String::new(),
        entries: vec![RadrootsListEntry {
            tag: "p".to_string(),
            values: vec![farm_pubkey.clone()],
        }],
        title: Some("title".to_string()),
        description: None,
        image: None,
    };
    let metadata_event = list_set_event(
        401,
        &profile_pubkey,
        301,
        KIND_LIST_SET_GENERIC,
        &metadata_list_set,
    );
    let metadata_err = radroots_tangle_ingest_event(&exec, &metadata_event)
        .expect_err("metadata must be rejected");
    assert!(metadata_err.to_string().contains("must omit metadata"));

    let content_list_set = RadrootsListSet {
        d_tag: "member_of.farms".to_string(),
        content: "not-empty".to_string(),
        entries: vec![RadrootsListEntry {
            tag: "p".to_string(),
            values: vec![farm_pubkey.clone()],
        }],
        title: None,
        description: None,
        image: None,
    };
    let content_event = list_set_event(
        402,
        &profile_pubkey,
        302,
        KIND_LIST_SET_GENERIC,
        &content_list_set,
    );
    let content_err =
        radroots_tangle_ingest_event(&exec, &content_event).expect_err("content must be rejected");
    assert!(content_err.to_string().contains("must not include content"));

    let invalid_member_of = RadrootsListSet {
        d_tag: "member_of.farms".to_string(),
        content: String::new(),
        entries: vec![RadrootsListEntry {
            tag: "a".to_string(),
            values: vec![farm_pubkey.clone()],
        }],
        title: None,
        description: None,
        image: None,
    };
    let invalid_member_of_event = list_set_event(
        403,
        &profile_pubkey,
        303,
        KIND_LIST_SET_GENERIC,
        &invalid_member_of,
    );
    let invalid_member_of_err = radroots_tangle_ingest_event(&exec, &invalid_member_of_event)
        .expect_err("member_of requires p tags");
    assert!(
        invalid_member_of_err
            .to_string()
            .contains("must only include p tags")
    );

    let member_of_valid = RadrootsListSet {
        d_tag: "member_of.farms".to_string(),
        content: String::new(),
        entries: vec![
            RadrootsListEntry {
                tag: "p".to_string(),
                values: vec![farm_pubkey.clone()],
            },
            RadrootsListEntry {
                tag: "p".to_string(),
                values: vec![farm_pubkey.clone()],
            },
        ],
        title: None,
        description: None,
        image: None,
    };
    let member_of_event = list_set_event(
        404,
        &profile_pubkey,
        304,
        KIND_LIST_SET_GENERIC,
        &member_of_valid,
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &member_of_event).expect("member_of apply"),
        RadrootsTangleIngestOutcome::Applied
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &member_of_event).expect("member_of skip"),
        RadrootsTangleIngestOutcome::Skipped
    );

    let claims = unwrap_sql(
        farm_member_claim::find_many(
            &exec,
            &IFarmMemberClaimFindMany {
                filter: Some(IFarmMemberClaimFieldsFilter {
                    id: None,
                    created_at: None,
                    updated_at: None,
                    member_pubkey: Some(profile_pubkey.clone()),
                    farm_pubkey: None,
                }),
            },
        ),
        "claims",
    )
    .results;
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].farm_pubkey, farm_pubkey);

    let invalid_members = RadrootsListSet {
        d_tag: format!("farm:{farm_d_tag}:members"),
        content: String::new(),
        entries: vec![RadrootsListEntry {
            tag: "a".to_string(),
            values: vec!["x".to_string()],
        }],
        title: None,
        description: None,
        image: None,
    };
    let invalid_members_event = list_set_event(
        405,
        &farm_pubkey,
        305,
        KIND_LIST_SET_GENERIC,
        &invalid_members,
    );
    let invalid_members_err = radroots_tangle_ingest_event(&exec, &invalid_members_event)
        .expect_err("members list requires p entries");
    assert!(
        invalid_members_err
            .to_string()
            .contains("must only include p tags")
    );

    let members_valid =
        farm_list_sets::farm_members_list_set(farm_d_tag, vec!["m".repeat(64), "m".repeat(64)])
            .expect("members list");
    let members_event = list_set_event(
        406,
        &farm_pubkey,
        306,
        KIND_LIST_SET_GENERIC,
        &members_valid,
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &members_event).expect("members apply"),
        RadrootsTangleIngestOutcome::Applied
    );
    let owners_valid =
        farm_list_sets::farm_owners_list_set(farm_d_tag, vec!["o".repeat(64)]).expect("owners");
    let owners_event = list_set_event(407, &farm_pubkey, 307, KIND_LIST_SET_GENERIC, &owners_valid);
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &owners_event).expect("owners apply"),
        RadrootsTangleIngestOutcome::Applied
    );
    let workers_valid =
        farm_list_sets::farm_workers_list_set(farm_d_tag, vec!["w".repeat(64)]).expect("workers");
    let workers_event = list_set_event(
        408,
        &farm_pubkey,
        308,
        KIND_LIST_SET_GENERIC,
        &workers_valid,
    );
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &workers_event).expect("workers apply"),
        RadrootsTangleIngestOutcome::Applied
    );

    let members = unwrap_sql(
        farm_member::find_many(
            &exec,
            &IFarmMemberFindMany {
                filter: Some(IFarmMemberFieldsFilter {
                    id: None,
                    created_at: None,
                    updated_at: None,
                    farm_id: Some(farm_id),
                    member_pubkey: None,
                    role: None,
                }),
            },
        ),
        "members",
    )
    .results;
    assert_eq!(members.len(), 3);

    let invalid_plots = RadrootsListSet {
        d_tag: format!("farm:{farm_d_tag}:plots"),
        content: String::new(),
        entries: vec![RadrootsListEntry {
            tag: "p".to_string(),
            values: vec!["x".to_string()],
        }],
        title: None,
        description: None,
        image: None,
    };
    let invalid_plots_event = list_set_event(
        409,
        &farm_pubkey,
        309,
        KIND_LIST_SET_GENERIC,
        &invalid_plots,
    );
    let invalid_plots_err = radroots_tangle_ingest_event(&exec, &invalid_plots_event)
        .expect_err("plots list requires a entries");
    assert!(
        invalid_plots_err
            .to_string()
            .contains("must only include a tags")
    );

    let plot_address = plot_encode::plot_address(&farm_pubkey, plot_d_tag).expect("plot address");
    let plots_valid = RadrootsListSet {
        d_tag: format!("farm:{farm_d_tag}:plots"),
        content: String::new(),
        entries: vec![RadrootsListEntry {
            tag: "a".to_string(),
            values: vec![plot_address],
        }],
        title: None,
        description: None,
        image: None,
    };
    let plots_event = list_set_event(410, &farm_pubkey, 310, KIND_LIST_SET_GENERIC, &plots_valid);
    assert_eq!(
        radroots_tangle_ingest_event(&exec, &plots_event).expect("plots apply"),
        RadrootsTangleIngestOutcome::Applied
    );

    let unsupported_list_set = RadrootsListSet {
        d_tag: "unsupported.list".to_string(),
        content: String::new(),
        entries: vec![RadrootsListEntry {
            tag: "p".to_string(),
            values: vec![farm_pubkey],
        }],
        title: None,
        description: None,
        image: None,
    };
    let unsupported_event = list_set_event(
        411,
        &profile_pubkey,
        311,
        KIND_LIST_SET_GENERIC,
        &unsupported_list_set,
    );
    let unsupported_err = radroots_tangle_ingest_event(&exec, &unsupported_event)
        .expect_err("unsupported list set d_tag");
    assert!(
        unsupported_err
            .to_string()
            .contains("unsupported list set d_tag")
    );
}

#[test]
fn sync_status_reports_pending_when_not_all_events_are_ingested() {
    let source = SqliteExecutor::open_memory().expect("source");
    let (_request, _farm_d_tag, _farm_pubkey, drafts) = seed_source(&source);
    let target = SqliteExecutor::open_memory().expect("target");
    migrations::run_all_up(&target).expect("migrations");

    for (index, draft) in drafts.iter().enumerate() {
        let event = draft_to_event(draft, index as u32);
        let _ = radroots_tangle_ingest_event(&target, &event).expect("ingest");
    }
    target
        .exec(
            "UPDATE nostr_event_state SET content_hash = ? WHERE id = (SELECT id FROM nostr_event_state LIMIT 1)",
            "[\"invalid_hash\"]",
        )
        .expect("mutate state hash");

    let status = radroots_tangle_sync_status(&target).expect("status pending");
    assert_eq!(status.expected_count, drafts.len());
    assert!(status.pending_count > 0);
}

#[test]
fn sync_all_rejects_invalid_selectors_and_non_unique_pair() {
    let exec = SqliteExecutor::open_memory().expect("db");
    migrations::run_all_up(&exec).expect("migrations");

    let missing_selector_err = radroots_tangle_sync_all(
        &exec,
        &RadrootsTangleSyncRequest {
            farm: RadrootsTangleFarmSelector {
                id: None,
                d_tag: None,
                pubkey: None,
            },
            options: None,
        },
    )
    .expect_err("selector validation");
    assert!(
        missing_selector_err
            .to_string()
            .contains("requires id or (d_tag + pubkey)")
    );

    let missing_id_err = radroots_tangle_sync_all(
        &exec,
        &RadrootsTangleSyncRequest {
            farm: RadrootsTangleFarmSelector {
                id: Some("00000000-0000-0000-0000-000000000000".to_string()),
                d_tag: None,
                pubkey: None,
            },
            options: None,
        },
    )
    .expect_err("missing farm id");
    assert!(missing_id_err.to_string().contains("farm not found"));

    let duplicate_d_tag = "AAAAAAAAAAAAAAAAAAAAAA".to_string();
    let duplicate_pubkey = "u".repeat(64);
    let fields = IFarmFields {
        d_tag: duplicate_d_tag.clone(),
        pubkey: duplicate_pubkey.clone(),
        name: "one".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location_primary: None,
        location_city: None,
        location_region: None,
        location_country: None,
    };
    let _ = unwrap_sql(farm::create(&exec, &fields), "farm one");
    let _ = unwrap_sql(farm::create(&exec, &fields), "farm two");

    let non_unique_err = radroots_tangle_sync_all(
        &exec,
        &RadrootsTangleSyncRequest {
            farm: RadrootsTangleFarmSelector {
                id: None,
                d_tag: Some(duplicate_d_tag),
                pubkey: Some(duplicate_pubkey),
            },
            options: None,
        },
    )
    .expect_err("non unique selector");
    assert!(
        non_unique_err
            .to_string()
            .contains("did not resolve to a single farm")
    );
}

#[test]
fn sync_emit_handles_invalid_geojson_and_unknown_profile_type() {
    let exec = SqliteExecutor::open_memory().expect("db");
    migrations::run_all_up(&exec).expect("migrations");

    let farm_pubkey = "g".repeat(64);
    let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA".to_string();
    let farm_row = unwrap_sql(
        farm::create(
            &exec,
            &IFarmFields {
                d_tag: farm_d_tag.clone(),
                pubkey: farm_pubkey.clone(),
                name: "farm".to_string(),
                about: Some("about".to_string()),
                website: None,
                picture: None,
                banner: None,
                location_primary: Some("primary".to_string()),
                location_city: Some("city".to_string()),
                location_region: Some("region".to_string()),
                location_country: Some("country".to_string()),
            },
        ),
        "farm",
    )
    .result;

    let bad_gcs = unwrap_sql(
        gcs_location::create(
            &exec,
            &IGcsLocationFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
                lat: 10.0,
                lng: 20.0,
                geohash: "s0".to_string(),
                point: "{".to_string(),
                polygon: "{\"type\":\"Polygon\",\"coordinates\":[[]]}".to_string(),
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
            },
        ),
        "bad gcs",
    )
    .result;
    let _ = unwrap_sql(
        farm_gcs_location::create(
            &exec,
            &IFarmGcsLocationFields {
                farm_id: farm_row.id.clone(),
                gcs_location_id: bad_gcs.id.clone(),
                role: "".to_string(),
            },
        ),
        "farm gcs",
    );

    let plot_row = unwrap_sql(
        plot::create(
            &exec,
            &IPlotFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
                farm_id: farm_row.id.clone(),
                name: "plot".to_string(),
                about: Some("plot about".to_string()),
                location_primary: Some("plot primary".to_string()),
                location_city: None,
                location_region: None,
                location_country: None,
            },
        ),
        "plot",
    )
    .result;
    let _ = unwrap_sql(
        plot_gcs_location::create(
            &exec,
            &IPlotGcsLocationFields {
                plot_id: plot_row.id.clone(),
                gcs_location_id: bad_gcs.id,
                role: "primary".to_string(),
            },
        ),
        "plot gcs",
    );

    let member_pubkey = "m".repeat(64);
    let _ = unwrap_sql(
        farm_member::create(
            &exec,
            &IFarmMemberFields {
                farm_id: farm_row.id.clone(),
                member_pubkey: member_pubkey.clone(),
                role: "owner".to_string(),
            },
        ),
        "member",
    );
    let _ = unwrap_sql(
        farm_member_claim::create(
            &exec,
            &IFarmMemberClaimFields {
                member_pubkey: member_pubkey.clone(),
                farm_pubkey: farm_pubkey.clone(),
            },
        ),
        "claim",
    );
    let _ = unwrap_sql(
        nostr_profile::create(
            &exec,
            &INostrProfileFields {
                public_key: farm_pubkey.clone(),
                profile_type: "farm".to_string(),
                name: "farm profile".to_string(),
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
        "farm profile",
    );
    let _ = unwrap_sql(
        nostr_profile::create(
            &exec,
            &INostrProfileFields {
                public_key: member_pubkey.clone(),
                profile_type: "legacy".to_string(),
                name: "legacy profile".to_string(),
                display_name: Some("legacy".to_string()),
                about: Some("about".to_string()),
                website: Some("https://example.com".to_string()),
                picture: Some("https://example.com/p.png".to_string()),
                banner: Some("https://example.com/b.png".to_string()),
                nip05: Some("legacy@example.com".to_string()),
                lud06: Some("lud06".to_string()),
                lud16: Some("lud16".to_string()),
            },
        ),
        "legacy profile",
    );

    let bundle = radroots_tangle_sync_all(
        &exec,
        &RadrootsTangleSyncRequest {
            farm: RadrootsTangleFarmSelector {
                id: Some(farm_row.id),
                d_tag: None,
                pubkey: None,
            },
            options: None,
        },
    )
    .expect("sync");
    assert_eq!(bundle.version, RADROOTS_TANGLE_TRANSFER_VERSION);
    assert!(bundle.events.iter().any(|event| event.kind == KIND_FARM));
    assert!(bundle.events.iter().any(|event| event.kind == KIND_PLOT));
    assert!(
        bundle
            .events
            .iter()
            .any(|event| event.kind == KIND_LIST_SET_GENERIC)
    );
    assert!(bundle.events.iter().any(|event| {
        event.kind == KIND_PROFILE
            && event.author == member_pubkey
            && event
                .tags
                .iter()
                .all(|tag| tag[0] != RADROOTS_PROFILE_TYPE_TAG_KEY)
    }));
}

#[test]
fn error_conversion_paths_are_exercised() {
    let sql: RadrootsTangleEventsError = IError::from(SqlError::Internal).into();
    assert!(matches!(sql, RadrootsTangleEventsError::Sql(_)));

    let encode: RadrootsTangleEventsError = EventEncodeError::Json.into();
    assert!(matches!(encode, RadrootsTangleEventsError::Encode(_)));

    let parse_number_err = "x".parse::<u32>().expect_err("parse should fail");
    let parse: RadrootsTangleEventsError =
        EventParseError::InvalidNumber("k", parse_number_err).into();
    assert!(matches!(parse, RadrootsTangleEventsError::Parse(_)));
}
