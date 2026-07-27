#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(feature = "std")]
use std::string::String;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(test, derive(dto_bindgen::Dto))]
#[cfg_attr(test, dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum DiscountScope {
    Bin,
    OrderTotal,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(test, derive(dto_bindgen::Dto))]
#[cfg_attr(test, dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
pub enum DiscountThreshold {
    BinCount { bin_id: String, min: u32 },
    OrderQuantity { min: crate::Quantity },
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(test, derive(dto_bindgen::Dto))]
#[cfg_attr(test, dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
pub enum DiscountValue {
    MoneyPerBin(crate::Money),
    Percent(crate::Percent),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(test, derive(dto_bindgen::Dto))]
#[cfg_attr(test, dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub struct Discount {
    pub scope: DiscountScope,
    pub threshold: DiscountThreshold,
    pub value: DiscountValue,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NegativeThreshold,
    NegativeValue,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeThreshold => f.write_str("discount threshold must be non-negative"),
            Self::NegativeValue => f.write_str("discount value must be non-negative"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl Discount {
    #[inline]
    pub fn try_new(
        scope: DiscountScope,
        threshold: DiscountThreshold,
        value: DiscountValue,
    ) -> Result<Self, Error> {
        let discount = Self {
            scope,
            threshold,
            value,
        };
        discount.validate()?;
        Ok(discount)
    }

    #[inline]
    pub fn scope(&self) -> &DiscountScope {
        &self.scope
    }

    #[inline]
    pub fn threshold(&self) -> &DiscountThreshold {
        &self.threshold
    }

    #[inline]
    pub fn value(&self) -> &DiscountValue {
        &self.value
    }

    pub fn validate(&self) -> Result<(), Error> {
        if let DiscountThreshold::OrderQuantity { min } = &self.threshold {
            min.ensure_non_negative()
                .map_err(|_| Error::NegativeThreshold)?;
        }
        match &self.value {
            DiscountValue::MoneyPerBin(money) => money
                .ensure_non_negative()
                .map_err(|_| Error::NegativeValue)?,
            DiscountValue::Percent(percent) if percent.value.is_sign_negative() => {
                return Err(Error::NegativeValue);
            }
            DiscountValue::Percent(_) => {}
        }
        Ok(())
    }

    pub fn is_non_negative(&self) -> bool {
        self.validate().is_ok()
    }
}

#[deprecated(since = "0.1.0", note = "renamed to `pricing::Discount`")]
pub use self::Discount as RadrootsCoreDiscount;

#[deprecated(since = "0.1.0", note = "renamed to `pricing::DiscountScope`")]
pub use self::DiscountScope as RadrootsCoreDiscountScope;

#[deprecated(since = "0.1.0", note = "renamed to `pricing::DiscountThreshold`")]
pub use self::DiscountThreshold as RadrootsCoreDiscountThreshold;

#[deprecated(since = "0.1.0", note = "renamed to `pricing::DiscountValue`")]
pub use self::DiscountValue as RadrootsCoreDiscountValue;
use core::fmt;
