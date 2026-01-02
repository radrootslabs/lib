mod common;

use radroots_core::{RadrootsCoreQuantityInvariantError, RadrootsCoreUnit};

#[test]
fn label_helpers_set_and_clear() {
    let q = common::qty("1", RadrootsCoreUnit::Each).with_label("box");
    assert_eq!(q.label.as_deref(), Some("box"));

    let q = q.clear_label();
    assert!(q.label.is_none());

    let q = common::qty("1", RadrootsCoreUnit::Each).with_optional_label(Some("case"));
    assert_eq!(q.label.as_deref(), Some("case"));

    let q = q.with_optional_label::<&str>(None);
    assert!(q.label.is_none());
}

#[test]
fn ensure_non_negative_rejects_negative_amount() {
    let q = common::qty("-1", RadrootsCoreUnit::Each);
    assert_eq!(
        q.ensure_non_negative(),
        Err(RadrootsCoreQuantityInvariantError::NegativeAmount)
    );
}

#[test]
fn try_add_and_try_sub_require_matching_units() {
    let a = common::qty("1", RadrootsCoreUnit::Each).with_label("lhs");
    let b = common::qty("2", RadrootsCoreUnit::Each);
    let c = common::qty("1", RadrootsCoreUnit::MassKg);

    let sum = a.try_add(&b).unwrap();
    assert_eq!(sum.amount, common::dec("3"));
    assert_eq!(sum.label.as_deref(), Some("lhs"));

    assert_eq!(
        a.try_add(&c),
        Err(RadrootsCoreQuantityInvariantError::UnitMismatch)
    );
    assert_eq!(
        b.try_sub(&c),
        Err(RadrootsCoreQuantityInvariantError::UnitMismatch)
    );
}

#[test]
fn checked_add_and_sub_return_none_on_mismatch() {
    let a = common::qty("1", RadrootsCoreUnit::Each);
    let b = common::qty("2", RadrootsCoreUnit::MassG);
    assert!(a.checked_add(&b).is_none());
    assert!(a.checked_sub(&b).is_none());
}

#[test]
fn mul_and_div_preserve_unit_and_label() {
    let q = common::qty("2", RadrootsCoreUnit::Each).with_label("unit");
    let scaled = q.clone().mul_decimal(common::dec("2.5"));
    assert_eq!(scaled.amount, common::dec("5"));
    assert_eq!(scaled.unit, RadrootsCoreUnit::Each);
    assert_eq!(scaled.label.as_deref(), Some("unit"));

    let divided = q.div_decimal(common::dec("2"));
    assert_eq!(divided.amount, common::dec("1"));
    assert_eq!(divided.unit, RadrootsCoreUnit::Each);
    assert_eq!(divided.label.as_deref(), Some("unit"));
}

#[test]
fn display_includes_label_when_present() {
    let q = common::qty("1.5", RadrootsCoreUnit::Each).with_label("bag");
    assert_eq!(q.to_string(), "1.5 each (bag)");
}

#[test]
fn try_convert_to_changes_unit_and_amount() {
    let q = common::qty("1", RadrootsCoreUnit::MassKg);
    let converted = q.try_convert_to(RadrootsCoreUnit::MassG).unwrap();
    assert_eq!(converted.amount, common::dec("1000"));
    assert_eq!(converted.unit, RadrootsCoreUnit::MassG);
}

#[test]
fn to_canonical_converts_mass_and_volume() {
    let q = common::qty("2", RadrootsCoreUnit::VolumeL);
    let canonical = q.to_canonical().unwrap();
    assert_eq!(canonical.unit, RadrootsCoreUnit::VolumeMl);
    assert_eq!(canonical.amount, common::dec("2000"));
}

#[test]
fn try_convert_to_rejects_mismatched_dimensions() {
    let q = common::qty("1", RadrootsCoreUnit::Each);
    let err = q.try_convert_to(RadrootsCoreUnit::MassG).unwrap_err();
    assert_eq!(
        err,
        radroots_core::RadrootsCoreUnitConvertError::NotConvertibleUnits {
            from: RadrootsCoreUnit::Each,
            to: RadrootsCoreUnit::MassG
        }
    );
}
