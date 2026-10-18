//! Authenticated malformed records must fail closed without repairing storage.
#[cfg(test)]
mod cases {
    use super::super::*;
    use crate::workspace::tests::issuance_records::{DeliveryData, delivery_data};
    use nostr::JsonUtil;
    use serde_json::{Value, json};

    fn copy(report: &DeliveryAttempt) -> DeliveryAttempt {
        DeliveryAttempt::from_store_report(
            report.work(),
            report.intent().clone(),
            report.raw_json(),
            report.transaction_id(),
            report.retained_at(),
            report.target_ordinals().to_vec(),
        )
        .unwrap()
    }
    async fn restore(data: &mut DeliveryData, original: &ProtectedAttempt) {
        sqlx::query("DELETE FROM chat_delivery_parts")
            .execute(&mut data.database)
            .await
            .unwrap();
        sqlx::query("DELETE FROM chat_delivery_attempts")
            .execute(&mut data.database)
            .await
            .unwrap();
        insert(&mut data.database, original).await.unwrap();
    }
    async fn refused(data: &mut DeliveryData) {
        let key = request_key(data.report.intent().request_id().as_str());
        let error = read(&mut data.database, data.report.work(), key, &data.crypto)
            .await
            .err()
            .expect("malformed retained record must be refused");
        assert_eq!(error.code(), "recovery_required");
    }
    async fn manifest(data: &mut DeliveryData, original: &ProtectedAttempt, value: &Value) {
        let selected = scope(
            data.report.work(),
            request_key(data.report.intent().request_id().as_str()),
        );
        let protected = data
            .crypto
            .seal_delivery(
                selected,
                DeliveryPayload::Attempt,
                0,
                &serde_json::to_vec(value).unwrap(),
            )
            .await
            .unwrap();
        sqlx::query("UPDATE chat_delivery_attempts SET protected=?")
            .bind(protected)
            .execute(&mut data.database)
            .await
            .unwrap();
        assert!(!original.parts.is_empty());
    }
    async fn frame(data: &mut DeliveryData, raw: &[u8]) {
        let selected = scope(
            data.report.work(),
            request_key(data.report.intent().request_id().as_str()),
        );
        let pieces = raw.len().div_ceil(PRIVATE_PAYLOAD_MAX_BYTES);
        let value = json!({"version":1,"work":WorkRef::new(data.report.work()),"request":data.report.intent().request_id().as_str(),"event":data.report.intent().event_id(),"transaction":data.report.transaction_id(),"parts":pieces,"bytes":raw.len(),"digest":<[u8;32]>::from(Sha256::digest(raw))});
        let protected = data
            .crypto
            .seal_delivery(
                selected,
                DeliveryPayload::Attempt,
                0,
                &serde_json::to_vec(&value).unwrap(),
            )
            .await
            .unwrap();
        sqlx::query("DELETE FROM chat_delivery_parts")
            .execute(&mut data.database)
            .await
            .unwrap();
        for (index, piece) in raw.chunks(PRIVATE_PAYLOAD_MAX_BYTES).enumerate() {
            let protected = data
                .crypto
                .seal_delivery(selected, DeliveryPayload::Attempt, (index + 1) as u8, piece)
                .await
                .unwrap();
            sqlx::query("INSERT INTO chat_delivery_parts(workspace,operation_id,recipient,request_key,part,protected) VALUES(?,?,?,?,?,?)").bind(selected.workspace.as_bytes().as_slice()).bind(selected.operation.as_bytes().as_slice()).bind(selected.recipient.as_bytes().as_slice()).bind(selected.request.as_slice()).bind(index as i64+1).bind(protected).execute(&mut data.database).await.unwrap();
        }
        sqlx::query("UPDATE chat_delivery_attempts SET part_count=?,byte_count=?,protected=?")
            .bind(pieces as i64)
            .bind(raw.len() as i64)
            .bind(protected)
            .execute(&mut data.database)
            .await
            .unwrap();
    }
    fn encoded(header: &Value, tail: &[u8]) -> Vec<u8> {
        let header = serde_json::to_vec(header).unwrap();
        let mut result = (header.len() as u32).to_be_bytes().to_vec();
        result.extend_from_slice(&header);
        result.extend_from_slice(tail);
        result
    }

    #[tokio::test]
    async fn protected_delivery_headers_lengths_parts_and_fences_refuse_each_inconsistency() {
        let mut data = delivery_data().await;
        let key = request_key(data.report.intent().request_id().as_str());
        assert!(
            read(&mut data.database, data.report.work(), key, &data.crypto)
                .await
                .unwrap()
                .is_none()
        );
        absent(&mut data.database, data.report.work(), key)
            .await
            .unwrap();
        let original = protect(copy(&data.report), &data.crypto).await.unwrap();
        assert!(original.parts.len() > 1);
        insert(&mut data.database, &original).await.unwrap();
        assert!(
            absent(&mut data.database, data.report.work(), key)
                .await
                .is_err()
        );
        let fact = original.fact();
        fact.unchanged(&mut data.database).await.unwrap();
        let selected = scope(data.report.work(), key);
        let decoded = data
            .crypto
            .open_delivery(selected, DeliveryPayload::Attempt, 0, &original.manifest)
            .await
            .unwrap();
        let base: Value = decoded.expose_secret(|raw| serde_json::from_slice(raw).unwrap());
        let mut rebound = base["work"].clone();
        rebound["transaction"] = json!("99".repeat(32));
        for (field, value) in [
            ("version", json!(2)),
            ("work", rebound),
            ("request", json!("02".repeat(32))),
            ("event", json!("99".repeat(32))),
            ("transaction", json!("99".repeat(32))),
            ("parts", json!(original.parts.len() + 1)),
            ("bytes", json!(original.bytes - 1)),
            ("digest", json!(vec![0u8; 32])),
        ] {
            restore(&mut data, &original).await;
            let mut changed = base.clone();
            changed[field] = value;
            manifest(&mut data, &original, &changed).await;
            refused(&mut data).await;
            assert!(fact.unchanged(&mut data.database).await.is_err());
        }
        let nested: Value =
            serde_json::from_str(&format!("{}0{}", "[".repeat(17), "]".repeat(17))).unwrap();
        restore(&mut data, &original).await;
        manifest(&mut data, &original, &nested).await;
        refused(&mut data).await;
        for statement in [
            "UPDATE chat_delivery_attempts SET event_id=zeroblob(31)",
            "UPDATE chat_delivery_attempts SET transaction_id=zeroblob(31)",
            "UPDATE chat_delivery_attempts SET event_id=zeroblob(32)",
            "UPDATE chat_delivery_attempts SET transaction_id=zeroblob(32)",
            "UPDATE chat_delivery_attempts SET part_count=0",
            "UPDATE chat_delivery_attempts SET part_count=25",
            "UPDATE chat_delivery_attempts SET byte_count=0",
            "UPDATE chat_delivery_attempts SET byte_count=393217",
            "UPDATE chat_delivery_attempts SET byte_count=1",
            "UPDATE chat_delivery_attempts SET protected=zeroblob(262145)",
            "DELETE FROM chat_delivery_parts WHERE part=1",
            "UPDATE chat_delivery_parts SET part=0 WHERE part=1",
            "UPDATE chat_delivery_parts SET protected=zeroblob(262145) WHERE part=1",
            "UPDATE chat_delivery_parts SET protected=zeroblob(1) WHERE part=1",
            "UPDATE chat_delivery_parts SET protected=zeroblob(0)",
        ] {
            restore(&mut data, &original).await;
            sqlx::query(statement)
                .execute(&mut data.database)
                .await
                .unwrap();
            refused(&mut data).await;
            assert!(
                fact.unchanged(&mut data.database).await.is_err(),
                "{statement}"
            );
        }
        restore(&mut data, &original).await;
        let part = original.parts.len() + 1;
        sqlx::query("INSERT INTO chat_delivery_parts SELECT workspace,operation_id,recipient,request_key,?,protected FROM chat_delivery_parts WHERE part=1").bind(part as i64).execute(&mut data.database).await.unwrap();
        assert!(fact.unchanged(&mut data.database).await.is_err());
        refused(&mut data).await;
        restore(&mut data, &original).await;
        for increase in [false, true] {
            let index = if increase { original.parts.len() } else { 1 };
            let decoded = data
                .crypto
                .open_delivery(
                    selected,
                    DeliveryPayload::Attempt,
                    index as u8,
                    &original.parts[index - 1],
                )
                .await
                .unwrap();
            let mut plain = decoded.expose_secret(<[u8]>::to_vec);
            if increase {
                plain.push(0);
            } else {
                plain.pop();
            }
            let changed = data
                .crypto
                .seal_delivery(selected, DeliveryPayload::Attempt, index as u8, &plain)
                .await
                .unwrap();
            sqlx::query("UPDATE chat_delivery_parts SET protected=? WHERE part=?")
                .bind(changed)
                .bind(index as i64)
                .execute(&mut data.database)
                .await
                .unwrap();
            refused(&mut data).await;
            restore(&mut data, &original).await;
        }
        sqlx::query("DELETE FROM chat_delivery_parts")
            .execute(&mut data.database)
            .await
            .unwrap();
        for index in 1..=24i64 {
            sqlx::query("INSERT INTO chat_delivery_parts(workspace,operation_id,recipient,request_key,part,protected) VALUES(?,?,?,?,?,zeroblob(262144))").bind(selected.workspace.as_bytes().as_slice()).bind(selected.operation.as_bytes().as_slice()).bind(selected.recipient.as_bytes().as_slice()).bind(selected.request.as_slice()).bind(index).execute(&mut data.database).await.unwrap();
        }
        sqlx::query("UPDATE chat_delivery_attempts SET part_count=24,byte_count=393216")
            .execute(&mut data.database)
            .await
            .unwrap();
        refused(&mut data).await;
        restore(&mut data, &original).await;
        let retained = read(&mut data.database, data.report.work(), key, &data.crypto)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.report.raw_json(), data.report.raw_json());
        fact.unchanged(&mut data.database).await.unwrap();
    }

    #[tokio::test]
    async fn authenticated_delivery_frames_reject_rebound_fields_and_invalid_nested_encodings() {
        let mut data = delivery_data().await;
        let original = protect(copy(&data.report), &data.crypto).await.unwrap();
        insert(&mut data.database, &original).await.unwrap();
        let base = serde_json::to_value(Header::new(&data.report)).unwrap();
        let mut rebound = base["work"].clone();
        rebound["transaction"] = json!("99".repeat(32));
        let header = Header::new(&data.report);
        assert!(
            header
                .into_attempt(data.report.work(), data.report.raw_json())
                .is_ok()
        );
        let foreign = radroots_transport::Target::nostr_relay("wss://foreign.example/").unwrap();
        for (field, value) in [
            ("version", json!(2)),
            ("work", rebound),
            ("selection", json!([])),
            ("selection", json!([foreign.fingerprint().as_str()])),
            (
                "selection",
                json!([
                    data.report.intent().selection()[0].as_str(),
                    data.report.intent().selection()[0].as_str()
                ]),
            ),
            ("intent", json!(vec![0u8; 32])),
            ("ordinals", json!([0])),
            ("unknown", json!(true)),
        ] {
            let mut changed = base.clone();
            changed[field] = value;
            let raw = encoded(&changed, data.report.raw_json().as_bytes());
            frame(&mut data, &raw).await;
            refused(&mut data).await;
        }
        let nested: Value =
            serde_json::from_str(&format!("{}0{}", "[".repeat(17), "]".repeat(17))).unwrap();
        for raw in [
            vec![0],
            u32::MAX.to_be_bytes().to_vec(),
            encoded(&nested, b"tail"),
            encoded(&base, &[0xff]),
        ] {
            frame(&mut data, &raw).await;
            refused(&mut data).await;
        }
        for field in ["request", "transaction"] {
            let mut changed = base.clone();
            if field == "request" {
                let next = DeliveryIntent::new(
                    &"02".repeat(32),
                    data.report.intent().event_id(),
                    data.report.intent().targets(),
                    data.report.intent().policy(),
                    data.report.intent().deadline_unix_ms(),
                )
                .unwrap();
                changed["request"] = json!(next.request_id().as_str());
                changed["intent"] = json!(next.digest());
            } else {
                changed["transaction"] = json!("99".repeat(32));
            }
            let raw = encoded(&changed, data.report.raw_json().as_bytes());
            frame(&mut data, &raw).await;
            refused(&mut data).await;
        }
        let original_event = nostr::Event::from_json(data.report.raw_json()).unwrap();
        let mut scalar = [0; 32];
        scalar[31] = 3;
        let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&scalar).unwrap());
        let other = nostr::UnsignedEvent {
            id: None,
            pubkey: keys.public_key(),
            created_at: original_event.created_at,
            kind: original_event.kind,
            tags: original_event.tags.clone(),
            content: format!("{} ", original_event.content),
        }
        .sign_with_keys(&keys)
        .unwrap();
        let intent = DeliveryIntent::new(
            data.report.intent().request_id().as_str(),
            EnvelopeEventId::from_bytes(*other.id.as_bytes()),
            data.report.intent().targets(),
            data.report.intent().policy(),
            data.report.intent().deadline_unix_ms(),
        )
        .unwrap();
        let mut changed = base.clone();
        changed["event"] = json!(intent.event_id());
        changed["intent"] = json!(intent.digest());
        frame(&mut data, &encoded(&changed, other.as_json().as_bytes())).await;
        refused(&mut data).await;
        restore(&mut data, &original).await;
        assert_eq!(
            read(
                &mut data.database,
                data.report.work(),
                request_key(data.report.intent().request_id().as_str()),
                &data.crypto
            )
            .await
            .unwrap()
            .unwrap()
            .report
            .raw_json(),
            data.report.raw_json()
        );
    }
}
