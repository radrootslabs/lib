use crate::{FixedPrice, VariableWeight};

/// Public sale-option economics. Variable bounds and cap describe one sale unit;
/// an agreement must explicitly resolve its complete absolute line terms.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Price {
    /// Complete fixed rate, basis, single discount and ordered tiers.
    Fixed(FixedPrice),
    /// An explicit request for a quote, without an invented zero or currency.
    QuoteRequired,
    /// Complete per-sale-unit measurement bounds, exact rate and monetary cap.
    VariableWeight(VariableWeight),
}

#[cfg(feature = "serde")]
impl serde::Serialize for Price {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        match self {
            Self::Fixed(value) => {
                let mut out = serializer.serialize_struct("Price", 5)?;
                out.serialize_field("type", "fixed")?;
                out.serialize_field("rate", &value.rate())?;
                out.serialize_field("basis", &value.basis())?;
                out.serialize_field("discount_bps", &value.discount_bps())?;
                out.serialize_field("tiers", value.tiers())?;
                out.end()
            }
            Self::QuoteRequired => {
                let mut out = serializer.serialize_struct("Price", 1)?;
                out.serialize_field("type", "quote_required")?;
                out.end()
            }
            Self::VariableWeight(value) => {
                let mut out = serializer.serialize_struct("Price", 7)?;
                out.serialize_field("type", "variable_weight")?;
                out.serialize_field("measurement_unit", &value.measurement_unit())?;
                out.serialize_field("minimum_per_sale_unit", &value.minimum())?;
                out.serialize_field("maximum_per_sale_unit", &value.maximum())?;
                out.serialize_field("rate", &value.rate())?;
                out.serialize_field("basis", &value.basis())?;
                out.serialize_field("maximum_per_sale_unit_total", &value.maximum_total())?;
                out.end()
            }
        }
    }
}
