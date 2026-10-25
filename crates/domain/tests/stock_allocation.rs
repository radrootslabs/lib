use radroots_domain::{
    AgreementLineId, PoolId, Quantity, Sequence, listing::MeasurementUnit, stock::*,
};

fn ticks(value: u64) -> StockTicks {
    StockTicks::new(value).unwrap()
}
fn pool(version: u64) -> PoolSnapshot {
    PoolSnapshot::new(
        PoolId::from_bytes([1; 32]),
        PoolUnit::new(MeasurementUnit::Each, 0).unwrap(),
        PoolBalances::new(
            ticks(10),
            PhysicalBalances {
                available: ticks(10),
                ..PhysicalBalances::ZERO
            },
        )
        .unwrap(),
        Sequence::new(version).unwrap(),
    )
}
#[test]
fn positive_proposals_are_data_and_reject_missing_versions_or_unavailable_backing() {
    let before = pool(1);
    let proposal = PoolHoldProposal::new(before, ticks(3)).unwrap();
    assert_eq!(proposal.before(), before);
    assert_eq!(proposal.after().balances().physical().available, ticks(7));
    assert_eq!(
        proposal.after().balances().physical().preparatory_hold,
        ticks(3)
    );
    assert_eq!(proposal.after().version().get(), 2);
    assert_eq!(proposal.after().balances().introduced(), ticks(10));
    assert_eq!(
        PoolHoldProposal::new(before, ticks(11)),
        Err(AllocationPlanError::StaleState)
    );
    assert_eq!(
        PoolHoldProposal::new(before, StockTicks::ZERO),
        Err(AllocationPlanError::InvalidInput)
    );
    assert_eq!(
        PoolHoldProposal::new(pool(0), ticks(1)),
        Err(AllocationPlanError::InvalidInput)
    );
    let quantity = Quantity::new("1".parse().unwrap(), MeasurementUnit::Each).unwrap();
    assert!(
        AllocationClaim::new(
            AgreementLineId::from_bytes([2; 32]),
            before.id(),
            quantity,
            Sequence::new(0).unwrap()
        )
        .is_err()
    );
    assert!(
        StockAllocation::new(
            AgreementLineId::from_bytes([2; 32]),
            before.id(),
            StockTicks::ZERO
        )
        .is_err()
    );
    assert_eq!(
        format!("{proposal:?}"),
        "PoolHoldProposal(<private proposal>)"
    );
}
#[cfg(feature = "serde")]
#[test]
fn restored_hold_proposal_must_recompute_the_entire_after_image() {
    let proposal = PoolHoldProposal::new(pool(1), ticks(3)).unwrap();
    let original = serde_json::to_value(proposal).unwrap();
    let restored: PoolHoldProposal = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(restored, proposal);
    for path in ["/after/version", "/after/id"] {
        let mut changed = original.clone();
        if path.ends_with("version") {
            *changed.pointer_mut(path).unwrap() = serde_json::json!("3");
        } else {
            *changed.pointer_mut(path).unwrap() = serde_json::json!("ab".repeat(32));
        }
        assert!(
            serde_json::from_value::<PoolHoldProposal>(changed).is_err(),
            "{path}"
        );
    }
    let mut changed = original.clone();
    changed["quantity"] = serde_json::json!("2");
    assert!(serde_json::from_value::<PoolHoldProposal>(changed).is_err());
    let mut changed = original;
    changed["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<PoolHoldProposal>(changed).is_err());
}
#[cfg(feature = "serde")]
#[test]
fn restored_allocations_cannot_create_zero_draws_or_ambiguous_objects() {
    let allocation = StockAllocation::new(
        AgreementLineId::from_bytes([2; 32]),
        PoolId::from_bytes([1; 32]),
        ticks(3),
    )
    .unwrap();
    let original = serde_json::to_value(allocation).unwrap();
    assert_eq!(
        serde_json::from_value::<StockAllocation>(original.clone()).unwrap(),
        allocation
    );
    let mut changed = original.clone();
    changed["ticks"] = serde_json::json!("0");
    assert!(serde_json::from_value::<StockAllocation>(changed).is_err());
    let mut changed = original;
    changed["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<StockAllocation>(changed).is_err());
    let duplicate = format!(
        "{{\"line\":\"{}\",\"line\":\"{}\",\"pool\":\"{}\",\"ticks\":\"3\"}}",
        allocation.line_id(),
        allocation.line_id(),
        allocation.pool_id()
    );
    assert!(serde_json::from_str::<StockAllocation>(&duplicate).is_err());
}
