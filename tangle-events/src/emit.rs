#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::{String, ToString}, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use radroots_events::farm::{
    RadrootsFarm,
    RadrootsFarmLocation,
    RadrootsFarmRef,
    RadrootsGcsLocation,
    RadrootsGeoJsonPoint,
    RadrootsGeoJsonPolygon,
};
use radroots_events::kinds::{KIND_FARM, KIND_LIST_SET_GENERIC, KIND_PLOT};
use radroots_events::plot::RadrootsPlot;
use radroots_events::profile::{
    radroots_profile_type_from_tag_value,
    radroots_profile_type_tag_value,
    RadrootsProfile,
    RadrootsProfileType,
    RADROOTS_PROFILE_TYPE_TAG_KEY,
};
use radroots_events_codec::farm::encode as farm_encode;
use radroots_events_codec::farm::list_sets as farm_list_sets;
use radroots_events_codec::list_set::encode as list_set_encode;
use radroots_events_codec::plot::encode as plot_encode;
use radroots_events_codec::wire::WireEventParts;
use radroots_sql_core::SqlExecutor;
use radroots_tangle_db_schema::farm::{
    Farm,
    IFarmFindMany,
    IFarmFindOne,
    IFarmFindOneArgs,
    IFarmFieldsFilter,
};
use radroots_tangle_db_schema::farm_gcs_location::{
    FarmGcsLocation,
    IFarmGcsLocationFindMany,
    IFarmGcsLocationFieldsFilter,
};
use radroots_tangle_db_schema::farm_member::{
    FarmMember,
    IFarmMemberFindMany,
    IFarmMemberFieldsFilter,
};
use radroots_tangle_db_schema::farm_member_claim::{
    FarmMemberClaim,
    IFarmMemberClaimFindMany,
    IFarmMemberClaimFieldsFilter,
};
use radroots_tangle_db_schema::farm_tag::{IFarmTagFindMany, IFarmTagFieldsFilter};
use radroots_tangle_db_schema::gcs_location::{
    GcsLocation,
    IGcsLocationFindOne,
    IGcsLocationFindOneArgs,
    GcsLocationQueryBindValues,
};
use radroots_tangle_db_schema::nostr_profile::{
    INostrProfileFindOne,
    INostrProfileFindOneArgs,
    NostrProfileQueryBindValues,
};
use radroots_tangle_db_schema::plot::{Plot, IPlotFindMany, IPlotFieldsFilter};
use radroots_tangle_db_schema::plot_gcs_location::{
    PlotGcsLocation,
    IPlotGcsLocationFindMany,
    IPlotGcsLocationFieldsFilter,
};
use radroots_tangle_db_schema::plot_tag::{IPlotTagFindMany, IPlotTagFieldsFilter};
use radroots_tangle_db::{
    farm,
    farm_gcs_location,
    farm_member,
    farm_member_claim,
    farm_tag,
    gcs_location,
    nostr_profile,
    plot,
    plot_gcs_location,
    plot_tag,
};
use serde_json::Value;

use crate::error::RadrootsTangleEventsError;
use crate::canonical::canonical_json_string;
use crate::geo::{geojson_point_from_lat_lng, geojson_polygon_circle_wgs84};
use crate::types::{
    RADROOTS_TANGLE_TRANSFER_VERSION,
    RadrootsTangleEventDraft,
    RadrootsTangleFarmSelector,
    RadrootsTangleSyncBundle,
    RadrootsTangleSyncOptions,
    RadrootsTangleSyncRequest,
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
    let include_profiles = options
        .and_then(|opt| opt.include_profiles)
        .unwrap_or(true);
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

    let members_list = farm_list_sets::farm_members_list_set(
        &farm.d_tag,
        role_pubkeys(&members, ROLE_MEMBER),
    )?;
    let owners_list = farm_list_sets::farm_owners_list_set(
        &farm.d_tag,
        role_pubkeys(&members, ROLE_OWNER),
    )?;
    let workers_list = farm_list_sets::farm_workers_list_set(
        &farm.d_tag,
        role_pubkeys(&members, ROLE_WORKER),
    )?;

    let plot_ids = sorted_plot_ids(&plots);
    let plots_list = farm_list_sets::farm_plots_list_set(
        &farm.d_tag,
        &farm.pubkey,
        plot_ids,
    )?;

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
        let result = farm::find_one(
            exec,
            &IFarmFindOne::On(IFarmFindOneArgs {
                on: radroots_tangle_db_schema::farm::FarmQueryBindValues::Id { id: id.clone() },
            }),
        )?;
        return result
            .result
            .ok_or_else(|| RadrootsTangleEventsError::InvalidSelector(format!("farm not found: {id}")));
    }

    let d_tag = selector.d_tag.as_ref().map(|v| v.trim()).filter(|v| !v.is_empty());
    let pubkey = selector.pubkey.as_ref().map(|v| v.trim()).filter(|v| !v.is_empty());

    let (d_tag, pubkey) = match (d_tag, pubkey) {
        (Some(d_tag), Some(pubkey)) => (d_tag, pubkey),
        _ => {
            return Err(RadrootsTangleEventsError::InvalidSelector(
                "farm selector requires id or (d_tag + pubkey)".to_string(),
            ))
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
    let result = farm::find_many(exec, &IFarmFindMany { filter: Some(filter) })?;
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
    let result = farm_tag::find_many(exec, &IFarmTagFindMany { filter: Some(filter) })?;
    let mut tags = result.results.into_iter().map(|row| row.tag).collect::<Vec<_>>();
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
    let result = plot_tag::find_many(exec, &IPlotTagFindMany { filter: Some(filter) })?;
    let mut tags = result.results.into_iter().map(|row| row.tag).collect::<Vec<_>>();
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
    let result = farm_member::find_many(exec, &IFarmMemberFindMany { filter: Some(filter) })?;
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
    let mut ids = plots.iter().map(|plot| plot.d_tag.clone()).collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn load_plots<E: SqlExecutor>(exec: &E, farm_id: &str) -> Result<Vec<Plot>, RadrootsTangleEventsError> {
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
    let result = plot::find_many(exec, &IPlotFindMany { filter: Some(filter) })?;
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
    Ok(location.map(|gcs| radroots_events::plot::RadrootsPlotLocation {
        primary: plot.location_primary.clone(),
        city: plot.location_city.clone(),
        region: plot.location_region.clone(),
        country: plot.location_country.clone(),
        gcs,
    }))
}

fn load_gcs_location_for_farm<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
) -> Result<Option<RadrootsGcsLocation>, RadrootsTangleEventsError> {
    let primary = load_relation_by_role(
        exec,
        farm_id,
        ROLE_PRIMARY,
        RelationType::Farm,
    )?;
    match primary {
        Some(gcs) => Ok(Some(gcs)),
        None => load_relation_by_role(exec, farm_id, "", RelationType::Farm),
    }
}

fn load_gcs_location_for_plot<E: SqlExecutor>(
    exec: &E,
    plot_id: &str,
) -> Result<Option<RadrootsGcsLocation>, RadrootsTangleEventsError> {
    let primary = load_relation_by_role(
        exec,
        plot_id,
        ROLE_PRIMARY,
        RelationType::Plot,
    )?;
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
                role: if role.is_empty() { None } else { Some(role.to_string()) },
            };
            let result = farm_gcs_location::find_many(
                exec,
                &IFarmGcsLocationFindMany { filter: Some(filter) },
            )?;
            result.results.into_iter().map(RelationRow::Farm).collect::<Vec<_>>()
        }
        RelationType::Plot => {
            let filter = IPlotGcsLocationFieldsFilter {
                id: None,
                created_at: None,
                updated_at: None,
                plot_id: Some(id.to_string()),
                gcs_location_id: None,
                role: if role.is_empty() { None } else { Some(role.to_string()) },
            };
            let result = plot_gcs_location::find_many(
                exec,
                &IPlotGcsLocationFindMany { filter: Some(filter) },
            )?;
            result.results.into_iter().map(RelationRow::Plot).collect::<Vec<_>>()
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
    let gcs = gcs_location::find_one(
        exec,
        &IGcsLocationFindOne::On(IGcsLocationFindOneArgs {
            on: GcsLocationQueryBindValues::Id { id: gcs_id },
        }),
    )?
    .result
    .ok_or_else(|| RadrootsTangleEventsError::InvalidData("gcs_location not found".to_string()))?;
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
    if role == ROLE_PRIMARY {
        0
    } else {
        1
    }
}

fn gcs_location_to_event(gcs: &GcsLocation) -> Result<RadrootsGcsLocation, RadrootsTangleEventsError> {
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
) -> Result<Option<radroots_tangle_db_schema::nostr_profile::NostrProfile>, RadrootsTangleEventsError> {
    let result = nostr_profile::find_one(
        exec,
        &INostrProfileFindOne::On(INostrProfileFindOneArgs {
            on: NostrProfileQueryBindValues::PublicKey {
                public_key: pubkey.to_string(),
            },
        }),
    )?;
    Ok(result.result)
}

fn profile_event(
    pubkey: &str,
    profile: radroots_tangle_db_schema::nostr_profile::NostrProfile,
) -> Result<RadrootsTangleEventDraft, RadrootsTangleEventsError> {
    let profile_type = match profile.profile_type.as_str() {
        "individual" | "farmer" => Some(RadrootsProfileType::Individual),
        "farm" => Some(RadrootsProfileType::Farm),
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

fn serialize_profile_content(profile: &RadrootsProfile) -> Result<String, RadrootsTangleEventsError> {
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
    let mut pubkeys = members.into_iter().map(|row| row.member_pubkey).collect::<Vec<_>>();
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
    let result = farm_member_claim::find_many(exec, &IFarmMemberClaimFindMany { filter: Some(filter) })?;
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
    let result = farm_member_claim::find_many(exec, &IFarmMemberClaimFindMany { filter: Some(filter) })?;
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
