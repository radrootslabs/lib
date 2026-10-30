//! Retained T2 wire shapes must distinguish absent extensions from null.
use super::*;
use radroots_domain::{
    PoolId,
    listing::MeasurementUnit,
    stock::{PhysicalBalances, PoolBalances, PoolUnit},
};

#[test]
fn initial_installation_wire_rejects_null_extensions_and_keeps_its_exact_shape() {
    let before = PoolSnapshot::new(
        PoolId::from_bytes([1; 32]),
        PoolUnit::new(MeasurementUnit::Each, 0).unwrap(),
        PoolBalances::new(
            StockTicks::new(1).unwrap(),
            PhysicalBalances {
                preparatory_hold: StockTicks::new(1).unwrap(),
                ..PhysicalBalances::ZERO
            },
        )
        .unwrap(),
        Sequence::new(1).unwrap(),
    );
    let work = Work {
        recipient: PublicKey::from_bytes([2; 32]).unwrap(),
        intent: [3; 32],
        seal: [4; 32],
        evidence: None,
    };
    let body = Body {
        version: 1,
        commit_sequence: None,
        changes: None,
        operation: OperationId::from_bytes([5; 32]),
        plan_entry: OperationId::from_bytes([6; 32]),
        plan_head: CheckpointDigest::from_bytes([7; 32]),
        record: RecordId::from_bytes([8; 32]),
        installed_at: UnixSeconds::new(100).unwrap(),
        authorization: StockAuthorizationRef::from_host_metadata(
            Sequence::new(1).unwrap(),
            CheckpointDigest::from_bytes([9; 32]),
        )
        .unwrap(),
        effects: vec![Effect {
            before,
            after: after(before, StockTicks::new(1).unwrap(), StockTicks::ZERO).unwrap(),
            quantity: StockTicks::new(1).unwrap(),
            released: None,
            movement: [10; 32],
        }],
        work: [work; 2],
    };
    let bytes = serde_json::to_vec(&body).unwrap();
    let retained: Body = crate::workspace::stock::closed_record(&bytes).unwrap();
    assert_eq!(serde_json::to_vec(&retained).unwrap(), bytes);
    let original = serde_json::to_value(&body).unwrap();
    assert!(original.get("commit_sequence").is_none());
    assert!(original.get("changes").is_none());
    assert!(original["effects"][0].get("released").is_none());
    for field in ["commit_sequence", "changes"] {
        let mut damaged = original.clone();
        damaged[field] = serde_json::Value::Null;
        assert!(
            crate::workspace::stock::closed_record::<Body>(&serde_json::to_vec(&damaged).unwrap())
                .is_err()
        );
    }
    let mut damaged = original;
    damaged["effects"][0]["released"] = serde_json::Value::Null;
    assert!(
        crate::workspace::stock::closed_record::<Body>(&serde_json::to_vec(&damaged).unwrap())
            .is_err()
    );
    let replacement = Body {
        version: 2,
        commit_sequence: Some(Sequence::new(2).unwrap()),
        changes: Some(Vec::new()),
        ..body
    };
    let bytes = serde_json::to_vec(&replacement).unwrap();
    let retained: Body = crate::workspace::stock::closed_record(&bytes).unwrap();
    assert_eq!(serde_json::to_vec(&retained).unwrap(), bytes);
}
