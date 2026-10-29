use radroots_domain::stock::{
    HeldStockCategory, HeldStockLoss, ObligationBacking, PhysicalBalances, PoolBalances,
    StockError, StockTicks,
};

fn ticks(value: u64) -> StockTicks {
    StockTicks::new(value).unwrap()
}

fn total(physical: PhysicalBalances) -> u128 {
    [
        physical.available,
        physical.preparatory_hold,
        physical.committed_hold,
        physical.physical_quarantine,
        physical.consumed,
        physical.waste,
        physical.removed,
    ]
    .into_iter()
    .map(|quantity| u128::from(quantity.get()))
    .sum()
}

#[test]
fn shortage_handover_and_residual_release_never_convert_deficits_into_physical_stock() {
    let physical = PoolBalances::new(
        ticks(100),
        PhysicalBalances {
            available: ticks(20),
            committed_hold: ticks(80),
            ..PhysicalBalances::ZERO
        },
    )
    .unwrap();
    let obligation = ObligationBacking::new(ticks(100), ticks(80), ticks(20)).unwrap();
    let depleted = physical
        .deplete_held(
            HeldStockCategory::Committed,
            HeldStockLoss::Spoilage,
            ticks(30),
        )
        .unwrap();
    let shortage = obligation.lose_backing(ticks(30)).unwrap();
    assert_eq!(shortage.remaining(), ticks(100));
    assert_eq!(shortage.backed(), ticks(50));
    assert_eq!(shortage.deficit(), ticks(50));
    assert_eq!(depleted.physical().waste, ticks(30));
    assert_eq!(depleted.physical().available, ticks(20));

    let delivered = depleted.consume_committed(ticks(20)).unwrap();
    let remaining = shortage.consume_backed(ticks(20)).unwrap();
    assert_eq!(remaining.remaining(), ticks(80));
    assert_eq!(remaining.backed(), ticks(30));
    assert_eq!(remaining.deficit(), ticks(50));
    assert_eq!(delivered.physical().consumed, ticks(20));
    assert_eq!(
        delivered.release_committed(ticks(80)),
        Err(StockError::Underflow)
    );

    let released = delivered.release_committed(ticks(30)).unwrap();
    let closed = remaining.release_residual(ticks(30), ticks(50)).unwrap();
    assert_eq!(closed.remaining(), StockTicks::ZERO);
    assert_eq!(closed.backed(), StockTicks::ZERO);
    assert_eq!(closed.deficit(), StockTicks::ZERO);
    assert_eq!(released.physical().available, ticks(50));
    assert_eq!(released.physical().consumed, ticks(20));
    assert_eq!(released.physical().waste, ticks(30));
    assert_eq!(total(released.physical()), 100);
    assert_eq!(physical.physical().committed_hold, ticks(80));
    assert_eq!(obligation.backed(), ticks(80));
}

#[test]
fn quarantine_stays_nonspendable_until_an_explicit_proposed_resolution() {
    let original = PoolBalances::new(
        ticks(10),
        PhysicalBalances {
            preparatory_hold: ticks(4),
            committed_hold: ticks(6),
            ..PhysicalBalances::ZERO
        },
    )
    .unwrap();
    let held = original
        .quarantine_preparatory(ticks(4))
        .unwrap()
        .quarantine_committed(ticks(6))
        .unwrap();
    assert_eq!(held.physical().physical_quarantine, ticks(10));
    assert_eq!(held.physical().available, StockTicks::ZERO);
    assert_eq!(held.consume_committed(ticks(1)), Err(StockError::Underflow));
    assert_eq!(held.hold_available(ticks(1)), Err(StockError::Underflow));

    let reconciled = held
        .restore_quarantined(ticks(3))
        .unwrap()
        .consume_committed(ticks(3))
        .unwrap()
        .consume_quarantined(ticks(2))
        .unwrap()
        .release_quarantined(ticks(5))
        .unwrap();
    assert_eq!(reconciled.physical().available, ticks(5));
    assert_eq!(reconciled.physical().consumed, ticks(5));
    assert_eq!(reconciled.physical().physical_quarantine, StockTicks::ZERO);
    assert_eq!(total(reconciled.physical()), 10);
    assert_eq!(original.physical().committed_hold, ticks(6));
}

type Movement = fn(PoolBalances, StockTicks) -> Result<PoolBalances, StockError>;

#[test]
fn every_held_transition_conserves_lifetime_stock_and_refuses_zero_or_overdrawn_sources() {
    let movements: [(HeldStockCategory, Movement); 7] = [
        (
            HeldStockCategory::Committed,
            PoolBalances::release_committed,
        ),
        (
            HeldStockCategory::Committed,
            PoolBalances::consume_committed,
        ),
        (
            HeldStockCategory::Preparatory,
            PoolBalances::quarantine_preparatory,
        ),
        (
            HeldStockCategory::Committed,
            PoolBalances::quarantine_committed,
        ),
        (
            HeldStockCategory::Quarantined,
            PoolBalances::restore_quarantined,
        ),
        (
            HeldStockCategory::Quarantined,
            PoolBalances::release_quarantined,
        ),
        (
            HeldStockCategory::Quarantined,
            PoolBalances::consume_quarantined,
        ),
    ];
    for seed in 0..=16 {
        let physical = PhysicalBalances {
            available: ticks(seed),
            preparatory_hold: ticks(seed + 1),
            committed_hold: ticks(seed + 2),
            physical_quarantine: ticks(seed + 3),
            consumed: ticks(seed + 4),
            waste: ticks(seed + 5),
            removed: ticks(seed + 6),
        };
        let introduced = ticks(7 * seed + 21);
        let original = PoolBalances::new(introduced, physical).unwrap();
        for (category, movement) in movements {
            let source = match category {
                HeldStockCategory::Preparatory => seed + 1,
                HeldStockCategory::Committed => seed + 2,
                HeldStockCategory::Quarantined => seed + 3,
            };
            assert_eq!(
                movement(original, StockTicks::ZERO),
                Err(StockError::ZeroMovement)
            );
            assert_eq!(
                movement(original, ticks(source + 1)),
                Err(StockError::Underflow)
            );
            for quantity in [ticks(1), ticks(source)] {
                let changed = movement(original, quantity).unwrap();
                assert_eq!(changed.introduced(), introduced);
                assert_eq!(total(changed.physical()), u128::from(introduced.get()));
                assert!(changed.physical().consumed >= physical.consumed);
                assert_eq!(changed.physical().waste, physical.waste);
                assert_eq!(changed.physical().removed, physical.removed);
            }
            for loss in [HeldStockLoss::Spoilage, HeldStockLoss::Removal] {
                let changed = original
                    .deplete_held(category, loss, ticks(source))
                    .unwrap();
                assert_eq!(changed.introduced(), introduced);
                assert_eq!(total(changed.physical()), u128::from(introduced.get()));
                assert_eq!(changed.physical().available, physical.available);
                assert_eq!(changed.physical().consumed, physical.consumed);
                assert_eq!(
                    original.deplete_held(category, loss, StockTicks::ZERO),
                    Err(StockError::ZeroMovement)
                );
                assert_eq!(
                    original.deplete_held(category, loss, ticks(source + 1)),
                    Err(StockError::Underflow)
                );
            }
        }
        assert_eq!(original.physical(), physical);
    }
}

#[test]
fn maximum_backing_can_be_consumed_released_or_lost_without_minting_capacity() {
    let original = PoolBalances::new(
        StockTicks::MAX,
        PhysicalBalances {
            committed_hold: StockTicks::MAX,
            ..PhysicalBalances::ZERO
        },
    )
    .unwrap();
    let consumed = original.consume_committed(StockTicks::MAX).unwrap();
    assert_eq!(consumed.physical().consumed, StockTicks::MAX);
    assert_eq!(consumed.physical().available, StockTicks::ZERO);
    assert_eq!(
        consumed.consume_committed(ticks(1)),
        Err(StockError::Underflow)
    );
    let released = original.release_committed(StockTicks::MAX).unwrap();
    assert_eq!(released.physical().available, StockTicks::MAX);
    assert_eq!(released.introduced(), StockTicks::MAX);
    let quarantined = original.quarantine_committed(StockTicks::MAX).unwrap();
    assert_eq!(
        quarantined.consume_quarantined(StockTicks::MAX).unwrap(),
        consumed
    );
    let lost = original
        .deplete_held(
            HeldStockCategory::Committed,
            HeldStockLoss::Removal,
            StockTicks::MAX,
        )
        .unwrap();
    assert_eq!(lost.physical().removed, StockTicks::MAX);
    assert_eq!(lost.physical().available, StockTicks::ZERO);
    assert_eq!(total(lost.physical()), u128::from(StockTicks::MAX.get()));
}

#[test]
fn obligation_bounds_never_clamp_deficits_or_consume_unbacked_quantities() {
    assert_eq!(
        ObligationBacking::new(ticks(3), ticks(1), ticks(1)),
        Err(StockError::ObligationMismatch)
    );
    assert_eq!(
        ObligationBacking::new(StockTicks::MAX, StockTicks::MAX, ticks(1)),
        Err(StockError::OutOfRange)
    );
    let original =
        ObligationBacking::new(StockTicks::MAX, StockTicks::MAX, StockTicks::ZERO).unwrap();
    let lost = original.lose_backing(StockTicks::MAX).unwrap();
    assert_eq!(lost.remaining(), StockTicks::MAX);
    assert_eq!(lost.backed(), StockTicks::ZERO);
    assert_eq!(lost.deficit(), StockTicks::MAX);
    assert_eq!(lost.consume_backed(ticks(1)), Err(StockError::Underflow));
    assert_eq!(
        original.restore_backing(ticks(1)),
        Err(StockError::Underflow)
    );
    assert_eq!(lost.restore_backing(StockTicks::MAX).unwrap(), original);
    for proposal in [
        original.lose_backing(StockTicks::ZERO),
        lost.restore_backing(StockTicks::ZERO),
        original.consume_backed(StockTicks::ZERO),
        original.release_residual(StockTicks::ZERO, StockTicks::ZERO),
    ] {
        assert_eq!(proposal, Err(StockError::ZeroMovement));
    }
    let closed = lost
        .release_residual(StockTicks::ZERO, StockTicks::MAX)
        .unwrap();
    assert_eq!(closed.remaining(), StockTicks::ZERO);
    assert_eq!(
        closed.release_residual(StockTicks::ZERO, ticks(1)),
        Err(StockError::Underflow)
    );
    assert_eq!(
        format!("{original:?}"),
        "ObligationBacking(<private proposal>)"
    );
}

#[cfg(feature = "serde")]
#[test]
fn backing_codec_uses_closed_exact_strings_and_revalidates_the_obligation_equation() {
    let original = ObligationBacking::new(ticks(100), ticks(80), ticks(20)).unwrap();
    let raw = serde_json::to_string(&original).unwrap();
    assert_eq!(raw, r#"{"remaining":"100","backed":"80","deficit":"20"}"#);
    assert_eq!(
        serde_json::from_str::<ObligationBacking>(&raw).unwrap(),
        original
    );
    for bad in [
        r#"{"remaining":"100","backed":"80","deficit":"19"}"#,
        r#"{"remaining":"100","backed":80,"deficit":"20"}"#,
        r#"{"remaining":"100","backed":"080","deficit":"20"}"#,
        r#"{"remaining":"100","backed":"80","deficit":"20","available":"20"}"#,
        r#"{"remaining":"100","remaining":"100","backed":"80","deficit":"20"}"#,
        r#"{"remaining":"100","backed":"80"}"#,
        r#"["100","80","20"]"#,
    ] {
        assert!(
            serde_json::from_str::<ObligationBacking>(bad).is_err(),
            "{bad}"
        );
    }
}
