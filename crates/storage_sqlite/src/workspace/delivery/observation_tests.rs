#[cfg(test)]
mod cases {
    use super::super::*;
    use crate::workspace::tests::issuance_records::delivery_data;
    use radroots_transport::outcome::DeliveryOutcome;
    use serde_json::{Value, json};

    #[tokio::test]
    async fn observation_wire_binds_every_original_fact_and_preserves_nullable_time() {
        let mut data = delivery_data().await;
        let report = DeliveryObservation::from_store_report(
            &data.report,
            &DeliveryTargetReceipt::attempted(
                data.report.intent().targets().targets()[0].clone(),
                DeliveryOutcome::accepted(),
            ),
            None,
        )
        .unwrap();
        let protected = protect(report.clone(), &data.crypto).await.unwrap();
        protected.insert(&mut data.database).await.unwrap();
        let kind = report.kind().code();
        let target = target_key(&report);
        let original = read_body(&mut data.database, protected.scope, target, kind)
            .await
            .unwrap();
        let plain = data
            .crypto
            .open_delivery(
                context(protected.scope, target, kind),
                DeliveryPayload::Observation,
                0,
                &original,
            )
            .await
            .unwrap();
        let base: Value = plain.expose_secret(|raw| serde_json::from_slice(raw).unwrap());
        let wire: Wire = serde_json::from_slice(&serde_json::to_vec(&base).unwrap()).unwrap();
        assert_eq!(decode(wire, &data.report, target, kind).unwrap(), report);
        let mut rebound = base["work"].clone();
        rebound["transaction"] = json!("99".repeat(32));
        for (field, value) in [
            ("version", json!(2)),
            ("work", rebound),
            ("request", json!("02".repeat(32))),
            ("event", json!("99".repeat(32))),
            ("envelope_digest", json!(vec![0u8; 32])),
            ("kind", json!(2)),
            ("observed_at", json!(253_402_300_800_000u64)),
        ] {
            let mut changed = base.clone();
            changed[field] = value;
            let decoded: Wire =
                serde_json::from_slice(&serde_json::to_vec(&changed).unwrap()).unwrap();
            assert!(
                decode(decoded, &data.report, target, kind).is_err(),
                "{field}"
            );
        }
        let wire: Wire = serde_json::from_slice(&serde_json::to_vec(&base).unwrap()).unwrap();
        assert!(decode(wire, &data.report, [0; 32], kind).is_err());
        let mut changed = base.clone();
        changed["target"] = serde_json::to_value(DeliveryTargetReceipt::attempted(
            data.report.intent().targets().targets()[0].clone(),
            DeliveryOutcome::rejected(),
        ))
        .unwrap();
        let wire: Wire = serde_json::from_slice(&serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(decode(wire, &data.report, target, kind).is_err());
        sqlx::query("UPDATE chat_delivery_observations SET protected=zeroblob(262145)")
            .execute(&mut data.database)
            .await
            .unwrap();
        assert!(
            read_body(&mut data.database, protected.scope, target, kind)
                .await
                .is_err()
        );
        sqlx::query("DELETE FROM chat_delivery_observations")
            .execute(&mut data.database)
            .await
            .unwrap();
        assert!(
            read_body(&mut data.database, protected.scope, target, kind)
                .await
                .is_err()
        );
    }
}
