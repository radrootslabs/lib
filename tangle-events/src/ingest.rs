#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

#[cfg(feature = "std")]
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
#[cfg(feature = "std")]
use base64::Engine;

use radroots_events::kinds::{
    is_nip51_list_set_kind,
    KIND_FARM,
    KIND_PLOT,
    KIND_PROFILE,
};
use radroots_events::RadrootsNostrEvent;
use radroots_events_codec::farm::decode as farm_decode;
use radroots_events_codec::list_set::decode as list_set_decode;
use radroots_events_codec::plot::decode as plot_decode;
use radroots_events_codec::profile::decode as profile_decode;
use radroots_sql_core::SqlExecutor;
use radroots_sql_core::error::SqlError;
use radroots_tangle_db_schema::farm::{
    FarmQueryBindValues,
    IFarmFields,
    IFarmFieldsFilter,
    IFarmFindMany,
    IFarmUpdate,
    IFarmFieldsPartial,
};
use radroots_tangle_db_schema::farm_gcs_location::{
    IFarmGcsLocationFields,
    IFarmGcsLocationFindMany,
    IFarmGcsLocationFieldsFilter,
    IFarmGcsLocationDelete,
    IFarmGcsLocationFindOneArgs,
    FarmGcsLocationQueryBindValues,
};
use radroots_tangle_db_schema::farm_member::{
    IFarmMemberFields,
    IFarmMemberFindMany,
    IFarmMemberFieldsFilter,
    IFarmMemberDelete,
    IFarmMemberFindOneArgs,
    FarmMemberQueryBindValues,
};
use radroots_tangle_db_schema::farm_member_claim::{
    IFarmMemberClaimFields,
    IFarmMemberClaimFindMany,
    IFarmMemberClaimFieldsFilter,
    IFarmMemberClaimDelete,
    IFarmMemberClaimFindOneArgs,
    FarmMemberClaimQueryBindValues,
};
use radroots_tangle_db_schema::farm_tag::{
    IFarmTagFields,
    IFarmTagFindMany,
    IFarmTagFieldsFilter,
    IFarmTagDelete,
    IFarmTagFindOneArgs,
    FarmTagQueryBindValues,
};
use radroots_tangle_db_schema::gcs_location::{
    IGcsLocationFields,
};
use radroots_tangle_db_schema::nostr_event_state::{
    INostrEventStateFields,
    INostrEventStateFindOne,
    INostrEventStateFindOneArgs,
    INostrEventStateUpdate,
    INostrEventStateFieldsPartial,
    NostrEventStateQueryBindValues,
};
use radroots_tangle_db_schema::nostr_profile::{
    INostrProfileFields,
    INostrProfileFindOne,
    INostrProfileFindOneArgs,
    INostrProfileUpdate,
    INostrProfileFieldsPartial,
    NostrProfileQueryBindValues,
};
use radroots_tangle_db_schema::plot::{
    IPlotFields,
    IPlotFieldsFilter,
    IPlotFindMany,
    IPlotUpdate,
    PlotQueryBindValues,
    IPlotFieldsPartial,
};
use radroots_tangle_db_schema::plot_gcs_location::{
    IPlotGcsLocationFields,
    IPlotGcsLocationFindMany,
    IPlotGcsLocationFieldsFilter,
    IPlotGcsLocationDelete,
    IPlotGcsLocationFindOneArgs,
    PlotGcsLocationQueryBindValues,
};
use radroots_tangle_db_schema::plot_tag::{
    IPlotTagFields,
    IPlotTagFindMany,
    IPlotTagFieldsFilter,
    IPlotTagDelete,
    IPlotTagFindOneArgs,
    PlotTagQueryBindValues,
};
use radroots_tangle_db::{
    farm,
    farm_gcs_location,
    farm_member,
    farm_member_claim,
    farm_tag,
    gcs_location,
    nostr_event_state,
    nostr_profile,
    plot,
    plot_gcs_location,
    plot_tag,
};
use serde_json::Value;

use crate::error::RadrootsTangleEventsError;
use crate::event_state::{event_content_hash, event_state_key};
const ROLE_PRIMARY: &str = "primary";
const ROLE_MEMBER: &str = "member";
const ROLE_OWNER: &str = "owner";
const ROLE_WORKER: &str = "worker";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTangleIngestOutcome {
    Applied,
    Skipped,
}

pub trait RadrootsTangleIdFactory {
    fn new_d_tag(&self) -> String;
}

#[cfg(feature = "std")]
pub struct RadrootsTangleDefaultIdFactory;

#[cfg(feature = "std")]
impl RadrootsTangleIdFactory for RadrootsTangleDefaultIdFactory {
    fn new_d_tag(&self) -> String {
        let uuid = uuid::Uuid::now_v7();
        let bytes = uuid.as_bytes();
        URL_SAFE_NO_PAD.encode(bytes)
    }
}

#[cfg(feature = "std")]
pub fn radroots_tangle_ingest_event<E: SqlExecutor>(
    exec: &E,
    event: &RadrootsNostrEvent,
) -> Result<RadrootsTangleIngestOutcome, RadrootsTangleEventsError> {
    radroots_tangle_ingest_event_with_factory(exec, event, &RadrootsTangleDefaultIdFactory)
}

pub fn radroots_tangle_ingest_event_with_factory<E: SqlExecutor, F: RadrootsTangleIdFactory>(
    exec: &E,
    event: &RadrootsNostrEvent,
    factory: &F,
) -> Result<RadrootsTangleIngestOutcome, RadrootsTangleEventsError> {
    exec.begin().map_err(|e| RadrootsTangleEventsError::from(radroots_types::types::IError::from(e)))?;

    let outcome = match ingest_event_inner(exec, event, factory) {
        Ok(outcome) => {
            exec.commit().map_err(|e| RadrootsTangleEventsError::from(radroots_types::types::IError::from(e)))?;
            Ok(outcome)
        }
        Err(err) => {
            let _ = exec.rollback();
            Err(err)
        }
    };

    outcome
}

fn ingest_event_inner<E: SqlExecutor, F: RadrootsTangleIdFactory>(
    exec: &E,
    event: &RadrootsNostrEvent,
    factory: &F,
) -> Result<RadrootsTangleIngestOutcome, RadrootsTangleEventsError> {
    match event.kind {
        KIND_PROFILE => ingest_profile_event(exec, event),
        KIND_FARM => ingest_farm_event(exec, event, factory),
        KIND_PLOT => ingest_plot_event(exec, event, factory),
        kind if is_nip51_list_set_kind(kind) => ingest_list_set_event(exec, event),
        _ => Err(RadrootsTangleEventsError::InvalidData(format!(
            "unsupported kind {}",
            event.kind
        ))),
    }
}

fn ingest_profile_event<E: SqlExecutor>(
    exec: &E,
    event: &RadrootsNostrEvent,
) -> Result<RadrootsTangleIngestOutcome, RadrootsTangleEventsError> {
    let metadata = profile_decode::metadata_from_event(
        event.id.clone(),
        event.author.clone(),
        event.created_at,
        event.kind,
        event.content.clone(),
        event.tags.clone(),
    )?;
    let profile_type = metadata
        .profile_type
        .ok_or_else(|| RadrootsTangleEventsError::InvalidData("profile_type required".to_string()))?;

    let d_tag = "".to_string();
    let decision = event_state_decision(exec, event, &d_tag)?;
    if !decision.apply {
        return Ok(RadrootsTangleIngestOutcome::Skipped);
    }

    let profile_type = match profile_type {
        radroots_events::profile::RadrootsProfileType::Individual => "individual",
        radroots_events::profile::RadrootsProfileType::Farm => "farm",
    };

    let existing = nostr_profile::find_one(
        exec,
        &INostrProfileFindOne::On(INostrProfileFindOneArgs {
            on: NostrProfileQueryBindValues::PublicKey {
                public_key: metadata.author.clone(),
            },
        }),
    )?
    .result;

    match existing {
        Some(profile) => {
            let fields = INostrProfileFieldsPartial {
                public_key: None,
                profile_type: Some(Value::from(profile_type)),
                name: Some(Value::from(metadata.profile.name)),
                display_name: to_value_opt(metadata.profile.display_name),
                about: to_value_opt(metadata.profile.about),
                website: to_value_opt(metadata.profile.website),
                picture: to_value_opt(metadata.profile.picture),
                banner: to_value_opt(metadata.profile.banner),
                nip05: to_value_opt(metadata.profile.nip05),
                lud06: to_value_opt(metadata.profile.lud06),
                lud16: to_value_opt(metadata.profile.lud16),
            };
            let _ = nostr_profile::update(
                exec,
                &INostrProfileUpdate {
                    on: NostrProfileQueryBindValues::Id { id: profile.id },
                    fields,
                },
            )?;
        }
        None => {
            let fields = INostrProfileFields {
                public_key: metadata.author.clone(),
                profile_type: profile_type.to_string(),
                name: metadata.profile.name,
                display_name: metadata.profile.display_name,
                about: metadata.profile.about,
                website: metadata.profile.website,
                picture: metadata.profile.picture,
                banner: metadata.profile.banner,
                nip05: metadata.profile.nip05,
                lud06: metadata.profile.lud06,
                lud16: metadata.profile.lud16,
            };
            let _ = nostr_profile::create(exec, &fields)?;
        }
    }

    radroots_tangle_ingest_event_state(exec, event, &d_tag, &decision.content_hash)?;
    Ok(RadrootsTangleIngestOutcome::Applied)
}

fn ingest_farm_event<E: SqlExecutor, F: RadrootsTangleIdFactory>(
    exec: &E,
    event: &RadrootsNostrEvent,
    factory: &F,
) -> Result<RadrootsTangleIngestOutcome, RadrootsTangleEventsError> {
    let farm = farm_decode::farm_from_event(event.kind, &event.tags, &event.content)?;
    let decision = event_state_decision(exec, event, &farm.d_tag)?;
    if !decision.apply {
        return Ok(RadrootsTangleIngestOutcome::Skipped);
    }

    let filter = IFarmFieldsFilter {
        id: None,
        created_at: None,
        updated_at: None,
        d_tag: Some(farm.d_tag.clone()),
        pubkey: Some(event.author.clone()),
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
    let existing = farm::find_many(exec, &IFarmFindMany { filter: Some(filter) })?;
    let location = farm.location.clone();
    let (location_primary, location_city, location_region, location_country) =
        unpack_farm_location_strings(location.as_ref());
    let farm_id = if let Some(row) = existing.results.get(0) {
        let fields = IFarmFieldsPartial {
            d_tag: Some(Value::from(farm.d_tag.clone())),
            pubkey: Some(Value::from(event.author.clone())),
            name: Some(Value::from(farm.name.clone())),
            about: to_value_opt(farm.about.clone()),
            website: to_value_opt(farm.website.clone()),
            picture: to_value_opt(farm.picture.clone()),
            banner: to_value_opt(farm.banner.clone()),
            location_primary: to_value_opt(location_primary),
            location_city: to_value_opt(location_city),
            location_region: to_value_opt(location_region),
            location_country: to_value_opt(location_country),
        };
        let _ = farm::update(
            exec,
            &IFarmUpdate {
                on: FarmQueryBindValues::Id { id: row.id.clone() },
                fields,
            },
        )?;
        row.id.clone()
    } else {
        let fields = IFarmFields {
            d_tag: farm.d_tag.clone(),
            pubkey: event.author.clone(),
            name: farm.name.clone(),
            about: farm.about.clone(),
            website: farm.website.clone(),
            picture: farm.picture.clone(),
            banner: farm.banner.clone(),
            location_primary,
            location_city,
            location_region,
            location_country,
        };
        farm::create(exec, &fields)?.result.id
    };

    upsert_farm_tags(exec, &farm_id, farm.tags)?;
    upsert_farm_location(exec, &farm_id, location, factory)?;

    radroots_tangle_ingest_event_state(exec, event, &farm.d_tag, &decision.content_hash)?;
    Ok(RadrootsTangleIngestOutcome::Applied)
}

fn ingest_plot_event<E: SqlExecutor, F: RadrootsTangleIdFactory>(
    exec: &E,
    event: &RadrootsNostrEvent,
    factory: &F,
) -> Result<RadrootsTangleIngestOutcome, RadrootsTangleEventsError> {
    let plot = plot_decode::plot_from_event(event.kind, &event.tags, &event.content)?;
    let decision = event_state_decision(exec, event, &plot.d_tag)?;
    if !decision.apply {
        return Ok(RadrootsTangleIngestOutcome::Skipped);
    }

    let farm = find_farm_by_ref(exec, &plot.farm.pubkey, &plot.farm.d_tag)?;
    let filter = IPlotFieldsFilter {
        id: None,
        created_at: None,
        updated_at: None,
        d_tag: Some(plot.d_tag.clone()),
        farm_id: Some(farm.id.clone()),
        name: None,
        about: None,
        location_primary: None,
        location_city: None,
        location_region: None,
        location_country: None,
    };
    let existing = plot::find_many(exec, &IPlotFindMany { filter: Some(filter) })?;
    let location = plot.location.clone();
    let (location_primary, location_city, location_region, location_country) =
        unpack_plot_location_strings(location.as_ref());
    let plot_id = if let Some(row) = existing.results.get(0) {
        let fields = IPlotFieldsPartial {
            d_tag: Some(Value::from(plot.d_tag.clone())),
            farm_id: Some(Value::from(farm.id.clone())),
            name: Some(Value::from(plot.name.clone())),
            about: to_value_opt(plot.about.clone()),
            location_primary: to_value_opt(location_primary),
            location_city: to_value_opt(location_city),
            location_region: to_value_opt(location_region),
            location_country: to_value_opt(location_country),
        };
        let _ = plot::update(
            exec,
            &IPlotUpdate {
                on: PlotQueryBindValues::Id { id: row.id.clone() },
                fields,
            },
        )?;
        row.id.clone()
    } else {
        let fields = IPlotFields {
            d_tag: plot.d_tag.clone(),
            farm_id: farm.id.clone(),
            name: plot.name.clone(),
            about: plot.about.clone(),
            location_primary,
            location_city,
            location_region,
            location_country,
        };
        plot::create(exec, &fields)?.result.id
    };

    upsert_plot_tags(exec, &plot_id, plot.tags)?;
    upsert_plot_location(exec, &plot_id, location, factory)?;

    radroots_tangle_ingest_event_state(exec, event, &plot.d_tag, &decision.content_hash)?;
    Ok(RadrootsTangleIngestOutcome::Applied)
}

fn ingest_list_set_event<E: SqlExecutor>(
    exec: &E,
    event: &RadrootsNostrEvent,
) -> Result<RadrootsTangleIngestOutcome, RadrootsTangleEventsError> {
    if event.kind != radroots_events::kinds::KIND_LIST_SET_GENERIC {
        return Ok(RadrootsTangleIngestOutcome::Skipped);
    }
    let list_set = list_set_decode::list_set_from_tags(event.kind, event.content.clone(), &event.tags)?;

    if list_set.title.is_some() || list_set.description.is_some() || list_set.image.is_some() {
        return Err(RadrootsTangleEventsError::InvalidData(
            "domain:farm list sets must omit metadata".to_string(),
        ));
    }
    if !list_set.content.is_empty() {
        return Err(RadrootsTangleEventsError::InvalidData(
            "domain:farm list sets must not include content".to_string(),
        ));
    }

    let d_tag = list_set.d_tag.clone();
    let decision = event_state_decision(exec, event, &d_tag)?;
    if !decision.apply {
        return Ok(RadrootsTangleIngestOutcome::Skipped);
    }

    if d_tag == "member_of.farms" {
        ensure_list_set_entries_tag(&list_set, "p", "member_of.farms")?;
        upsert_member_claims(exec, &event.author, &list_set)?;
        radroots_tangle_ingest_event_state(exec, event, &d_tag, &decision.content_hash)?;
        return Ok(RadrootsTangleIngestOutcome::Applied);
    }

    if let Some((farm_d_tag, role)) = parse_farm_list_set_d_tag(&d_tag) {
        if role == ListSetRole::Plots {
            ensure_list_set_entries_tag(&list_set, "a", "farm plots")?;
            radroots_tangle_ingest_event_state(exec, event, &d_tag, &decision.content_hash)?;
            return Ok(RadrootsTangleIngestOutcome::Applied);
        }
        ensure_list_set_entries_tag(&list_set, "p", "farm members")?;
        let farm = find_farm_by_ref(exec, &event.author, &farm_d_tag)?;
        upsert_farm_members(exec, &farm.id, role, &list_set)?;
        radroots_tangle_ingest_event_state(exec, event, &d_tag, &decision.content_hash)?;
        return Ok(RadrootsTangleIngestOutcome::Applied);
    }

    Err(RadrootsTangleEventsError::InvalidData(
        "unsupported list set d_tag".to_string(),
    ))
}

pub fn radroots_tangle_ingest_event_state<E: SqlExecutor>(
    exec: &E,
    event: &RadrootsNostrEvent,
    d_tag: &str,
    content_hash: &str,
) -> Result<(), RadrootsTangleEventsError> {
    let key = event_state_key(event.kind, &event.author, d_tag);
    let existing = nostr_event_state::find_one(
        exec,
        &INostrEventStateFindOne::On(INostrEventStateFindOneArgs {
            on: NostrEventStateQueryBindValues::Key { key: key.clone() },
        }),
    )?
    .result;

    match existing {
        Some(state) => {
            let fields = INostrEventStateFieldsPartial {
                key: None,
                kind: None,
                pubkey: None,
                d_tag: None,
                last_event_id: Some(Value::from(event.id.clone())),
                last_created_at: Some(Value::from(event.created_at)),
                content_hash: Some(Value::from(content_hash.to_string())),
            };
            let _ = nostr_event_state::update(
                exec,
                &INostrEventStateUpdate {
                    on: NostrEventStateQueryBindValues::Id { id: state.id },
                    fields,
                },
            )?;
        }
        None => {
            let fields = INostrEventStateFields {
                key,
                kind: event.kind,
                pubkey: event.author.clone(),
                d_tag: d_tag.to_string(),
                last_event_id: event.id.clone(),
                last_created_at: event.created_at,
                content_hash: content_hash.to_string(),
            };
            let _ = nostr_event_state::create(exec, &fields)?;
        }
    }

    Ok(())
}

fn event_state_decision<E: SqlExecutor>(
    exec: &E,
    event: &RadrootsNostrEvent,
    d_tag: &str,
) -> Result<EventStateDecision, RadrootsTangleEventsError> {
    let key = event_state_key(event.kind, &event.author, d_tag);
    let content_hash = event_content_hash(&event.content, &event.tags)?;
    let existing = nostr_event_state::find_one(
        exec,
        &INostrEventStateFindOne::On(INostrEventStateFindOneArgs {
            on: NostrEventStateQueryBindValues::Key { key },
        }),
    )?
    .result;

    if let Some(state) = existing {
        if event.created_at < state.last_created_at {
            return Ok(EventStateDecision { apply: false, content_hash });
        }
        if event.created_at == state.last_created_at && content_hash == state.content_hash {
            return Ok(EventStateDecision { apply: false, content_hash });
        }
    }

    Ok(EventStateDecision { apply: true, content_hash })
}

fn find_farm_by_ref<E: SqlExecutor>(
    exec: &E,
    pubkey: &str,
    d_tag: &str,
) -> Result<radroots_tangle_db_schema::farm::Farm, RadrootsTangleEventsError> {
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
    result
        .results
        .into_iter()
        .next()
        .ok_or_else(|| RadrootsTangleEventsError::InvalidData("farm not found".to_string()))
}

fn upsert_farm_tags<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
    tags: Option<Vec<String>>,
) -> Result<(), RadrootsTangleEventsError> {
    let existing = farm_tag::find_many(
        exec,
        &IFarmTagFindMany {
            filter: Some(IFarmTagFieldsFilter {
                id: None,
                created_at: None,
                updated_at: None,
                farm_id: Some(farm_id.to_string()),
                tag: None,
            }),
        },
    )?;
    for row in existing.results {
        match farm_tag::delete(
            exec,
            &IFarmTagDelete::On(IFarmTagFindOneArgs {
                on: FarmTagQueryBindValues::Id { id: row.id },
            }),
        ) {
            Ok(_) => {}
            Err(err) => {
                if !matches!(err.err, SqlError::NotFound(_)) {
                    return Err(err.into());
                }
            }
        }
    }

    let mut tags = tags.unwrap_or_default();
    tags.sort();
    tags.dedup();
    for tag in tags {
        if tag.trim().is_empty() {
            continue;
        }
        let fields = IFarmTagFields {
            farm_id: farm_id.to_string(),
            tag,
        };
        let _ = farm_tag::create(exec, &fields)?;
    }
    Ok(())
}

fn upsert_plot_tags<E: SqlExecutor>(
    exec: &E,
    plot_id: &str,
    tags: Option<Vec<String>>,
) -> Result<(), RadrootsTangleEventsError> {
    let existing = plot_tag::find_many(
        exec,
        &IPlotTagFindMany {
            filter: Some(IPlotTagFieldsFilter {
                id: None,
                created_at: None,
                updated_at: None,
                plot_id: Some(plot_id.to_string()),
                tag: None,
            }),
        },
    )?;
    for row in existing.results {
        match plot_tag::delete(
            exec,
            &IPlotTagDelete::On(IPlotTagFindOneArgs {
                on: PlotTagQueryBindValues::Id { id: row.id },
            }),
        ) {
            Ok(_) => {}
            Err(err) => {
                if !matches!(err.err, SqlError::NotFound(_)) {
                    return Err(err.into());
                }
            }
        }
    }

    let mut tags = tags.unwrap_or_default();
    tags.sort();
    tags.dedup();
    for tag in tags {
        if tag.trim().is_empty() {
            continue;
        }
        let fields = IPlotTagFields {
            plot_id: plot_id.to_string(),
            tag,
        };
        let _ = plot_tag::create(exec, &fields)?;
    }
    Ok(())
}

fn upsert_farm_location<E: SqlExecutor, F: RadrootsTangleIdFactory>(
    exec: &E,
    farm_id: &str,
    location: Option<radroots_events::farm::RadrootsFarmLocation>,
    factory: &F,
) -> Result<(), RadrootsTangleEventsError> {
    clear_farm_locations(exec, farm_id)?;
    if let Some(location) = location {
        let gcs_id = create_gcs_location(exec, location.gcs, factory)?;
        let fields = IFarmGcsLocationFields {
            farm_id: farm_id.to_string(),
            gcs_location_id: gcs_id,
            role: ROLE_PRIMARY.to_string(),
        };
        let _ = farm_gcs_location::create(exec, &fields)?;
    }
    Ok(())
}

fn upsert_plot_location<E: SqlExecutor, F: RadrootsTangleIdFactory>(
    exec: &E,
    plot_id: &str,
    location: Option<radroots_events::plot::RadrootsPlotLocation>,
    factory: &F,
) -> Result<(), RadrootsTangleEventsError> {
    clear_plot_locations(exec, plot_id)?;
    if let Some(location) = location {
        let gcs_id = create_gcs_location(exec, location.gcs, factory)?;
        let fields = IPlotGcsLocationFields {
            plot_id: plot_id.to_string(),
            gcs_location_id: gcs_id,
            role: ROLE_PRIMARY.to_string(),
        };
        let _ = plot_gcs_location::create(exec, &fields)?;
    }
    Ok(())
}

fn clear_farm_locations<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
) -> Result<(), RadrootsTangleEventsError> {
    let existing = farm_gcs_location::find_many(
        exec,
        &IFarmGcsLocationFindMany {
            filter: Some(IFarmGcsLocationFieldsFilter {
                id: None,
                created_at: None,
                updated_at: None,
                farm_id: Some(farm_id.to_string()),
                gcs_location_id: None,
                role: None,
            }),
        },
    )?;
    for row in existing.results {
        match farm_gcs_location::delete(
            exec,
            &IFarmGcsLocationDelete::On(IFarmGcsLocationFindOneArgs {
                on: FarmGcsLocationQueryBindValues::Id { id: row.id },
            }),
        ) {
            Ok(_) => {}
            Err(err) => {
                if !matches!(err.err, SqlError::NotFound(_)) {
                    return Err(err.into());
                }
            }
        }
    }
    Ok(())
}

fn clear_plot_locations<E: SqlExecutor>(
    exec: &E,
    plot_id: &str,
) -> Result<(), RadrootsTangleEventsError> {
    let existing = plot_gcs_location::find_many(
        exec,
        &IPlotGcsLocationFindMany {
            filter: Some(IPlotGcsLocationFieldsFilter {
                id: None,
                created_at: None,
                updated_at: None,
                plot_id: Some(plot_id.to_string()),
                gcs_location_id: None,
                role: None,
            }),
        },
    )?;
    for row in existing.results {
        match plot_gcs_location::delete(
            exec,
            &IPlotGcsLocationDelete::On(IPlotGcsLocationFindOneArgs {
                on: PlotGcsLocationQueryBindValues::Id { id: row.id },
            }),
        ) {
            Ok(_) => {}
            Err(err) => {
                if !matches!(err.err, SqlError::NotFound(_)) {
                    return Err(err.into());
                }
            }
        }
    }
    Ok(())
}

fn create_gcs_location<E: SqlExecutor, F: RadrootsTangleIdFactory>(
    exec: &E,
    gcs: radroots_events::farm::RadrootsGcsLocation,
    factory: &F,
) -> Result<String, RadrootsTangleEventsError> {
    let d_tag = factory.new_d_tag();
    let point = serde_json::to_string(&gcs.point)
        .map_err(|_| RadrootsTangleEventsError::InvalidData("gcs.point".to_string()))?;
    let polygon = serde_json::to_string(&gcs.polygon)
        .map_err(|_| RadrootsTangleEventsError::InvalidData("gcs.polygon".to_string()))?;

    let fields = IGcsLocationFields {
        d_tag,
        lat: gcs.lat,
        lng: gcs.lng,
        geohash: gcs.geohash,
        point,
        polygon,
        accuracy: gcs.accuracy,
        altitude: gcs.altitude,
        tag_0: gcs.tag_0,
        label: gcs.label,
        area: gcs.area,
        elevation: gcs.elevation,
        soil: gcs.soil,
        climate: gcs.climate,
        gc_id: gcs.gc_id,
        gc_name: gcs.gc_name,
        gc_admin1_id: gcs.gc_admin1_id,
        gc_admin1_name: gcs.gc_admin1_name,
        gc_country_id: gcs.gc_country_id,
        gc_country_name: gcs.gc_country_name,
    };
    let result = gcs_location::create(exec, &fields)?;
    Ok(result.result.id)
}

fn upsert_farm_members<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
    role: ListSetRole,
    list_set: &radroots_events::list_set::RadrootsListSet,
) -> Result<(), RadrootsTangleEventsError> {
    let role_value = match role {
        ListSetRole::Members => ROLE_MEMBER,
        ListSetRole::Owners => ROLE_OWNER,
        ListSetRole::Workers => ROLE_WORKER,
        ListSetRole::Plots => return Ok(()),
    };
    let existing = farm_member::find_many(
        exec,
        &IFarmMemberFindMany {
            filter: Some(IFarmMemberFieldsFilter {
                id: None,
                created_at: None,
                updated_at: None,
                farm_id: Some(farm_id.to_string()),
                member_pubkey: None,
                role: Some(role_value.to_string()),
            }),
        },
    )?;
    for row in existing.results {
        match farm_member::delete(
            exec,
            &IFarmMemberDelete::On(IFarmMemberFindOneArgs {
                on: FarmMemberQueryBindValues::Id { id: row.id },
            }),
        ) {
            Ok(_) => {}
            Err(err) => {
                if !matches!(err.err, SqlError::NotFound(_)) {
                    return Err(err.into());
                }
            }
        }
    }

    let mut entries = list_set
        .entries
        .iter()
        .filter(|entry| entry.tag == "p")
        .filter_map(|entry| entry.values.get(0))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    entries.sort();
    entries.dedup();

    for pubkey in entries {
        let fields = IFarmMemberFields {
            farm_id: farm_id.to_string(),
            member_pubkey: pubkey,
            role: role_value.to_string(),
        };
        let _ = farm_member::create(exec, &fields)?;
    }
    Ok(())
}

fn upsert_member_claims<E: SqlExecutor>(
    exec: &E,
    member_pubkey: &str,
    list_set: &radroots_events::list_set::RadrootsListSet,
) -> Result<(), RadrootsTangleEventsError> {
    let existing = farm_member_claim::find_many(
        exec,
        &IFarmMemberClaimFindMany {
            filter: Some(IFarmMemberClaimFieldsFilter {
                id: None,
                created_at: None,
                updated_at: None,
                member_pubkey: Some(member_pubkey.to_string()),
                farm_pubkey: None,
            }),
        },
    )?;
    for row in existing.results {
        match farm_member_claim::delete(
            exec,
            &IFarmMemberClaimDelete::On(IFarmMemberClaimFindOneArgs {
                on: FarmMemberClaimQueryBindValues::Id { id: row.id },
            }),
        ) {
            Ok(_) => {}
            Err(err) => {
                if !matches!(err.err, SqlError::NotFound(_)) {
                    return Err(err.into());
                }
            }
        }
    }

    let mut entries = list_set
        .entries
        .iter()
        .filter(|entry| entry.tag == "p")
        .filter_map(|entry| entry.values.get(0))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    entries.sort();
    entries.dedup();

    for farm_pubkey in entries {
        let fields = IFarmMemberClaimFields {
            member_pubkey: member_pubkey.to_string(),
            farm_pubkey,
        };
        let _ = farm_member_claim::create(exec, &fields)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListSetRole {
    Members,
    Owners,
    Workers,
    Plots,
}

fn unpack_farm_location_strings(
    location: Option<&radroots_events::farm::RadrootsFarmLocation>,
) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    match location {
        Some(location) => (
            location.primary.clone(),
            location.city.clone(),
            location.region.clone(),
            location.country.clone(),
        ),
        None => (None, None, None, None),
    }
}

fn unpack_plot_location_strings(
    location: Option<&radroots_events::plot::RadrootsPlotLocation>,
) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    match location {
        Some(location) => (
            location.primary.clone(),
            location.city.clone(),
            location.region.clone(),
            location.country.clone(),
        ),
        None => (None, None, None, None),
    }
}

fn ensure_list_set_entries_tag(
    list_set: &radroots_events::list_set::RadrootsListSet,
    expected: &str,
    label: &str,
) -> Result<(), RadrootsTangleEventsError> {
    for entry in list_set.entries.iter() {
        if entry.tag != expected {
            return Err(RadrootsTangleEventsError::InvalidData(format!(
                "domain:farm list set {label} must only include {expected} tags"
            )));
        }
        if entry.values.get(0).map(|v| v.trim().is_empty()).unwrap_or(true) {
            return Err(RadrootsTangleEventsError::InvalidData(format!(
                "domain:farm list set {label} contains empty entries"
            )));
        }
    }
    Ok(())
}

fn parse_farm_list_set_d_tag(d_tag: &str) -> Option<(String, ListSetRole)> {
    let mut parts = d_tag.splitn(3, ':');
    if parts.next()? != "farm" {
        return None;
    }
    let farm_d_tag = parts.next()?.to_string();
    let suffix = parts.next()?;
    let role = match suffix {
        "members" => ListSetRole::Members,
        "members.owners" => ListSetRole::Owners,
        "members.workers" => ListSetRole::Workers,
        "plots" => ListSetRole::Plots,
        _ => return None,
    };
    Some((farm_d_tag, role))
}

fn to_value_opt(value: Option<String>) -> Option<Value> {
    Some(match value {
        Some(value) => Value::from(value),
        None => Value::Null,
    })
}

struct EventStateDecision {
    apply: bool,
    content_hash: String,
}
