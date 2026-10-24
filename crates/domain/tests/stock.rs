use radroots_domain::stock::{
    PhysicalBalances, PoolBalances, PoolSnapshot, PoolUnit, StockError, StockTicks,
};
use radroots_domain::{Decimal, PoolId, Quantity, Sequence, listing::MeasurementUnit::*};

fn ticks(value: u64) -> StockTicks {
    StockTicks::new(value).unwrap()
}

#[test]
fn available_corrections_conserve_audited_totals_without_touching_backed_claims() {
    use radroots_domain::stock::AvailableStockCorrection::{Add, Remove, Spoil};
    let original = PoolBalances::new(
        ticks(100),
        PhysicalBalances {
            available: ticks(30),
            preparatory_hold: ticks(10),
            committed_hold: ticks(20),
            physical_quarantine: ticks(5),
            consumed: ticks(15),
            waste: ticks(10),
            removed: ticks(10),
        },
    )
    .unwrap();
    let added = original.correct_available(Add, ticks(7)).unwrap();
    assert_eq!(added.introduced(), ticks(107));
    assert_eq!(
        added.physical(),
        PhysicalBalances {
            available: ticks(37),
            ..original.physical()
        }
    );
    let removed = added.correct_available(Remove, ticks(11)).unwrap();
    assert_eq!(removed.introduced(), ticks(107));
    assert_eq!(
        removed.physical(),
        PhysicalBalances {
            available: ticks(26),
            removed: ticks(21),
            ..original.physical()
        }
    );
    let wasted = removed.correct_available(Spoil, ticks(26)).unwrap();
    assert_eq!(wasted.introduced(), ticks(107));
    assert_eq!(
        wasted.physical(),
        PhysicalBalances {
            available: StockTicks::ZERO,
            removed: ticks(21),
            waste: ticks(36),
            ..original.physical()
        }
    );
    for kind in [Remove, Spoil] {
        assert_eq!(
            original.correct_available(kind, ticks(31)),
            Err(StockError::Underflow)
        );
        assert_eq!(
            wasted.correct_available(kind, ticks(1)),
            Err(StockError::Underflow)
        );
    }
    for kind in [Add, Remove, Spoil] {
        assert_eq!(
            original.correct_available(kind, StockTicks::ZERO),
            Err(StockError::ZeroMovement)
        );
    }
    assert_eq!(original.physical().available, ticks(30));
}

#[test]
fn introduced_capacity_is_not_reused_after_removal_or_spoilage() {
    use radroots_domain::stock::AvailableStockCorrection::{Add, Remove, Spoil};
    let full = PoolBalances::new(
        StockTicks::MAX,
        PhysicalBalances {
            available: StockTicks::MAX,
            ..PhysicalBalances::ZERO
        },
    )
    .unwrap();
    for kind in [Remove, Spoil] {
        let depleted = full.correct_available(kind, StockTicks::MAX).unwrap();
        assert_eq!(depleted.physical().available, StockTicks::ZERO);
        assert_eq!(depleted.introduced(), StockTicks::MAX);
        assert_eq!(
            depleted.correct_available(Add, ticks(1)),
            Err(StockError::OutOfRange)
        );
    }
    assert_eq!(
        full.correct_available(Add, ticks(1)),
        Err(StockError::OutOfRange)
    );
}

#[test]
fn tick_bounds_and_canonical_integer_strings_never_round_or_wrap() {
    for value in [
        0,
        1,
        9_007_199_254_740_991,
        9_007_199_254_740_993,
        StockTicks::MAX.get(),
    ] {
        let exact = ticks(value);
        assert_eq!(
            exact.to_string().parse::<StockTicks>().unwrap().get(),
            value
        );
    }
    for value in [StockTicks::MAX.get() + 1, u64::MAX] {
        assert_eq!(StockTicks::new(value), Err(StockError::OutOfRange));
    }
    for value in [
        "",
        "00",
        "01",
        "+1",
        "-1",
        "1.0",
        "1e3",
        " 1",
        "1\n",
        "１",
        "١",
        "10000000000000000000",
    ] {
        assert!(value.parse::<StockTicks>().is_err(), "{value:?}");
    }
    assert_eq!(
        "1000000000000000001".parse::<StockTicks>(),
        Err(StockError::OutOfRange)
    );
    assert_eq!(
        StockTicks::MAX.checked_add(ticks(1)),
        Err(StockError::OutOfRange)
    );
    assert_eq!(
        StockTicks::ZERO.checked_sub(ticks(1)),
        Err(StockError::Underflow)
    );
    assert_eq!(
        StockTicks::MAX.checked_sub(StockTicks::MAX),
        Ok(StockTicks::ZERO)
    );
}

#[test]
fn every_physical_unit_has_explicit_precision_and_each_is_integral() {
    for unit in [Gram, Kilogram, Millilitre, Litre, Each] {
        for scale in 0..=6 {
            let result = PoolUnit::new(unit, scale);
            if unit == Each && scale != 0 {
                assert_eq!(result, Err(StockError::FractionalCount));
            } else {
                let value = result.unwrap();
                assert_eq!(value.unit(), unit);
                assert_eq!(value.scale(), scale);
            }
        }
        for scale in [7, u8::MAX] {
            assert_eq!(PoolUnit::new(unit, scale), Err(StockError::InvalidScale));
        }
    }
}

#[test]
fn quantities_convert_to_integral_ticks_only_with_exact_explicit_pool_units() {
    for unit in [Gram, Kilogram, Millilitre, Litre] {
        for (amount, scale, expected) in [
            ("0", 0, 0),
            ("1.25", 2, 125),
            ("0.000001", 6, 1),
            ("999999999999", 6, 999_999_999_999_000_000),
        ] {
            let quantity = Quantity::new(amount.parse().unwrap(), unit).unwrap();
            let pool = PoolUnit::new(unit, scale).unwrap();
            assert_eq!(pool.ticks_for(quantity).unwrap().get(), expected);
        }
        let quantity = Quantity::new("1.000001".parse().unwrap(), unit).unwrap();
        for scale in 0..6 {
            assert_eq!(
                PoolUnit::new(unit, scale).unwrap().ticks_for(quantity),
                Err(StockError::PrecisionLoss)
            );
        }
        let quantity = Quantity::new(Decimal::ONE, unit).unwrap();
        for other in [Gram, Kilogram, Millilitre, Litre, Each] {
            if other != unit {
                assert_eq!(
                    PoolUnit::new(other, 0).unwrap().ticks_for(quantity),
                    Err(StockError::UnitMismatch)
                );
            }
        }
    }
    let kg = Quantity::new("1.25".parse().unwrap(), Kilogram).unwrap();
    let gram_pool = PoolUnit::new(Gram, 0).unwrap();
    assert_eq!(gram_pool.ticks_for(kg), Err(StockError::UnitMismatch));
    assert_eq!(
        gram_pool.ticks_for(kg.convert(Gram).unwrap()).unwrap(),
        ticks(1250)
    );
    let each = Quantity::new("999999999999".parse().unwrap(), Each).unwrap();
    assert_eq!(
        PoolUnit::new(Each, 0).unwrap().ticks_for(each).unwrap(),
        ticks(999_999_999_999)
    );
}

#[test]
fn every_category_is_counted_once_and_released_stock_is_a_movement() {
    for position in 0..7 {
        let mut physical = PhysicalBalances::ZERO;
        let target = match position {
            0 => &mut physical.available,
            1 => &mut physical.preparatory_hold,
            2 => &mut physical.committed_hold,
            3 => &mut physical.physical_quarantine,
            4 => &mut physical.consumed,
            5 => &mut physical.waste,
            6 => &mut physical.removed,
            _ => unreachable!(),
        };
        *target = StockTicks::MAX;
        let exact = PoolBalances::new(StockTicks::MAX, physical).unwrap();
        assert_eq!(exact.physical(), physical);
        assert_eq!(exact.introduced(), StockTicks::MAX);
        assert!(PoolBalances::new(ticks(StockTicks::MAX.get() - 1), physical).is_err());
    }
    let before = PhysicalBalances {
        available: ticks(70),
        preparatory_hold: ticks(30),
        ..PhysicalBalances::ZERO
    };
    let after = PhysicalBalances {
        available: ticks(100),
        ..PhysicalBalances::ZERO
    };
    assert!(PoolBalances::new(ticks(100), before).is_ok());
    assert!(PoolBalances::new(ticks(100), after).is_ok());
    // Treating the release as introduced stock would double count it.
    assert_eq!(
        PoolBalances::new(ticks(130), after),
        Err(StockError::ConservationViolation)
    );
}

#[test]
fn mixed_physical_categories_conserve_and_promises_or_deficits_cannot_fill_a_gap() {
    let physical = PhysicalBalances {
        available: ticks(1),
        preparatory_hold: ticks(2),
        committed_hold: ticks(3),
        physical_quarantine: ticks(4),
        consumed: ticks(5),
        waste: ticks(6),
        removed: ticks(7),
    };
    let balances = PoolBalances::new(ticks(28), physical).unwrap();
    assert_eq!(balances.physical(), physical);
    for introduced in [0, 27, 29] {
        assert_eq!(
            PoolBalances::new(ticks(introduced), physical),
            Err(StockError::ConservationViolation)
        );
    }
    let over = PhysicalBalances {
        available: StockTicks::MAX,
        waste: StockTicks::MAX,
        ..PhysicalBalances::ZERO
    };
    assert_eq!(
        PoolBalances::new(StockTicks::MAX, over),
        Err(StockError::OutOfRange)
    );
    assert_eq!(
        PoolBalances::new(ticks(1), PhysicalBalances::ZERO),
        Err(StockError::ConservationViolation)
    );
}

#[test]
fn snapshots_keep_pool_identity_precision_and_declared_version_distinct() {
    let id = PoolId::from_bytes([7; 32]);
    let balances = PoolBalances::new(
        ticks(5),
        PhysicalBalances {
            available: ticks(5),
            ..PhysicalBalances::ZERO
        },
    )
    .unwrap();
    let unit = PoolUnit::new(Kilogram, 3).unwrap();
    let version = Sequence::new(0).unwrap();
    let snapshot = PoolSnapshot::new(id, unit, balances, version);
    assert_eq!(snapshot.id(), id);
    assert_eq!(snapshot.unit(), unit);
    assert_eq!(snapshot.balances(), balances);
    assert_eq!(snapshot.version(), version);
    assert!(
        !format!(
            "{snapshot:?} {balances:?} {:?} {:?}",
            balances.physical(),
            ticks(123456789)
        )
        .contains("123456789")
    );
}

#[cfg(feature = "serde")]
#[test]
fn foreign_tick_serialization_uses_exact_strings_and_rejects_json_numbers() {
    for value in [
        StockTicks::ZERO,
        ticks(9_007_199_254_740_993),
        StockTicks::MAX,
    ] {
        let wire = serde_json::to_string(&value).unwrap();
        assert_eq!(wire, format!("\"{value}\""));
        assert_eq!(serde_json::from_str::<StockTicks>(&wire).unwrap(), value);
    }
    for value in [
        "0",
        "9007199254740993",
        "1.0",
        "null",
        "true",
        "[]",
        "{}",
        "\"01\"",
        "\"1e3\"",
        "\"1000000000000000001\"",
    ] {
        assert!(
            serde_json::from_str::<StockTicks>(value).is_err(),
            "{value}"
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn protected_projection_value_codec_is_lossless_and_checks_conservation() {
    use serde_json::{Value, json};
    let amount = StockTicks::MAX;
    let snapshot = PoolSnapshot::new(
        PoolId::from_bytes([9; 32]),
        PoolUnit::new(Kilogram, 6).unwrap(),
        PoolBalances::new(
            amount,
            PhysicalBalances {
                available: amount,
                ..PhysicalBalances::ZERO
            },
        )
        .unwrap(),
        Sequence::new(i64::MAX as u64).unwrap(),
    );
    let value = serde_json::to_value(snapshot).unwrap();
    assert_eq!(value["unit"], json!({"unit":"kg","scale":"6"}));
    assert_eq!(
        value["balances"]["introduced"],
        json!("1000000000000000000")
    );
    assert_eq!(value["version"], json!("9223372036854775807"));
    assert_eq!(
        serde_json::from_value::<PoolSnapshot>(value.clone()).unwrap(),
        snapshot
    );
    for (path, bad) in [
        ("/balances/physical/available", json!("999999999999999999")),
        ("/balances/physical/available", json!("1000000000000000001")),
        ("/balances/physical/available", json!(100)),
        ("/balances/introduced", json!("-1")),
        ("/version", json!("01")),
        ("/unit/scale", json!(6)),
        ("/unit/scale", json!("06")),
        ("/unit/scale", json!("7")),
        ("/unit/unit", json!("each")),
        ("/unit", json!(["kg", "6"])),
        ("/balances", json!(["1000000000000000000", {}])),
        (
            "/balances/physical",
            json!(["1000000000000000000", "0", "0", "0", "0", "0", "0"]),
        ),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert!(
            serde_json::from_value::<PoolSnapshot>(changed).is_err(),
            "{path}"
        );
    }
    for path in ["", "/unit", "/balances", "/balances/physical"] {
        let mut changed = value.clone();
        changed
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), Value::Null);
        assert!(
            serde_json::from_value::<PoolSnapshot>(changed).is_err(),
            "{path}"
        );
    }
    assert!(
        serde_json::from_value::<PoolSnapshot>(json!([
            value["id"],
            value["unit"],
            value["balances"],
            value["version"]
        ]))
        .is_err()
    );
}
#[cfg(feature = "serde")]
#[test]
fn unit_codecs_accept_only_exact_symbols_and_canonical_precision_strings() {
    use serde_json::json;
    for unit in [Gram, Kilogram, Millilitre, Litre, Each] {
        for scale in 0..=6 {
            if unit == Each && scale != 0 {
                continue;
            }
            let value = PoolUnit::new(unit, scale).unwrap();
            assert_eq!(
                serde_json::from_str::<PoolUnit>(&serde_json::to_string(&value).unwrap()).unwrap(),
                value
            );
        }
    }
    for symbol in [
        json!("ml"),
        json!("l"),
        json!("KG"),
        json!("piece"),
        json!({"g":null}),
        json!(["g"]),
    ] {
        assert!(
            serde_json::from_value::<radroots_domain::listing::MeasurementUnit>(symbol).is_err()
        );
    }
}
