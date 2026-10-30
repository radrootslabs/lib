use radroots_domain::{AgreementLineId, PoolId, Sequence, listing::MeasurementUnit, stock::*};

fn ticks(value: u64) -> StockTicks {
    StockTicks::new(value).unwrap()
}
fn allocation(line: u8, pool: u8, quantity: u64) -> StockAllocation {
    StockAllocation::new(
        AgreementLineId::from_bytes([line; 32]),
        PoolId::from_bytes([pool; 32]),
        ticks(quantity),
    )
    .unwrap()
}
fn pool(id: u8, available: u64, committed: u64) -> PoolSnapshot {
    PoolSnapshot::new(
        PoolId::from_bytes([id; 32]),
        PoolUnit::new(MeasurementUnit::Each, 0).unwrap(),
        PoolBalances::new(
            ticks(available + committed),
            PhysicalBalances {
                available: ticks(available),
                committed_hold: ticks(committed),
                ..PhysicalBalances::ZERO
            },
        )
        .unwrap(),
        Sequence::new(7).unwrap(),
    )
}

#[test]
fn positive_differences_hold_before_issuance_and_leave_original_commitments_untouched() {
    let before = pool(1, 10, 3);
    let plan =
        plan_replacement_allocations(&[allocation(1, 1, 3)], &[allocation(1, 1, 5)], &[before])
            .unwrap();
    assert_eq!(plan.additional_allocations(), &[allocation(1, 1, 2)]);
    assert!(plan.pending_releases().is_empty());
    let hold = plan.preparatory_holds()[0];
    assert_eq!(hold.before(), before);
    assert_eq!(hold.after().balances().physical().available, ticks(8));
    assert_eq!(
        hold.after().balances().physical().preparatory_hold,
        ticks(2)
    );
    assert_eq!(hold.after().balances().physical().committed_hold, ticks(3));
    assert_eq!(
        hold.after().balances().introduced(),
        before.balances().introduced()
    );
    assert_eq!(before.balances().physical().available, ticks(10));
}

#[test]
fn negative_changes_are_only_pending_original_draws_not_available_stock_or_version_steps() {
    let before = pool(1, 2, 5);
    let plan =
        plan_replacement_allocations(&[allocation(1, 1, 5)], &[allocation(1, 1, 2)], &[before])
            .unwrap();
    assert_eq!(plan.pending_releases(), &[allocation(1, 1, 3)]);
    assert!(plan.additional_allocations().is_empty());
    assert!(plan.preparatory_holds().is_empty());
    assert_eq!(before.version().get(), 7);
    assert_eq!(before.balances().physical().available, ticks(2));
    assert_eq!(before.balances().physical().committed_hold, ticks(5));
}

#[test]
fn neither_same_pool_line_swaps_nor_cross_pool_releases_can_finance_positive_holds() {
    for (current, desired, pools) in [
        (
            vec![allocation(1, 1, 3)],
            vec![allocation(2, 1, 3)],
            vec![pool(1, 0, 3)],
        ),
        (
            vec![allocation(1, 1, 3)],
            vec![allocation(1, 2, 3)],
            vec![pool(1, 0, 3), pool(2, 2, 0)],
        ),
    ] {
        assert_eq!(
            plan_replacement_allocations(&current, &desired, &pools).unwrap_err(),
            AllocationPlanError::StaleState
        );
    }
    let current = [allocation(1, 1, 3)];
    let desired = [allocation(2, 1, 3)];
    let plan = plan_replacement_allocations(&current, &desired, &[pool(1, 3, 3)]).unwrap();
    assert_eq!(
        plan.preparatory_holds()[0]
            .after()
            .balances()
            .physical()
            .available,
        ticks(0)
    );
    assert_eq!(
        plan.preparatory_holds()[0]
            .after()
            .balances()
            .physical()
            .committed_hold,
        ticks(3)
    );
    assert_eq!(plan.pending_releases(), &current);
}

#[test]
fn unchanged_obligations_do_not_cure_deficits_or_replace_original_pool_identity() {
    let outstanding = [allocation(1, 1, 5)];
    let original = pool(1, 10, 3); // The other two obligation ticks have no backing.
    let plan = plan_replacement_allocations(&outstanding, &outstanding, &[original]).unwrap();
    assert!(plan.additional_allocations().is_empty());
    assert!(plan.pending_releases().is_empty());
    assert!(plan.preparatory_holds().is_empty());
    assert_eq!(original.balances().physical().committed_hold, ticks(3));
    let changed = [allocation(1, 2, 5)];
    let plan =
        plan_replacement_allocations(&outstanding, &changed, &[original, pool(2, 5, 0)]).unwrap();
    assert_eq!(plan.pending_releases(), &outstanding);
    assert_eq!(plan.additional_allocations(), &changed);
    assert_eq!(
        plan.preparatory_holds()[0].before().id(),
        changed[0].pool_id()
    );
}

#[test]
fn complete_multi_pool_plan_is_stable_sorted_and_aggregates_only_additional_ticks() {
    let current = [allocation(2, 2, 3), allocation(1, 1, 2)];
    let desired = [
        allocation(3, 2, 1),
        allocation(2, 2, 5),
        allocation(1, 1, 1),
    ];
    let pools = [pool(2, 10, 3), pool(1, 4, 2)];
    let plan = plan_replacement_allocations(&current, &desired, &pools).unwrap();
    assert_eq!(
        plan.additional_allocations(),
        &[allocation(2, 2, 2), allocation(3, 2, 1)]
    );
    assert_eq!(plan.pending_releases(), &[allocation(1, 1, 1)]);
    assert_eq!(plan.preparatory_holds().len(), 1);
    assert_eq!(plan.preparatory_holds()[0].quantity(), ticks(3));
    let reverse = plan_replacement_allocations(
        &current.into_iter().rev().collect::<Vec<_>>(),
        &desired.into_iter().rev().collect::<Vec<_>>(),
        &pools.into_iter().rev().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        plan.additional_allocations(),
        reverse.additional_allocations()
    );
    assert_eq!(plan.pending_releases(), reverse.pending_releases());
    assert_eq!(plan.preparatory_holds(), reverse.preparatory_holds());
}

#[test]
fn duplicate_unknown_stale_and_over_budget_data_never_make_partial_plans() {
    let value = allocation(1, 1, 1);
    for (current, desired, pools) in [
        (vec![value, value], vec![], vec![pool(1, 4, 2)]),
        (vec![], vec![value, value], vec![pool(1, 4, 2)]),
        (vec![value], vec![], vec![]),
        (vec![], vec![value], vec![]),
        (vec![], vec![value], vec![pool(1, 4, 2), pool(1, 4, 2)]),
    ] {
        assert!(plan_replacement_allocations(&current, &desired, &pools).is_err());
    }
    assert_eq!(
        plan_replacement_allocations(&vec![value; 65], &[], &[pool(1, 4, 2)]).unwrap_err(),
        AllocationPlanError::Capacity
    );
    assert_eq!(
        plan_replacement_allocations(&[], &vec![value; 65], &[pool(1, 4, 2)]).unwrap_err(),
        AllocationPlanError::Capacity
    );
    assert_eq!(
        plan_replacement_allocations(&[], &[], &vec![pool(1, 4, 2); 97]).unwrap_err(),
        AllocationPlanError::Capacity
    );
    let old = pool(1, 4, 2);
    let absent = PoolSnapshot::new(
        old.id(),
        old.unit(),
        old.balances(),
        Sequence::new(0).unwrap(),
    );
    assert!(plan_replacement_allocations(&[], &[value], &[absent]).is_err());
}

#[test]
fn bounded_small_state_grid_conserves_each_positive_hold_without_crediting_any_release() {
    for old in 0..=5 {
        for new in 0..=5 {
            let current = if old == 0 {
                vec![]
            } else {
                vec![allocation(1, 1, old)]
            };
            let desired = if new == 0 {
                vec![]
            } else {
                vec![allocation(1, 1, new)]
            };
            let before = pool(1, 5, old);
            let plan = plan_replacement_allocations(&current, &desired, &[before]).unwrap();
            if new > old {
                let effect = plan.preparatory_holds()[0];
                assert_eq!(effect.quantity(), ticks(new - old));
                assert_eq!(
                    effect.after().balances().physical().available,
                    ticks(5 - (new - old))
                );
                assert_eq!(
                    effect.after().balances().physical().committed_hold,
                    ticks(old)
                );
                assert_eq!(effect.after().balances().introduced(), ticks(5 + old));
            } else {
                assert!(plan.preparatory_holds().is_empty());
            }
            if old > new {
                assert_eq!(plan.pending_releases(), &[allocation(1, 1, old - new)]);
            } else {
                assert!(plan.pending_releases().is_empty());
            }
        }
    }
}
