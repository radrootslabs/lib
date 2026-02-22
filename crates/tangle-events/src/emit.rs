#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use radroots_events::farm::{
    RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef, RadrootsGcsLocation, RadrootsGeoJsonPoint,
    RadrootsGeoJsonPolygon,
};
use radroots_events::kinds::{KIND_FARM, KIND_LIST_SET_GENERIC, KIND_PLOT};
use radroots_events::plot::RadrootsPlot;
use radroots_events::profile::{
    RADROOTS_PROFILE_TYPE_TAG_KEY, RadrootsProfile, RadrootsProfileType,
    radroots_profile_type_from_tag_value, radroots_profile_type_tag_value,
};
use radroots_events_codec::farm::encode as farm_encode;
use radroots_events_codec::farm::list_sets as farm_list_sets;
use radroots_events_codec::list_set::encode as list_set_encode;
use radroots_events_codec::plot::encode as plot_encode;
use radroots_events_codec::wire::WireEventParts;
use radroots_sql_core::SqlExecutor;
use radroots_tangle_db::{
    farm, farm_gcs_location, farm_member, farm_member_claim, farm_tag, gcs_location, nostr_profile,
    plot, plot_gcs_location, plot_tag,
};
use radroots_tangle_db_schema::farm::{
    Farm, IFarmFieldsFilter, IFarmFindMany, IFarmFindOne, IFarmFindOneArgs,
};
use radroots_tangle_db_schema::farm_gcs_location::{
    FarmGcsLocation, IFarmGcsLocationFieldsFilter, IFarmGcsLocationFindMany,
};
use radroots_tangle_db_schema::farm_member::{
    FarmMember, IFarmMemberFieldsFilter, IFarmMemberFindMany,
};
use radroots_tangle_db_schema::farm_member_claim::{
    FarmMemberClaim, IFarmMemberClaimFieldsFilter, IFarmMemberClaimFindMany,
};
use radroots_tangle_db_schema::farm_tag::{IFarmTagFieldsFilter, IFarmTagFindMany};
use radroots_tangle_db_schema::gcs_location::{
    GcsLocation, GcsLocationQueryBindValues, IGcsLocationFindOne, IGcsLocationFindOneArgs,
};
use radroots_tangle_db_schema::nostr_profile::{
    INostrProfileFindOne, INostrProfileFindOneArgs, NostrProfileQueryBindValues,
};
use radroots_tangle_db_schema::plot::{IPlotFieldsFilter, IPlotFindMany, Plot};
use radroots_tangle_db_schema::plot_gcs_location::{
    IPlotGcsLocationFieldsFilter, IPlotGcsLocationFindMany, PlotGcsLocation,
};
use radroots_tangle_db_schema::plot_tag::{IPlotTagFieldsFilter, IPlotTagFindMany};
use serde_json::Value;

use crate::canonical::canonical_json_string;
use crate::error::RadrootsTangleEventsError;
use crate::geo::{geojson_point_from_lat_lng, geojson_polygon_circle_wgs84};
use crate::types::{
    RADROOTS_TANGLE_TRANSFER_VERSION, RadrootsTangleEventDraft, RadrootsTangleFarmSelector,
    RadrootsTangleSyncBundle, RadrootsTangleSyncOptions, RadrootsTangleSyncRequest,
};

const ROLE_PRIMARY: &str = "primary";
const ROLE_MEMBER: &str = "member";
const ROLE_OWNER: &str = "owner";
const ROLE_WORKER: &str = "worker";

pub fn radroots_tangle_sync_all<E: SqlExecutor>(
    exec: &E,
    request: &RadrootsTangleSyncRequest,
) -> Result<RadrootsTangleSyncBundle, RadrootsTangleEventsError> {
    radroots_tangle_sync_all_with_options(exec, &request.farm, request.options.as_ref())
}

pub fn radroots_tangle_sync_all_with_options<E: SqlExecutor>(
    exec: &E,
    farm_selector: &RadrootsTangleFarmSelector,
    options: Option<&RadrootsTangleSyncOptions>,
) -> Result<RadrootsTangleSyncBundle, RadrootsTangleEventsError> {
    let farm = resolve_farm(exec, farm_selector)?;
    let include_profiles = options.and_then(|opt| opt.include_profiles).unwrap_or(true);
    let include_list_sets = options
        .and_then(|opt| opt.include_list_sets)
        .unwrap_or(true);
    let include_claims = options
        .and_then(|opt| opt.include_membership_claims)
        .unwrap_or(true);

    let mut events = Vec::new();

    if include_profiles {
        let profiles = radroots_tangle_profile_events(exec, &farm)?;
        events.extend(profiles);
    }

    events.push(radroots_tangle_farm_event(exec, &farm)?);

    let plots = radroots_tangle_plot_events(exec, &farm)?;
    events.extend(plots);

    if include_list_sets {
        let list_sets = radroots_tangle_list_set_events(exec, &farm)?;
        events.extend(list_sets);
    }

    if include_claims {
        let claims = radroots_tangle_membership_claim_events(exec, &farm.pubkey)?;
        events.extend(claims);
    }

    Ok(RadrootsTangleSyncBundle {
        version: RADROOTS_TANGLE_TRANSFER_VERSION,
        events,
    })
}

pub fn radroots_tangle_profile_events<E: SqlExecutor>(
    exec: &E,
    farm: &Farm,
) -> Result<Vec<RadrootsTangleEventDraft>, RadrootsTangleEventsError> {
    let mut pubkeys = collect_profile_pubkeys(exec, farm)?;
    pubkeys.sort();
    pubkeys.dedup();

    let mut events = Vec::new();
    for pubkey in pubkeys {
        if let Some(profile) = load_profile(exec, &pubkey)? {
            events.push(profile_event(&pubkey, profile)?);
        }
    }
    Ok(events)
}

pub fn radroots_tangle_farm_event<E: SqlExecutor>(
    exec: &E,
    farm: &Farm,
) -> Result<RadrootsTangleEventDraft, RadrootsTangleEventsError> {
    let tags = collect_farm_tags(exec, &farm.id)?;
    let location = load_farm_location(exec, farm)?;
    let farm_event = RadrootsFarm {
        d_tag: farm.d_tag.clone(),
        name: farm.name.clone(),
        about: farm.about.clone(),
        website: farm.website.clone(),
        picture: farm.picture.clone(),
        banner: farm.banner.clone(),
        location,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };
    let tags = farm_encode::farm_build_tags(&farm_event)?;
    let content = canonical_json_string(&farm_event)?;
    let parts = WireEventParts {
        kind: KIND_FARM,
        content,
        tags,
    };
    Ok(parts_to_draft(&farm.pubkey, parts))
}

pub fn radroots_tangle_plot_events<E: SqlExecutor>(
    exec: &E,
    farm: &Farm,
) -> Result<Vec<RadrootsTangleEventDraft>, RadrootsTangleEventsError> {
    let plots = load_plots(exec, &farm.id)?;
    let mut events = Vec::new();
    for plot_row in plots {
        let tags = collect_plot_tags(exec, &plot_row.id)?;
        let location = load_plot_location(exec, &plot_row)?;
        let plot_event = RadrootsPlot {
            d_tag: plot_row.d_tag.clone(),
            farm: RadrootsFarmRef {
                pubkey: farm.pubkey.clone(),
                d_tag: farm.d_tag.clone(),
            },
            name: plot_row.name.clone(),
            about: plot_row.about.clone(),
            location,
            tags: if tags.is_empty() { None } else { Some(tags) },
        };
        let tags = plot_encode::plot_build_tags(&plot_event)?;
        let content = canonical_json_string(&plot_event)?;
        let parts = WireEventParts {
            kind: KIND_PLOT,
            content,
            tags,
        };
        events.push(parts_to_draft(&farm.pubkey, parts));
    }
    Ok(events)
}

pub fn radroots_tangle_list_set_events<E: SqlExecutor>(
    exec: &E,
    farm: &Farm,
) -> Result<Vec<RadrootsTangleEventDraft>, RadrootsTangleEventsError> {
    let members = load_farm_members(exec, &farm.id)?;
    let plots = load_plots(exec, &farm.id)?;

    let members_list =
        farm_list_sets::farm_members_list_set(&farm.d_tag, role_pubkeys(&members, ROLE_MEMBER))?;
    let owners_list =
        farm_list_sets::farm_owners_list_set(&farm.d_tag, role_pubkeys(&members, ROLE_OWNER))?;
    let workers_list =
        farm_list_sets::farm_workers_list_set(&farm.d_tag, role_pubkeys(&members, ROLE_WORKER))?;

    let plot_ids = sorted_plot_ids(&plots);
    let plots_list = farm_list_sets::farm_plots_list_set(&farm.d_tag, &farm.pubkey, plot_ids)?;

    let list_sets = [members_list, owners_list, workers_list, plots_list];
    let mut events = Vec::new();
    for list_set in list_sets {
        let parts = list_set_encode::to_wire_parts_with_kind(&list_set, KIND_LIST_SET_GENERIC)?;
        events.push(parts_to_draft(&farm.pubkey, parts));
    }
    Ok(events)
}

pub fn radroots_tangle_membership_claim_events<E: SqlExecutor>(
    exec: &E,
    farm_pubkey: &str,
) -> Result<Vec<RadrootsTangleEventDraft>, RadrootsTangleEventsError> {
    let claims = load_member_claims(exec, farm_pubkey)?;
    let mut by_member: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for claim in claims {
        by_member
            .entry(claim.member_pubkey.clone())
            .or_default()
            .push(claim.farm_pubkey.clone());
    }

    let mut events = Vec::new();
    for (member_pubkey, _) in by_member.iter() {
        let all_claims = load_member_claims_for_member(exec, member_pubkey)?;
        let mut farm_pubkeys = all_claims
            .into_iter()
            .map(|claim| claim.farm_pubkey)
            .collect::<Vec<String>>();
        farm_pubkeys.sort();
        farm_pubkeys.dedup();
        let list_set = farm_list_sets::member_of_farms_list_set(farm_pubkeys)?;
        let parts = list_set_encode::to_wire_parts_with_kind(&list_set, KIND_LIST_SET_GENERIC)?;
        events.push(parts_to_draft(member_pubkey, parts));
    }

    Ok(events)
}

fn resolve_farm<E: SqlExecutor>(
    exec: &E,
    selector: &RadrootsTangleFarmSelector,
) -> Result<Farm, RadrootsTangleEventsError> {
    if let Some(id) = selector.id.as_ref().filter(|v| !v.trim().is_empty()) {
        let result_query = farm::find_one(
            exec,
            &IFarmFindOne::On(IFarmFindOneArgs {
                on: radroots_tangle_db_schema::farm::FarmQueryBindValues::Id { id: id.clone() },
            }),
        );
        let result = result_query?;
        return result.result.ok_or_else(|| {
            RadrootsTangleEventsError::InvalidSelector(format!("farm not found: {id}"))
        });
    }

    let d_tag = selector
        .d_tag
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty());
    let pubkey = selector
        .pubkey
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty());

    let (d_tag, pubkey) = match (d_tag, pubkey) {
        (Some(d_tag), Some(pubkey)) => (d_tag, pubkey),
        _ => {
            return Err(RadrootsTangleEventsError::InvalidSelector(
                "farm selector requires id or (d_tag + pubkey)".to_string(),
            ));
        }
    };

    let filter = IFarmFieldsFilter {
        id: None,
        created_at: None,
        updated_at: None,
        d_tag: Some(d_tag.to_string()),
        pubkey: Some(pubkey.to_string()),
        name: None,
        about: None,
        website: None,
        picture: None,
        banner: None,
        location_primary: None,
        location_city: None,
        location_region: None,
        location_country: None,
    };
    let result_query = farm::find_many(
        exec,
        &IFarmFindMany {
            filter: Some(filter),
        },
    );
    let result = result_query?;
    if result.results.len() == 1 {
        return Ok(result.results.into_iter().next().expect("farm result"));
    }
    Err(RadrootsTangleEventsError::InvalidSelector(
        "farm selector did not resolve to a single farm".to_string(),
    ))
}

fn collect_farm_tags<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
) -> Result<Vec<String>, RadrootsTangleEventsError> {
    let filter = IFarmTagFieldsFilter {
        id: None,
        created_at: None,
        updated_at: None,
        farm_id: Some(farm_id.to_string()),
        tag: None,
    };
    let result_query = farm_tag::find_many(
        exec,
        &IFarmTagFindMany {
            filter: Some(filter),
        },
    );
    let result = result_query?;
    let mut tags = result
        .results
        .into_iter()
        .map(|row| row.tag)
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    Ok(tags)
}

fn collect_plot_tags<E: SqlExecutor>(
    exec: &E,
    plot_id: &str,
) -> Result<Vec<String>, RadrootsTangleEventsError> {
    let filter = IPlotTagFieldsFilter {
        id: None,
        created_at: None,
        updated_at: None,
        plot_id: Some(plot_id.to_string()),
        tag: None,
    };
    let result_query = plot_tag::find_many(
        exec,
        &IPlotTagFindMany {
            filter: Some(filter),
        },
    );
    let result = result_query?;
    let mut tags = result
        .results
        .into_iter()
        .map(|row| row.tag)
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    Ok(tags)
}

fn load_farm_members<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
) -> Result<Vec<FarmMember>, RadrootsTangleEventsError> {
    let filter = IFarmMemberFieldsFilter {
        id: None,
        created_at: None,
        updated_at: None,
        farm_id: Some(farm_id.to_string()),
        member_pubkey: None,
        role: None,
    };
    let result_query = farm_member::find_many(
        exec,
        &IFarmMemberFindMany {
            filter: Some(filter),
        },
    );
    let result = result_query?;
    Ok(result.results)
}

fn role_pubkeys(members: &[FarmMember], role: &str) -> Vec<String> {
    let mut values = members
        .iter()
        .filter(|member| member.role == role)
        .map(|member| member.member_pubkey.clone())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn sorted_plot_ids(plots: &[Plot]) -> Vec<String> {
    let mut ids = plots
        .iter()
        .map(|plot| plot.d_tag.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn load_plots<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
) -> Result<Vec<Plot>, RadrootsTangleEventsError> {
    let filter = IPlotFieldsFilter {
        id: None,
        created_at: None,
        updated_at: None,
        d_tag: None,
        farm_id: Some(farm_id.to_string()),
        name: None,
        about: None,
        location_primary: None,
        location_city: None,
        location_region: None,
        location_country: None,
    };
    let result_query = plot::find_many(
        exec,
        &IPlotFindMany {
            filter: Some(filter),
        },
    );
    let result = result_query?;
    let mut plots = result.results;
    plots.sort_by(|a, b| a.d_tag.cmp(&b.d_tag));
    Ok(plots)
}

fn load_farm_location<E: SqlExecutor>(
    exec: &E,
    farm: &Farm,
) -> Result<Option<RadrootsFarmLocation>, RadrootsTangleEventsError> {
    let location = load_gcs_location_for_farm(exec, &farm.id)?;
    Ok(location.map(|gcs| RadrootsFarmLocation {
        primary: farm.location_primary.clone(),
        city: farm.location_city.clone(),
        region: farm.location_region.clone(),
        country: farm.location_country.clone(),
        gcs,
    }))
}

fn load_plot_location<E: SqlExecutor>(
    exec: &E,
    plot: &Plot,
) -> Result<Option<radroots_events::plot::RadrootsPlotLocation>, RadrootsTangleEventsError> {
    let location = load_gcs_location_for_plot(exec, &plot.id)?;
    Ok(
        location.map(|gcs| radroots_events::plot::RadrootsPlotLocation {
            primary: plot.location_primary.clone(),
            city: plot.location_city.clone(),
            region: plot.location_region.clone(),
            country: plot.location_country.clone(),
            gcs,
        }),
    )
}

fn load_gcs_location_for_farm<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
) -> Result<Option<RadrootsGcsLocation>, RadrootsTangleEventsError> {
    let primary = load_relation_by_role(exec, farm_id, ROLE_PRIMARY, RelationType::Farm)?;
    match primary {
        Some(gcs) => Ok(Some(gcs)),
        None => load_relation_by_role(exec, farm_id, "", RelationType::Farm),
    }
}

fn load_gcs_location_for_plot<E: SqlExecutor>(
    exec: &E,
    plot_id: &str,
) -> Result<Option<RadrootsGcsLocation>, RadrootsTangleEventsError> {
    let primary = load_relation_by_role(exec, plot_id, ROLE_PRIMARY, RelationType::Plot)?;
    match primary {
        Some(gcs) => Ok(Some(gcs)),
        None => load_relation_by_role(exec, plot_id, "", RelationType::Plot),
    }
}

enum RelationType {
    Farm,
    Plot,
}

fn load_relation_by_role<E: SqlExecutor>(
    exec: &E,
    id: &str,
    role: &str,
    relation: RelationType,
) -> Result<Option<RadrootsGcsLocation>, RadrootsTangleEventsError> {
    let mut rels = match relation {
        RelationType::Farm => {
            let filter = IFarmGcsLocationFieldsFilter {
                id: None,
                created_at: None,
                updated_at: None,
                farm_id: Some(id.to_string()),
                gcs_location_id: None,
                role: if role.is_empty() {
                    None
                } else {
                    Some(role.to_string())
                },
            };
            let result_query = farm_gcs_location::find_many(
                exec,
                &IFarmGcsLocationFindMany {
                    filter: Some(filter),
                },
            );
            let result = result_query?;
            result
                .results
                .into_iter()
                .map(RelationRow::Farm)
                .collect::<Vec<_>>()
        }
        RelationType::Plot => {
            let filter = IPlotGcsLocationFieldsFilter {
                id: None,
                created_at: None,
                updated_at: None,
                plot_id: Some(id.to_string()),
                gcs_location_id: None,
                role: if role.is_empty() {
                    None
                } else {
                    Some(role.to_string())
                },
            };
            let result_query = plot_gcs_location::find_many(
                exec,
                &IPlotGcsLocationFindMany {
                    filter: Some(filter),
                },
            );
            let result = result_query?;
            result
                .results
                .into_iter()
                .map(RelationRow::Plot)
                .collect::<Vec<_>>()
        }
    };

    if rels.is_empty() {
        return Ok(None);
    }

    rels.sort_by(|a, b| {
        let rank = location_role_rank(a.role()).cmp(&location_role_rank(b.role()));
        rank.then_with(|| a.gcs_location_id().cmp(b.gcs_location_id()))
    });
    let gcs_id = rels[0].gcs_location_id().to_string();
    let gcs_result = gcs_location::find_one(
        exec,
        &IGcsLocationFindOne::On(IGcsLocationFindOneArgs {
            on: GcsLocationQueryBindValues::Id { id: gcs_id },
        }),
    );
    let gcs = gcs_result?.result.ok_or_else(|| {
        RadrootsTangleEventsError::InvalidData("gcs_location not found".to_string())
    })?;
    Ok(Some(gcs_location_to_event(&gcs)?))
}

enum RelationRow {
    Farm(FarmGcsLocation),
    Plot(PlotGcsLocation),
}

impl RelationRow {
    fn gcs_location_id(&self) -> &str {
        match self {
            Self::Farm(row) => row.gcs_location_id.as_str(),
            Self::Plot(row) => row.gcs_location_id.as_str(),
        }
    }

    fn role(&self) -> &str {
        match self {
            Self::Farm(row) => row.role.as_str(),
            Self::Plot(row) => row.role.as_str(),
        }
    }
}

fn location_role_rank(role: &str) -> u8 {
    if role == ROLE_PRIMARY { 0 } else { 1 }
}

fn gcs_location_to_event(
    gcs: &GcsLocation,
) -> Result<RadrootsGcsLocation, RadrootsTangleEventsError> {
    let point = parse_point(&gcs.point, gcs.lat, gcs.lng);
    let polygon = parse_polygon(&gcs.polygon, gcs.lat, gcs.lng);
    Ok(RadrootsGcsLocation {
        lat: gcs.lat,
        lng: gcs.lng,
        geohash: gcs.geohash.clone(),
        point,
        polygon,
        accuracy: gcs.accuracy,
        altitude: gcs.altitude,
        tag_0: gcs.tag_0.clone(),
        label: gcs.label.clone(),
        area: gcs.area,
        elevation: gcs.elevation,
        soil: gcs.soil.clone(),
        climate: gcs.climate.clone(),
        gc_id: gcs.gc_id.clone(),
        gc_name: gcs.gc_name.clone(),
        gc_admin1_id: gcs.gc_admin1_id.clone(),
        gc_admin1_name: gcs.gc_admin1_name.clone(),
        gc_country_id: gcs.gc_country_id.clone(),
        gc_country_name: gcs.gc_country_name.clone(),
    })
}

fn parse_point(value: &str, lat: f64, lng: f64) -> RadrootsGeoJsonPoint {
    if !value.trim().is_empty() {
        if let Ok(parsed) = serde_json::from_str::<RadrootsGeoJsonPoint>(value) {
            return parsed;
        }
    }
    geojson_point_from_lat_lng(lat, lng)
}

fn parse_polygon(value: &str, lat: f64, lng: f64) -> RadrootsGeoJsonPolygon {
    if !value.trim().is_empty() {
        if let Ok(parsed) = serde_json::from_str::<RadrootsGeoJsonPolygon>(value) {
            if !parsed.coordinates.is_empty() && !parsed.coordinates[0].is_empty() {
                return parsed;
            }
        }
    }
    geojson_polygon_circle_wgs84(lat, lng, 100.0, 64)
}

fn load_profile<E: SqlExecutor>(
    exec: &E,
    pubkey: &str,
) -> Result<Option<radroots_tangle_db_schema::nostr_profile::NostrProfile>, RadrootsTangleEventsError>
{
    let result_query = nostr_profile::find_one(
        exec,
        &INostrProfileFindOne::On(INostrProfileFindOneArgs {
            on: NostrProfileQueryBindValues::PublicKey {
                public_key: pubkey.to_string(),
            },
        }),
    );
    let result = result_query?;
    Ok(result.result)
}

fn profile_event(
    pubkey: &str,
    profile: radroots_tangle_db_schema::nostr_profile::NostrProfile,
) -> Result<RadrootsTangleEventDraft, RadrootsTangleEventsError> {
    let profile_type = match profile.profile_type.as_str() {
        "individual" | "farmer" => Some(RadrootsProfileType::Individual),
        "farm" => Some(RadrootsProfileType::Farm),
        "coop" => Some(RadrootsProfileType::Coop),
        "any" => Some(RadrootsProfileType::Any),
        other => radroots_profile_type_from_tag_value(other),
    };
    let profile_event = RadrootsProfile {
        name: profile.name,
        display_name: profile.display_name,
        nip05: profile.nip05,
        about: profile.about,
        website: profile.website,
        picture: profile.picture,
        banner: profile.banner,
        lud06: profile.lud06,
        lud16: profile.lud16,
        bot: None,
    };
    let content = serialize_profile_content(&profile_event)?;
    let mut tags = Vec::new();
    if let Some(profile_type) = profile_type {
        let mut tag = Vec::with_capacity(2);
        tag.push(RADROOTS_PROFILE_TYPE_TAG_KEY.to_string());
        tag.push(radroots_profile_type_tag_value(profile_type).to_string());
        tags.push(tag);
    }
    Ok(RadrootsTangleEventDraft {
        kind: radroots_events::kinds::KIND_PROFILE,
        author: pubkey.to_string(),
        content,
        tags,
    })
}

fn serialize_profile_content(
    profile: &RadrootsProfile,
) -> Result<String, RadrootsTangleEventsError> {
    let mut obj = serde_json::Map::new();
    obj.insert("name".to_string(), Value::from(profile.name.clone()));
    if let Some(value) = profile.display_name.as_ref() {
        obj.insert("display_name".to_string(), Value::from(value.clone()));
    }
    if let Some(value) = profile.nip05.as_ref() {
        obj.insert("nip05".to_string(), Value::from(value.clone()));
    }
    if let Some(value) = profile.about.as_ref() {
        obj.insert("about".to_string(), Value::from(value.clone()));
    }
    if let Some(value) = profile.website.as_ref() {
        obj.insert("website".to_string(), Value::from(value.clone()));
    }
    if let Some(value) = profile.picture.as_ref() {
        obj.insert("picture".to_string(), Value::from(value.clone()));
    }
    if let Some(value) = profile.banner.as_ref() {
        obj.insert("banner".to_string(), Value::from(value.clone()));
    }
    if let Some(value) = profile.lud06.as_ref() {
        obj.insert("lud06".to_string(), Value::from(value.clone()));
    }
    if let Some(value) = profile.lud16.as_ref() {
        obj.insert("lud16".to_string(), Value::from(value.clone()));
    }
    canonical_json_string(&Value::Object(obj))
}

fn collect_member_pubkeys<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
) -> Result<Vec<String>, RadrootsTangleEventsError> {
    let members = load_farm_members(exec, farm_id)?;
    let mut pubkeys = members
        .into_iter()
        .map(|row| row.member_pubkey)
        .collect::<Vec<_>>();
    pubkeys.sort();
    pubkeys.dedup();
    Ok(pubkeys)
}

fn collect_profile_pubkeys<E: SqlExecutor>(
    exec: &E,
    farm: &Farm,
) -> Result<Vec<String>, RadrootsTangleEventsError> {
    let mut pubkeys = collect_member_pubkeys(exec, &farm.id)?;
    let claims = load_member_claims(exec, &farm.pubkey)?;
    pubkeys.extend(claims.into_iter().map(|claim| claim.member_pubkey));
    pubkeys.push(farm.pubkey.clone());
    Ok(pubkeys)
}

fn load_member_claims<E: SqlExecutor>(
    exec: &E,
    farm_pubkey: &str,
) -> Result<Vec<FarmMemberClaim>, RadrootsTangleEventsError> {
    let filter = IFarmMemberClaimFieldsFilter {
        id: None,
        created_at: None,
        updated_at: None,
        member_pubkey: None,
        farm_pubkey: Some(farm_pubkey.to_string()),
    };
    let result_query = farm_member_claim::find_many(
        exec,
        &IFarmMemberClaimFindMany {
            filter: Some(filter),
        },
    );
    let result = result_query?;
    Ok(result.results)
}

fn load_member_claims_for_member<E: SqlExecutor>(
    exec: &E,
    member_pubkey: &str,
) -> Result<Vec<FarmMemberClaim>, RadrootsTangleEventsError> {
    let filter = IFarmMemberClaimFieldsFilter {
        id: None,
        created_at: None,
        updated_at: None,
        member_pubkey: Some(member_pubkey.to_string()),
        farm_pubkey: None,
    };
    let result_query = farm_member_claim::find_many(
        exec,
        &IFarmMemberClaimFindMany {
            filter: Some(filter),
        },
    );
    let result = result_query?;
    Ok(result.results)
}

fn parts_to_draft(author: &str, parts: WireEventParts) -> RadrootsTangleEventDraft {
    RadrootsTangleEventDraft {
        kind: parts.kind,
        author: author.to_string(),
        content: parts.content,
        tags: parts.tags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_sql_core::SqliteExecutor;
    use radroots_tangle_db::{
        farm, farm_gcs_location, farm_member, farm_member_claim, farm_tag, gcs_location,
        migrations, nostr_profile, plot, plot_gcs_location, plot_tag,
    };
    use radroots_tangle_db_schema::farm::{IFarmFields, IFarmFieldsFilter, IFarmFindMany};
    use radroots_tangle_db_schema::farm_gcs_location::{
        IFarmGcsLocationFields, IFarmGcsLocationFindMany,
    };
    use radroots_tangle_db_schema::farm_member::IFarmMemberFields;
    use radroots_tangle_db_schema::farm_member_claim::IFarmMemberClaimFields;
    use radroots_tangle_db_schema::farm_tag::IFarmTagFields;
    use radroots_tangle_db_schema::gcs_location::IGcsLocationFields;
    use radroots_tangle_db_schema::nostr_profile::INostrProfileFields;
    use radroots_tangle_db_schema::plot::{IPlotFields, IPlotFindMany};
    use radroots_tangle_db_schema::plot_gcs_location::{
        IPlotGcsLocationFields, IPlotGcsLocationFindMany,
    };
    use radroots_tangle_db_schema::plot_tag::IPlotTagFields;

    fn seed(exec: &SqliteExecutor) -> (Farm, Plot, Plot) {
        migrations::run_all_up(exec).expect("migrations");
        let farm = farm::create(
            exec,
            &IFarmFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
                pubkey: "f".repeat(64),
                name: "farm".to_string(),
                about: Some("about".to_string()),
                website: Some("https://farm.example.com".to_string()),
                picture: Some("https://farm.example.com/p.png".to_string()),
                banner: Some("https://farm.example.com/b.png".to_string()),
                location_primary: Some("primary".to_string()),
                location_city: Some("city".to_string()),
                location_region: Some("region".to_string()),
                location_country: Some("country".to_string()),
            },
        )
        .expect("farm")
        .result;

        let gcs_primary = gcs_location::create(
            exec,
            &IGcsLocationFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
                lat: 10.0,
                lng: 20.0,
                geohash: "s0".to_string(),
                point: "{\"type\":\"Point\",\"coordinates\":[20.0,10.0]}".to_string(),
                polygon:
                    "{\"type\":\"Polygon\",\"coordinates\":[[[20.0,10.0],[20.1,10.1],[19.9,10.1],[20.0,10.0]]]}".to_string(),
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
        )
        .expect("gcs primary")
        .result;
        let gcs_secondary = gcs_location::create(
            exec,
            &IGcsLocationFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
                lat: 11.0,
                lng: 21.0,
                geohash: "s1".to_string(),
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
        )
        .expect("gcs secondary")
        .result;

        let _ = farm_gcs_location::create(
            exec,
            &IFarmGcsLocationFields {
                farm_id: farm.id.clone(),
                gcs_location_id: gcs_secondary.id.clone(),
                role: "".to_string(),
            },
        )
        .expect("farm gcs secondary");
        let _ = farm_gcs_location::create(
            exec,
            &IFarmGcsLocationFields {
                farm_id: farm.id.clone(),
                gcs_location_id: gcs_primary.id.clone(),
                role: "primary".to_string(),
            },
        )
        .expect("farm gcs primary");

        let plot_primary = plot::create(
            exec,
            &IPlotFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
                farm_id: farm.id.clone(),
                name: "plot-primary".to_string(),
                about: Some("plot about".to_string()),
                location_primary: Some("plot primary".to_string()),
                location_city: Some("plot city".to_string()),
                location_region: Some("plot region".to_string()),
                location_country: Some("plot country".to_string()),
            },
        )
        .expect("plot primary")
        .result;
        let plot_secondary = plot::create(
            exec,
            &IPlotFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
                farm_id: farm.id.clone(),
                name: "plot-secondary".to_string(),
                about: Some("plot secondary about".to_string()),
                location_primary: Some("plot secondary primary".to_string()),
                location_city: None,
                location_region: None,
                location_country: None,
            },
        )
        .expect("plot secondary")
        .result;

        let _ = plot_gcs_location::create(
            exec,
            &IPlotGcsLocationFields {
                plot_id: plot_primary.id.clone(),
                gcs_location_id: gcs_secondary.id.clone(),
                role: "secondary".to_string(),
            },
        )
        .expect("plot primary secondary relation");
        let _ = plot_gcs_location::create(
            exec,
            &IPlotGcsLocationFields {
                plot_id: plot_primary.id.clone(),
                gcs_location_id: gcs_primary.id.clone(),
                role: "primary".to_string(),
            },
        )
        .expect("plot primary relation");
        let _ = plot_gcs_location::create(
            exec,
            &IPlotGcsLocationFields {
                plot_id: plot_secondary.id.clone(),
                gcs_location_id: gcs_secondary.id.clone(),
                role: "secondary".to_string(),
            },
        )
        .expect("plot secondary relation");

        let _ = farm_tag::create(
            exec,
            &IFarmTagFields {
                farm_id: farm.id.clone(),
                tag: "coffee".to_string(),
            },
        )
        .expect("farm tag");
        let _ = plot_tag::create(
            exec,
            &IPlotTagFields {
                plot_id: plot_primary.id.clone(),
                tag: "orchard".to_string(),
            },
        )
        .expect("plot tag");

        let _ = farm_member::create(
            exec,
            &IFarmMemberFields {
                farm_id: farm.id.clone(),
                member_pubkey: "m".repeat(64),
                role: "member".to_string(),
            },
        )
        .expect("member");
        let _ = farm_member::create(
            exec,
            &IFarmMemberFields {
                farm_id: farm.id.clone(),
                member_pubkey: "o".repeat(64),
                role: "owner".to_string(),
            },
        )
        .expect("owner");
        let _ = farm_member::create(
            exec,
            &IFarmMemberFields {
                farm_id: farm.id.clone(),
                member_pubkey: "u".repeat(64),
                role: "worker".to_string(),
            },
        )
        .expect("worker");
        let _ = farm_member::create(
            exec,
            &IFarmMemberFields {
                farm_id: farm.id.clone(),
                member_pubkey: "x".repeat(64),
                role: "member".to_string(),
            },
        )
        .expect("member no profile");

        let _ = farm_member_claim::create(
            exec,
            &IFarmMemberClaimFields {
                member_pubkey: "m".repeat(64),
                farm_pubkey: farm.pubkey.clone(),
            },
        )
        .expect("claim member");
        let _ = farm_member_claim::create(
            exec,
            &IFarmMemberClaimFields {
                member_pubkey: "x".repeat(64),
                farm_pubkey: farm.pubkey.clone(),
            },
        )
        .expect("claim member no profile");

        let _ = nostr_profile::create(
            exec,
            &INostrProfileFields {
                public_key: farm.pubkey.clone(),
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
        )
        .expect("farm profile");
        let _ = nostr_profile::create(
            exec,
            &INostrProfileFields {
                public_key: "m".repeat(64),
                profile_type: "legacy".to_string(),
                name: "member profile".to_string(),
                display_name: Some("member".to_string()),
                about: Some("about".to_string()),
                website: Some("https://member.example.com".to_string()),
                picture: Some("https://member.example.com/p.png".to_string()),
                banner: Some("https://member.example.com/b.png".to_string()),
                nip05: Some("member@example.com".to_string()),
                lud06: Some("lud06".to_string()),
                lud16: Some("lud16".to_string()),
            },
        )
        .expect("member profile");

        (farm, plot_primary, plot_secondary)
    }

    #[test]
    fn emit_paths_cover_private_and_public_helpers() {
        let exec = SqliteExecutor::open_memory().expect("db");
        let (farm_row, plot_primary, plot_secondary) = seed(&exec);

        let by_id = resolve_farm(
            &exec,
            &RadrootsTangleFarmSelector {
                id: Some(farm_row.id.clone()),
                d_tag: None,
                pubkey: None,
            },
        )
        .expect("resolve by id");
        assert_eq!(by_id.id, farm_row.id);

        assert!(
            resolve_farm(
                &exec,
                &RadrootsTangleFarmSelector {
                    id: Some("00000000-0000-0000-0000-000000000000".to_string()),
                    d_tag: None,
                    pubkey: None,
                },
            )
            .is_err()
        );
        assert!(
            resolve_farm(
                &exec,
                &RadrootsTangleFarmSelector {
                    id: None,
                    d_tag: None,
                    pubkey: None,
                },
            )
            .is_err()
        );

        let _ = farm::create(
            &exec,
            &IFarmFields {
                d_tag: farm_row.d_tag.clone(),
                pubkey: farm_row.pubkey.clone(),
                name: "duplicate".to_string(),
                about: None,
                website: None,
                picture: None,
                banner: None,
                location_primary: None,
                location_city: None,
                location_region: None,
                location_country: None,
            },
        )
        .expect("duplicate farm");
        assert!(
            resolve_farm(
                &exec,
                &RadrootsTangleFarmSelector {
                    id: None,
                    d_tag: Some(farm_row.d_tag.clone()),
                    pubkey: Some(farm_row.pubkey.clone()),
                },
            )
            .is_err()
        );

        let tags = collect_farm_tags(&exec, &farm_row.id).expect("farm tags");
        assert_eq!(tags, vec!["coffee".to_string()]);
        let plot_tags = collect_plot_tags(&exec, &plot_primary.id).expect("plot tags");
        assert_eq!(plot_tags, vec!["orchard".to_string()]);

        let members = load_farm_members(&exec, &farm_row.id).expect("members");
        assert_eq!(role_pubkeys(&members, ROLE_MEMBER).len(), 2);
        assert_eq!(role_pubkeys(&members, ROLE_OWNER).len(), 1);
        assert_eq!(role_pubkeys(&members, ROLE_WORKER).len(), 1);
        let plots = load_plots(&exec, &farm_row.id).expect("plots");
        assert_eq!(sorted_plot_ids(&plots).len(), 2);

        let farm_location = load_farm_location(&exec, &farm_row).expect("farm location");
        assert!(farm_location.is_some());
        let plot_location_primary = load_plot_location(&exec, &plot_primary).expect("plot primary");
        assert!(plot_location_primary.is_some());
        let plot_location_secondary =
            load_plot_location(&exec, &plot_secondary).expect("plot secondary");
        assert!(plot_location_secondary.is_some());

        assert!(
            load_relation_by_role(&exec, &farm_row.id, "primary", RelationType::Farm)
                .expect("farm primary")
                .is_some()
        );
        assert!(
            load_relation_by_role(&exec, &farm_row.id, "", RelationType::Farm)
                .expect("farm fallback")
                .is_some()
        );
        assert!(
            load_relation_by_role(&exec, &plot_secondary.id, "", RelationType::Plot)
                .expect("plot fallback")
                .is_some()
        );

        let mut farm_rel =
            farm_gcs_location::find_many(&exec, &IFarmGcsLocationFindMany { filter: None })
                .expect("farm rels")
                .results;
        let mut plot_rel =
            plot_gcs_location::find_many(&exec, &IPlotGcsLocationFindMany { filter: None })
                .expect("plot rels")
                .results;
        let farm_row_role = RelationRow::Farm(farm_rel.remove(0)).role().to_string();
        let plot_row_role = RelationRow::Plot(plot_rel.remove(0)).role().to_string();
        let _ = farm_row_role;
        let _ = plot_row_role;
        assert_eq!(location_role_rank(ROLE_PRIMARY), 0);
        assert_eq!(location_role_rank("secondary"), 1);

        let point_valid = parse_point("{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}", 3.0, 4.0);
        assert_eq!(point_valid.coordinates, [1.0, 2.0]);
        let point_invalid = parse_point("{", 3.0, 4.0);
        assert_eq!(point_invalid.coordinates, [4.0, 3.0]);
        let point_empty = parse_point("", 3.0, 4.0);
        assert_eq!(point_empty.coordinates, [4.0, 3.0]);

        let polygon_valid = parse_polygon(
            "{\"type\":\"Polygon\",\"coordinates\":[[[1.0,2.0],[1.1,2.1],[1.0,2.0]]]}",
            3.0,
            4.0,
        );
        assert!(!polygon_valid.coordinates[0].is_empty());
        let polygon_empty_outer =
            parse_polygon("{\"type\":\"Polygon\",\"coordinates\":[]}", 3.0, 4.0);
        assert!(!polygon_empty_outer.coordinates[0].is_empty());
        let polygon_empty_inner =
            parse_polygon("{\"type\":\"Polygon\",\"coordinates\":[[]]}", 3.0, 4.0);
        assert!(!polygon_empty_inner.coordinates[0].is_empty());
        let polygon_invalid = parse_polygon("{", 3.0, 4.0);
        assert!(!polygon_invalid.coordinates[0].is_empty());
        let polygon_blank = parse_polygon("", 3.0, 4.0);
        assert!(!polygon_blank.coordinates[0].is_empty());

        assert!(
            load_profile(&exec, &farm_row.pubkey)
                .expect("farm profile")
                .is_some()
        );
        assert!(
            load_profile(&exec, &"z".repeat(64))
                .expect("missing profile")
                .is_none()
        );

        let profile_event_farm = profile_event(
            &farm_row.pubkey,
            radroots_tangle_db_schema::nostr_profile::NostrProfile {
                id: "00000000-0000-0000-0000-000000000001".to_string(),
                created_at: "2024-01-01T00:00:00.000Z".to_string(),
                updated_at: "2024-01-01T00:00:00.000Z".to_string(),
                public_key: farm_row.pubkey.clone(),
                profile_type: "farm".to_string(),
                name: "farm".to_string(),
                display_name: None,
                about: None,
                website: None,
                picture: None,
                banner: None,
                nip05: None,
                lud06: None,
                lud16: None,
            },
        )
        .expect("profile farm");
        assert!(!profile_event_farm.tags.is_empty());
        let profile_event_unknown = profile_event(
            &"m".repeat(64),
            radroots_tangle_db_schema::nostr_profile::NostrProfile {
                id: "00000000-0000-0000-0000-000000000002".to_string(),
                created_at: "2024-01-01T00:00:00.000Z".to_string(),
                updated_at: "2024-01-01T00:00:00.000Z".to_string(),
                public_key: "m".repeat(64),
                profile_type: "legacy".to_string(),
                name: "legacy".to_string(),
                display_name: None,
                about: None,
                website: None,
                picture: None,
                banner: None,
                nip05: None,
                lud06: None,
                lud16: None,
            },
        )
        .expect("profile legacy");
        assert!(profile_event_unknown.tags.is_empty());

        let profile_content = serialize_profile_content(&RadrootsProfile {
            name: "name".to_string(),
            display_name: Some("display".to_string()),
            nip05: Some("nip05".to_string()),
            about: Some("about".to_string()),
            website: Some("website".to_string()),
            picture: Some("picture".to_string()),
            banner: Some("banner".to_string()),
            lud06: Some("lud06".to_string()),
            lud16: Some("lud16".to_string()),
            bot: None,
        })
        .expect("serialize profile");
        assert!(profile_content.contains("\"name\":\"name\""));

        let member_pubkeys = collect_member_pubkeys(&exec, &farm_row.id).expect("member pubkeys");
        assert!(!member_pubkeys.is_empty());
        let profile_pubkeys = collect_profile_pubkeys(&exec, &farm_row).expect("profile pubkeys");
        assert!(!profile_pubkeys.is_empty());
        let claims = load_member_claims(&exec, &farm_row.pubkey).expect("claims");
        assert!(!claims.is_empty());
        let member_claims =
            load_member_claims_for_member(&exec, &"m".repeat(64)).expect("claims by member");
        assert!(!member_claims.is_empty());

        let profile_events = radroots_tangle_profile_events(&exec, &farm_row).expect("profiles");
        assert!(!profile_events.is_empty());
        let farm_event = radroots_tangle_farm_event(&exec, &farm_row).expect("farm event");
        assert_eq!(farm_event.kind, KIND_FARM);
        let plot_events = radroots_tangle_plot_events(&exec, &farm_row).expect("plot events");
        assert_eq!(plot_events.len(), 2);
        let list_sets = radroots_tangle_list_set_events(&exec, &farm_row).expect("list sets");
        assert_eq!(list_sets.len(), 4);
        let membership_claims =
            radroots_tangle_membership_claim_events(&exec, &farm_row.pubkey).expect("membership");
        assert!(!membership_claims.is_empty());
        let bundle = radroots_tangle_sync_all_with_options(
            &exec,
            &RadrootsTangleFarmSelector {
                id: Some(farm_row.id.clone()),
                d_tag: None,
                pubkey: None,
            },
            Some(&RadrootsTangleSyncOptions {
                include_profiles: Some(true),
                include_list_sets: Some(true),
                include_membership_claims: Some(true),
            }),
        )
        .expect("sync all");
        assert!(!bundle.events.is_empty());

        let _ = exec.exec("PRAGMA foreign_keys = OFF", "[]");
        let _ = plot_gcs_location::create(
            &exec,
            &IPlotGcsLocationFields {
                plot_id: plot_secondary.id.clone(),
                gcs_location_id: "00000000-0000-0000-0000-000000000000".to_string(),
                role: "".to_string(),
            },
        );
        assert!(load_relation_by_role(&exec, &plot_secondary.id, "", RelationType::Plot).is_err());

        let by_pair = farm::find_many(
            &exec,
            &IFarmFindMany {
                filter: Some(IFarmFieldsFilter {
                    id: None,
                    created_at: None,
                    updated_at: None,
                    d_tag: Some("AAAAAAAAAAAAAAAAAAAAAA".to_string()),
                    pubkey: Some("f".repeat(64)),
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
        )
        .expect("by pair");
        assert!(!by_pair.results.is_empty());

        let plots_lookup = plot::find_many(
            &exec,
            &IPlotFindMany {
                filter: Some(IPlotFieldsFilter {
                    id: None,
                    created_at: None,
                    updated_at: None,
                    d_tag: None,
                    farm_id: Some(farm_row.id),
                    name: None,
                    about: None,
                    location_primary: None,
                    location_city: None,
                    location_region: None,
                    location_country: None,
                }),
            },
        )
        .expect("plots lookup");
        assert_eq!(plots_lookup.results.len(), 2);
    }
}
