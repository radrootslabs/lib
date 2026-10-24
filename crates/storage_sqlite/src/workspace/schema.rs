//! Exact retained outbox catalogs, shared by creation and read-only admission.

use super::{WorkspaceError, attempt, delivery, evidence, installation, listing, operation, plan};
use sqlx::SqliteConnection;

pub(super) const CURRENT_VERSION: i64 = 11;
const GRANT: &str = "CREATE TABLE workspace_grant (singleton INTEGER PRIMARY KEY CHECK(singleton=1), workspace BLOB NOT NULL CHECK(length(workspace)=32), grant_id BLOB NOT NULL CHECK(length(grant_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT";

pub(super) struct Object {
    kind: &'static str,
    name: &'static str,
    owner: &'static str,
    pub sql: &'static str,
}
impl Object {
    const fn table(name: &'static str, sql: &'static str) -> Self {
        Self {
            kind: "table",
            name,
            owner: name,
            sql,
        }
    }
    const fn index(name: &'static str, owner: &'static str, sql: &'static str) -> Self {
        Self {
            kind: "index",
            name,
            owner,
            sql,
        }
    }
}

/// These are retained identities, never automatic migrations. Adding a schema
/// requires an explicit catalog and its independently qualified admission path.
pub(super) fn catalog(version: i64) -> Result<Vec<Object>, WorkspaceError> {
    if !matches!(version, 7..=11) {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let artifacts = version >= 9;
    let mut objects = vec![
        Object::table("workspace_grant", GRANT),
        Object::table(
            "operations",
            if version == 7 {
                operation::TABLE_V7
            } else {
                operation::TABLE
            },
        ),
        Object::index(
            "operation_transaction_identity",
            "operations",
            operation::INDEX,
        ),
        Object::table("signing_plans", plan::TABLE),
        Object::table("signing_plan_parts", plan::PARTS),
        Object::table(
            "signing_attempts",
            if artifacts {
                attempt::TABLE
            } else {
                attempt::TABLE_V8
            },
        ),
        Object::index(
            "signing_attempt_identity",
            "signing_attempts",
            attempt::ID_INDEX,
        ),
        Object::index(
            "signing_attempt_transaction_identity",
            "signing_attempts",
            attempt::TX_INDEX,
        ),
        Object::table(
            if artifacts {
                "artifact_evidence"
            } else {
                "seal_evidence"
            },
            if artifacts {
                evidence::TABLE
            } else {
                evidence::TABLE_V8
            },
        ),
        Object::table(
            if artifacts {
                "artifact_evidence_parts"
            } else {
                "seal_evidence_parts"
            },
            if artifacts {
                evidence::PARTS
            } else {
                evidence::PARTS_V8
            },
        ),
        Object::index(
            if artifacts {
                "artifact_evidence_transaction_identity"
            } else {
                "seal_evidence_transaction_identity"
            },
            if artifacts {
                "artifact_evidence"
            } else {
                "seal_evidence"
            },
            if artifacts {
                evidence::TX_INDEX
            } else {
                evidence::TX_INDEX_V8
            },
        ),
        Object::table("chat_messages", installation::TABLE),
        Object::index(
            "chat_message_operation_identity",
            "chat_messages",
            installation::OP_INDEX,
        ),
        Object::index(
            "chat_message_transaction_identity",
            "chat_messages",
            installation::TX_INDEX,
        ),
        Object::index(
            "chat_message_thread",
            "chat_messages",
            installation::THREAD_INDEX,
        ),
        Object::table(
            "chat_publication_work",
            if artifacts {
                installation::WORK
            } else {
                installation::WORK_V8
            },
        ),
        Object::table(
            if version == 11 {
                "artifact_delivery_attempts"
            } else {
                "chat_delivery_attempts"
            },
            if version == 11 {
                delivery::TABLE
            } else {
                delivery::TABLE_V10
            },
        ),
        Object::table(
            if version == 11 {
                "artifact_delivery_parts"
            } else {
                "chat_delivery_parts"
            },
            if version == 11 {
                delivery::PARTS
            } else {
                delivery::PARTS_V10
            },
        ),
        Object::index(
            if version == 11 {
                "artifact_delivery_transaction_identity"
            } else {
                "chat_delivery_transaction_identity"
            },
            if version == 11 {
                "artifact_delivery_attempts"
            } else {
                "chat_delivery_attempts"
            },
            if version == 11 {
                delivery::TX_INDEX
            } else {
                delivery::TX_INDEX_V10
            },
        ),
        Object::table(
            if version == 11 {
                "artifact_delivery_observations"
            } else {
                "chat_delivery_observations"
            },
            if version == 11 {
                delivery::OBSERVATIONS
            } else {
                delivery::OBSERVATIONS_V10
            },
        ),
    ];
    if version >= 10 {
        objects.extend([
            Object::table("listing_creations", listing::CREATIONS),
            Object::index(
                "listing_coordinate_identity",
                "listing_creations",
                listing::COORDINATE_INDEX,
            ),
            Object::table("listing_installations", super::listing_installation::TABLE),
            Object::index(
                "listing_installation_transaction_identity",
                "listing_installations",
                super::listing_installation::TX_INDEX,
            ),
            Object::table("public_publication_work", super::listing_installation::WORK),
        ]);
    }
    Ok(objects)
}

/// The aggregate materializes one scalar, never attacker-sized schema text.
/// Both the complete object count and each exact identity/definition must match.
pub(super) async fn admit(
    connection: &mut SqliteConnection,
    version: i64,
) -> Result<(), WorkspaceError> {
    let objects = catalog(version)?;
    let mut sql = String::from("SELECT CASE WHEN count(*)=? AND sum(CASE");
    for _ in &objects {
        sql.push_str(" WHEN type=? AND name=? AND tbl_name=? AND sql=? THEN 1");
    }
    sql.push_str(" ELSE 0 END)=? THEN 1 ELSE 0 END FROM sqlite_schema");
    let mut query = sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).bind(objects.len() as i64);
    for object in &objects {
        query = query
            .bind(object.kind)
            .bind(object.name)
            .bind(object.owner)
            .bind(object.sql);
    }
    let matched: i64 = query
        .bind(objects.len() as i64)
        .fetch_one(connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if matched != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(())
}
