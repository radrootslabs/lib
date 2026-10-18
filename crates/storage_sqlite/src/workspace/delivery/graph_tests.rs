#[cfg(test)]
mod cases {
    use super::super::*;
    use crate::workspace::tests::issuance_records::delivery_data;
    use nostr::JsonUtil;
    use radroots_domain::EnvelopeEventId;
    use radroots_protocol::chat::{CapturedDmRoute, ChatRumor, ChatSeal, ChatSigningPlan};
    use radroots_transport::{Target, TargetSet};

    fn route_plan(
        original: BoundSigningPlan,
        raw: &str,
        source: EnvelopeEventId,
        selected: &[String],
    ) -> BoundSigningPlan {
        let transaction = original.transaction_id();
        let time = original.bound_at();
        let (prepared, plan) = original.into_parts();
        let command = prepared.request().command();
        let rumor = ChatRumor::from_command(command, plan.created_at()).unwrap();
        let seals = plan
            .seals()
            .iter()
            .map(|seal| {
                ChatSeal::new(
                    &rumor,
                    seal.recipient(),
                    radroots_domain::UnixSeconds::new(seal.fields().created_at).unwrap(),
                    seal.fields().content,
                )
                .unwrap()
            })
            .collect();
        let route = CapturedDmRoute::from_observed_descriptor(
            plan.routes()[0].participant(),
            source,
            raw,
            selected,
        )
        .unwrap();
        let replacement = ChatSigningPlan::new(
            command,
            plan.created_at(),
            seals,
            vec![route],
            plan.replay(),
        )
        .unwrap();
        BoundSigningPlan::from_committed_store(prepared, replacement, transaction, time).unwrap()
    }
    fn signed(kind: u16, time: u64, scalar: u8, tags: Vec<nostr::Tag>) -> nostr::Event {
        let mut bytes = [0; 32];
        bytes[31] = scalar;
        let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&bytes).unwrap());
        nostr::UnsignedEvent {
            id: None,
            pubkey: keys.public_key(),
            created_at: nostr::Timestamp::from_secs(time),
            kind: nostr::Kind::Custom(kind),
            tags: nostr::Tags::from_list(tags),
            content: String::new(),
        }
        .sign_with_keys(&keys)
        .unwrap()
    }
    fn relay() -> nostr::Tag {
        nostr::Tag::parse(["relay", "wss://route.example/"]).unwrap()
    }

    #[tokio::test]
    async fn original_route_authentication_refuses_invalid_sources_and_unadvertised_selection() {
        let mut data = delivery_data().await;
        let recipient = data.report.work().evidence().recipient();
        let selected = [String::from("wss://route.example/")];
        assert!(
            allowed_routes(&data.bound().await, recipient)
                .unwrap()
                .contains_key(&recipient)
        );
        for event in [
            signed(14, 99, 1, vec![relay()]),
            signed(10050, 99, 2, vec![relay()]),
            signed(10050, 0, 1, vec![relay()]),
            signed(10050, 253_402_300_800, 1, vec![relay()]),
            signed(10050, 99, 1, vec![nostr::Tag::parse(["relay"]).unwrap()]),
            signed(
                10050,
                99,
                1,
                vec![nostr::Tag::parse(["relay", "wss://route.example/", "extra"]).unwrap()],
            ),
            signed(
                10050,
                99,
                1,
                vec![
                    nostr::Tag::parse([
                        "relay",
                        &format!("wss://route.example/{}", "x".repeat(2048)),
                    ])
                    .unwrap(),
                ],
            ),
        ] {
            let source = EnvelopeEventId::from_bytes(*event.id.as_bytes());
            let bound = route_plan(data.bound().await, &event.as_json(), source, &selected);
            assert!(allowed_routes(&bound, recipient).is_err());
        }
        let event = signed(10050, 99, 1, vec![relay()]);
        let source = EnvelopeEventId::from_bytes(*event.id.as_bytes());
        let bound = route_plan(
            data.bound().await,
            &event.as_json(),
            EnvelopeEventId::from_bytes([0; 32]),
            &selected,
        );
        assert!(allowed_routes(&bound, recipient).is_err());
        let missing = [String::from("wss://unadvertised.example/")];
        let bound = route_plan(data.bound().await, &event.as_json(), source, &missing);
        assert!(matches!(
            allowed_routes(&bound, recipient),
            Err(OperationError::UnverifiedApproval)
        ));
        let mut raw: serde_json::Value = serde_json::from_str(&event.as_json()).unwrap();
        raw["sig"] = serde_json::Value::String("0".repeat(128));
        let bound = route_plan(data.bound().await, &raw.to_string(), source, &selected);
        assert!(allowed_routes(&bound, recipient).is_err());
        let metadata = signed(
            10050,
            99,
            1,
            vec![
                nostr::Tag::parse(["client", "wss://unadvertised.example/"]).unwrap(),
                nostr::Tag::parse(["metadata"]).unwrap(),
                relay(),
            ],
        );
        let source = EnvelopeEventId::from_bytes(*metadata.id.as_bytes());
        let bound = route_plan(data.bound().await, &metadata.as_json(), source, &selected);
        assert!(
            allowed_routes(&bound, recipient)
                .unwrap()
                .contains_key(&recipient)
        );
        let bound = route_plan(data.bound().await, &metadata.as_json(), source, &missing);
        assert!(matches!(
            allowed_routes(&bound, recipient),
            Err(OperationError::UnverifiedApproval)
        ));
        let unrelated =
            PublicKey::from_bytes(signed(10050, 99, 2, vec![relay()]).pubkey.to_bytes()).unwrap();
        assert!(
            allowed_routes(&data.bound().await, unrelated)
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn route_scope_budget_and_complete_fact_count_fences_are_independent() {
        let mut data = delivery_data().await;
        let recipient = data.report.work().evidence().recipient();
        let mut graph = Graph {
            attempts: vec![],
            facts: vec![],
            parts: 0,
            counters: BTreeMap::new(),
            routes: BTreeMap::new(),
        };
        assert!(
            graph
                .validate_routes(recipient, data.report.intent())
                .is_err()
        );
        graph
            .authorize_routes(&data.bound().await, recipient)
            .unwrap();
        let unrelated =
            PublicKey::from_bytes(signed(10050, 99, 2, vec![relay()]).pubkey.to_bytes()).unwrap();
        assert!(matches!(
            graph.authorize_routes(&data.bound().await, unrelated),
            Err(OperationError::UnverifiedApproval)
        ));
        assert_eq!(
            graph.ordinals(recipient, data.report.intent()).unwrap(),
            [1]
        );
        let set = TargetSet::new(vec![
            Target::nostr_relay("wss://unadvertised.example/").unwrap(),
        ])
        .unwrap();
        let foreign = DeliveryIntent::new(
            data.report.intent().request_id().as_str(),
            data.report.intent().event_id(),
            &set,
            data.report.intent().policy(),
            data.report.intent().deadline_unix_ms(),
        )
        .unwrap();
        assert!(graph.validate_routes(recipient, &foreign).is_err());
        graph.counters.insert(
            (
                recipient,
                data.report.intent().targets().targets()[0]
                    .uri()
                    .as_str()
                    .to_owned(),
            ),
            (1..=12).collect(),
        );
        assert!(matches!(
            graph.ordinals(recipient, data.report.intent()),
            Err(OperationError::DeliveryAttemptLimit)
        ));
        let selected = data.selection();
        graph
            .unchanged(
                &mut data.database,
                &selected,
                data.report.work().evidence().operation_id(),
            )
            .await
            .unwrap();
        graph.parts = 1;
        assert!(
            graph
                .unchanged(
                    &mut data.database,
                    &selected,
                    data.report.work().evidence().operation_id()
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn observation_keys_ciphertexts_and_excess_counts_cannot_escape_the_actual_graph_fence() {
        use crate::workspace::delivery::{codec, observation};
        use radroots_storage::command::DeliveryObservation;
        use radroots_transport::{outcome::DeliveryOutcome, sink::DeliveryTargetReceipt};
        let mut data = delivery_data().await;
        let selected = data.selection();
        let e = data.report.work().evidence();
        let request = codec::request_key(data.report.intent().request_id().as_str());
        let report = DeliveryAttempt::from_store_report(
            data.report.work(),
            data.report.intent().clone(),
            data.report.raw_json(),
            data.report.transaction_id(),
            data.report.retained_at(),
            data.report.target_ordinals().to_vec(),
        )
        .unwrap();
        let graph = Graph {
            attempts: vec![report],
            facts: vec![],
            parts: 0,
            counters: BTreeMap::new(),
            routes: BTreeMap::new(),
        };
        let target = graph.attempts[0].intent().targets().targets()[0].clone();
        let original = DeliveryObservation::from_store_report(
            &graph.attempts[0],
            &DeliveryTargetReceipt::attempted(target, DeliveryOutcome::accepted()),
            None,
        )
        .unwrap();
        let protected = observation::protect(original.clone(), &data.crypto)
            .await
            .unwrap();
        protected.insert(&mut data.database).await.unwrap();
        let observed = observation::Observations::read(
            &mut data.database,
            &graph,
            &selected,
            e.operation_id(),
            &data.crypto,
        )
        .await
        .unwrap();
        observed
            .unchanged(&mut data.database, &selected, e.operation_id())
            .await
            .unwrap();
        for statement in [
            "UPDATE chat_delivery_observations SET recipient=zeroblob(31)",
            "UPDATE chat_delivery_observations SET recipient=zeroblob(32)",
            "UPDATE chat_delivery_observations SET request_key=zeroblob(32)",
            "UPDATE chat_delivery_observations SET target_key=zeroblob(31)",
            "UPDATE chat_delivery_observations SET kind=7",
            "UPDATE chat_delivery_observations SET protected=zeroblob(262145)",
            "UPDATE chat_delivery_observations SET protected=zeroblob(1)",
        ] {
            sqlx::query(statement)
                .execute(&mut data.database)
                .await
                .unwrap();
            let error = observation::Observations::read(
                &mut data.database,
                &graph,
                &selected,
                e.operation_id(),
                &data.crypto,
            )
            .await
            .err()
            .expect("malformed observation must fail");
            assert_eq!(error.code(), "recovery_required", "{statement}");
            assert!(
                observed
                    .unchanged(&mut data.database, &selected, e.operation_id())
                    .await
                    .is_err()
            );
            sqlx::query("DELETE FROM chat_delivery_observations")
                .execute(&mut data.database)
                .await
                .unwrap();
            protected.insert(&mut data.database).await.unwrap();
        }
        sqlx::query("DELETE FROM chat_delivery_observations")
            .execute(&mut data.database)
            .await
            .unwrap();
        assert!(
            observed
                .unchanged(&mut data.database, &selected, e.operation_id())
                .await
                .is_err()
        );
        protected.insert(&mut data.database).await.unwrap();
        sqlx::query("BEGIN")
            .execute(&mut data.database)
            .await
            .unwrap();
        sqlx::query("DELETE FROM chat_delivery_observations")
            .execute(&mut data.database)
            .await
            .unwrap();
        for index in 0..2305u32 {
            let mut target = [0; 32];
            target[..4].copy_from_slice(&index.to_be_bytes());
            sqlx::query("INSERT INTO chat_delivery_observations(workspace,operation_id,recipient,request_key,target_key,kind,protected) VALUES(?,?,?,?,?,1,x'01')").bind(e.workspace_id().as_bytes().as_slice()).bind(e.operation_id().as_bytes().as_slice()).bind(e.recipient().as_bytes().as_slice()).bind(request.as_slice()).bind(target.as_slice()).execute(&mut data.database).await.unwrap();
        }
        let error = observation::Observations::read(
            &mut data.database,
            &graph,
            &selected,
            e.operation_id(),
            &data.crypto,
        )
        .await
        .err()
        .expect("excess observation keys must fail");
        assert_eq!(error.code(), "recovery_required");
        sqlx::query("ROLLBACK")
            .execute(&mut data.database)
            .await
            .unwrap();
        let restored = observation::Observations::read(
            &mut data.database,
            &graph,
            &selected,
            e.operation_id(),
            &data.crypto,
        )
        .await
        .unwrap();
        assert_eq!(restored.reports, [original]);
    }
}
