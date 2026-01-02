#[cfg(feature = "std")]
use std::string::String;
#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum RadrootsCoreDiscountScope {
    Bin,
    OrderTotal,
}

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
pub enum RadrootsCoreDiscountThreshold {
    BinCount { bin_id: String, min: u32 },
    OrderQuantity { min: crate::RadrootsCoreQuantity },
}

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
pub enum RadrootsCoreDiscountValue {
    MoneyPerBin(crate::RadrootsCoreMoney),
    Percent(crate::RadrootsCorePercent),
}

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub struct RadrootsCoreDiscount {
    pub scope: RadrootsCoreDiscountScope,
    pub threshold: RadrootsCoreDiscountThreshold,
    pub value: RadrootsCoreDiscountValue,
}

impl RadrootsCoreDiscount {
    pub fn is_non_negative(&self) -> bool {
        let threshold_ok = match &self.threshold {
            RadrootsCoreDiscountThreshold::BinCount { .. } => true,
            RadrootsCoreDiscountThreshold::OrderQuantity { min } => !min.amount.is_sign_negative(),
        };
        let value_ok = match &self.value {
            RadrootsCoreDiscountValue::MoneyPerBin(m) => !m.amount.is_sign_negative(),
            RadrootsCoreDiscountValue::Percent(p) => !p.value.is_sign_negative(),
        };
        threshold_ok && value_ok
    }
}
