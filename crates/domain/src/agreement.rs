use crate::{
    AgreementLineId, Decimal, DiscountBps, FulfillmentGroupId, LinePrice, ListingOptionId,
    MAX_PRICED_LINES, Money, PRICING_PROFILE, PositiveDecimal, PricingError, PrivateFulfillment,
    PublicKey, VariableWeight, Window, checked_order_total,
    listing::{ListingError, ListingOption, ListingRevision, Product},
    pricing::net_line_minor,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt;
use zeroize::Zeroizing;

/// One resolved fixed agreement rate, independent of its public option snapshot.
/// Selected-tier provenance is retained; no tier is selected again or invented.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AgreedFixedPrice {
    rate: Money,
    basis: PositiveDecimal,
    discount_bps: DiscountBps,
    selected_tier_from: Option<Decimal>,
}
impl AgreedFixedPrice {
    /// Captures validated numeric values. A line additionally checks that any
    /// declared selected threshold does not exceed its purchased amount.
    pub const fn new(
        rate: Money,
        basis: PositiveDecimal,
        discount_bps: DiscountBps,
        selected_tier_from: Option<Decimal>,
    ) -> Self {
        Self {
            rate,
            basis,
            discount_bps,
            selected_tier_from,
        }
    }
    /// Retains the exact selected inputs of a qualified fixed-price calculation.
    pub const fn from_line(value: LinePrice) -> Self {
        Self::new(
            value.rate(),
            value.basis(),
            value.discount_bps(),
            value.selected_tier_from(),
        )
    }
    /// Returns the explicitly negotiated or resolved rate.
    pub const fn rate(self) -> Money {
        self.rate
    }
    /// Returns the positive basis in this line's selected sale unit.
    pub const fn basis(self) -> PositiveDecimal {
        self.basis
    }
    /// Returns the single discount, applied before rounding.
    pub const fn discount_bps(self) -> DiscountBps {
        self.discount_bps
    }
    /// Returns retained selected-tier provenance, including explicit None.
    pub const fn selected_tier_from(self) -> Option<Decimal> {
        self.selected_tier_from
    }
    /// Recomputes one net line with the shared checked half-even primitive.
    pub fn total(self, amount: PositiveDecimal) -> Result<Money, TermsError> {
        if self
            .selected_tier_from
            .is_some_and(|threshold| threshold > amount.amount())
        {
            return Err(TermsError::InvalidSelectedTier);
        }
        Ok(Money::new(
            self.rate.currency(),
            net_line_minor(self.rate.minor(), amount, self.basis, self.discount_bps)
                .map_err(TermsError::Pricing)?,
        ))
    }
}

/// Complete agreed price: quote-required has no representation here.
/// Variable measurement bounds/cap are absolute for this entire line.
#[derive(Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum AgreementPrice {
    /// Exact selected fixed inputs; no future tier reselection occurs.
    Fixed(AgreedFixedPrice),
    /// Explicit complete line bounds, unit, rate, basis and monetary cap.
    VariableWeight(VariableWeight),
}
impl AgreementPrice {
    fn maximum_total(self, amount: PositiveDecimal) -> Result<Money, TermsError> {
        match self {
            Self::Fixed(value) => value.total(amount),
            Self::VariableWeight(value) => Ok(value.maximum_total()),
        }
    }
}

/// Complete immutable line data. Its source and snapshot are claims pending
/// independent authenticated source admission; construction grants no consent.
#[derive(Clone, Eq, PartialEq)]
pub struct AgreementLine {
    id: AgreementLineId,
    source: ListingRevision,
    product: Product,
    option: ListingOption,
    amount: PositiveDecimal,
    fulfillment_group: FulfillmentGroupId,
    source_lot_label: Option<Zeroizing<String>>,
    price: AgreementPrice,
    maximum_line_total: Money,
}
impl AgreementLine {
    /// Validates the explicit option identity, purchase rules and independently
    /// recomputed fixed total or variable cap. Negotiated prices need not equal
    /// the original public price. No stock mapping or physical conversion occurs.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: AgreementLineId,
        source: ListingRevision,
        option_id: ListingOptionId,
        product: Product,
        option: ListingOption,
        amount: PositiveDecimal,
        fulfillment_group: FulfillmentGroupId,
        source_lot_label: Option<&str>,
        price: AgreementPrice,
        maximum_line_total: Money,
    ) -> Result<Self, TermsError> {
        if option_id != option.id() {
            return Err(TermsError::OptionMismatch);
        }
        option
            .validate_amount(amount)
            .map_err(TermsError::Purchase)?;
        if let Some(label) = source_lot_label {
            text(label, 120, false)?;
        }
        if price.maximum_total(amount)? != maximum_line_total {
            return Err(TermsError::TotalMismatch);
        }
        Ok(Self {
            id,
            source,
            product,
            option,
            amount,
            fulfillment_group,
            source_lot_label: source_lot_label.map(|label| Zeroizing::new(label.into())),
            price,
            maximum_line_total,
        })
    }
    /// Returns the stable line identity, without lineage authentication.
    pub const fn id(&self) -> AgreementLineId {
        self.id
    }
    /// Returns the exact immutable source revision claim.
    pub const fn source(&self) -> ListingRevision {
        self.source
    }
    /// Returns the option identity, kept equal to the retained option snapshot.
    pub const fn option_id(&self) -> ListingOptionId {
        self.option.id()
    }
    /// Borrows the exact public product snapshot; no current-head lookup occurs.
    pub const fn product(&self) -> &Product {
        &self.product
    }
    /// Borrows the unchanged original sale option, including its public price.
    pub const fn option(&self) -> &ListingOption {
        &self.option
    }
    /// Returns the positive validated sale amount.
    pub const fn amount(&self) -> PositiveDecimal {
        self.amount
    }
    /// Returns the group identity checked by complete enclosing terms.
    pub const fn fulfillment_group(&self) -> FulfillmentGroupId {
        self.fulfillment_group
    }
    /// Explicitly borrows the optional private descriptive lot claim.
    pub fn source_lot_label(&self) -> Option<&str> {
        self.source_lot_label.as_ref().map(|v| v.as_str())
    }
    /// Returns the complete agreed price, without changing its public provenance.
    pub const fn price(&self) -> AgreementPrice {
        self.price
    }
    /// Returns the independently checked fixed total or explicit variable cap.
    pub const fn maximum_line_total(&self) -> Money {
        self.maximum_line_total
    }
}

/// One stable private fulfillment identity and its complete validated data.
#[derive(Clone, Eq, PartialEq)]
pub struct FulfillmentGroup {
    id: FulfillmentGroupId,
    terms: PrivateFulfillment,
}
impl FulfillmentGroup {
    /// Captures validated values; enclosing terms check uniqueness and links.
    pub const fn new(id: FulfillmentGroupId, terms: PrivateFulfillment) -> Self {
        Self { id, terms }
    }
    /// Returns the exact group identity.
    pub const fn id(&self) -> FulfillmentGroupId {
        self.id
    }
    /// Explicitly borrows private fulfillment data.
    pub const fn terms(&self) -> &PrivateFulfillment {
        &self.terms
    }
}

/// Explicit non-stock-backed expectation and risk claim, without scheduling.
#[derive(Clone, Eq, PartialEq)]
pub struct FuturePromise {
    expected_window: Window,
    risk_disclosure: Zeroizing<String>,
}
impl FuturePromise {
    /// Requires an explicit bounded nonempty risk disclosure before copying.
    pub fn new(expected_window: Window, risk_disclosure: &str) -> Result<Self, TermsError> {
        text(risk_disclosure, 512, true)?;
        Ok(Self {
            expected_window,
            risk_disclosure: Zeroizing::new(risk_disclosure.into()),
        })
    }
    /// Returns the exact expected window without current-time judgment.
    pub const fn expected_window(&self) -> Window {
        self.expected_window
    }
    /// Explicitly borrows the exact private risk text.
    pub fn risk_disclosure(&self) -> &str {
        &self.risk_disclosure
    }
}

/// Declared commitment mode, without a physical hold or installed promise.
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum SupplyCommitment {
    /// Requires the executor's separately verified stock-backed installation.
    StockBacked,
    /// Explicit expectation and risk, never physical stock backing.
    FuturePromise(FuturePromise),
}

/// Complete validated bilateral data, not participant consent, authenticated
/// source evidence, an installed agreement or a physical stock reservation.
/// Private wire/hash encoding remains owned by the admitting protocol.
#[derive(Clone, Eq, PartialEq)]
pub struct Terms {
    buyer: PublicKey,
    seller: PublicKey,
    lines: Box<[AgreementLine]>,
    fulfillment: Box<[FulfillmentGroup]>,
    fulfillment_charge: Money,
    maximum_total: Money,
    supply: SupplyCommitment,
}
impl Terms {
    /// Greatest supported number of private fulfillment groups.
    pub const MAX_FULFILLMENT_GROUPS: usize = 8;
    /// Bounds collections before capture; checks distinct IDs, complete group
    /// links, source seller identities and recomputed same-currency line totals.
    /// No live lookup, signing, authorization or stock effect occurs.
    pub fn new(
        buyer: PublicKey,
        seller: PublicKey,
        lines: Vec<AgreementLine>,
        fulfillment: Vec<FulfillmentGroup>,
        fulfillment_charge: Money,
        maximum_total: Money,
        supply: SupplyCommitment,
    ) -> Result<Self, TermsError> {
        if buyer == seller {
            return Err(TermsError::PartiesCoincide);
        }
        if lines.is_empty() || lines.len() > MAX_PRICED_LINES {
            return Err(TermsError::InvalidLineCount);
        }
        if fulfillment.is_empty() || fulfillment.len() > Self::MAX_FULFILLMENT_GROUPS {
            return Err(TermsError::InvalidFulfillmentCount);
        }
        for (index, group) in fulfillment.iter().enumerate() {
            if fulfillment[..index]
                .iter()
                .any(|prior| prior.id == group.id)
            {
                return Err(TermsError::DuplicateFulfillment);
            }
        }
        for (index, line) in lines.iter().enumerate() {
            if lines[..index].iter().any(|prior| prior.id == line.id) {
                return Err(TermsError::DuplicateLine);
            }
            if line.source.coordinate().author() != seller {
                return Err(TermsError::SellerMismatch);
            }
            if !fulfillment
                .iter()
                .any(|group| group.id == line.fulfillment_group)
            {
                return Err(TermsError::UnknownFulfillment);
            }
        }
        let totals: Vec<_> = lines
            .iter()
            .map(AgreementLine::maximum_line_total)
            .collect();
        let recomputed =
            checked_order_total(&totals, fulfillment_charge).map_err(TermsError::Pricing)?;
        if recomputed != maximum_total {
            return Err(TermsError::TotalMismatch);
        }
        Ok(Self {
            buyer,
            seller,
            lines: lines.into_boxed_slice(),
            fulfillment: fulfillment.into_boxed_slice(),
            fulfillment_charge,
            maximum_total,
            supply,
        })
    }
    /// Returns the named buyer without authenticating its consent or signer.
    pub const fn buyer(&self) -> PublicKey {
        self.buyer
    }
    /// Returns the named seller, matching all source coordinate authors.
    pub const fn seller(&self) -> PublicKey {
        self.seller
    }
    /// Borrows the complete immutable ordered line set.
    pub fn lines(&self) -> &[AgreementLine] {
        &self.lines
    }
    /// Explicitly borrows the bounded private fulfillment groups.
    pub fn fulfillment(&self) -> &[FulfillmentGroup] {
        &self.fulfillment
    }
    /// Returns the explicit same-currency fulfillment fee, including valid zero.
    pub const fn fulfillment_charge(&self) -> Money {
        self.fulfillment_charge
    }
    /// Returns the independently recomputed aggregate maximum.
    pub const fn maximum_total(&self) -> Money {
        self.maximum_total
    }
    /// Borrows the explicit commitment declaration; no hold is minted.
    pub const fn supply(&self) -> &SupplyCommitment {
        &self.supply
    }
    /// Returns the fixed supported residual-cancellation contract identity.
    pub const fn cancellation(&self) -> &'static str {
        "mutual_residual_v1"
    }
    /// Returns the fixed supported net-line rounding contract identity.
    pub const fn rounding(&self) -> &'static str {
        PRICING_PROFILE
    }
}

fn text(value: &str, maximum: usize, nonempty: bool) -> Result<(), TermsError> {
    if (nonempty && value.is_empty())
        || value.len() > maximum * 4
        || value.chars().count() > maximum
    {
        return Err(TermsError::InvalidText);
    }
    Ok(())
}

/// Input-free bilateral-value admission failure; no private data is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TermsError {
    /// Buyer and seller name the same participant in bilateral terms.
    PartiesCoincide,
    /// The complete agreement requires one through32 lines.
    InvalidLineCount,
    /// The complete agreement requires one through8 private groups.
    InvalidFulfillmentCount,
    /// A line identity repeats within one complete line set.
    DuplicateLine,
    /// A private fulfillment identity repeats.
    DuplicateFulfillment,
    /// A line references no declared private group.
    UnknownFulfillment,
    /// An explicit option ID differs from the retained original snapshot.
    OptionMismatch,
    /// A listing coordinate author differs from the named seller.
    SellerMismatch,
    /// Required risk text is empty or private text exceeds its bound.
    InvalidText,
    /// A selected threshold exceeds the purchased sale amount.
    InvalidSelectedTier,
    /// A declared line or order total differs from exact recomputation.
    TotalMismatch,
    /// The original option's selected-amount rules were not met.
    Purchase(ListingError),
    /// Checked arithmetic, currency or supported aggregate bounds failed.
    Pricing(PricingError),
}
impl fmt::Display for TermsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PartiesCoincide => "bilateral participants must be distinct",
            Self::InvalidLineCount => "invalid agreement line count",
            Self::InvalidFulfillmentCount => "invalid private fulfillment count",
            Self::DuplicateLine => "duplicate agreement line",
            Self::DuplicateFulfillment => "duplicate private fulfillment group",
            Self::UnknownFulfillment => "unknown private fulfillment group",
            Self::OptionMismatch => "agreement option identity mismatch",
            Self::SellerMismatch => "agreement source seller mismatch",
            Self::InvalidText => "invalid private agreement text",
            Self::InvalidSelectedTier => "invalid selected agreement tier",
            Self::TotalMismatch => "agreement total differs from recomputation",
            Self::Purchase(error) => return error.fmt(f),
            Self::Pricing(error) => return error.fmt(f),
        })
    }
}
impl core::error::Error for TermsError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Purchase(error) => Some(error),
            Self::Pricing(error) => Some(error),
            _ => None,
        }
    }
}

macro_rules! redacted {
    ($($name:ident),+)=> { $(impl fmt::Debug for $name {
        fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result {
            f.write_str(concat!(stringify!($name),"(<redacted>)"))
        }
    })+ };
}
redacted!(
    AgreedFixedPrice,
    AgreementPrice,
    AgreementLine,
    FulfillmentGroup,
    FuturePromise,
    SupplyCommitment,
    Terms
);
