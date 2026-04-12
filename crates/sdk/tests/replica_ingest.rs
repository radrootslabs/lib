use radroots_replica_db::ReplicaSql;
use radroots_replica_db_schema::farm::IFarmFindMany;
use radroots_replica_sync::{RadrootsReplicaIngestOutcome, radroots_replica_ingest_event};
use radroots_sdk::{RadrootsFarm, RadrootsNostrEvent, farm};
use radroots_sql_core::{SqlExecutor, SqliteExecutor};
use tempfile::{TempDir, tempdir};

fn seller_pubkey() -> String {
    "a".repeat(64)
}

fn sdk_event(
    id: u64,
    author: &str,
    created_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: format!("{id:064x}"),
        author: author.to_owned(),
        created_at,
        kind,
        tags,
        content,
        sig: "f".repeat(128),
    }
}

fn sample_farm() -> RadrootsFarm {
    RadrootsFarm {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        name: "North Farm".into(),
        about: Some("Organic coffee".into()),
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: Some(vec!["coffee".into()]),
    }
}

fn open_replica() -> (TempDir, ReplicaSql<SqliteExecutor>) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("replica.sqlite");
    let executor = SqliteExecutor::open(&db_path).expect("open sqlite");
    executor
        .exec("PRAGMA foreign_keys = ON;", "[]")
        .expect("enable foreign keys");
    let replica = ReplicaSql::new(executor);
    replica.migrate_up().expect("migrate");
    (dir, replica)
}

fn ingest_farm(replica: &ReplicaSql<SqliteExecutor>) -> RadrootsNostrEvent {
    let farm_value = sample_farm();
    let author = seller_pubkey();
    let parts = farm::build_draft(&farm_value).expect("farm draft");
    let event = sdk_event(1, &author, 1_720_000_000, parts.kind, parts.content, parts.tags);
    let outcome = radroots_replica_ingest_event(replica.executor(), &event).expect("ingest farm");
    assert_eq!(outcome, RadrootsReplicaIngestOutcome::Applied);
    event
}

#[test]
fn sdk_farm_draft_ingests_into_replica_projection() {
    let (_dir, replica) = open_replica();
    let event = ingest_farm(&replica);
    let farms = replica
        .farm_find_many(&IFarmFindMany { filter: None })
        .expect("query farms")
        .results;
    assert_eq!(farms.len(), 1);
    assert_eq!(farms[0].d_tag, sample_farm().d_tag);
    assert_eq!(farms[0].name, sample_farm().name);
    assert_eq!(farms[0].pubkey, event.author);
}
