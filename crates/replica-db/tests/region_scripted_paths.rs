use std::collections::VecDeque;
use std::sync::Mutex;

use radroots_replica_db::{ExecOutcome, ReplicaSql, SqlError, SqlExecutor};
use radroots_replica_db_schema::farm::{
    IFarmCreate, IFarmDelete, IFarmFindMany, IFarmFindOne, IFarmUpdate,
};
use radroots_replica_db_schema::farm_gcs_location::{
    IFarmGcsLocationCreate, IFarmGcsLocationDelete, IFarmGcsLocationFindMany,
    IFarmGcsLocationFindOne, IFarmGcsLocationUpdate,
};
use radroots_replica_db_schema::farm_member::{
    IFarmMemberCreate, IFarmMemberDelete, IFarmMemberFindMany, IFarmMemberFindOne,
    IFarmMemberUpdate,
};
use radroots_replica_db_schema::farm_member_claim::{
    IFarmMemberClaimCreate, IFarmMemberClaimDelete, IFarmMemberClaimFindMany,
    IFarmMemberClaimFindOne, IFarmMemberClaimUpdate,
};
use radroots_replica_db_schema::farm_tag::{
    IFarmTagCreate, IFarmTagDelete, IFarmTagFindMany, IFarmTagFindOne, IFarmTagUpdate,
};
use radroots_replica_db_schema::gcs_location::{
    GcsLocationFarmArgs, GcsLocationFindManyRel, GcsLocationPlotArgs, GcsLocationTradeProductArgs,
    IGcsLocationCreate, IGcsLocationDelete, IGcsLocationFindMany, IGcsLocationFindOne,
    IGcsLocationFindOneRelArgs, IGcsLocationUpdate,
};
use radroots_replica_db_schema::log_error::{
    ILogErrorCreate, ILogErrorDelete, ILogErrorFindMany, ILogErrorFindOne, ILogErrorUpdate,
};
use radroots_replica_db_schema::media_image::{
    IMediaImageCreate, IMediaImageDelete, IMediaImageFindMany, IMediaImageFindOne,
    IMediaImageFindOneRelArgs, IMediaImageUpdate, MediaImageFindManyRel,
    MediaImageTradeProductArgs,
};
use radroots_replica_db_schema::nostr_event_state::{
    INostrEventStateCreate, INostrEventStateDelete, INostrEventStateFindMany,
    INostrEventStateFindOne, INostrEventStateUpdate,
};
use radroots_replica_db_schema::nostr_profile::{
    INostrProfileCreate, INostrProfileDelete, INostrProfileFindMany, INostrProfileFindOne,
    INostrProfileFindOneRelArgs, INostrProfileUpdate, NostrProfileFindManyRel,
    NostrProfileRelayArgs,
};
use radroots_replica_db_schema::nostr_relay::{
    INostrRelayCreate, INostrRelayDelete, INostrRelayFindMany, INostrRelayFindOne,
    INostrRelayFindOneRelArgs, INostrRelayUpdate, NostrRelayFindManyRel, NostrRelayProfileArgs,
};
use radroots_replica_db_schema::plot::{
    IPlotCreate, IPlotDelete, IPlotFindMany, IPlotFindOne, IPlotUpdate,
};
use radroots_replica_db_schema::plot_gcs_location::{
    IPlotGcsLocationCreate, IPlotGcsLocationDelete, IPlotGcsLocationFindMany,
    IPlotGcsLocationFindOne, IPlotGcsLocationUpdate,
};
use radroots_replica_db_schema::plot_tag::{
    IPlotTagCreate, IPlotTagDelete, IPlotTagFindMany, IPlotTagFindOne, IPlotTagUpdate,
};
use radroots_replica_db_schema::trade_product::{
    ITradeProductCreate, ITradeProductFindMany, ITradeProductFindOne, ITradeProductUpdate,
};
use radroots_types::types::IError;
use serde::de::DeserializeOwned;
use serde_json::json;

struct ScriptedExecutor {
    exec_results: Mutex<VecDeque<Result<ExecOutcome, SqlError>>>,
    query_results: Mutex<VecDeque<Result<String, SqlError>>>,
    begin_results: Mutex<VecDeque<Result<(), SqlError>>>,
    commit_results: Mutex<VecDeque<Result<(), SqlError>>>,
    rollback_results: Mutex<VecDeque<Result<(), SqlError>>>,
}

impl ScriptedExecutor {
    fn new(
        exec_results: Vec<Result<ExecOutcome, SqlError>>,
        query_results: Vec<Result<String, SqlError>>,
    ) -> Self {
        Self {
            exec_results: Mutex::new(VecDeque::from(exec_results)),
            query_results: Mutex::new(VecDeque::from(query_results)),
            begin_results: Mutex::new(VecDeque::new()),
            commit_results: Mutex::new(VecDeque::new()),
            rollback_results: Mutex::new(VecDeque::new()),
        }
    }
}

impl SqlExecutor for ScriptedExecutor {
    fn exec(&self, _sql: &str, _params_json: &str) -> Result<ExecOutcome, SqlError> {
        if let Some(result) = self
            .exec_results
            .lock()
            .expect("lock exec queue")
            .pop_front()
        {
            result
        } else {
            Ok(ExecOutcome {
                changes: 1,
                last_insert_id: 1,
            })
        }
    }

    fn query_raw(&self, _sql: &str, _params_json: &str) -> Result<String, SqlError> {
        if let Some(result) = self
            .query_results
            .lock()
            .expect("lock query queue")
            .pop_front()
        {
            result
        } else {
            Ok(String::from("[]"))
        }
    }

    fn begin(&self) -> Result<(), SqlError> {
        if let Some(result) = self
            .begin_results
            .lock()
            .expect("lock begin queue")
            .pop_front()
        {
            result
        } else {
            Ok(())
        }
    }

    fn commit(&self) -> Result<(), SqlError> {
        if let Some(result) = self
            .commit_results
            .lock()
            .expect("lock commit queue")
            .pop_front()
        {
            result
        } else {
            Ok(())
        }
    }

    fn rollback(&self) -> Result<(), SqlError> {
        if let Some(result) = self
            .rollback_results
            .lock()
            .expect("lock rollback queue")
            .pop_front()
        {
            result
        } else {
            Ok(())
        }
    }
}

fn parse_json<T: DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("valid test payload")
}

fn hex64(ch: char) -> String {
    std::iter::repeat_n(ch, 64).collect()
}

fn err_query() -> Result<String, SqlError> {
    Err(SqlError::InvalidQuery(String::from("forced query error")))
}

fn bad_json() -> Result<String, SqlError> {
    Ok(String::from("{"))
}

fn ok_rows() -> Result<String, SqlError> {
    Ok(String::from("[]"))
}

fn ok_exec() -> Result<ExecOutcome, SqlError> {
    Ok(ExecOutcome {
        changes: 1,
        last_insert_id: 1,
    })
}

fn db_with_scripts(
    exec_results: Vec<Result<ExecOutcome, SqlError>>,
    query_results: Vec<Result<String, SqlError>>,
) -> ReplicaSql<ScriptedExecutor> {
    ReplicaSql::new(ScriptedExecutor::new(exec_results, query_results))
}

fn assert_ierror_code<T>(result: Result<T, IError<SqlError>>, code: &str) {
    let err = match result {
        Ok(_) => panic!("expected ierror"),
        Err(err) => err,
    };
    assert_eq!(err.err.code(), code);
}

macro_rules! assert_secondary_model_paths {
    (
        $test_name:ident,
        $create_ty:ty, $create_json:expr, $create_call:ident,
        $find_many_ty:ty, $find_many_json:expr, $find_many_call:ident,
        $find_one_ty:ty, $find_one_json:expr, $find_one_call:ident,
        $update_ty:ty, $update_id_json:expr, $update_lookup_json:expr, $update_call:ident,
        $delete_ty:ty, $delete_lookup_json:expr, $delete_call:ident
    ) => {
        #[test]
        fn $test_name() {
            let create_opts: $create_ty = parse_json($create_json);

            let db = db_with_scripts(vec![ok_exec()], vec![err_query()]);
            assert_ierror_code(db.$create_call(&create_opts), "ERR_INVALID_QUERY");

            let db = db_with_scripts(vec![ok_exec()], vec![ok_rows()]);
            assert_ierror_code(db.$create_call(&create_opts), "ERR_NOT_FOUND");

            let find_many_opts: $find_many_ty = parse_json($find_many_json);
            let db = db_with_scripts(vec![], vec![bad_json()]);
            assert_ierror_code(db.$find_many_call(&find_many_opts), "ERR_SERIALIZATION");

            let find_one_opts: $find_one_ty = parse_json($find_one_json);
            let db = db_with_scripts(vec![], vec![bad_json()]);
            assert_ierror_code(db.$find_one_call(&find_one_opts), "ERR_SERIALIZATION");

            let update_id_opts: $update_ty = parse_json($update_id_json);
            let db = db_with_scripts(vec![ok_exec()], vec![err_query()]);
            assert_ierror_code(db.$update_call(&update_id_opts), "ERR_INVALID_QUERY");

            let db = db_with_scripts(vec![ok_exec()], vec![bad_json()]);
            assert_ierror_code(db.$update_call(&update_id_opts), "ERR_SERIALIZATION");

            let update_lookup_opts: $update_ty = parse_json($update_lookup_json);
            let db = db_with_scripts(vec![], vec![err_query()]);
            assert_ierror_code(db.$update_call(&update_lookup_opts), "ERR_INVALID_QUERY");

            let delete_lookup_opts: $delete_ty = parse_json($delete_lookup_json);
            let db = db_with_scripts(vec![], vec![err_query()]);
            assert_ierror_code(db.$delete_call(&delete_lookup_opts), "ERR_INVALID_QUERY");
        }
    };
}

macro_rules! assert_rel_model_paths {
    (
        $test_name:ident,
        $create_ty:ty, $create_json:expr, $create_call:ident,
        $find_many_ty:ty, $find_many_filter_json:expr, $find_many_rel_expr:expr, $find_many_call:ident,
        $find_one_ty:ty, $find_one_on_json:expr, $find_one_rel_expr:expr, $find_one_call:ident,
        $update_ty:ty, $update_id_json:expr, $update_lookup_json:expr, $update_call:ident,
        $delete_ty:ty, $delete_lookup_json:expr, $delete_rel_expr:expr, $delete_call:ident
    ) => {
        #[test]
        fn $test_name() {
            let create_opts: $create_ty = parse_json($create_json);

            let db = db_with_scripts(vec![ok_exec()], vec![err_query()]);
            assert_ierror_code(db.$create_call(&create_opts), "ERR_INVALID_QUERY");

            let db = db_with_scripts(vec![ok_exec()], vec![ok_rows()]);
            assert_ierror_code(db.$create_call(&create_opts), "ERR_NOT_FOUND");

            let find_many_filter_opts: $find_many_ty = parse_json($find_many_filter_json);
            let db = db_with_scripts(vec![], vec![bad_json()]);
            assert_ierror_code(
                db.$find_many_call(&find_many_filter_opts),
                "ERR_SERIALIZATION",
            );

            let find_many_rel_opts: $find_many_ty = $find_many_rel_expr;
            let db = db_with_scripts(vec![], vec![bad_json()]);
            assert_ierror_code(db.$find_many_call(&find_many_rel_opts), "ERR_SERIALIZATION");

            let find_many_rel_opts: $find_many_ty = $find_many_rel_expr;
            let db = db_with_scripts(vec![], vec![err_query()]);
            assert_ierror_code(db.$find_many_call(&find_many_rel_opts), "ERR_INVALID_QUERY");

            let find_one_on_opts: $find_one_ty = parse_json($find_one_on_json);
            let db = db_with_scripts(vec![], vec![bad_json()]);
            assert_ierror_code(db.$find_one_call(&find_one_on_opts), "ERR_SERIALIZATION");

            let find_one_rel_opts: $find_one_ty = $find_one_rel_expr;
            let db = db_with_scripts(vec![], vec![bad_json()]);
            assert_ierror_code(db.$find_one_call(&find_one_rel_opts), "ERR_SERIALIZATION");

            let update_id_opts: $update_ty = parse_json($update_id_json);
            let db = db_with_scripts(vec![ok_exec()], vec![err_query()]);
            assert_ierror_code(db.$update_call(&update_id_opts), "ERR_INVALID_QUERY");

            let db = db_with_scripts(vec![ok_exec()], vec![bad_json()]);
            assert_ierror_code(db.$update_call(&update_id_opts), "ERR_SERIALIZATION");

            let update_lookup_opts: $update_ty = parse_json($update_lookup_json);
            let db = db_with_scripts(vec![], vec![err_query()]);
            assert_ierror_code(db.$update_call(&update_lookup_opts), "ERR_INVALID_QUERY");

            let delete_lookup_opts: $delete_ty = parse_json($delete_lookup_json);
            let db = db_with_scripts(vec![], vec![err_query()]);
            assert_ierror_code(db.$delete_call(&delete_lookup_opts), "ERR_INVALID_QUERY");

            let delete_rel_opts: $delete_ty = $delete_rel_expr;
            let db = db_with_scripts(vec![], vec![err_query()]);
            assert_ierror_code(db.$delete_call(&delete_rel_opts), "ERR_INVALID_QUERY");
        }
    };
}

macro_rules! assert_trade_product_paths {
    (
        $test_name:ident,
        $create_ty:ty, $create_json:expr, $create_call:ident,
        $find_many_ty:ty, $find_many_json:expr, $find_many_call:ident,
        $find_one_ty:ty, $find_one_json:expr, $find_one_call:ident,
        $update_ty:ty, $update_json:expr, $update_call:ident
    ) => {
        #[test]
        fn $test_name() {
            let create_opts: $create_ty = parse_json($create_json);

            let db = db_with_scripts(vec![ok_exec()], vec![err_query()]);
            assert_ierror_code(db.$create_call(&create_opts), "ERR_INVALID_QUERY");

            let db = db_with_scripts(vec![ok_exec()], vec![ok_rows()]);
            assert_ierror_code(db.$create_call(&create_opts), "ERR_NOT_FOUND");

            let find_many_opts: $find_many_ty = parse_json($find_many_json);
            let db = db_with_scripts(vec![], vec![bad_json()]);
            assert_ierror_code(db.$find_many_call(&find_many_opts), "ERR_SERIALIZATION");

            let find_one_opts: $find_one_ty = parse_json($find_one_json);
            let db = db_with_scripts(vec![], vec![bad_json()]);
            assert_ierror_code(db.$find_one_call(&find_one_opts), "ERR_SERIALIZATION");

            let update_opts: $update_ty = parse_json($update_json);
            let db = db_with_scripts(vec![ok_exec()], vec![err_query()]);
            assert_ierror_code(db.$update_call(&update_opts), "ERR_INVALID_QUERY");

            let db = db_with_scripts(vec![ok_exec()], vec![bad_json()]);
            assert_ierror_code(db.$update_call(&update_opts), "ERR_SERIALIZATION");
        }
    };
}

assert_secondary_model_paths!(
    farm_scripted_region_paths,
    IFarmCreate,
    json!({ "d_tag": "farm-a", "pubkey": hex64('a'), "name": "farm a" }),
    farm_create,
    IFarmFindMany,
    json!({ "filter": { "id": "id-1" } }),
    farm_find_many,
    IFarmFindOne,
    json!({ "on": { "id": "id-1" } }),
    farm_find_one,
    IFarmUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "name": "farm z" } }),
    json!({ "on": { "d_tag": "farm-a" }, "fields": { "name": "farm y" } }),
    farm_update,
    IFarmDelete,
    json!({ "on": { "d_tag": "farm-a" } }),
    farm_delete
);

assert_secondary_model_paths!(
    plot_scripted_region_paths,
    IPlotCreate,
    json!({ "d_tag": "plot-a", "farm_id": "farm-1", "name": "plot a" }),
    plot_create,
    IPlotFindMany,
    json!({ "filter": { "id": "id-1" } }),
    plot_find_many,
    IPlotFindOne,
    json!({ "on": { "id": "id-1" } }),
    plot_find_one,
    IPlotUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "name": "plot z" } }),
    json!({ "on": { "d_tag": "plot-a" }, "fields": { "name": "plot y" } }),
    plot_update,
    IPlotDelete,
    json!({ "on": { "d_tag": "plot-a" } }),
    plot_delete
);

assert_secondary_model_paths!(
    farm_gcs_location_scripted_region_paths,
    IFarmGcsLocationCreate,
    json!({ "farm_id": "farm-1", "gcs_location_id": "gcs-1", "role": "primary" }),
    farm_gcs_location_create,
    IFarmGcsLocationFindMany,
    json!({ "filter": { "id": "id-1" } }),
    farm_gcs_location_find_many,
    IFarmGcsLocationFindOne,
    json!({ "on": { "id": "id-1" } }),
    farm_gcs_location_find_one,
    IFarmGcsLocationUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "role": "z" } }),
    json!({ "on": { "farm_id": "farm-1" }, "fields": { "role": "y" } }),
    farm_gcs_location_update,
    IFarmGcsLocationDelete,
    json!({ "on": { "farm_id": "farm-1" } }),
    farm_gcs_location_delete
);

assert_secondary_model_paths!(
    plot_gcs_location_scripted_region_paths,
    IPlotGcsLocationCreate,
    json!({ "plot_id": "plot-1", "gcs_location_id": "gcs-1", "role": "primary" }),
    plot_gcs_location_create,
    IPlotGcsLocationFindMany,
    json!({ "filter": { "id": "id-1" } }),
    plot_gcs_location_find_many,
    IPlotGcsLocationFindOne,
    json!({ "on": { "id": "id-1" } }),
    plot_gcs_location_find_one,
    IPlotGcsLocationUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "role": "z" } }),
    json!({ "on": { "plot_id": "plot-1" }, "fields": { "role": "y" } }),
    plot_gcs_location_update,
    IPlotGcsLocationDelete,
    json!({ "on": { "plot_id": "plot-1" } }),
    plot_gcs_location_delete
);

assert_secondary_model_paths!(
    farm_tag_scripted_region_paths,
    IFarmTagCreate,
    json!({ "farm_id": "farm-1", "tag": "organic" }),
    farm_tag_create,
    IFarmTagFindMany,
    json!({ "filter": { "id": "id-1" } }),
    farm_tag_find_many,
    IFarmTagFindOne,
    json!({ "on": { "id": "id-1" } }),
    farm_tag_find_one,
    IFarmTagUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "tag": "z" } }),
    json!({ "on": { "farm_id": "farm-1" }, "fields": { "tag": "y" } }),
    farm_tag_update,
    IFarmTagDelete,
    json!({ "on": { "farm_id": "farm-1" } }),
    farm_tag_delete
);

assert_secondary_model_paths!(
    plot_tag_scripted_region_paths,
    IPlotTagCreate,
    json!({ "plot_id": "plot-1", "tag": "north" }),
    plot_tag_create,
    IPlotTagFindMany,
    json!({ "filter": { "id": "id-1" } }),
    plot_tag_find_many,
    IPlotTagFindOne,
    json!({ "on": { "id": "id-1" } }),
    plot_tag_find_one,
    IPlotTagUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "tag": "z" } }),
    json!({ "on": { "plot_id": "plot-1" }, "fields": { "tag": "y" } }),
    plot_tag_update,
    IPlotTagDelete,
    json!({ "on": { "plot_id": "plot-1" } }),
    plot_tag_delete
);

assert_secondary_model_paths!(
    farm_member_scripted_region_paths,
    IFarmMemberCreate,
    json!({ "farm_id": "farm-1", "member_pubkey": hex64('b'), "role": "owner" }),
    farm_member_create,
    IFarmMemberFindMany,
    json!({ "filter": { "id": "id-1" } }),
    farm_member_find_many,
    IFarmMemberFindOne,
    json!({ "on": { "id": "id-1" } }),
    farm_member_find_one,
    IFarmMemberUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "role": "z" } }),
    json!({ "on": { "member_pubkey": hex64('b') }, "fields": { "role": "y" } }),
    farm_member_update,
    IFarmMemberDelete,
    json!({ "on": { "member_pubkey": hex64('b') } }),
    farm_member_delete
);

assert_secondary_model_paths!(
    farm_member_claim_scripted_region_paths,
    IFarmMemberClaimCreate,
    json!({ "member_pubkey": hex64('b'), "farm_pubkey": hex64('a') }),
    farm_member_claim_create,
    IFarmMemberClaimFindMany,
    json!({ "filter": { "id": "id-1" } }),
    farm_member_claim_find_many,
    IFarmMemberClaimFindOne,
    json!({ "on": { "id": "id-1" } }),
    farm_member_claim_find_one,
    IFarmMemberClaimUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "farm_pubkey": hex64('c') } }),
    json!({ "on": { "member_pubkey": hex64('b') }, "fields": { "farm_pubkey": hex64('d') } }),
    farm_member_claim_update,
    IFarmMemberClaimDelete,
    json!({ "on": { "member_pubkey": hex64('b') } }),
    farm_member_claim_delete
);

assert_secondary_model_paths!(
    log_error_scripted_region_paths,
    ILogErrorCreate,
    json!({
        "error": "panic",
        "message": "boom",
        "app_system": "studio",
        "app_version": "1.0.0",
        "nostr_pubkey": hex64('c')
    }),
    log_error_create,
    ILogErrorFindMany,
    json!({ "filter": { "id": "id-1" } }),
    log_error_find_many,
    ILogErrorFindOne,
    json!({ "on": { "id": "id-1" } }),
    log_error_find_one,
    ILogErrorUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "message": "z" } }),
    json!({ "on": { "nostr_pubkey": hex64('c') }, "fields": { "message": "y" } }),
    log_error_update,
    ILogErrorDelete,
    json!({ "on": { "nostr_pubkey": hex64('c') } }),
    log_error_delete
);

assert_secondary_model_paths!(
    nostr_event_state_scripted_region_paths,
    INostrEventStateCreate,
    json!({
        "key": "state-a",
        "kind": 30023,
        "pubkey": hex64('d'),
        "d_tag": "listing-a",
        "last_event_id": hex64('e'),
        "last_created_at": 1,
        "content_hash": "hash-a"
    }),
    nostr_event_state_create,
    INostrEventStateFindMany,
    json!({ "filter": { "id": "id-1" } }),
    nostr_event_state_find_many,
    INostrEventStateFindOne,
    json!({ "on": { "id": "id-1" } }),
    nostr_event_state_find_one,
    INostrEventStateUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "content_hash": "hash-z" } }),
    json!({ "on": { "key": "state-a" }, "fields": { "content_hash": "hash-y" } }),
    nostr_event_state_update,
    INostrEventStateDelete,
    json!({ "on": { "key": "state-a" } }),
    nostr_event_state_delete
);

assert_rel_model_paths!(
    gcs_location_scripted_region_paths,
    IGcsLocationCreate,
    json!({
        "d_tag": "gcs-a",
        "lat": 59.33,
        "lng": 18.06,
        "geohash": "u6sce4f",
        "point": "POINT(18.06 59.33)",
        "polygon": "POLYGON((18.06 59.33,18.07 59.33,18.07 59.34,18.06 59.34,18.06 59.33))"
    }),
    gcs_location_create,
    IGcsLocationFindMany,
    json!({ "filter": { "id": "id-1" } }),
    IGcsLocationFindMany::Rel {
        rel: GcsLocationFindManyRel::OnFarm(GcsLocationFarmArgs {
            id: String::from("farm-1")
        })
    },
    gcs_location_find_many,
    IGcsLocationFindOne,
    json!({ "on": { "id": "id-1" } }),
    IGcsLocationFindOne::Rel(IGcsLocationFindOneRelArgs {
        rel: GcsLocationFindManyRel::OffTradeProduct(GcsLocationTradeProductArgs {
            id: String::from("tp-1")
        })
    }),
    gcs_location_find_one,
    IGcsLocationUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "label": "z" } }),
    json!({ "on": { "d_tag": "gcs-a" }, "fields": { "label": "y" } }),
    gcs_location_update,
    IGcsLocationDelete,
    json!({ "on": { "d_tag": "gcs-a" } }),
    IGcsLocationDelete::Rel(IGcsLocationFindOneRelArgs {
        rel: GcsLocationFindManyRel::OnPlot(GcsLocationPlotArgs {
            id: String::from("plot-1")
        })
    }),
    gcs_location_delete
);

assert_rel_model_paths!(
    media_image_scripted_region_paths,
    IMediaImageCreate,
    json!({
        "file_path": "/img/a.jpg",
        "mime_type": "image/jpeg",
        "res_base": "https://cdn.example.com",
        "res_path": "img/a.jpg"
    }),
    media_image_create,
    IMediaImageFindMany,
    json!({ "filter": { "id": "id-1" } }),
    IMediaImageFindMany::Rel {
        rel: MediaImageFindManyRel::OnTradeProduct(MediaImageTradeProductArgs {
            id: String::from("tp-1")
        })
    },
    media_image_find_many,
    IMediaImageFindOne,
    json!({ "on": { "id": "id-1" } }),
    IMediaImageFindOne::Rel(IMediaImageFindOneRelArgs {
        rel: MediaImageFindManyRel::OffTradeProduct(MediaImageTradeProductArgs {
            id: String::from("tp-1")
        })
    }),
    media_image_find_one,
    IMediaImageUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "label": "z" } }),
    json!({ "on": { "file_path": "/img/a.jpg" }, "fields": { "label": "y" } }),
    media_image_update,
    IMediaImageDelete,
    json!({ "on": { "file_path": "/img/a.jpg" } }),
    IMediaImageDelete::Rel(IMediaImageFindOneRelArgs {
        rel: MediaImageFindManyRel::OnTradeProduct(MediaImageTradeProductArgs {
            id: String::from("tp-1")
        })
    }),
    media_image_delete
);

assert_rel_model_paths!(
    nostr_profile_scripted_region_paths,
    INostrProfileCreate,
    json!({ "public_key": hex64('d'), "profile_type": "farm", "name": "profile a" }),
    nostr_profile_create,
    INostrProfileFindMany,
    json!({ "filter": { "id": "id-1" } }),
    INostrProfileFindMany::Rel {
        rel: NostrProfileFindManyRel::OnRelay(NostrProfileRelayArgs {
            id: String::from("relay-1")
        })
    },
    nostr_profile_find_many,
    INostrProfileFindOne,
    json!({ "on": { "id": "id-1" } }),
    INostrProfileFindOne::Rel(INostrProfileFindOneRelArgs {
        rel: NostrProfileFindManyRel::OffRelay(NostrProfileRelayArgs {
            id: String::from("relay-1")
        })
    }),
    nostr_profile_find_one,
    INostrProfileUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "name": "z" } }),
    json!({ "on": { "public_key": hex64('d') }, "fields": { "name": "y" } }),
    nostr_profile_update,
    INostrProfileDelete,
    json!({ "on": { "public_key": hex64('d') } }),
    INostrProfileDelete::Rel(INostrProfileFindOneRelArgs {
        rel: NostrProfileFindManyRel::OnRelay(NostrProfileRelayArgs {
            id: String::from("relay-1")
        })
    }),
    nostr_profile_delete
);

assert_rel_model_paths!(
    nostr_relay_scripted_region_paths,
    INostrRelayCreate,
    json!({ "url": "wss://relay.example.com" }),
    nostr_relay_create,
    INostrRelayFindMany,
    json!({ "filter": { "id": "id-1" } }),
    INostrRelayFindMany::Rel {
        rel: NostrRelayFindManyRel::OnProfile(NostrRelayProfileArgs {
            public_key: hex64('d')
        })
    },
    nostr_relay_find_many,
    INostrRelayFindOne,
    json!({ "on": { "id": "id-1" } }),
    INostrRelayFindOne::Rel(INostrRelayFindOneRelArgs {
        rel: NostrRelayFindManyRel::OffProfile(NostrRelayProfileArgs {
            public_key: hex64('d')
        })
    }),
    nostr_relay_find_one,
    INostrRelayUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "name": "z" } }),
    json!({ "on": { "url": "wss://relay.example.com" }, "fields": { "name": "y" } }),
    nostr_relay_update,
    INostrRelayDelete,
    json!({ "on": { "url": "wss://relay.example.com" } }),
    INostrRelayDelete::Rel(INostrRelayFindOneRelArgs {
        rel: NostrRelayFindManyRel::OnProfile(NostrRelayProfileArgs {
            public_key: hex64('d')
        })
    }),
    nostr_relay_delete
);

assert_trade_product_paths!(
    trade_product_scripted_region_paths,
    ITradeProductCreate,
    json!({
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
    }),
    trade_product_create,
    ITradeProductFindMany,
    json!({ "filter": { "id": "id-1" } }),
    trade_product_find_many,
    ITradeProductFindOne,
    json!({ "on": { "id": "id-1" } }),
    trade_product_find_one,
    ITradeProductUpdate,
    json!({ "on": { "id": "id-1" }, "fields": { "title": "z" } }),
    trade_product_update
);
