use super::CommerceError;
use crate::{AgreementLineId, Decimal, Money, Quantity, RecordId};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt;
use zeroize::Zeroizing;

/// An exact nonnegative line assertion, not a physical movement or authorization.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct LineQuantity {
    line_id: AgreementLineId,
    amount: Decimal,
}
impl LineQuantity {
    /// Captures exact data. Operation policy must separately admit a positive
    /// movement where required; a zero observation is not a stock release.
    pub const fn new(line_id: AgreementLineId, amount: Decimal) -> Self {
        Self { line_id, amount }
    }
    /// Returns the exact line identity claim.
    pub const fn line_id(self) -> AgreementLineId {
        self.line_id
    }
    /// Returns the exact asserted amount, without conversion or stock effect.
    pub const fn amount(self) -> Decimal {
        self.amount
    }
}

/// A measurement/price assertion pending agreement-relative verification.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FinalMeasurement {
    line_id: AgreementLineId,
    quantity: Quantity,
    line_total: Money,
}
impl FinalMeasurement {
    /// Captures validated representations. The admitting executor must check
    /// unit, range, price and cumulative quantities against the exact agreement.
    pub const fn new(line_id: AgreementLineId, quantity: Quantity, line_total: Money) -> Self {
        Self {
            line_id,
            quantity,
            line_total,
        }
    }
    /// Returns the line identity claim.
    pub const fn line_id(self) -> AgreementLineId {
        self.line_id
    }
    /// Returns the exact measured quantity claim.
    pub const fn quantity(self) -> Quantity {
        self.quantity
    }
    /// Returns the claimed price; it is never a trusted total override.
    pub const fn line_total(self) -> Money {
        self.line_total
    }
}

/// Closed issue vocabulary, without a unilateral change to agreement terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum IssueCode {
    /// A participant reports a shortage.
    Shortage,
    /// A participant reports a quality concern.
    Quality,
    /// A participant reports a substitution concern.
    Substitution,
    /// A participant reports a collection concern.
    Collection,
    /// A participant reports an excess.
    Excess,
    /// Another explicitly described issue.
    Other,
}
impl IssueCode {
    /// Returns the exact supported profile token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shortage => "shortage",
            Self::Quality => "quality",
            Self::Substitution => "substitution",
            Self::Collection => "collection",
            Self::Excess => "excess",
            Self::Other => "other",
        }
    }
}

/// Complete bounded issue data; it establishes neither physical truth nor consent.
#[derive(Clone, Eq, PartialEq)]
pub struct IssueReport {
    code: IssueCode,
    description: Zeroizing<String>,
    lines: Box<[LineQuantity]>,
    source_records: Box<[RecordId]>,
}
impl IssueReport {
    /// Validates bounds and duplicate identities before owned text capture.
    pub fn new(
        code: IssueCode,
        description: &str,
        lines: Vec<LineQuantity>,
        source_records: Vec<RecordId>,
    ) -> Result<Self, CommerceError> {
        check_text(description, 2048)?;
        check_lines(&lines, 0)?;
        check_records(&source_records, 0)?;
        Ok(Self {
            code,
            description: Zeroizing::new(description.into()),
            lines: lines.into_boxed_slice(),
            source_records: source_records.into_boxed_slice(),
        })
    }
    /// Returns the explicit issue vocabulary value.
    pub const fn code(&self) -> IssueCode {
        self.code
    }
    /// Explicitly borrows private issue text.
    pub fn description(&self) -> &str {
        &self.description
    }
    /// Borrows the complete ordered line assertions.
    pub fn lines(&self) -> &[LineQuantity] {
        &self.lines
    }
    /// Borrows exact structured-record references; they remain unauthenticated.
    pub fn source_records(&self) -> &[RecordId] {
        &self.source_records
    }
}

/// Complete proposed resolution, without agreement installation or release.
#[derive(Clone, Eq, PartialEq)]
pub struct Resolution {
    issues: Box<[RecordId]>,
    accepted_receipts: Box<[RecordId]>,
    release_residual: Box<[LineQuantity]>,
    summary: Zeroizing<String>,
}
impl Resolution {
    /// Bounds the complete proposal and rejects duplicate references/line IDs.
    pub fn new(
        issues: Vec<RecordId>,
        accepted_receipts: Vec<RecordId>,
        release_residual: Vec<LineQuantity>,
        summary: &str,
    ) -> Result<Self, CommerceError> {
        check_records(&issues, 1)?;
        check_records(&accepted_receipts, 0)?;
        check_lines(&release_residual, 0)?;
        check_text(summary, 2048)?;
        Ok(Self {
            issues: issues.into_boxed_slice(),
            accepted_receipts: accepted_receipts.into_boxed_slice(),
            release_residual: release_residual.into_boxed_slice(),
            summary: Zeroizing::new(summary.into()),
        })
    }
    /// Borrows the nonempty exact issue-reference list.
    pub fn issues(&self) -> &[RecordId] {
        &self.issues
    }
    /// Borrows explicitly accepted receipt references, without inferred truth.
    pub fn accepted_receipts(&self) -> &[RecordId] {
        &self.accepted_receipts
    }
    /// Borrows proposed residual quantities; nothing is released by construction.
    pub fn release_residual(&self) -> &[LineQuantity] {
        &self.release_residual
    }
    /// Explicitly borrows private resolution text.
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

pub(super) fn check_text(value: &str, maximum: usize) -> Result<(), CommerceError> {
    if value.len() > maximum * 4 || value.chars().count() > maximum {
        return Err(CommerceError::InvalidText);
    }
    Ok(())
}
pub(super) fn check_lines(lines: &[LineQuantity], minimum: usize) -> Result<(), CommerceError> {
    if lines.len() < minimum || lines.len() > 32 {
        return Err(CommerceError::InvalidLines);
    }
    for (index, line) in lines.iter().enumerate() {
        if lines[..index]
            .iter()
            .any(|previous| previous.line_id == line.line_id)
        {
            return Err(CommerceError::InvalidLines);
        }
    }
    Ok(())
}
pub(super) fn check_records(records: &[RecordId], minimum: usize) -> Result<(), CommerceError> {
    if records.len() < minimum || records.len() > 32 {
        return Err(CommerceError::InvalidReferences);
    }
    for (index, record) in records.iter().enumerate() {
        if records[..index].contains(record) {
            return Err(CommerceError::InvalidReferences);
        }
    }
    Ok(())
}

macro_rules! redacted {
    ($($name:ident),+)=> { $(impl fmt::Debug for $name {
        fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result { f.write_str(concat!(stringify!($name),"(<redacted>)")) }
    })+ };
}
redacted!(LineQuantity, FinalMeasurement, IssueReport, Resolution);
