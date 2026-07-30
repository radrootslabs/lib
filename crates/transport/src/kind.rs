#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportImplementationState {
    Real,
    Mock,
}

#[doc(hidden)]
pub type RadrootsTransportCapabilityMaturity = crate::capability::Maturity;

#[doc(hidden)]
pub type RadrootsTransportCapabilityAvailability = crate::capability::Availability;
