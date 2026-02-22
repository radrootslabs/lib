#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "std")]
use base64::Engine;
#[cfg(feature = "std")]
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use radroots_events::RadrootsNostrEvent;
use radroots_events::kinds::{KIND_FARM, KIND_PLOT, KIND_PROFILE, is_nip51_list_set_kind};
use radroots_events_codec::farm::decode as farm_decode;
use radroots_events_codec::list_set::decode as list_set_decode;
use radroots_events_codec::plot::decode as plot_decode;
use radroots_events_codec::profile::decode as profile_decode;
use radroots_sql_core::SqlExecutor;
use radroots_sql_core::error::SqlError;
use radroots_tangle_db::{
    farm, farm_gcs_location, farm_member, farm_member_claim, farm_tag, gcs_location,
    nostr_event_state, nostr_profile, plot, plot_gcs_location, plot_tag,
};
use radroots_tangle_db_schema::farm::{
    FarmQueryBindValues, IFarmFields, IFarmFieldsFilter, IFarmFieldsPartial, IFarmFindMany,
    IFarmUpdate,
};
use radroots_tangle_db_schema::farm_gcs_location::{
    FarmGcsLocationQueryBindValues, IFarmGcsLocationDelete, IFarmGcsLocationFields,
    IFarmGcsLocationFieldsFilter, IFarmGcsLocationFindMany, IFarmGcsLocationFindOneArgs,
};
use radroots_tangle_db_schema::farm_member::{
    FarmMemberQueryBindValues, IFarmMemberDelete, IFarmMemberFields, IFarmMemberFieldsFilter,
    IFarmMemberFindMany, IFarmMemberFindOneArgs,
};
use radroots_tangle_db_schema::farm_member_claim::{
    FarmMemberClaimQueryBindValues, IFarmMemberClaimDelete, IFarmMemberClaimFields,
    IFarmMemberClaimFieldsFilter, IFarmMemberClaimFindMany, IFarmMemberClaimFindOneArgs,
};
use radroots_tangle_db_schema::farm_tag::{
    FarmTagQueryBindValues, IFarmTagDelete, IFarmTagFields, IFarmTagFieldsFilter, IFarmTagFindMany,
    IFarmTagFindOneArgs,
};
use radroots_tangle_db_schema::gcs_location::IGcsLocationFields;
use radroots_tangle_db_schema::nostr_event_state::{
    INostrEventStateFields, INostrEventStateFieldsPartial, INostrEventStateFindOne,
    INostrEventStateFindOneArgs, INostrEventStateUpdate, NostrEventStateQueryBindValues,
};
use radroots_tangle_db_schema::nostr_profile::{
    INostrProfileFields, INostrProfileFieldsPartial, INostrProfileFindOne,
    INostrProfileFindOneArgs, INostrProfileUpdate, NostrProfileQueryBindValues,
};
use radroots_tangle_db_schema::plot::{
    IPlotFields, IPlotFieldsFilter, IPlotFieldsPartial, IPlotFindMany, IPlotUpdate,
    PlotQueryBindValues,
};
use radroots_tangle_db_schema::plot_gcs_location::{
    IPlotGcsLocationDelete, IPlotGcsLocationFields, IPlotGcsLocationFieldsFilter,
    IPlotGcsLocationFindMany, IPlotGcsLocationFindOneArgs, PlotGcsLocationQueryBindValues,
};
use radroots_tangle_db_schema::plot_tag::{
    IPlotTagDelete, IPlotTagFields, IPlotTagFieldsFilter, IPlotTagFindMany, IPlotTagFindOneArgs,
    PlotTagQueryBindValues,
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
    exec.begin()
        .map_err(|e| RadrootsTangleEventsError::from(radroots_types::types::IError::from(e)))?;

    let outcome = match ingest_event_inner(exec, event, factory) {
        Ok(outcome) => {
            exec.commit().map_err(|e| {
                RadrootsTangleEventsError::from(radroots_types::types::IError::from(e))
            })?;
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
    let metadata_result = profile_decode::metadata_from_event(
        event.id.clone(),
        event.author.clone(),
        event.created_at,
        event.kind,
        event.content.clone(),
        event.tags.clone(),
    );
    let metadata = metadata_result?;
    let profile_type = metadata.profile_type.ok_or_else(|| {
        RadrootsTangleEventsError::InvalidData("profile_type required".to_string())
    })?;

    let d_tag = "".to_string();
    let decision = event_state_decision(exec, event, &d_tag)?;
    if !decision.apply {
        return Ok(RadrootsTangleIngestOutcome::Skipped);
    }

    let profile_type = match profile_type {
        radroots_events::profile::RadrootsProfileType::Individual => "individual",
        radroots_events::profile::RadrootsProfileType::Farm => "farm",
        radroots_events::profile::RadrootsProfileType::Coop => "coop",
        radroots_events::profile::RadrootsProfileType::Any => "any",
        radroots_events::profile::RadrootsProfileType::Radrootsd => "radrootsd",
    };

    let existing_result = nostr_profile::find_one(
        exec,
        &INostrProfileFindOne::On(INostrProfileFindOneArgs {
            on: NostrProfileQueryBindValues::PublicKey {
                public_key: metadata.author.clone(),
            },
        }),
    );
    let existing = existing_result?.result;

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
            let update_result = nostr_profile::update(
                exec,
                &INostrProfileUpdate {
                    on: NostrProfileQueryBindValues::Id { id: profile.id },
                    fields,
                },
            );
            let _updated = update_result?;
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
    let existing_result = farm::find_many(
        exec,
        &IFarmFindMany {
            filter: Some(filter),
        },
    );
    let existing = existing_result?;
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
        let update_result = farm::update(
            exec,
            &IFarmUpdate {
                on: FarmQueryBindValues::Id { id: row.id.clone() },
                fields,
            },
        );
        let _updated = update_result?;
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
    let existing_result = plot::find_many(
        exec,
        &IPlotFindMany {
            filter: Some(filter),
        },
    );
    let existing = existing_result?;
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
        let update_result = plot::update(
            exec,
            &IPlotUpdate {
                on: PlotQueryBindValues::Id { id: row.id.clone() },
                fields,
            },
        );
        let _updated = update_result?;
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
    let list_set =
        list_set_decode::list_set_from_tags(event.kind, event.content.clone(), &event.tags)?;

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
    let existing_result = nostr_event_state::find_one(
        exec,
        &INostrEventStateFindOne::On(INostrEventStateFindOneArgs {
            on: NostrEventStateQueryBindValues::Key { key: key.clone() },
        }),
    );
    let existing = existing_result?.result;

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
            let update_result = nostr_event_state::update(
                exec,
                &INostrEventStateUpdate {
                    on: NostrEventStateQueryBindValues::Id { id: state.id },
                    fields,
                },
            );
            let _updated = update_result?;
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
    let existing_result = nostr_event_state::find_one(
        exec,
        &INostrEventStateFindOne::On(INostrEventStateFindOneArgs {
            on: NostrEventStateQueryBindValues::Key { key },
        }),
    );
    let existing = existing_result?.result;

    if let Some(state) = existing {
        if event.created_at < state.last_created_at {
            return Ok(EventStateDecision {
                apply: false,
                content_hash,
            });
        }
        if event.created_at == state.last_created_at && content_hash == state.content_hash {
            return Ok(EventStateDecision {
                apply: false,
                content_hash,
            });
        }
    }

    Ok(EventStateDecision {
        apply: true,
        content_hash,
    })
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
    let result_query = farm::find_many(
        exec,
        &IFarmFindMany {
            filter: Some(filter),
        },
    );
    let result = result_query?;
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
    let existing_query = farm_tag::find_many(
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
    );
    let existing = existing_query?;
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
    let existing_query = plot_tag::find_many(
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
    );
    let existing = existing_query?;
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
    let existing_query = farm_gcs_location::find_many(
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
    );
    let existing = existing_query?;
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
    let existing_query = plot_gcs_location::find_many(
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
    );
    let existing = existing_query?;
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
    let point = serde_json::to_string(&gcs.point).map_err(map_gcs_point_serialize_error)?;
    let polygon = serde_json::to_string(&gcs.polygon).map_err(map_gcs_polygon_serialize_error)?;

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

fn map_gcs_point_serialize_error(_err: serde_json::Error) -> RadrootsTangleEventsError {
    RadrootsTangleEventsError::InvalidData("gcs.point".to_string())
}

fn map_gcs_polygon_serialize_error(_err: serde_json::Error) -> RadrootsTangleEventsError {
    RadrootsTangleEventsError::InvalidData("gcs.polygon".to_string())
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
    let existing_query = farm_member::find_many(
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
    );
    let existing = existing_query?;
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
    let existing_query = farm_member_claim::find_many(
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
    );
    let existing = existing_query?;
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
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
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
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use radroots_events::farm::{
        RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef, RadrootsGcsLocation,
        RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon,
    };
    use radroots_events::kinds::{KIND_LIST_SET_FOLLOW, KIND_LIST_SET_GENERIC};
    use radroots_events::list::RadrootsListEntry;
    use radroots_events::list_set::RadrootsListSet;
    use radroots_events::plot::{RadrootsPlot, RadrootsPlotLocation};
    use radroots_events::profile::{
        RADROOTS_PROFILE_TYPE_TAG_KEY, RadrootsProfile, RadrootsProfileType,
        radroots_profile_type_tag_value,
    };
    use radroots_events_codec::farm::encode as farm_encode;
    use radroots_events_codec::farm::list_sets as farm_list_sets;
    use radroots_events_codec::list_set::encode as list_set_encode;
    use radroots_events_codec::plot::encode as plot_encode;
    use radroots_sql_core::{ExecOutcome, SqlExecutor, SqliteExecutor};
    use radroots_tangle_db::{
        farm, farm_gcs_location, farm_member, farm_member_claim, farm_tag, gcs_location,
        migrations, plot, plot_gcs_location, plot_tag,
    };
    use radroots_tangle_db_schema::farm::IFarmFields;
    use radroots_tangle_db_schema::farm_gcs_location::IFarmGcsLocationFields;
    use radroots_tangle_db_schema::farm_member::IFarmMemberFields;
    use radroots_tangle_db_schema::farm_member_claim::IFarmMemberClaimFields;
    use radroots_tangle_db_schema::farm_tag::IFarmTagFields;
    use radroots_tangle_db_schema::gcs_location::IGcsLocationFields;
    use radroots_tangle_db_schema::plot::IPlotFields;
    use radroots_tangle_db_schema::plot_gcs_location::IPlotGcsLocationFields;
    use radroots_tangle_db_schema::plot_tag::IPlotTagFields;

    struct FixedFactory;

    impl RadrootsTangleIdFactory for FixedFactory {
        fn new_d_tag(&self) -> String {
            "AAAAAAAAAAAAAAAAAAAAAZ".to_string()
        }
    }

    struct TxnExecutor {
        begin_err: Option<SqlError>,
        commit_err: Option<SqlError>,
        rollback_count: Arc<AtomicUsize>,
    }

    impl SqlExecutor for TxnExecutor {
        fn exec(&self, _sql: &str, _params_json: &str) -> Result<ExecOutcome, SqlError> {
            Err(SqlError::UnsupportedPlatform)
        }

        fn query_raw(&self, _sql: &str, _params_json: &str) -> Result<String, SqlError> {
            Err(SqlError::UnsupportedPlatform)
        }

        fn begin(&self) -> Result<(), SqlError> {
            match self.begin_err.clone() {
                Some(err) => Err(err),
                None => Ok(()),
            }
        }

        fn commit(&self) -> Result<(), SqlError> {
            match self.commit_err.clone() {
                Some(err) => Err(err),
                None => Ok(()),
            }
        }

        fn rollback(&self) -> Result<(), SqlError> {
            self.rollback_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct DeleteErrorExecutor<'a> {
        inner: &'a SqliteExecutor,
        table_name: &'static str,
        err: SqlError,
    }

    impl SqlExecutor for DeleteErrorExecutor<'_> {
        fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
            let normalized = sql.to_ascii_lowercase();
            if normalized.contains("delete from") && normalized.contains(self.table_name) {
                return Err(self.err.clone());
            }
            self.inner.exec(sql, params_json)
        }

        fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
            self.inner.query_raw(sql, params_json)
        }

        fn begin(&self) -> Result<(), SqlError> {
            self.inner.begin()
        }

        fn commit(&self) -> Result<(), SqlError> {
            self.inner.commit()
        }

        fn rollback(&self) -> Result<(), SqlError> {
            self.inner.rollback()
        }
    }

    fn sample_gcs(lat: f64, lng: f64, geohash: &str) -> RadrootsGcsLocation {
        RadrootsGcsLocation {
            lat,
            lng,
            geohash: geohash.to_string(),
            point: RadrootsGeoJsonPoint {
                r#type: "Point".to_string(),
                coordinates: [lng, lat],
            },
            polygon: RadrootsGeoJsonPolygon {
                r#type: "Polygon".to_string(),
                coordinates: vec![vec![
                    [lng, lat],
                    [lng, lat + 0.001],
                    [lng - 0.001, lat + 0.001],
                    [lng, lat],
                ]],
            },
            accuracy: Some(1.0),
            altitude: Some(2.0),
            tag_0: Some("tag".to_string()),
            label: Some("label".to_string()),
            area: Some(3.0),
            elevation: Some(4),
            soil: Some("soil".to_string()),
            climate: Some("climate".to_string()),
            gc_id: Some("gc_id".to_string()),
            gc_name: Some("gc_name".to_string()),
            gc_admin1_id: Some("gc_admin1_id".to_string()),
            gc_admin1_name: Some("gc_admin1_name".to_string()),
            gc_country_id: Some("gc_country_id".to_string()),
            gc_country_name: Some("gc_country_name".to_string()),
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
            display_name: Some(format!("{name}-display")),
            nip05: Some(format!("{name}@example.com")),
            about: Some(format!("{name}-about")),
            website: Some("https://example.com".to_string()),
            picture: Some("https://example.com/p.png".to_string()),
            banner: Some("https://example.com/b.png".to_string()),
            lud06: Some("lud06".to_string()),
            lud16: Some("lud16".to_string()),
            bot: None,
        };
        let mut tags = Vec::new();
        if let Some(profile_type) = profile_type {
            tags.push(vec![
                RADROOTS_PROFILE_TYPE_TAG_KEY.to_string(),
                radroots_profile_type_tag_value(profile_type).to_string(),
            ]);
        }
        RadrootsNostrEvent {
            id: format!("{id:064x}"),
            author: author.to_string(),
            created_at,
            kind: KIND_PROFILE,
            tags,
            content: serde_json::to_string(&profile).expect("profile json"),
            sig: "f".repeat(128),
        }
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
            about: Some("about".to_string()),
            website: Some("https://farm.example.com".to_string()),
            picture: Some("https://farm.example.com/p.png".to_string()),
            banner: Some("https://farm.example.com/b.png".to_string()),
            location,
            tags,
        };
        let tags = farm_encode::farm_build_tags(&farm).expect("farm tags");
        RadrootsNostrEvent {
            id: format!("{id:064x}"),
            author: author.to_string(),
            created_at,
            kind: KIND_FARM,
            tags,
            content: serde_json::to_string(&farm).expect("farm json"),
            sig: "f".repeat(128),
        }
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
            about: Some("plot-about".to_string()),
            location,
            tags,
        };
        let tags = plot_encode::plot_build_tags(&plot).expect("plot tags");
        RadrootsNostrEvent {
            id: format!("{id:064x}"),
            author: author.to_string(),
            created_at,
            kind: KIND_PLOT,
            tags,
            content: serde_json::to_string(&plot).expect("plot json"),
            sig: "f".repeat(128),
        }
    }

    fn list_set_event(
        id: u64,
        author: &str,
        created_at: u32,
        kind: u32,
        list_set: &RadrootsListSet,
    ) -> RadrootsNostrEvent {
        let parts = list_set_encode::to_wire_parts_with_kind(list_set, kind).expect("list set");
        RadrootsNostrEvent {
            id: format!("{id:064x}"),
            author: author.to_string(),
            created_at,
            kind,
            tags: parts.tags,
            content: parts.content,
            sig: "f".repeat(128),
        }
    }

    fn seed_rows(exec: &SqliteExecutor) -> (String, String, String, String) {
        migrations::run_all_up(exec).expect("migrations");
        let farm_row = farm::create(
            exec,
            &IFarmFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
                pubkey: "f".repeat(64),
                name: "farm".to_string(),
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
        .expect("farm")
        .result;
        let plot_row = plot::create(
            exec,
            &IPlotFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
                farm_id: farm_row.id.clone(),
                name: "plot".to_string(),
                about: None,
                location_primary: None,
                location_city: None,
                location_region: None,
                location_country: None,
            },
        )
        .expect("plot")
        .result;
        let gcs_row = gcs_location::create(
            exec,
            &IGcsLocationFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
                lat: 1.0,
                lng: 2.0,
                geohash: "s0".to_string(),
                point: "{\"type\":\"Point\",\"coordinates\":[2.0,1.0]}".to_string(),
                polygon:
                    "{\"type\":\"Polygon\",\"coordinates\":[[[2.0,1.0],[2.1,1.1],[1.9,1.1],[2.0,1.0]]]}".to_string(),
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
        .expect("gcs")
        .result;

        let _ = farm_tag::create(
            exec,
            &IFarmTagFields {
                farm_id: farm_row.id.clone(),
                tag: "alpha".to_string(),
            },
        )
        .expect("farm tag");
        let _ = plot_tag::create(
            exec,
            &IPlotTagFields {
                plot_id: plot_row.id.clone(),
                tag: "beta".to_string(),
            },
        )
        .expect("plot tag");
        let _ = farm_gcs_location::create(
            exec,
            &IFarmGcsLocationFields {
                farm_id: farm_row.id.clone(),
                gcs_location_id: gcs_row.id.clone(),
                role: "primary".to_string(),
            },
        )
        .expect("farm gcs");
        let _ = plot_gcs_location::create(
            exec,
            &IPlotGcsLocationFields {
                plot_id: plot_row.id.clone(),
                gcs_location_id: gcs_row.id.clone(),
                role: "primary".to_string(),
            },
        )
        .expect("plot gcs");
        let _ = farm_member::create(
            exec,
            &IFarmMemberFields {
                farm_id: farm_row.id.clone(),
                member_pubkey: "m".repeat(64),
                role: "member".to_string(),
            },
        )
        .expect("member");
        let _ = farm_member_claim::create(
            exec,
            &IFarmMemberClaimFields {
                member_pubkey: "m".repeat(64),
                farm_pubkey: farm_row.pubkey.clone(),
            },
        )
        .expect("claim");
        (
            farm_row.id,
            farm_row.pubkey,
            farm_row.d_tag,
            plot_row.d_tag.clone(),
        )
    }

    #[test]
    fn ingest_transaction_paths_are_covered() {
        let begin_executor = TxnExecutor {
            begin_err: Some(SqlError::Internal),
            commit_err: None,
            rollback_count: Arc::new(AtomicUsize::new(0)),
        };
        let event = RadrootsNostrEvent {
            id: format!("{:064x}", 1u64),
            author: "a".repeat(64),
            created_at: 1,
            kind: KIND_LIST_SET_FOLLOW,
            tags: Vec::new(),
            content: String::new(),
            sig: "f".repeat(128),
        };
        let begin_err =
            radroots_tangle_ingest_event_with_factory(&begin_executor, &event, &FixedFactory)
                .expect_err("begin");
        assert!(matches!(begin_err, RadrootsTangleEventsError::Sql(_)));
        assert!(begin_executor.commit().is_ok());
        assert!(matches!(
            begin_executor.exec("select 1", "[]").expect_err("exec"),
            SqlError::UnsupportedPlatform
        ));
        assert!(matches!(
            begin_executor
                .query_raw("select 1", "[]")
                .expect_err("query"),
            SqlError::UnsupportedPlatform
        ));

        let rollback_count = Arc::new(AtomicUsize::new(0));
        let commit_executor = TxnExecutor {
            begin_err: None,
            commit_err: Some(SqlError::Internal),
            rollback_count: rollback_count.clone(),
        };
        let commit_err =
            radroots_tangle_ingest_event_with_factory(&commit_executor, &event, &FixedFactory)
                .expect_err("commit");
        assert!(matches!(commit_err, RadrootsTangleEventsError::Sql(_)));
        assert_eq!(rollback_count.load(Ordering::SeqCst), 0);

        let rollback_executor = TxnExecutor {
            begin_err: None,
            commit_err: None,
            rollback_count: Arc::new(AtomicUsize::new(0)),
        };
        let unsupported = RadrootsNostrEvent {
            id: format!("{:064x}", 2u64),
            author: "a".repeat(64),
            created_at: 2,
            kind: 42,
            tags: Vec::new(),
            content: String::new(),
            sig: "f".repeat(128),
        };
        let err = radroots_tangle_ingest_event_with_factory(
            &rollback_executor,
            &unsupported,
            &FixedFactory,
        )
        .expect_err("rollback");
        assert!(err.to_string().contains("unsupported kind"));
        assert_eq!(rollback_executor.rollback_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ingest_core_paths_cover_helpers_and_decisions() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");

        let factory = RadrootsTangleDefaultIdFactory;
        assert_eq!(factory.new_d_tag().len(), 22);

        let profile_pubkey = "p".repeat(64);
        let profile = profile_event(
            10,
            &profile_pubkey,
            1,
            Some(RadrootsProfileType::Individual),
            "alice",
        );
        let profile_no_type = profile_event(9, &profile_pubkey, 0, None, "alice-none");
        assert!(ingest_profile_event(&exec, &profile_no_type).is_err());
        assert_eq!(
            radroots_tangle_ingest_event(&exec, &profile).expect("ingest wrapper"),
            RadrootsTangleIngestOutcome::Applied
        );
        let profile_update = profile_event(
            11,
            &profile_pubkey,
            2,
            Some(RadrootsProfileType::Individual),
            "alice-2",
        );
        assert_eq!(
            ingest_profile_event(&exec, &profile_update).expect("profile update"),
            RadrootsTangleIngestOutcome::Applied
        );
        let profile_same_time_diff_hash = profile_event(
            12,
            &profile_pubkey,
            2,
            Some(RadrootsProfileType::Individual),
            "alice-3",
        );
        let decision =
            event_state_decision(&exec, &profile_same_time_diff_hash, "").expect("decision");
        assert!(decision.apply);

        let farm_pubkey = "f".repeat(64);
        let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
        let farm = farm_event(
            20,
            &farm_pubkey,
            10,
            farm_d_tag,
            "farm-a",
            Some(RadrootsFarmLocation {
                primary: Some("primary".to_string()),
                city: Some("city".to_string()),
                region: Some("region".to_string()),
                country: Some("country".to_string()),
                gcs: sample_gcs(10.0, 20.0, "s0"),
            }),
            Some(vec![
                "coffee".to_string(),
                "coffee".to_string(),
                " ".to_string(),
            ]),
        );
        assert_eq!(
            ingest_farm_event(&exec, &farm, &FixedFactory).expect("farm"),
            RadrootsTangleIngestOutcome::Applied
        );
        let farm_update = farm_event(
            21,
            &farm_pubkey,
            11,
            farm_d_tag,
            "farm-b",
            None,
            Some(vec!["market".to_string()]),
        );
        assert_eq!(
            ingest_farm_event(&exec, &farm_update, &FixedFactory).expect("farm update"),
            RadrootsTangleIngestOutcome::Applied
        );

        let plot_d_tag = "AAAAAAAAAAAAAAAAAAAAAQ";
        let plot = plot_event(
            30,
            &farm_pubkey,
            20,
            plot_d_tag,
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-a",
            Some(RadrootsPlotLocation {
                primary: Some("p".to_string()),
                city: Some("c".to_string()),
                region: Some("r".to_string()),
                country: Some("k".to_string()),
                gcs: sample_gcs(11.0, 21.0, "s1"),
            }),
            Some(vec!["tag".to_string()]),
        );
        assert_eq!(
            ingest_plot_event(&exec, &plot, &FixedFactory).expect("plot"),
            RadrootsTangleIngestOutcome::Applied
        );
        let plot_update = plot_event(
            31,
            &farm_pubkey,
            21,
            plot_d_tag,
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-b",
            None,
            Some(vec!["tag2".to_string()]),
        );
        assert_eq!(
            ingest_plot_event(&exec, &plot_update, &FixedFactory).expect("plot update"),
            RadrootsTangleIngestOutcome::Applied
        );

        let members = farm_list_sets::farm_members_list_set(farm_d_tag, vec!["m".repeat(64)])
            .expect("members");
        let owners =
            farm_list_sets::farm_owners_list_set(farm_d_tag, vec!["o".repeat(64)]).expect("owners");
        let workers = farm_list_sets::farm_workers_list_set(farm_d_tag, vec!["w".repeat(64)])
            .expect("workers");
        let plots = farm_list_sets::farm_plots_list_set(
            farm_d_tag,
            &farm_pubkey,
            vec![plot_d_tag.to_string()],
        )
        .expect("plots");
        let member_of =
            farm_list_sets::member_of_farms_list_set(vec![farm_pubkey.clone()]).expect("member_of");

        for (idx, list_set) in [members, owners, workers, plots, member_of]
            .iter()
            .enumerate()
        {
            let event = list_set_event(
                40 + idx as u64,
                if list_set.d_tag == "member_of.farms" {
                    &profile_pubkey
                } else {
                    &farm_pubkey
                },
                30 + idx as u32,
                KIND_LIST_SET_GENERIC,
                list_set,
            );
            assert_eq!(
                ingest_list_set_event(&exec, &event).expect("list set"),
                RadrootsTangleIngestOutcome::Applied
            );
        }

        let bad_description = RadrootsListSet {
            d_tag: "member_of.farms".to_string(),
            content: String::new(),
            entries: vec![RadrootsListEntry {
                tag: "p".to_string(),
                values: vec![farm_pubkey.clone()],
            }],
            title: None,
            description: Some("bad".to_string()),
            image: None,
        };
        let bad_description_event = list_set_event(
            90,
            &profile_pubkey,
            100,
            KIND_LIST_SET_GENERIC,
            &bad_description,
        );
        assert!(ingest_list_set_event(&exec, &bad_description_event).is_err());

        let bad_image = RadrootsListSet {
            d_tag: "member_of.farms".to_string(),
            content: String::new(),
            entries: vec![RadrootsListEntry {
                tag: "p".to_string(),
                values: vec![farm_pubkey.clone()],
            }],
            title: None,
            description: None,
            image: Some("bad".to_string()),
        };
        let bad_image_event =
            list_set_event(91, &profile_pubkey, 101, KIND_LIST_SET_GENERIC, &bad_image);
        assert!(ingest_list_set_event(&exec, &bad_image_event).is_err());

        assert!(parse_farm_list_set_d_tag("farm:AAAAAAAAAAAAAAAAAAAAAA:unknown").is_none());
        assert!(parse_farm_list_set_d_tag("farm:AAAAAAAAAAAAAAAAAAAAAA:plots").is_some());
        assert_eq!(to_value_opt(Some("x".to_string())), Some(Value::from("x")));
        assert_eq!(to_value_opt(None), Some(Value::Null));
        let location = RadrootsFarmLocation {
            primary: Some("p".to_string()),
            city: Some("c".to_string()),
            region: Some("r".to_string()),
            country: Some("k".to_string()),
            gcs: sample_gcs(12.0, 22.0, "s2"),
        };
        assert_eq!(
            unpack_farm_location_strings(Some(&location)).0,
            Some("p".to_string())
        );
        assert_eq!(
            unpack_plot_location_strings(Some(&RadrootsPlotLocation {
                primary: Some("p".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(13.0, 23.0, "s3"),
            }))
            .0,
            Some("p".to_string())
        );
        assert!(ensure_list_set_entries_tag(&bad_image, "p", "x").is_ok());
        assert!(
            ensure_list_set_entries_tag(
                &RadrootsListSet {
                    d_tag: "x".to_string(),
                    content: String::new(),
                    entries: vec![RadrootsListEntry {
                        tag: "a".to_string(),
                        values: vec!["x".to_string()],
                    }],
                    title: None,
                    description: None,
                    image: None,
                },
                "p",
                "x",
            )
            .is_err()
        );
    }

    #[test]
    fn ingest_delete_error_paths_are_covered() {
        let exec = SqliteExecutor::open_memory().expect("db");
        let (farm_id, _farm_pubkey, farm_d_tag, _plot_d_tag) = seed_rows(&exec);

        let not_found_farm_tags = DeleteErrorExecutor {
            inner: &exec,
            table_name: "farm_tag",
            err: SqlError::NotFound("farm_tag".to_string()),
        };
        assert!(
            upsert_farm_tags(
                &not_found_farm_tags,
                &farm_id,
                Some(vec!["next".to_string()])
            )
            .is_ok()
        );

        let not_found_plot_tags = DeleteErrorExecutor {
            inner: &exec,
            table_name: "plot_tag",
            err: SqlError::NotFound("plot_tag".to_string()),
        };
        let plot_id = plot::find_many(&exec, &IPlotFindMany { filter: None })
            .expect("plots")
            .results[0]
            .id
            .clone();
        assert!(
            upsert_plot_tags(
                &not_found_plot_tags,
                &plot_id,
                Some(vec!["next".to_string()])
            )
            .is_ok()
        );

        let not_found_farm_locations = DeleteErrorExecutor {
            inner: &exec,
            table_name: "farm_gcs_location",
            err: SqlError::NotFound("farm_gcs_location".to_string()),
        };
        assert!(
            upsert_farm_location(
                &not_found_farm_locations,
                &farm_id,
                Some(RadrootsFarmLocation {
                    primary: None,
                    city: None,
                    region: None,
                    country: None,
                    gcs: sample_gcs(1.0, 2.0, "s4"),
                }),
                &FixedFactory,
            )
            .is_ok()
        );

        let not_found_plot_locations = DeleteErrorExecutor {
            inner: &exec,
            table_name: "plot_gcs_location",
            err: SqlError::NotFound("plot_gcs_location".to_string()),
        };
        assert!(
            upsert_plot_location(
                &not_found_plot_locations,
                &plot_id,
                Some(RadrootsPlotLocation {
                    primary: None,
                    city: None,
                    region: None,
                    country: None,
                    gcs: sample_gcs(1.1, 2.1, "s5"),
                }),
                &FixedFactory,
            )
            .is_ok()
        );

        let members_list_set =
            farm_list_sets::farm_members_list_set(&farm_d_tag, vec!["n".repeat(64)])
                .expect("members");
        assert!(
            upsert_farm_members(&exec, &farm_id, ListSetRole::Members, &members_list_set).is_ok()
        );
        let not_found_members = DeleteErrorExecutor {
            inner: &exec,
            table_name: "farm_member",
            err: SqlError::NotFound("farm_member".to_string()),
        };
        let not_found_members_list_set =
            farm_list_sets::farm_members_list_set(&farm_d_tag, vec!["q".repeat(64)])
                .expect("not found members");
        assert!(
            upsert_farm_members(
                &not_found_members,
                &farm_id,
                ListSetRole::Members,
                &not_found_members_list_set,
            )
            .is_ok()
        );
        assert!(
            upsert_farm_members(
                &not_found_members,
                &farm_id,
                ListSetRole::Plots,
                &not_found_members_list_set,
            )
            .is_ok()
        );

        let member_claims =
            farm_list_sets::member_of_farms_list_set(vec!["z".repeat(64)]).expect("claims");
        assert!(upsert_member_claims(&exec, &"m".repeat(64), &member_claims).is_ok());
        let not_found_claims = DeleteErrorExecutor {
            inner: &exec,
            table_name: "farm_member_claim",
            err: SqlError::NotFound("farm_member_claim".to_string()),
        };
        let not_found_member_claims =
            farm_list_sets::member_of_farms_list_set(vec!["y".repeat(64)]).expect("claims nf");
        assert!(
            upsert_member_claims(&not_found_claims, &"m".repeat(64), &not_found_member_claims)
                .is_ok()
        );
        assert!(not_found_claims.begin().is_ok());
        assert!(not_found_claims.commit().is_ok());
        let _ = not_found_claims.rollback();
        assert!(not_found_claims.query_raw("SELECT 1", "[]").is_ok());
        assert!(matches!(
            not_found_claims.exec("DELETE FROM farm_member_claim WHERE id = 1", "[]"),
            Err(SqlError::NotFound(_))
        ));
        let _ = not_found_claims.exec("DELETE FROM other_table WHERE id = 1", "[]");

        let internal_farm_tags = DeleteErrorExecutor {
            inner: &exec,
            table_name: "farm_tag",
            err: SqlError::Internal,
        };
        assert!(
            upsert_farm_tags(&internal_farm_tags, &farm_id, Some(vec!["x".to_string()])).is_err()
        );

        let internal_plot_tags = DeleteErrorExecutor {
            inner: &exec,
            table_name: "plot_tag",
            err: SqlError::Internal,
        };
        assert!(
            upsert_plot_tags(&internal_plot_tags, &plot_id, Some(vec!["x".to_string()])).is_err()
        );

        let internal_farm_locations = DeleteErrorExecutor {
            inner: &exec,
            table_name: "farm_gcs_location",
            err: SqlError::Internal,
        };
        assert!(
            upsert_farm_location(
                &internal_farm_locations,
                &farm_id,
                Some(RadrootsFarmLocation {
                    primary: None,
                    city: None,
                    region: None,
                    country: None,
                    gcs: sample_gcs(2.0, 3.0, "s6"),
                }),
                &FixedFactory,
            )
            .is_err()
        );

        let internal_plot_locations = DeleteErrorExecutor {
            inner: &exec,
            table_name: "plot_gcs_location",
            err: SqlError::Internal,
        };
        assert!(
            upsert_plot_location(
                &internal_plot_locations,
                &plot_id,
                Some(RadrootsPlotLocation {
                    primary: None,
                    city: None,
                    region: None,
                    country: None,
                    gcs: sample_gcs(2.1, 3.1, "s7"),
                }),
                &FixedFactory,
            )
            .is_err()
        );

        let internal_members = DeleteErrorExecutor {
            inner: &exec,
            table_name: "farm_member",
            err: SqlError::Internal,
        };
        assert!(
            upsert_farm_members(
                &internal_members,
                &farm_id,
                ListSetRole::Members,
                &members_list_set,
            )
            .is_err()
        );

        let internal_claims = DeleteErrorExecutor {
            inner: &exec,
            table_name: "farm_member_claim",
            err: SqlError::Internal,
        };
        assert!(upsert_member_claims(&internal_claims, &"m".repeat(64), &member_claims).is_err());
    }

    #[test]
    fn create_gcs_location_error_mapping_helpers_are_covered() {
        let point_json_err = serde_json::from_str::<Value>("{").expect_err("invalid json");
        let point_err = map_gcs_point_serialize_error(point_json_err);
        assert_eq!(point_err.to_string(), "tangle_events.data: gcs.point");

        let polygon_json_err = serde_json::from_str::<Value>("{").expect_err("invalid json");
        let polygon_err = map_gcs_polygon_serialize_error(polygon_json_err);
        assert_eq!(polygon_err.to_string(), "tangle_events.data: gcs.polygon");
    }
}
