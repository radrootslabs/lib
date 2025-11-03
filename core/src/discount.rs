#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "amount")]
pub enum RadrootsCoreDiscountValue {
    Money(crate::RadrootsCoreMoney),
    Percent(crate::RadrootsCorePercent),
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "amount")]
pub enum RadrootsCoreDiscount {
    QuantityThreshold {
        ref_key: Option<String>,
        threshold: crate::RadrootsCoreQuantity,
        value: crate::RadrootsCoreMoney,
    },
    MassThreshold {
        threshold: crate::RadrootsCoreQuantity,
        value: crate::RadrootsCoreMoney,
    },
    SubtotalThreshold {
        threshold: crate::RadrootsCoreMoney,
        value: RadrootsCoreDiscountValue,
    },
    TotalThreshold {
        total_min: crate::RadrootsCoreMoney,
        value: crate::RadrootsCorePercent,
    },
}

impl RadrootsCoreDiscount {
    pub fn is_non_negative(&self) -> bool {
        match self {
            RadrootsCoreDiscount::QuantityThreshold {
                threshold, value, ..
            } => !threshold.amount.is_sign_negative() && !value.amount.is_sign_negative(),
            RadrootsCoreDiscount::MassThreshold { threshold, value } => {
                !threshold.amount.is_sign_negative() && !value.amount.is_sign_negative()
            }
            RadrootsCoreDiscount::SubtotalThreshold { threshold, value } => {
                let money_ok = !threshold.amount.is_sign_negative();
                let val_ok = match value {
                    RadrootsCoreDiscountValue::Money(m) => !m.amount.is_sign_negative(),
                    RadrootsCoreDiscountValue::Percent(p) => !p.value.is_sign_negative(),
                };
                money_ok && val_ok
            }
            RadrootsCoreDiscount::TotalThreshold { total_min, value } => {
                !total_min.amount.is_sign_negative() && !value.value.is_sign_negative()
            }
        }
    }
}
