use radroots_domain::*;
use radroots_identity::PublicKey;
#[cfg(feature = "serde")]
use serde_json::{Value, json};
use std::str::FromStr;

const KEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
fn actor() -> ActorRef {
    ActorRef::new(
        PublicKey::from_str(KEY).unwrap(),
        ActorRole::Buyer,
        None,
        Some(GrantId::from_bytes([4; 32])),
    )
}
fn checkpoint() -> CheckpointRef {
    CheckpointRef::new(
        CheckpointDigest::from_bytes([8; 32]),
        Sequence::new(0).unwrap(),
        Coverage::Partial,
        vec![SourceId::from_bytes([9; 32])],
    )
    .unwrap()
}
fn context() -> CommandContextData {
    CommandContextData::new(
        OperationId::from_bytes([1; 32]),
        actor(),
        WorkspaceId::from_bytes([2; 32]),
        AuthorityRef::new(
            ExecutorId::from_bytes([3; 32]),
            Generation::new(9_007_199_254_740_993).unwrap(),
        ),
        RouteDigest::from_bytes([5; 32]),
        None,
        None,
        UnixSeconds::new(1_900_000_000).unwrap(),
        None,
    )
    .unwrap()
}

#[cfg(feature = "serde")]
#[test]
fn context_descriptors_require_objects_when_decoded_directly() {
    fn check<T>(value: T, fields: &[&str])
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let object = serde_json::to_value(&value).unwrap();
        assert_eq!(serde_json::from_value::<T>(object.clone()).unwrap(), value);
        let array = Value::Array(fields.iter().map(|field| object[field].clone()).collect());
        assert!(serde_json::from_value::<T>(array).is_err());
    }
    check(
        actor(),
        &["participant", "role", "organization", "grant_id"],
    );
    check(context().authority(), &["executor_id", "generation"]);
    check(checkpoint(), &["digest", "sequence", "coverage", "missing"]);
    check(
        Approval::new(
            ApprovalIntentDigest::from_bytes([6; 32]),
            "inbox.inquire",
            UnixSeconds::new(190).unwrap(),
        )
        .unwrap(),
        &["intent_digest", "scope", "expires_at"],
    );
    check(
        ExpectedOrder::new(
            OrderId::from_bytes([7; 32]),
            None,
            Sequence::new(4).unwrap(),
            None,
            checkpoint(),
        ),
        &[
            "order_id",
            "agreement",
            "seller_sequence",
            "buyer_head",
            "checkpoint",
        ],
    );
    check(
        context(),
        &[
            "operation_id",
            "actor",
            "workspace_id",
            "authority",
            "route_digest",
            "approval",
            "expected",
            "not_after",
            "correlation_id",
        ],
    );
}

#[test]
fn exact_integers_never_round_or_accept_numeric_json_coercions() {
    for value in [0, 1, 9_007_199_254_740_993, i64::MAX as u64] {
        let sequence = Sequence::new(value).unwrap();
        assert_eq!(sequence.get(), value);
        assert_eq!(sequence.to_string().parse::<Sequence>(), Ok(sequence));
        #[cfg(feature = "serde")]
        {
            assert_eq!(
                serde_json::to_value(sequence).unwrap(),
                json!(value.to_string())
            );
            assert_eq!(
                serde_json::from_value::<Sequence>(json!(value.to_string())).unwrap(),
                sequence
            );
            assert!(serde_json::from_value::<Sequence>(json!(value)).is_err());
        }
        if value > 0 {
            let generation = Generation::new(value).unwrap();
            assert_eq!(generation.get(), value);
            assert_eq!(generation.to_string().parse::<Generation>(), Ok(generation));
            #[cfg(feature = "serde")]
            assert_eq!(
                serde_json::from_value::<Generation>(json!(value.to_string())).unwrap(),
                generation
            );
        }
    }
    for text in [
        "",
        "00",
        "01",
        "-1",
        "+1",
        "1.0",
        "1e2",
        " 1",
        "1 ",
        "１",
        "18446744073709551616",
        "9223372036854775808",
    ] {
        assert!(text.parse::<Sequence>().is_err(), "{text:?}");
        assert!(text.parse::<Generation>().is_err());
        #[cfg(feature = "serde")]
        assert!(serde_json::from_value::<Sequence>(json!(text)).is_err());
    }
    assert!(Generation::new(0).is_err());
    assert!("0".parse::<Generation>().is_err());
    assert!(Sequence::new(u64::MAX).is_err());
    assert!(Generation::new(u64::MAX).is_err());
    for value in [0, 1, 253_402_300_799] {
        let timestamp = UnixSeconds::new(value).unwrap();
        assert_eq!(timestamp.get(), value);
        assert_eq!(timestamp.to_string().parse::<UnixSeconds>(), Ok(timestamp));
        #[cfg(feature = "serde")]
        {
            assert_eq!(
                serde_json::from_value::<UnixSeconds>(json!(value.to_string())).unwrap(),
                timestamp
            );
            assert_eq!(
                serde_json::to_value(timestamp).unwrap(),
                json!(value.to_string())
            );
        }
    }
    assert!(UnixSeconds::new(253_402_300_800).is_err());
    assert!("253402300800".parse::<UnixSeconds>().is_err());
    #[cfg(feature = "serde")]
    assert!(serde_json::from_value::<UnixSeconds>(json!(1)).is_err());
}

#[test]
fn captured_descriptors_preserve_every_field_and_explicit_null() {
    let data = context();
    #[cfg(feature = "serde")]
    let wire = {
        let wire = serde_json::to_value(&data).unwrap();
        assert_eq!(wire["operation_id"], json!("01".repeat(32)));
        assert_eq!(wire["authority"]["generation"], json!("9007199254740993"));
        for name in ["approval", "expected", "correlation_id"] {
            assert!(wire[name].is_null());
        }
        assert_eq!(
            serde_json::from_value::<CommandContextData>(wire.clone()).unwrap(),
            data
        );
        wire
    };
    assert_eq!(data.operation_id(), OperationId::from_bytes([1; 32]));
    assert_eq!(data.actor(), actor());
    assert_eq!(data.workspace_id(), WorkspaceId::from_bytes([2; 32]));
    assert_eq!(
        data.authority().executor_id(),
        ExecutorId::from_bytes([3; 32])
    );
    assert_eq!(data.authority().generation().get(), 9_007_199_254_740_993);
    assert_eq!(data.route_digest(), RouteDigest::from_bytes([5; 32]));
    assert_eq!(data.not_after().get(), 1_900_000_000);
    assert!(
        data.approval().is_none() && data.expected().is_none() && data.correlation_id().is_none()
    );
    assert_eq!(
        data.actor().participant(),
        PublicKey::from_str(KEY).unwrap()
    );
    assert_eq!(data.actor().role(), ActorRole::Buyer);
    assert_eq!(data.actor().organization(), None);
    assert_eq!(data.actor().grant_id(), Some(GrantId::from_bytes([4; 32])));
    #[cfg(feature = "serde")]
    {
        for field in ["approval", "expected", "correlation_id"] {
            let mut missing = wire.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<CommandContextData>(missing).is_err(),
                "required {field}"
            );
        }
        for field in ["organization", "grant_id"] {
            let mut missing = wire.clone();
            missing["actor"].as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<CommandContextData>(missing).is_err(),
                "required actor {field}"
            );
        }
        let raw = serde_json::to_string(&data).unwrap();
        let duplicate = format!("{{\"operation_id\":\"{}\",{}", "ff".repeat(32), &raw[1..]);
        assert!(serde_json::from_str::<CommandContextData>(&duplicate).is_err());
        let mut unknown = wire.clone();
        unknown["authenticated"] = json!(true);
        assert!(serde_json::from_value::<CommandContextData>(unknown).is_err());
        let mut unknown = wire;
        unknown["actor"]["role"] = json!("operator");
        assert!(serde_json::from_value::<CommandContextData>(unknown).is_err());
    }
    assert!(!format!("{data:?}").contains(KEY));
}

#[test]
fn complete_preconditions_and_approval_data_remain_untrusted_bounded_values() {
    let approval = Approval::new(
        ApprovalIntentDigest::from_bytes([6; 32]),
        "orders.accept",
        UnixSeconds::new(1_900_000_000).unwrap(),
    )
    .unwrap();
    assert_eq!(
        approval.intent_digest(),
        ApprovalIntentDigest::from_bytes([6; 32])
    );
    assert_eq!(approval.scope(), "orders.accept");
    assert_eq!(approval.expires_at().get(), 1_900_000_000);
    let expected = ExpectedOrder::new(
        OrderId::from_bytes([7; 32]),
        Some(RecordId::from_bytes([10; 32])),
        Sequence::new(4).unwrap(),
        Some(RecordId::from_bytes([11; 32])),
        checkpoint(),
    );
    assert_eq!(expected.order_id(), OrderId::from_bytes([7; 32]));
    assert_eq!(expected.agreement(), Some(RecordId::from_bytes([10; 32])));
    assert_eq!(expected.seller_sequence().get(), 4);
    assert_eq!(expected.buyer_head(), Some(RecordId::from_bytes([11; 32])));
    assert_eq!(
        expected.checkpoint().digest(),
        CheckpointDigest::from_bytes([8; 32])
    );
    assert_eq!(expected.checkpoint().sequence().get(), 0);
    assert_eq!(expected.checkpoint().coverage(), Coverage::Partial);
    assert_eq!(
        expected.checkpoint().missing(),
        &[SourceId::from_bytes([9; 32])]
    );
    #[cfg(feature = "serde")]
    {
        let mut wire = serde_json::to_value(context()).unwrap();
        wire["approval"] = serde_json::to_value(&approval).unwrap();
        wire["expected"] = serde_json::to_value(&expected).unwrap();
        wire["correlation_id"] = json!("synthetic diagnostic");
        let data: CommandContextData = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(data.approval(), Some(&approval));
        assert_eq!(data.expected(), Some(&expected));
        assert_eq!(
            data.correlation_id().unwrap().as_str(),
            "synthetic diagnostic"
        );
        assert_eq!(serde_json::to_value(&data).unwrap(), wire);
        for field in ["agreement", "buyer_head"] {
            let mut missing = wire.clone();
            missing["expected"].as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<CommandContextData>(missing).is_err());
        }
        for nested in ["authority", "approval", "expected"] {
            let mut changed = wire.clone();
            changed[nested]["extra"] = Value::Bool(true);
            assert!(serde_json::from_value::<CommandContextData>(changed).is_err());
        }
        assert!(!format!("{data:?}").contains("synthetic diagnostic"));
    }
    assert!(!format!("{approval:?}").contains("orders.accept"));
}

#[test]
fn bounds_and_coverage_claims_fail_without_echoing_sensitive_text() {
    assert!(CorrelationId::new("é".repeat(64)).is_ok());
    assert!(CorrelationId::new("é".repeat(65)).is_err());
    assert!(CorrelationId::new("diagnostic\nsecret").is_err());
    #[cfg(feature = "serde")]
    {
        assert_eq!(
            serde_json::from_str::<CorrelationId>("\"hello\"")
                .unwrap()
                .as_str(),
            "hello"
        );
        assert!(serde_json::from_value::<CorrelationId>(json!("x".repeat(129))).is_err());
    }
    assert!(
        !format!("{:?}", CorrelationId::new("private diagnostic").unwrap())
            .contains("private diagnostic")
    );
    for scope in [String::new(), "x".repeat(161), "private\ncontext".into()] {
        let error = Approval::new(
            ApprovalIntentDigest::from_bytes([1; 32]),
            scope.clone(),
            UnixSeconds::new(1).unwrap(),
        )
        .unwrap_err();
        assert_eq!(error, ContextError::InvalidText);
        assert!(!error.to_string().contains(&scope) || scope.is_empty());
    }
    assert!(
        Approval::new(
            ApprovalIntentDigest::from_bytes([1; 32]),
            "valid",
            UnixSeconds::new(0).unwrap()
        )
        .is_err()
    );
    let digest = CheckpointDigest::from_bytes([2; 32]);
    let sequence = Sequence::new(0).unwrap();
    assert!(
        CheckpointRef::new(
            digest,
            sequence,
            Coverage::Partial,
            vec![SourceId::from_bytes([1; 32]); 128]
        )
        .is_ok()
    );
    assert_eq!(
        CheckpointRef::new(
            digest,
            sequence,
            Coverage::Partial,
            vec![SourceId::from_bytes([1; 32]); 129]
        )
        .unwrap_err(),
        ContextError::TooManyReferences
    );
    assert_eq!(
        CheckpointRef::new(
            digest,
            sequence,
            Coverage::ScopeSatisfied,
            vec![SourceId::from_bytes([1; 32])]
        )
        .unwrap_err(),
        ContextError::InconsistentCoverage
    );
    assert!(CheckpointRef::new(digest, sequence, Coverage::ScopeSatisfied, vec![]).is_ok());
    #[cfg(feature = "serde")]
    {
        let mut invalid = serde_json::to_value(context()).unwrap();
        invalid["not_after"] = json!("0");
        assert!(serde_json::from_value::<CommandContextData>(invalid).is_err());
    }
}
