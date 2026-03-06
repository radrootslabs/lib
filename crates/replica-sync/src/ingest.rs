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
use radroots_replica_db::{
    farm, farm_gcs_location, farm_member, farm_member_claim, farm_tag, gcs_location,
    nostr_event_state, nostr_profile, plot, plot_gcs_location, plot_tag,
};
use radroots_replica_db_schema::farm::{
    FarmQueryBindValues, IFarmFields, IFarmFieldsFilter, IFarmFieldsPartial, IFarmFindMany,
    IFarmUpdate,
};
use radroots_replica_db_schema::farm_gcs_location::{
    FarmGcsLocationQueryBindValues, IFarmGcsLocationDelete, IFarmGcsLocationFields,
    IFarmGcsLocationFieldsFilter, IFarmGcsLocationFindMany, IFarmGcsLocationFindOneArgs,
};
use radroots_replica_db_schema::farm_member::{
    FarmMemberQueryBindValues, IFarmMemberDelete, IFarmMemberFields, IFarmMemberFieldsFilter,
    IFarmMemberFindMany, IFarmMemberFindOneArgs,
};
use radroots_replica_db_schema::farm_member_claim::{
    FarmMemberClaimQueryBindValues, IFarmMemberClaimDelete, IFarmMemberClaimFields,
    IFarmMemberClaimFieldsFilter, IFarmMemberClaimFindMany, IFarmMemberClaimFindOneArgs,
};
use radroots_replica_db_schema::farm_tag::{
    FarmTagQueryBindValues, IFarmTagDelete, IFarmTagFields, IFarmTagFieldsFilter, IFarmTagFindMany,
    IFarmTagFindOneArgs,
};
use radroots_replica_db_schema::gcs_location::IGcsLocationFields;
use radroots_replica_db_schema::nostr_event_state::{
    INostrEventStateFields, INostrEventStateFieldsPartial, INostrEventStateFindOne,
    INostrEventStateFindOneArgs, INostrEventStateUpdate, NostrEventStateQueryBindValues,
};
use radroots_replica_db_schema::nostr_profile::{
    INostrProfileFields, INostrProfileFieldsPartial, INostrProfileFindOne,
    INostrProfileFindOneArgs, INostrProfileUpdate, NostrProfileQueryBindValues,
};
use radroots_replica_db_schema::plot::{
    IPlotFields, IPlotFieldsFilter, IPlotFieldsPartial, IPlotFindMany, IPlotUpdate,
    PlotQueryBindValues,
};
use radroots_replica_db_schema::plot_gcs_location::{
    IPlotGcsLocationDelete, IPlotGcsLocationFields, IPlotGcsLocationFieldsFilter,
    IPlotGcsLocationFindMany, IPlotGcsLocationFindOneArgs, PlotGcsLocationQueryBindValues,
};
use radroots_replica_db_schema::plot_tag::{
    IPlotTagDelete, IPlotTagFields, IPlotTagFieldsFilter, IPlotTagFindMany, IPlotTagFindOneArgs,
    PlotTagQueryBindValues,
};
use radroots_sql_core::SqlExecutor;
use radroots_sql_core::error::SqlError;
use serde_json::Value;

use crate::error::RadrootsReplicaEventsError;
use crate::event_state::{event_content_hash, event_state_key};
const ROLE_PRIMARY: &str = "primary";
const ROLE_MEMBER: &str = "member";
const ROLE_OWNER: &str = "owner";
const ROLE_WORKER: &str = "worker";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsReplicaIngestOutcome {
    Applied,
    Skipped,
}

pub trait RadrootsReplicaIdFactory {
    fn new_d_tag(&self) -> String;
}

#[cfg(feature = "std")]
pub struct RadrootsReplicaDefaultIdFactory;

#[cfg(feature = "std")]
impl RadrootsReplicaIdFactory for RadrootsReplicaDefaultIdFactory {
    fn new_d_tag(&self) -> String {
        let uuid = uuid::Uuid::now_v7();
        let bytes = uuid.as_bytes();
        URL_SAFE_NO_PAD.encode(bytes)
    }
}

#[cfg(feature = "std")]
pub fn radroots_replica_ingest_event<E: SqlExecutor>(
    exec: &E,
    event: &RadrootsNostrEvent,
) -> Result<RadrootsReplicaIngestOutcome, RadrootsReplicaEventsError> {
    radroots_replica_ingest_event_with_factory(exec, event, &RadrootsReplicaDefaultIdFactory)
}

pub fn radroots_replica_ingest_event_with_factory<E: SqlExecutor, F: RadrootsReplicaIdFactory>(
    exec: &E,
    event: &RadrootsNostrEvent,
    factory: &F,
) -> Result<RadrootsReplicaIngestOutcome, RadrootsReplicaEventsError> {
    if let Err(err) = exec.begin() {
        return Err(RadrootsReplicaEventsError::from(
            radroots_types::types::IError::from(err),
        ));
    }

    let outcome = match ingest_event_inner(exec, event, factory) {
        Ok(outcome) => {
            if let Err(err) = exec.commit() {
                return Err(RadrootsReplicaEventsError::from(
                    radroots_types::types::IError::from(err),
                ));
            }
            Ok(outcome)
        }
        Err(err) => {
            let _ = exec.rollback();
            Err(err)
        }
    };

    outcome
}

fn ingest_event_inner<E: SqlExecutor, F: RadrootsReplicaIdFactory>(
    exec: &E,
    event: &RadrootsNostrEvent,
    factory: &F,
) -> Result<RadrootsReplicaIngestOutcome, RadrootsReplicaEventsError> {
    match event.kind {
        KIND_PROFILE => ingest_profile_event(exec, event),
        KIND_FARM => ingest_farm_event(exec, event, factory),
        KIND_PLOT => ingest_plot_event(exec, event, factory),
        kind if is_nip51_list_set_kind(kind) => ingest_list_set_event(exec, event),
        _ => Err(RadrootsReplicaEventsError::InvalidData(format!(
            "unsupported kind {}",
            event.kind
        ))),
    }
}

fn ingest_profile_event<E: SqlExecutor>(
    exec: &E,
    event: &RadrootsNostrEvent,
) -> Result<RadrootsReplicaIngestOutcome, RadrootsReplicaEventsError> {
    let data_result = profile_decode::data_from_event(
        event.id.clone(),
        event.author.clone(),
        event.created_at,
        event.kind,
        event.content.clone(),
        event.tags.clone(),
    );
    let data = data_result?;
    let profile_type = match data.data.profile_type {
        Some(profile_type) => profile_type,
        None => {
            return Err(RadrootsReplicaEventsError::InvalidData(
                "profile_type required".to_string(),
            ));
        }
    };

    let d_tag = "".to_string();
    let decision = event_state_decision(exec, event, &d_tag)?;
    if !decision.apply {
        return Ok(RadrootsReplicaIngestOutcome::Skipped);
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
                public_key: data.author.clone(),
            },
        }),
    );
    let existing = existing_result?.result;

    match existing {
        Some(profile) => {
            let fields = INostrProfileFieldsPartial {
                public_key: None,
                profile_type: Some(Value::from(profile_type)),
                name: Some(Value::from(data.data.profile.name)),
                display_name: to_value_opt(data.data.profile.display_name),
                about: to_value_opt(data.data.profile.about),
                website: to_value_opt(data.data.profile.website),
                picture: to_value_opt(data.data.profile.picture),
                banner: to_value_opt(data.data.profile.banner),
                nip05: to_value_opt(data.data.profile.nip05),
                lud06: to_value_opt(data.data.profile.lud06),
                lud16: to_value_opt(data.data.profile.lud16),
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
                public_key: data.author.clone(),
                profile_type: profile_type.to_string(),
                name: data.data.profile.name,
                display_name: data.data.profile.display_name,
                about: data.data.profile.about,
                website: data.data.profile.website,
                picture: data.data.profile.picture,
                banner: data.data.profile.banner,
                nip05: data.data.profile.nip05,
                lud06: data.data.profile.lud06,
                lud16: data.data.profile.lud16,
            };
            let _ = nostr_profile::create(exec, &fields)?;
        }
    }

    radroots_replica_ingest_event_state(exec, event, &d_tag, &decision.content_hash)?;
    Ok(RadrootsReplicaIngestOutcome::Applied)
}

fn ingest_farm_event<E: SqlExecutor, F: RadrootsReplicaIdFactory>(
    exec: &E,
    event: &RadrootsNostrEvent,
    factory: &F,
) -> Result<RadrootsReplicaIngestOutcome, RadrootsReplicaEventsError> {
    let farm = farm_decode::farm_from_event(event.kind, &event.tags, &event.content)?;
    let decision = event_state_decision(exec, event, &farm.d_tag)?;
    if !decision.apply {
        return Ok(RadrootsReplicaIngestOutcome::Skipped);
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

    radroots_replica_ingest_event_state(exec, event, &farm.d_tag, &decision.content_hash)?;
    Ok(RadrootsReplicaIngestOutcome::Applied)
}

fn ingest_plot_event<E: SqlExecutor, F: RadrootsReplicaIdFactory>(
    exec: &E,
    event: &RadrootsNostrEvent,
    factory: &F,
) -> Result<RadrootsReplicaIngestOutcome, RadrootsReplicaEventsError> {
    let plot = plot_decode::plot_from_event(event.kind, &event.tags, &event.content)?;
    let decision = event_state_decision(exec, event, &plot.d_tag)?;
    if !decision.apply {
        return Ok(RadrootsReplicaIngestOutcome::Skipped);
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

    radroots_replica_ingest_event_state(exec, event, &plot.d_tag, &decision.content_hash)?;
    Ok(RadrootsReplicaIngestOutcome::Applied)
}

fn ingest_list_set_event<E: SqlExecutor>(
    exec: &E,
    event: &RadrootsNostrEvent,
) -> Result<RadrootsReplicaIngestOutcome, RadrootsReplicaEventsError> {
    if event.kind != radroots_events::kinds::KIND_LIST_SET_GENERIC {
        return Ok(RadrootsReplicaIngestOutcome::Skipped);
    }
    let list_set =
        list_set_decode::list_set_from_tags(event.kind, event.content.clone(), &event.tags)?;

    if list_set.title.is_some() || list_set.description.is_some() || list_set.image.is_some() {
        return Err(RadrootsReplicaEventsError::InvalidData(
            "domain:farm list sets must omit metadata".to_string(),
        ));
    }
    if !list_set.content.is_empty() {
        return Err(RadrootsReplicaEventsError::InvalidData(
            "domain:farm list sets must not include content".to_string(),
        ));
    }

    let d_tag = list_set.d_tag.clone();
    let decision = event_state_decision(exec, event, &d_tag)?;
    if !decision.apply {
        return Ok(RadrootsReplicaIngestOutcome::Skipped);
    }

    if d_tag == "member_of.farms" {
        ensure_list_set_entries_tag(&list_set, "p", "member_of.farms")?;
        upsert_member_claims(exec, &event.author, &list_set)?;
        radroots_replica_ingest_event_state(exec, event, &d_tag, &decision.content_hash)?;
        return Ok(RadrootsReplicaIngestOutcome::Applied);
    }

    if let Some((farm_d_tag, role)) = parse_farm_list_set_d_tag(&d_tag) {
        if role == ListSetRole::Plots {
            ensure_list_set_entries_tag(&list_set, "a", "farm plots")?;
            radroots_replica_ingest_event_state(exec, event, &d_tag, &decision.content_hash)?;
            return Ok(RadrootsReplicaIngestOutcome::Applied);
        }
        ensure_list_set_entries_tag(&list_set, "p", "farm members")?;
        let farm = find_farm_by_ref(exec, &event.author, &farm_d_tag)?;
        upsert_farm_members(exec, &farm.id, role, &list_set)?;
        radroots_replica_ingest_event_state(exec, event, &d_tag, &decision.content_hash)?;
        return Ok(RadrootsReplicaIngestOutcome::Applied);
    }

    Err(RadrootsReplicaEventsError::InvalidData(
        "unsupported list set d_tag".to_string(),
    ))
}

pub fn radroots_replica_ingest_event_state<E: SqlExecutor>(
    exec: &E,
    event: &RadrootsNostrEvent,
    d_tag: &str,
    content_hash: &str,
) -> Result<(), RadrootsReplicaEventsError> {
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
) -> Result<EventStateDecision, RadrootsReplicaEventsError> {
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
) -> Result<radroots_replica_db_schema::farm::Farm, RadrootsReplicaEventsError> {
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
    match result.results.into_iter().next() {
        Some(farm) => Ok(farm),
        None => Err(RadrootsReplicaEventsError::InvalidData(
            "farm not found".to_string(),
        )),
    }
}

fn upsert_farm_tags<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
    tags: Option<Vec<String>>,
) -> Result<(), RadrootsReplicaEventsError> {
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
) -> Result<(), RadrootsReplicaEventsError> {
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

fn upsert_farm_location<E: SqlExecutor, F: RadrootsReplicaIdFactory>(
    exec: &E,
    farm_id: &str,
    location: Option<radroots_events::farm::RadrootsFarmLocation>,
    factory: &F,
) -> Result<(), RadrootsReplicaEventsError> {
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

fn upsert_plot_location<E: SqlExecutor, F: RadrootsReplicaIdFactory>(
    exec: &E,
    plot_id: &str,
    location: Option<radroots_events::plot::RadrootsPlotLocation>,
    factory: &F,
) -> Result<(), RadrootsReplicaEventsError> {
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
) -> Result<(), RadrootsReplicaEventsError> {
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
) -> Result<(), RadrootsReplicaEventsError> {
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

fn create_gcs_location<E: SqlExecutor, F: RadrootsReplicaIdFactory>(
    exec: &E,
    gcs: radroots_events::farm::RadrootsGcsLocation,
    factory: &F,
) -> Result<String, RadrootsReplicaEventsError> {
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

fn map_gcs_point_serialize_error(_err: serde_json::Error) -> RadrootsReplicaEventsError {
    RadrootsReplicaEventsError::InvalidData("gcs.point".to_string())
}

fn map_gcs_polygon_serialize_error(_err: serde_json::Error) -> RadrootsReplicaEventsError {
    RadrootsReplicaEventsError::InvalidData("gcs.polygon".to_string())
}

fn upsert_farm_members<E: SqlExecutor>(
    exec: &E,
    farm_id: &str,
    role: ListSetRole,
    list_set: &radroots_events::list_set::RadrootsListSet,
) -> Result<(), RadrootsReplicaEventsError> {
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

    let mut entries = Vec::new();
    for entry in &list_set.entries {
        if entry.tag != "p" {
            continue;
        }
        if let Some(value) = entry.values.first() {
            entries.push(value.to_string());
        }
    }
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
) -> Result<(), RadrootsReplicaEventsError> {
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

    let mut entries = Vec::new();
    for entry in &list_set.entries {
        if entry.tag != "p" {
            continue;
        }
        if let Some(value) = entry.values.first() {
            entries.push(value.to_string());
        }
    }
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
) -> Result<(), RadrootsReplicaEventsError> {
    for entry in list_set.entries.iter() {
        if entry.tag != expected {
            return Err(RadrootsReplicaEventsError::InvalidData(format!(
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
    use radroots_replica_db::{
        farm, farm_gcs_location, farm_member, farm_member_claim, farm_tag, gcs_location,
        migrations, plot, plot_gcs_location, plot_tag,
    };
    use radroots_replica_db_schema::farm::IFarmFields;
    use radroots_replica_db_schema::farm_gcs_location::IFarmGcsLocationFields;
    use radroots_replica_db_schema::farm_member::IFarmMemberFields;
    use radroots_replica_db_schema::farm_member_claim::IFarmMemberClaimFields;
    use radroots_replica_db_schema::farm_tag::IFarmTagFields;
    use radroots_replica_db_schema::gcs_location::IGcsLocationFields;
    use radroots_replica_db_schema::plot::IPlotFields;
    use radroots_replica_db_schema::plot_gcs_location::IPlotGcsLocationFields;
    use radroots_replica_db_schema::plot_tag::IPlotTagFields;
    use radroots_sql_core::{ExecOutcome, SqlExecutor, SqliteExecutor};

    struct FixedFactory;

    impl RadrootsReplicaIdFactory for FixedFactory {
        fn new_d_tag(&self) -> String {
            "AAAAAAAAAAAAAAAAAAAAAZ".to_string()
        }
    }

    struct TxnExecutor<'a> {
        inner: Option<&'a SqliteExecutor>,
        begin_err: Option<SqlError>,
        commit_err: Option<SqlError>,
        rollback_count: Arc<AtomicUsize>,
    }

    impl SqlExecutor for TxnExecutor<'_> {
        fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
            match self.inner {
                Some(inner) => inner.exec(sql, params_json),
                None => Err(SqlError::UnsupportedPlatform),
            }
        }

        fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
            match self.inner {
                Some(inner) => inner.query_raw(sql, params_json),
                None => Err(SqlError::UnsupportedPlatform),
            }
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

    struct PassExecutor<'a> {
        inner: &'a SqliteExecutor,
    }

    impl SqlExecutor for PassExecutor<'_> {
        fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
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

    struct QueryFailExecutor<'a> {
        inner: &'a SqliteExecutor,
        needle: &'static str,
        err: SqlError,
    }

    impl SqlExecutor for QueryFailExecutor<'_> {
        fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
            let normalized = sql.to_ascii_lowercase();
            if normalized.contains(self.needle) {
                return Err(self.err.clone());
            }
            self.inner.exec(sql, params_json)
        }

        fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
            let normalized = sql.to_ascii_lowercase();
            if normalized.contains(self.needle) {
                return Err(self.err.clone());
            }
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
            inner: None,
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
            radroots_replica_ingest_event_with_factory(&begin_executor, &event, &FixedFactory)
                .expect_err("begin");
        assert!(begin_err.to_string().contains("replica_sync.sql"));
        assert!(begin_executor.commit().is_ok());
        assert_eq!(
            begin_executor
                .exec("select 1", "[]")
                .expect_err("exec")
                .code(),
            "ERR_UNSUPPORTED_PLATFORM"
        );
        assert_eq!(
            begin_executor
                .query_raw("select 1", "[]")
                .expect_err("query")
                .code(),
            "ERR_UNSUPPORTED_PLATFORM"
        );

        let rollback_count = Arc::new(AtomicUsize::new(0));
        let commit_executor = TxnExecutor {
            inner: None,
            begin_err: None,
            commit_err: Some(SqlError::Internal),
            rollback_count: rollback_count.clone(),
        };
        let commit_err =
            radroots_replica_ingest_event_with_factory(&commit_executor, &event, &FixedFactory)
                .expect_err("commit");
        assert!(commit_err.to_string().contains("replica_sync.sql"));
        assert_eq!(rollback_count.load(Ordering::SeqCst), 0);

        let rollback_executor = TxnExecutor {
            inner: None,
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
        let err = radroots_replica_ingest_event_with_factory(
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

        let factory = RadrootsReplicaDefaultIdFactory;
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
            radroots_replica_ingest_event(&exec, &profile).expect("ingest wrapper"),
            RadrootsReplicaIngestOutcome::Applied
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
            RadrootsReplicaIngestOutcome::Applied
        );
        assert_eq!(
            ingest_profile_event(&exec, &profile_update).expect("profile skip"),
            RadrootsReplicaIngestOutcome::Skipped
        );
        let profile_older = profile_event(
            8,
            &profile_pubkey,
            1,
            Some(RadrootsProfileType::Individual),
            "alice-old",
        );
        let decision_old = event_state_decision(&exec, &profile_older, "").expect("decision old");
        assert!(!decision_old.apply);
        let decision_same =
            event_state_decision(&exec, &profile_update, "").expect("decision same");
        assert!(!decision_same.apply);
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
            RadrootsReplicaIngestOutcome::Applied
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
            RadrootsReplicaIngestOutcome::Applied
        );
        assert_eq!(
            ingest_farm_event(&exec, &farm_update, &FixedFactory).expect("farm skip"),
            RadrootsReplicaIngestOutcome::Skipped
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
            RadrootsReplicaIngestOutcome::Applied
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
            RadrootsReplicaIngestOutcome::Applied
        );
        assert_eq!(
            ingest_plot_event(&exec, &plot_update, &FixedFactory).expect("plot skip"),
            RadrootsReplicaIngestOutcome::Skipped
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
                RadrootsReplicaIngestOutcome::Applied
            );
            assert_eq!(
                ingest_list_set_event(&exec, &event).expect("list set skip"),
                RadrootsReplicaIngestOutcome::Skipped
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
    fn upsert_location_none_paths_are_ok() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");

        let farm_row = farm::create(
            &exec,
            &IFarmFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
                pubkey: "f".repeat(64),
                name: "farm-none".to_string(),
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
            &exec,
            &IPlotFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
                farm_id: farm_row.id.clone(),
                name: "plot-none".to_string(),
                about: None,
                location_primary: None,
                location_city: None,
                location_region: None,
                location_country: None,
            },
        )
        .expect("plot")
        .result;

        let _ = upsert_farm_location(&exec, &farm_row.id, None, &FixedFactory).expect("farm none");
        let _ = upsert_plot_location(&exec, &plot_row.id, None, &FixedFactory).expect("plot none");
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
        assert_eq!(
            not_found_claims
                .exec("DELETE FROM farm_member_claim WHERE id = 1", "[]")
                .expect_err("exec not found")
                .code(),
            "ERR_NOT_FOUND"
        );
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
        assert_eq!(point_err.to_string(), "replica_sync.data: gcs.point");

        let polygon_json_err = serde_json::from_str::<Value>("{").expect_err("invalid json");
        let polygon_err = map_gcs_polygon_serialize_error(polygon_json_err);
        assert_eq!(polygon_err.to_string(), "replica_sync.data: gcs.polygon");
    }

    #[test]
    fn ingest_pass_executor_and_parse_edge_paths_are_covered() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");
        let pass = PassExecutor { inner: &exec };

        let profile_pubkey = "p".repeat(64);
        let farm_pubkey = "f".repeat(64);
        let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
        let plot_d_tag = "AAAAAAAAAAAAAAAAAAAAAQ";

        let profile = profile_event(
            500,
            &profile_pubkey,
            50,
            Some(RadrootsProfileType::Individual),
            "pass-profile",
        );
        assert_eq!(
            radroots_replica_ingest_event_with_factory(&pass, &profile, &FixedFactory)
                .expect("profile ingest"),
            RadrootsReplicaIngestOutcome::Applied
        );
        assert_eq!(
            ingest_profile_event(&pass, &profile).expect("profile skip"),
            RadrootsReplicaIngestOutcome::Skipped
        );

        let farm = farm_event(
            501,
            &farm_pubkey,
            51,
            farm_d_tag,
            "pass-farm",
            Some(RadrootsFarmLocation {
                primary: Some("primary".to_string()),
                city: Some("city".to_string()),
                region: Some("region".to_string()),
                country: Some("country".to_string()),
                gcs: sample_gcs(10.0, 20.0, "s0"),
            }),
            Some(vec!["coffee".to_string(), "coffee".to_string()]),
        );
        assert_eq!(
            ingest_farm_event(&pass, &farm, &FixedFactory).expect("farm ingest"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let plot = plot_event(
            502,
            &farm_pubkey,
            52,
            plot_d_tag,
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "pass-plot",
            Some(RadrootsPlotLocation {
                primary: Some("plot".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(11.0, 21.0, "s1"),
            }),
            Some(vec!["orchard".to_string()]),
        );
        assert_eq!(
            ingest_plot_event(&pass, &plot, &FixedFactory).expect("plot ingest"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let members =
            farm_list_sets::farm_members_list_set(farm_d_tag, vec!["m".repeat(64)]).expect("list");
        let members_event = list_set_event(503, &farm_pubkey, 53, KIND_LIST_SET_GENERIC, &members);
        assert_eq!(
            ingest_list_set_event(&pass, &members_event).expect("members list set"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let claims = farm_list_sets::member_of_farms_list_set(vec![farm_pubkey.clone()])
            .expect("claims list set");
        let claims_event = list_set_event(504, &profile_pubkey, 54, KIND_LIST_SET_GENERIC, &claims);
        assert_eq!(
            ingest_list_set_event(&pass, &claims_event).expect("claims list set"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let farm_row = find_farm_by_ref(&pass, &farm_pubkey, farm_d_tag).expect("farm row");
        let mixed_member_entries = RadrootsListSet {
            d_tag: format!("farm:{farm_d_tag}:members"),
            content: String::new(),
            entries: vec![
                RadrootsListEntry {
                    tag: "a".to_string(),
                    values: vec!["ignored".to_string()],
                },
                RadrootsListEntry {
                    tag: "p".to_string(),
                    values: Vec::new(),
                },
                RadrootsListEntry {
                    tag: "p".to_string(),
                    values: vec!["m".repeat(64)],
                },
            ],
            title: None,
            description: None,
            image: None,
        };
        assert!(
            upsert_farm_members(
                &pass,
                &farm_row.id,
                ListSetRole::Members,
                &mixed_member_entries
            )
            .is_ok()
        );
        let mixed_claim_entries = RadrootsListSet {
            d_tag: "member_of.farms".to_string(),
            content: String::new(),
            entries: vec![
                RadrootsListEntry {
                    tag: "a".to_string(),
                    values: vec!["ignored".to_string()],
                },
                RadrootsListEntry {
                    tag: "p".to_string(),
                    values: Vec::new(),
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
        assert!(upsert_member_claims(&pass, &profile_pubkey, &mixed_claim_entries).is_ok());
        assert!(pass.begin().is_ok());
        assert!(pass.rollback().is_ok());

        assert!(parse_farm_list_set_d_tag("coop:AAAAAAAAAAAAAAAAAAAAAA:members").is_none());
        assert!(parse_farm_list_set_d_tag("farm:AAAAAAAAAAAAAAAAAAAAAA").is_none());
        assert!(parse_farm_list_set_d_tag("farm:AAAAAAAAAAAAAAAAAAAAAA:members").is_some());
    }

    #[test]
    fn create_gcs_location_success_path_is_covered() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");

        let id = create_gcs_location(&exec, sample_gcs(1.0, 2.0, "s0"), &FixedFactory)
            .expect("create gcs");
        assert!(!id.trim().is_empty());
    }

    #[test]
    fn ingest_default_factory_wrapper_paths_are_covered() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");

        let farm_pubkey = "f".repeat(64);
        let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
        let farm_create = farm_event(
            600,
            &farm_pubkey,
            60,
            farm_d_tag,
            "wrapper-farm",
            Some(RadrootsFarmLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(10.0, 20.0, "s0"),
            }),
            Some(vec!["coffee".to_string()]),
        );
        assert_eq!(
            radroots_replica_ingest_event(&exec, &farm_create).expect("farm create"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let farm_update = farm_event(
            601,
            &farm_pubkey,
            61,
            farm_d_tag,
            "wrapper-farm-updated",
            None,
            Some(vec!["market".to_string()]),
        );
        assert_eq!(
            radroots_replica_ingest_event(&exec, &farm_update).expect("farm update"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let plot_d_tag = "AAAAAAAAAAAAAAAAAAAAAQ";
        let plot_create = plot_event(
            602,
            &farm_pubkey,
            62,
            plot_d_tag,
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "wrapper-plot",
            Some(RadrootsPlotLocation {
                primary: Some("plot-primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(11.0, 21.0, "s1"),
            }),
            Some(vec!["orchard".to_string()]),
        );
        assert_eq!(
            radroots_replica_ingest_event(&exec, &plot_create).expect("plot create"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let plot_update = plot_event(
            603,
            &farm_pubkey,
            63,
            plot_d_tag,
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "wrapper-plot-updated",
            None,
            Some(vec!["updated".to_string()]),
        );
        assert_eq!(
            radroots_replica_ingest_event(&exec, &plot_update).expect("plot update"),
            RadrootsReplicaIngestOutcome::Applied
        );
    }

    #[test]
    fn ingest_txn_executor_instantiation_error_paths_are_covered() {
        let pass_db = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&pass_db).expect("migrations");
        let pass_txn = TxnExecutor {
            inner: Some(&pass_db),
            begin_err: None,
            commit_err: None,
            rollback_count: Arc::new(AtomicUsize::new(0)),
        };

        let profile_pubkey = "p".repeat(64);
        let profile_event_row = profile_event(
            700,
            &profile_pubkey,
            70,
            Some(RadrootsProfileType::Individual),
            "txn-profile",
        );
        assert_eq!(
            ingest_profile_event(&pass_txn, &profile_event_row).expect("txn profile"),
            RadrootsReplicaIngestOutcome::Applied
        );
        let profile_decision =
            event_state_decision(&pass_txn, &profile_event_row, "").expect("profile decision");
        assert!(!profile_decision.apply);
        assert!(
            radroots_replica_ingest_event_state(
                &pass_txn,
                &profile_event_row,
                "",
                &profile_decision.content_hash,
            )
            .is_ok()
        );
        assert_eq!(
            radroots_replica_ingest_event_with_factory(
                &pass_txn,
                &profile_event_row,
                &FixedFactory
            )
            .expect("txn wrapper"),
            RadrootsReplicaIngestOutcome::Skipped
        );

        let farm_pubkey = "f".repeat(64);
        let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
        let farm_event_row = farm_event(
            701,
            &farm_pubkey,
            71,
            farm_d_tag,
            "txn-farm",
            Some(RadrootsFarmLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(12.0, 22.0, "s2"),
            }),
            Some(vec!["coffee".to_string()]),
        );
        assert_eq!(
            ingest_farm_event(&pass_txn, &farm_event_row, &FixedFactory).expect("txn farm"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let plot_event_row = plot_event(
            702,
            &farm_pubkey,
            72,
            "AAAAAAAAAAAAAAAAAAAAAQ",
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "txn-plot",
            Some(RadrootsPlotLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(13.0, 23.0, "s3"),
            }),
            Some(vec!["orchard".to_string()]),
        );
        assert_eq!(
            ingest_plot_event(&pass_txn, &plot_event_row, &FixedFactory).expect("txn plot"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let farm_row = find_farm_by_ref(&pass_txn, &farm_pubkey, farm_d_tag).expect("find farm");
        assert!(upsert_farm_tags(&pass_txn, &farm_row.id, Some(vec!["x".to_string()])).is_ok());
        let plot_id = plot::find_many(&pass_db, &IPlotFindMany { filter: None })
            .expect("plots")
            .results[0]
            .id
            .clone();
        assert!(upsert_plot_tags(&pass_txn, &plot_id, Some(vec!["y".to_string()])).is_ok());
        assert!(clear_farm_locations(&pass_txn, &farm_row.id).is_ok());
        assert!(clear_plot_locations(&pass_txn, &plot_id).is_ok());
        assert!(
            create_gcs_location(&pass_txn, sample_gcs(14.0, 24.0, "s4"), &FixedFactory).is_ok()
        );
        assert!(
            upsert_farm_location(
                &pass_txn,
                &farm_row.id,
                Some(RadrootsFarmLocation {
                    primary: Some("primary".to_string()),
                    city: None,
                    region: None,
                    country: None,
                    gcs: sample_gcs(15.0, 25.0, "s5"),
                }),
                &FixedFactory,
            )
            .is_ok()
        );
        assert!(
            upsert_plot_location(
                &pass_txn,
                &plot_id,
                Some(RadrootsPlotLocation {
                    primary: Some("primary".to_string()),
                    city: None,
                    region: None,
                    country: None,
                    gcs: sample_gcs(16.0, 26.0, "s6"),
                }),
                &FixedFactory,
            )
            .is_ok()
        );
        let members_list =
            farm_list_sets::farm_members_list_set(farm_d_tag, vec!["m".repeat(64)]).expect("list");
        assert!(
            upsert_farm_members(&pass_txn, &farm_row.id, ListSetRole::Members, &members_list)
                .is_ok()
        );
        let member_of_list =
            farm_list_sets::member_of_farms_list_set(vec![farm_pubkey.clone()]).expect("member_of");
        assert!(upsert_member_claims(&pass_txn, &"m".repeat(64), &member_of_list).is_ok());

        let rollback_count = Arc::new(AtomicUsize::new(0));
        let txn = TxnExecutor {
            inner: None,
            begin_err: None,
            commit_err: None,
            rollback_count,
        };

        assert!(ingest_profile_event(&txn, &profile_event_row).is_err());
        assert!(event_state_decision(&txn, &profile_event_row, "").is_err());
        assert!(radroots_replica_ingest_event_state(&txn, &profile_event_row, "", "hash").is_err());
        assert!(
            radroots_replica_ingest_event_with_factory(&txn, &profile_event_row, &FixedFactory)
                .is_err()
        );

        assert!(ingest_farm_event(&txn, &farm_event_row, &FixedFactory).is_err());

        assert!(ingest_plot_event(&txn, &plot_event_row, &FixedFactory).is_err());

        assert!(find_farm_by_ref(&txn, &farm_pubkey, farm_d_tag).is_err());
        assert!(upsert_farm_tags(&txn, "farm-id", Some(vec!["x".to_string()])).is_err());
        assert!(upsert_plot_tags(&txn, "plot-id", Some(vec!["y".to_string()])).is_err());
        assert!(clear_farm_locations(&txn, "farm-id").is_err());
        assert!(clear_plot_locations(&txn, "plot-id").is_err());
        assert!(create_gcs_location(&txn, sample_gcs(14.0, 24.0, "s4"), &FixedFactory).is_err());
        assert!(
            upsert_farm_location(
                &txn,
                "farm-id",
                Some(RadrootsFarmLocation {
                    primary: Some("primary".to_string()),
                    city: None,
                    region: None,
                    country: None,
                    gcs: sample_gcs(15.0, 25.0, "s5"),
                }),
                &FixedFactory,
            )
            .is_err()
        );
        assert!(
            upsert_plot_location(
                &txn,
                "plot-id",
                Some(RadrootsPlotLocation {
                    primary: Some("primary".to_string()),
                    city: None,
                    region: None,
                    country: None,
                    gcs: sample_gcs(16.0, 26.0, "s6"),
                }),
                &FixedFactory,
            )
            .is_err()
        );
        assert!(upsert_farm_members(&txn, "farm-id", ListSetRole::Members, &members_list).is_err());
        assert!(upsert_member_claims(&txn, &"m".repeat(64), &member_of_list).is_err());
    }

    #[test]
    fn ingest_sqlite_queryfail_and_parser_edges_are_covered() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");
        let pass_through = QueryFailExecutor {
            inner: &exec,
            needle: "__missing__",
            err: SqlError::Internal,
        };
        assert!(pass_through.query_raw("select 1", "[]").is_ok());
        assert!(
            pass_through
                .exec(
                    "create table if not exists coverage_probe (id integer)",
                    "[]"
                )
                .is_ok()
        );
        let _ = pass_through.begin();
        let _ = pass_through.rollback();
        let _ = pass_through.commit();

        let farm_pubkey = "f".repeat(64);
        let farm_d_tag = "AAAAAAAAAAAAAAAAAAAAAA";
        let plot_d_tag = "AAAAAAAAAAAAAAAAAAAAAQ";
        let profile_pubkey = "p".repeat(64);

        let profile = profile_event(
            800,
            &profile_pubkey,
            80,
            Some(RadrootsProfileType::Individual),
            "profile-base",
        );
        let mut profile_bad_content = profile.clone();
        profile_bad_content.content = "{".to_string();
        assert!(ingest_profile_event(&exec, &profile_bad_content).is_err());

        let profile_query_fail = QueryFailExecutor {
            inner: &exec,
            needle: "nostr_profile",
            err: SqlError::Internal,
        };
        assert!(ingest_profile_event(&profile_query_fail, &profile).is_err());

        assert_eq!(
            ingest_profile_event(&exec, &profile).expect("profile seed"),
            RadrootsReplicaIngestOutcome::Applied
        );
        let profile_update = profile_event(
            801,
            &profile_pubkey,
            81,
            Some(RadrootsProfileType::Individual),
            "profile-update",
        );
        let profile_update_fail = QueryFailExecutor {
            inner: &exec,
            needle: "update nostr_profile",
            err: SqlError::Internal,
        };
        assert!(ingest_profile_event(&profile_update_fail, &profile_update).is_err());

        let profile_create_fail = QueryFailExecutor {
            inner: &exec,
            needle: "insert into nostr_profile",
            err: SqlError::Internal,
        };
        let profile_new = profile_event(
            802,
            &"n".repeat(64),
            82,
            Some(RadrootsProfileType::Individual),
            "profile-new",
        );
        assert!(ingest_profile_event(&profile_create_fail, &profile_new).is_err());

        let profile_state_fail = QueryFailExecutor {
            inner: &exec,
            needle: "nostr_event_state",
            err: SqlError::Internal,
        };
        let profile_state_event = profile_event(
            803,
            &"s".repeat(64),
            83,
            Some(RadrootsProfileType::Individual),
            "profile-state",
        );
        assert!(ingest_profile_event(&profile_state_fail, &profile_state_event).is_err());

        let farm_seed = farm_event(
            810,
            &farm_pubkey,
            90,
            farm_d_tag,
            "farm-seed",
            Some(RadrootsFarmLocation {
                primary: Some("primary".to_string()),
                city: Some("city".to_string()),
                region: Some("region".to_string()),
                country: Some("country".to_string()),
                gcs: sample_gcs(10.0, 20.0, "s0"),
            }),
            Some(vec!["seed".to_string()]),
        );
        assert_eq!(
            ingest_farm_event(&exec, &farm_seed, &FixedFactory).expect("farm seed"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let mut farm_bad_content = farm_seed.clone();
        farm_bad_content.content = "{".to_string();
        assert!(ingest_farm_event(&exec, &farm_bad_content, &FixedFactory).is_err());

        let farm_query_fail = QueryFailExecutor {
            inner: &exec,
            needle: "from farm",
            err: SqlError::Internal,
        };
        let farm_query_event = farm_event(
            811,
            &"q".repeat(64),
            91,
            farm_d_tag,
            "farm-query",
            None,
            None,
        );
        assert!(ingest_farm_event(&farm_query_fail, &farm_query_event, &FixedFactory).is_err());

        let farm_update_fail = QueryFailExecutor {
            inner: &exec,
            needle: "update farm",
            err: SqlError::Internal,
        };
        let farm_update = farm_event(
            812,
            &farm_pubkey,
            92,
            farm_d_tag,
            "farm-update",
            None,
            Some(vec!["u".to_string()]),
        );
        assert!(ingest_farm_event(&farm_update_fail, &farm_update, &FixedFactory).is_err());

        let farm_create_fail = QueryFailExecutor {
            inner: &exec,
            needle: "insert into farm",
            err: SqlError::Internal,
        };
        let farm_create = farm_event(
            813,
            &"c".repeat(64),
            93,
            farm_d_tag,
            "farm-create",
            None,
            None,
        );
        assert!(ingest_farm_event(&farm_create_fail, &farm_create, &FixedFactory).is_err());

        let farm_tag_fail = QueryFailExecutor {
            inner: &exec,
            needle: "farm_tag",
            err: SqlError::Internal,
        };
        let farm_tag_event = farm_event(
            814,
            &"t".repeat(64),
            94,
            farm_d_tag,
            "farm-tag",
            None,
            Some(vec!["coffee".to_string()]),
        );
        assert!(ingest_farm_event(&farm_tag_fail, &farm_tag_event, &FixedFactory).is_err());

        let farm_gcs_fail = QueryFailExecutor {
            inner: &exec,
            needle: "gcs_location",
            err: SqlError::Internal,
        };
        let farm_gcs_event = farm_event(
            815,
            &"g".repeat(64),
            95,
            farm_d_tag,
            "farm-gcs",
            Some(RadrootsFarmLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(11.0, 21.0, "s1"),
            }),
            None,
        );
        assert!(ingest_farm_event(&farm_gcs_fail, &farm_gcs_event, &FixedFactory).is_err());

        let farm_rel_fail = QueryFailExecutor {
            inner: &exec,
            needle: "farm_gcs_location",
            err: SqlError::Internal,
        };
        let farm_rel_event = farm_event(
            816,
            &"r".repeat(64),
            96,
            farm_d_tag,
            "farm-rel",
            Some(RadrootsFarmLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(12.0, 22.0, "s2"),
            }),
            None,
        );
        assert!(ingest_farm_event(&farm_rel_fail, &farm_rel_event, &FixedFactory).is_err());

        let farm_state_fail = QueryFailExecutor {
            inner: &exec,
            needle: "nostr_event_state",
            err: SqlError::Internal,
        };
        let farm_state_event = farm_event(
            817,
            &"w".repeat(64),
            97,
            farm_d_tag,
            "farm-state",
            None,
            None,
        );
        assert!(ingest_farm_event(&farm_state_fail, &farm_state_event, &FixedFactory).is_err());

        let mut bad_point = sample_gcs(13.0, 23.0, "s3");
        bad_point.point.coordinates = [f64::NAN, 13.0];
        let farm_bad_point = farm_event(
            818,
            &"x".repeat(64),
            98,
            farm_d_tag,
            "farm-bad-point",
            Some(RadrootsFarmLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: bad_point,
            }),
            None,
        );
        assert!(ingest_farm_event(&exec, &farm_bad_point, &FixedFactory).is_err());

        let mut bad_polygon = sample_gcs(14.0, 24.0, "s4");
        bad_polygon.polygon.coordinates[0][1][0] = f64::NAN;
        let farm_bad_polygon = farm_event(
            819,
            &"y".repeat(64),
            99,
            farm_d_tag,
            "farm-bad-polygon",
            Some(RadrootsFarmLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: bad_polygon,
            }),
            None,
        );
        assert!(ingest_farm_event(&exec, &farm_bad_polygon, &FixedFactory).is_err());

        let plot_seed = plot_event(
            820,
            &farm_pubkey,
            100,
            plot_d_tag,
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-seed",
            Some(RadrootsPlotLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(15.0, 25.0, "s5"),
            }),
            Some(vec!["orchard".to_string()]),
        );
        assert_eq!(
            ingest_plot_event(&exec, &plot_seed, &FixedFactory).expect("plot seed"),
            RadrootsReplicaIngestOutcome::Applied
        );

        let mut plot_bad_content = plot_seed.clone();
        plot_bad_content.content = "{".to_string();
        assert!(ingest_plot_event(&exec, &plot_bad_content, &FixedFactory).is_err());

        let plot_query_fail = QueryFailExecutor {
            inner: &exec,
            needle: "from plot",
            err: SqlError::Internal,
        };
        let plot_query = plot_event(
            821,
            &farm_pubkey,
            101,
            plot_d_tag,
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-query",
            None,
            None,
        );
        assert!(ingest_plot_event(&plot_query_fail, &plot_query, &FixedFactory).is_err());

        let plot_update_fail = QueryFailExecutor {
            inner: &exec,
            needle: "update plot",
            err: SqlError::Internal,
        };
        let plot_update = plot_event(
            822,
            &farm_pubkey,
            102,
            plot_d_tag,
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-update",
            None,
            Some(vec!["u".to_string()]),
        );
        assert!(ingest_plot_event(&plot_update_fail, &plot_update, &FixedFactory).is_err());

        let plot_create_fail = QueryFailExecutor {
            inner: &exec,
            needle: "insert into plot",
            err: SqlError::Internal,
        };
        let plot_create = plot_event(
            823,
            &farm_pubkey,
            103,
            "AAAAAAAAAAAAAAAAAAAAAg",
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-create",
            None,
            None,
        );
        assert!(ingest_plot_event(&plot_create_fail, &plot_create, &FixedFactory).is_err());

        let plot_tag_fail = QueryFailExecutor {
            inner: &exec,
            needle: "plot_tag",
            err: SqlError::Internal,
        };
        let plot_tag_event = plot_event(
            824,
            &farm_pubkey,
            104,
            "AAAAAAAAAAAAAAAAAAAAAw",
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-tag",
            None,
            Some(vec!["tag".to_string()]),
        );
        assert!(ingest_plot_event(&plot_tag_fail, &plot_tag_event, &FixedFactory).is_err());

        let plot_gcs_fail = QueryFailExecutor {
            inner: &exec,
            needle: "gcs_location",
            err: SqlError::Internal,
        };
        let plot_gcs_event = plot_event(
            825,
            &farm_pubkey,
            105,
            "AAAAAAAAAAAAAAAAAAAAAw",
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-gcs",
            Some(RadrootsPlotLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(16.0, 26.0, "s6"),
            }),
            None,
        );
        assert!(ingest_plot_event(&plot_gcs_fail, &plot_gcs_event, &FixedFactory).is_err());

        let plot_rel_fail = QueryFailExecutor {
            inner: &exec,
            needle: "plot_gcs_location",
            err: SqlError::Internal,
        };
        let plot_rel_event = plot_event(
            826,
            &farm_pubkey,
            106,
            "AAAAAAAAAAAAAAAAAAAAAw",
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-rel",
            Some(RadrootsPlotLocation {
                primary: Some("primary".to_string()),
                city: None,
                region: None,
                country: None,
                gcs: sample_gcs(17.0, 27.0, "s7"),
            }),
            None,
        );
        assert!(ingest_plot_event(&plot_rel_fail, &plot_rel_event, &FixedFactory).is_err());

        let plot_state_fail = QueryFailExecutor {
            inner: &exec,
            needle: "nostr_event_state",
            err: SqlError::Internal,
        };
        let plot_state_event = plot_event(
            827,
            &farm_pubkey,
            107,
            "AAAAAAAAAAAAAAAAAAAAAw",
            RadrootsFarmRef {
                pubkey: farm_pubkey.clone(),
                d_tag: farm_d_tag.to_string(),
            },
            "plot-state",
            None,
            None,
        );
        assert!(ingest_plot_event(&plot_state_fail, &plot_state_event, &FixedFactory).is_err());

        let mut list_decode_fail = profile_event(
            830,
            &farm_pubkey,
            108,
            Some(RadrootsProfileType::Farm),
            "unused",
        );
        list_decode_fail.kind = KIND_LIST_SET_GENERIC;
        list_decode_fail.content = "{".to_string();
        list_decode_fail.tags = Vec::new();
        assert!(ingest_list_set_event(&exec, &list_decode_fail).is_err());

        let members_list = farm_list_sets::farm_members_list_set(farm_d_tag, vec!["m".repeat(64)])
            .expect("members list");
        let member_event =
            list_set_event(831, &farm_pubkey, 109, KIND_LIST_SET_GENERIC, &members_list);
        let list_decision_fail = QueryFailExecutor {
            inner: &exec,
            needle: "nostr_event_state",
            err: SqlError::Internal,
        };
        assert!(ingest_list_set_event(&list_decision_fail, &member_event).is_err());

        let member_of =
            farm_list_sets::member_of_farms_list_set(vec![farm_pubkey.clone()]).expect("member-of");
        let member_of_event =
            list_set_event(832, &"m".repeat(64), 110, KIND_LIST_SET_GENERIC, &member_of);
        let claims_fail = QueryFailExecutor {
            inner: &exec,
            needle: "farm_member_claim",
            err: SqlError::Internal,
        };
        assert!(ingest_list_set_event(&claims_fail, &member_of_event).is_err());

        let claims_state_fail = QueryFailExecutor {
            inner: &exec,
            needle: "nostr_event_state",
            err: SqlError::Internal,
        };
        assert!(ingest_list_set_event(&claims_state_fail, &member_of_event).is_err());

        let plots_list = farm_list_sets::farm_plots_list_set(
            farm_d_tag,
            &farm_pubkey,
            vec![plot_d_tag.to_string()],
        )
        .expect("plots list");
        let plots_event =
            list_set_event(833, &farm_pubkey, 111, KIND_LIST_SET_GENERIC, &plots_list);
        let plots_state_fail = QueryFailExecutor {
            inner: &exec,
            needle: "nostr_event_state",
            err: SqlError::Internal,
        };
        assert!(ingest_list_set_event(&plots_state_fail, &plots_event).is_err());

        let missing_farm_members =
            farm_list_sets::farm_members_list_set(farm_d_tag, vec!["n".repeat(64)]).expect("list");
        let missing_farm_event = list_set_event(
            834,
            &"z".repeat(64),
            112,
            KIND_LIST_SET_GENERIC,
            &missing_farm_members,
        );
        assert!(ingest_list_set_event(&exec, &missing_farm_event).is_err());

        let members_create_fail = QueryFailExecutor {
            inner: &exec,
            needle: "farm_member",
            err: SqlError::Internal,
        };
        assert!(ingest_list_set_event(&members_create_fail, &member_event).is_err());

        let members_state_fail = QueryFailExecutor {
            inner: &exec,
            needle: "nostr_event_state",
            err: SqlError::Internal,
        };
        assert!(ingest_list_set_event(&members_state_fail, &member_event).is_err());

        assert!(parse_farm_list_set_d_tag("").is_none());
        assert!(parse_farm_list_set_d_tag("farm").is_none());

        let state_create_fail = QueryFailExecutor {
            inner: &exec,
            needle: "nostr_event_state",
            err: SqlError::Internal,
        };
        assert!(
            radroots_replica_ingest_event_state(&state_create_fail, &profile, "", "hash").is_err()
        );

        radroots_replica_ingest_event_state(&exec, &profile, "", "hash").expect("seed state");
        let state_update_fail = QueryFailExecutor {
            inner: &exec,
            needle: "update nostr_event_state",
            err: SqlError::Internal,
        };
        assert!(
            radroots_replica_ingest_event_state(&state_update_fail, &profile, "", "hash2").is_err()
        );
    }
}
