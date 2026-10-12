use std::collections::BTreeSet;

use radroots_replica_schema::nostr_profile::{INostrProfileCreate, NostrProfileQueryBindValues};
use radroots_replica_schema::nostr_profile_relay::INostrProfileRelayRelation;
use radroots_replica_schema::nostr_relay::{
    INostrRelayCreate, INostrRelayDelete, INostrRelayFindMany, INostrRelayFindOne,
    INostrRelayFindOneRelArgs, NostrRelayFindManyRel, NostrRelayProfileArgs,
    NostrRelayQueryBindValues,
};
use radroots_replica_store::ReplicaSql;
use radroots_sql_core::{SqlError, SqlxSqliteExecutor};
use serde_json::json;

type Database = ReplicaSql<SqlxSqliteExecutor>;

const UNASSIGNED: &str = "wss://unassigned.example.test";
const TARGET_ONLY: &str = "wss://target.example.test";
const OTHER_ONLY: &str = "wss://other.example.test";
const SHARED: &str = "wss://shared.example.test";

fn target_key() -> String {
    "a".repeat(64)
}

fn fixture() -> Database {
    let db = ReplicaSql::new(SqlxSqliteExecutor::open_memory().unwrap());
    db.migrate_up().unwrap();
    let profiles: Vec<_> = [target_key(), "b".repeat(64)]
        .into_iter()
        .map(|public_key| {
            let input: INostrProfileCreate = serde_json::from_value(json!({
                "public_key": public_key,
                "profile_type": "person",
                "name": "synthetic relation fixture"
            }))
            .unwrap();
            db.nostr_profile_create(&input).unwrap().result.id
        })
        .collect();
    let relays: Vec<_> = [UNASSIGNED, TARGET_ONLY, OTHER_ONLY, SHARED]
        .into_iter()
        .map(|url| {
            let input: INostrRelayCreate = serde_json::from_value(json!({ "url": url })).unwrap();
            db.nostr_relay_create(&input).unwrap().result.id
        })
        .collect();
    for (profile, relay) in [(0, 1), (1, 2), (0, 3), (1, 3)] {
        db.nostr_profile_relay_set(&INostrProfileRelayRelation {
            nostr_profile: NostrProfileQueryBindValues::Id {
                id: profiles[profile].clone(),
            },
            nostr_relay: NostrRelayQueryBindValues::Id {
                id: relays[relay].clone(),
            },
        })
        .unwrap();
    }
    db
}

fn off_profile(public_key: String) -> NostrRelayFindManyRel {
    NostrRelayFindManyRel::OffProfile(NostrRelayProfileArgs { public_key })
}

fn many(db: &Database, rel: NostrRelayFindManyRel) -> BTreeSet<String> {
    let rows = db
        .nostr_relay_find_many(&INostrRelayFindMany::Rel { rel })
        .unwrap()
        .results;
    let count = rows.len();
    let urls: BTreeSet<_> = rows.into_iter().map(|row| row.url).collect();
    assert_eq!(
        urls.len(),
        count,
        "relation reads must not duplicate relays"
    );
    urls
}

fn urls(expected: &[&str]) -> BTreeSet<String> {
    expected.iter().map(|url| (*url).to_owned()).collect()
}

#[test]
fn off_profile_is_the_complete_membership_complement() {
    let db = fixture();
    assert_eq!(
        many(
            &db,
            NostrRelayFindManyRel::OnProfile(NostrRelayProfileArgs {
                public_key: target_key(),
            })
        ),
        urls(&[TARGET_ONLY, SHARED])
    );
    assert_eq!(
        many(&db, off_profile(target_key())),
        urls(&[UNASSIGNED, OTHER_ONLY])
    );
}

#[test]
fn absent_profile_has_no_members_and_all_relays_in_the_complement() {
    let db = fixture();
    assert!(
        many(
            &db,
            NostrRelayFindManyRel::OnProfile(NostrRelayProfileArgs {
                public_key: "c".repeat(64),
            })
        )
        .is_empty()
    );
    assert_eq!(
        many(&db, off_profile("c".repeat(64))),
        urls(&[UNASSIGNED, TARGET_ONLY, OTHER_ONLY, SHARED])
    );
}

#[test]
fn single_lookup_and_delete_preserve_assigned_relays() {
    let db = fixture();
    let mut remaining = urls(&[UNASSIGNED, OTHER_ONLY]);
    while !remaining.is_empty() {
        let selected = db
            .nostr_relay_find_one(&INostrRelayFindOne::Rel(INostrRelayFindOneRelArgs {
                rel: off_profile(target_key()),
            }))
            .unwrap()
            .result
            .expect("the complement contains a relay");
        assert!(remaining.contains(&selected.url));
        db.nostr_relay_delete(&INostrRelayDelete::Rel(INostrRelayFindOneRelArgs {
            rel: off_profile(target_key()),
        }))
        .unwrap();
        let after = many(&db, off_profile(target_key()));
        assert_eq!(after.len(), remaining.len() - 1);
        assert!(after.is_subset(&remaining));
        remaining = after;
    }
    assert_eq!(
        many(
            &db,
            NostrRelayFindManyRel::OnProfile(NostrRelayProfileArgs {
                public_key: target_key(),
            })
        ),
        urls(&[TARGET_ONLY, SHARED])
    );
    let missing = db.nostr_relay_delete(&INostrRelayDelete::Rel(INostrRelayFindOneRelArgs {
        rel: off_profile(target_key()),
    }));
    assert!(matches!(missing, Err(error) if matches!(error.error, SqlError::NotFound(_))));
}
