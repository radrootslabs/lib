mod common;

use core::str::FromStr;

use radroots_core::{
    RadrootsCoreUnit, RadrootsCoreUnitConvertError, RadrootsCoreUnitParseError,
    convert_mass_decimal, convert_unit_decimal, convert_volume_decimal, parse_mass_unit,
    parse_volume_unit,
};

#[test]
fn parses_units_and_synonyms() {
    use RadrootsCoreUnit::*;
    let cases = [
        ("each", Each),
        ("ea", Each),
        ("count", Each),
        ("kg", MassKg),
        ("kilograms", MassKg),
        ("g", MassG),
        ("grams", MassG),
        ("oz", MassOz),
        ("ounces", MassOz),
        ("lb", MassLb),
        ("pounds", MassLb),
        ("l", VolumeL),
        ("liters", VolumeL),
        ("ml", VolumeMl),
        ("milliliters", VolumeMl),
    ];
    for (input, expected) in cases {
        assert_eq!(RadrootsCoreUnit::from_str(input).unwrap(), expected);
    }
}

#[test]
fn rejects_unknown_units() {
    assert_eq!(
        RadrootsCoreUnit::from_str("unknown"),
        Err(RadrootsCoreUnitParseError::UnknownUnit)
    );
}

#[test]
fn same_dimension_matches_mass_and_volume_groups() {
    use RadrootsCoreUnit::*;
    assert!(RadrootsCoreUnit::same_dimension(MassKg, MassG));
    assert!(RadrootsCoreUnit::same_dimension(VolumeL, VolumeMl));
    assert!(RadrootsCoreUnit::same_dimension(Each, Each));
    assert!(!RadrootsCoreUnit::same_dimension(MassKg, VolumeL));
    assert!(!RadrootsCoreUnit::same_dimension(Each, MassG));
}

#[test]
fn canonical_unit_maps_dimensions() {
    use RadrootsCoreUnit::*;
    assert_eq!(Each.canonical_unit(), Each);
    assert_eq!(MassKg.canonical_unit(), MassG);
    assert_eq!(MassLb.canonical_unit(), MassG);
    assert_eq!(VolumeL.canonical_unit(), VolumeMl);
}

#[test]
fn parse_mass_unit_enforces_mass_only() {
    assert_eq!(parse_mass_unit("kg"), Ok(RadrootsCoreUnit::MassKg));
    assert_eq!(
        parse_mass_unit("each"),
        Err(RadrootsCoreUnitParseError::NotAMassUnit)
    );
}

#[test]
fn parse_volume_unit_enforces_volume_only() {
    assert_eq!(parse_volume_unit("l"), Ok(RadrootsCoreUnit::VolumeL));
    assert_eq!(
        parse_volume_unit("kg"),
        Err(RadrootsCoreUnitParseError::NotAVolumeUnit)
    );
}

#[test]
fn convert_mass_decimal_converts_between_mass_units() {
    use RadrootsCoreUnit::*;
    let kg_to_g = convert_mass_decimal(common::dec("1"), MassKg, MassG).unwrap();
    let g_to_kg = convert_mass_decimal(common::dec("1000"), MassG, MassKg).unwrap();
    let lb_to_g = convert_mass_decimal(common::dec("1"), MassLb, MassG).unwrap();

    assert_eq!(kg_to_g, common::dec("1000"));
    assert_eq!(g_to_kg, common::dec("1"));
    assert_eq!(lb_to_g, common::dec("453.59237"));
}

#[test]
fn convert_mass_decimal_rejects_non_mass_units() {
    let err = convert_mass_decimal(
        common::dec("1"),
        RadrootsCoreUnit::Each,
        RadrootsCoreUnit::MassG,
    )
    .unwrap_err();
    assert_eq!(
        err,
        RadrootsCoreUnitConvertError::NotMassUnit {
            from: RadrootsCoreUnit::Each,
            to: RadrootsCoreUnit::MassG
        }
    );
}

#[test]
fn convert_volume_decimal_converts_between_volume_units() {
    use RadrootsCoreUnit::*;
    let l_to_ml = convert_volume_decimal(common::dec("1"), VolumeL, VolumeMl).unwrap();
    let ml_to_l = convert_volume_decimal(common::dec("250"), VolumeMl, VolumeL).unwrap();
    assert_eq!(l_to_ml, common::dec("1000"));
    assert_eq!(ml_to_l, common::dec("0.25"));
}

#[test]
fn convert_unit_decimal_converts_matching_dimensions() {
    use RadrootsCoreUnit::*;
    let kg_to_g = convert_unit_decimal(common::dec("1"), MassKg, MassG).unwrap();
    let l_to_ml = convert_unit_decimal(common::dec("2"), VolumeL, VolumeMl).unwrap();
    let each_to_each = convert_unit_decimal(common::dec("3"), Each, Each).unwrap();
    assert_eq!(kg_to_g, common::dec("1000"));
    assert_eq!(l_to_ml, common::dec("2000"));
    assert_eq!(each_to_each, common::dec("3"));
}

#[test]
fn convert_unit_decimal_rejects_mismatched_dimensions() {
    let err = convert_unit_decimal(
        common::dec("1"),
        RadrootsCoreUnit::Each,
        RadrootsCoreUnit::MassG,
    )
    .unwrap_err();
    assert_eq!(
        err,
        RadrootsCoreUnitConvertError::NotConvertibleUnits {
            from: RadrootsCoreUnit::Each,
            to: RadrootsCoreUnit::MassG
        }
    );
}
