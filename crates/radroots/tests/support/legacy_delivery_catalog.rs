use sqlx::SqliteConnection;

// Synthetic exact predecessor catalog, never a claim that an old binary ran.
pub(super) async fn retain_legacy_delivery_catalog(db: &mut SqliteConnection) {
    use sqlx::Row;
    let public: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM artifact_delivery_attempts WHERE length(artifact_key)!=32",
    )
    .fetch_one(&mut *db)
    .await
    .unwrap();
    assert_eq!(public, 0);
    let rows=sqlx::query("SELECT name,sql FROM sqlite_schema WHERE name IN ('artifact_delivery_attempts','artifact_delivery_parts','artifact_delivery_observations','artifact_delivery_transaction_identity')").fetch_all(&mut *db).await.unwrap();
    let definitions = rows
        .into_iter()
        .map(|row| {
            let name: String = row.get(0);
            let ddl: String = row.get(1);
            let ddl = ddl
                .replace("artifact_delivery", "chat_delivery")
                .replace("artifact_key", "recipient")
                .replace(
                    "CHECK(length(recipient)=32 OR (length(recipient)=1 AND hex(recipient)='00'))",
                    "CHECK(length(recipient)=32)",
                )
                .replace(
                    "REFERENCES artifact_evidence(workspace,operation_id,recipient)",
                    "REFERENCES chat_publication_work(workspace,operation_id,recipient)",
                );
            (name, ddl)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for table in [
        "artifact_delivery_attempts",
        "artifact_delivery_parts",
        "artifact_delivery_observations",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE TEMP TABLE retained_{table} AS SELECT * FROM {table}"
        )))
        .execute(&mut *db)
        .await
        .unwrap();
    }
    for table in [
        "artifact_delivery_observations",
        "artifact_delivery_parts",
        "artifact_delivery_attempts",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP TABLE {table}")))
            .execute(&mut *db)
            .await
            .unwrap();
    }
    for table in [
        "artifact_delivery_attempts",
        "artifact_delivery_transaction_identity",
        "artifact_delivery_parts",
        "artifact_delivery_observations",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(definitions[table].clone()))
            .execute(&mut *db)
            .await
            .unwrap();
    }
    for table in [
        "artifact_delivery_attempts",
        "artifact_delivery_parts",
        "artifact_delivery_observations",
    ] {
        let legacy = table.replace("artifact_delivery", "chat_delivery");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO {legacy} SELECT * FROM retained_{table}"
        )))
        .execute(&mut *db)
        .await
        .unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP TABLE retained_{table}")))
            .execute(&mut *db)
            .await
            .unwrap();
    }
}
