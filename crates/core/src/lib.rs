#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod currency;
pub mod decimal;
pub mod discount;
pub mod money;
pub mod percent;
pub mod quantity;
pub mod quantity_price;
#[cfg(feature = "serde")]
pub mod serde_ext;
pub mod unit;

pub use currency::{RadrootsCoreCurrency, RadrootsCoreCurrencyParseError};
pub use decimal::RadrootsCoreDecimal;
pub use discount::{
    RadrootsCoreDiscount, RadrootsCoreDiscountScope, RadrootsCoreDiscountThreshold,
    RadrootsCoreDiscountValue,
};
pub use money::{RadrootsCoreMoney, RadrootsCoreMoneyInvariantError};
pub use percent::{RadrootsCorePercent, RadrootsCorePercentParseError};
pub use quantity::{RadrootsCoreQuantity, RadrootsCoreQuantityInvariantError};
pub use quantity_price::{
    RadrootsCoreQuantityPrice, RadrootsCoreQuantityPriceError, RadrootsCoreQuantityPriceOps,
};
pub use unit::{
    RadrootsCoreUnit, RadrootsCoreUnitConvertError, RadrootsCoreUnitDimension,
    RadrootsCoreUnitParseError, convert_mass_decimal, convert_unit_decimal, convert_volume_decimal,
    parse_mass_unit, parse_volume_unit,
};

#[cfg(all(test, feature = "ts-rs", feature = "std"))]
mod ts_export_tests {
    use crate::{RadrootsCoreDiscount, RadrootsCoreUnitDimension};
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    use ts_rs::TS;

    fn workspace_root(manifest_dir: &Path) -> PathBuf {
        let parent = manifest_dir.parent().unwrap_or(manifest_dir);
        if parent.file_name().and_then(|name| name.to_str()) == Some("crates") {
            parent.parent().unwrap_or(parent).to_path_buf()
        } else {
            parent.to_path_buf()
        }
    }

    fn ts_export_dir() -> PathBuf {
        if let Some(export_dir) = option_env!("TS_RS_EXPORT_DIR") {
            return PathBuf::from(export_dir);
        }
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        workspace_root(&manifest_dir)
            .join("target")
            .join("ts-rs")
            .join("core")
    }

    fn export_types() {
        RadrootsCoreDiscount::export_all().expect("export core ts-rs definitions");
        RadrootsCoreUnitDimension::export_all().expect("export core unit dimension definition");
    }

    #[test]
    fn exports_core_types_file() {
        export_types();
        let path = ts_export_dir().join("types.ts");
        let raw = fs::read_to_string(path).expect("read generated core types");
        assert!(raw.contains("export type RadrootsCoreMoney"));
        assert!(raw.contains("export type RadrootsCoreQuantity"));
        assert!(raw.contains("export type RadrootsCoreQuantityPrice"));
        assert!(raw.contains("export type RadrootsCoreDiscount"));
    }

    #[test]
    fn exports_unit_literal_values() {
        export_types();
        let path = ts_export_dir().join("types.ts");
        let raw = fs::read_to_string(path).expect("read generated core types");
        for literal in ["\"each\"", "\"kg\"", "\"g\"", "\"oz\"", "\"lb\"", "\"l\"", "\"ml\""] {
            assert!(raw.contains(literal), "missing core unit literal: {literal}");
        }
        for literal in ["\"count\"", "\"mass\"", "\"volume\""] {
            assert!(
                raw.contains(literal),
                "missing core unit dimension literal: {literal}"
            );
        }
    }
}
