use super::{ids, optional};
use crate::terms::{self, TermsWireError, writer::Writer};
use radroots_domain::{AuthorityRef, CommercePayload, LineQuantity, Resolution};

fn authority(value: AuthorityRef, out: &mut Writer) -> Result<(), TermsWireError> {
    out.raw("{\"executor_id\":")?;
    out.exact_string(value.executor_id())?;
    out.raw(",\"generation\":")?;
    out.exact_string(value.generation())?;
    out.raw("}")
}
fn lines(values: &[LineQuantity], out: &mut Writer) -> Result<(), TermsWireError> {
    out.raw("[")?;
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.raw(",")?;
        }
        out.raw("{\"amount\":")?;
        out.exact_string(value.amount())?;
        out.raw(",\"line_id\":")?;
        out.exact_string(value.line_id())?;
        out.raw("}")?;
    }
    out.raw("]")
}
fn resolution(value: &Resolution, out: &mut Writer) -> Result<(), TermsWireError> {
    out.raw("{\"accepted_receipts\":")?;
    ids(value.accepted_receipts(), out)?;
    out.raw(",\"issues\":")?;
    ids(value.issues(), out)?;
    out.raw(",\"release_residual\":")?;
    lines(value.release_residual(), out)?;
    out.raw(",\"summary\":")?;
    out.string(value.summary())?;
    out.raw("}")
}
pub(super) fn encode(value: &CommercePayload, out: &mut Writer) -> Result<(), TermsWireError> {
    out.raw("{")?;
    match value {
        CommercePayload::Quote(value) => {
            out.raw("\"expires_at\":")?;
            out.exact_string(value.expires_at())?;
            out.raw(",\"in_reply_to\":")?;
            optional(value.in_reply_to(), out)?;
            out.raw(",\"terms\":")?;
            terms::encode(value.terms(), out)?;
        }
        CommercePayload::Request(value) => {
            out.raw("\"expires_at\":")?;
            out.exact_string(value.expires_at())?;
            out.raw(",\"quote\":")?;
            optional(value.quote(), out)?;
            out.raw(",\"terms\":")?;
            terms::encode(value.terms(), out)?;
        }
        CommercePayload::Decline(value) => {
            out.raw("\"reason\":")?;
            out.string(value.reason())?;
            out.raw(",\"request\":")?;
            out.exact_string(value.target())?;
        }
        CommercePayload::TermsProposed(value) => {
            out.raw("\"base_agreement\":")?;
            optional(value.base_agreement(), out)?;
            out.raw(",\"expires_at\":")?;
            out.exact_string(value.expires_at())?;
            out.raw(",\"reason\":")?;
            out.string(value.reason())?;
            out.raw(",\"terms\":")?;
            terms::encode(value.terms(), out)?;
        }
        CommercePayload::TermsAuthorized(value) => {
            out.raw("\"expires_at\":")?;
            out.exact_string(value.expires_at())?;
            out.raw(",\"proposal\":")?;
            out.exact_string(value.proposal())?;
            out.raw(",\"terms_digest\":")?;
            out.exact_string(value.terms_digest())?;
        }
        CommercePayload::TermsRejected(value) => {
            out.raw("\"proposal\":")?;
            out.exact_string(value.target())?;
            out.raw(",\"reason\":")?;
            out.string(value.reason())?;
        }
        CommercePayload::AgreementCommitted(value) => {
            out.raw("\"authority\":")?;
            authority(value.authority(), out)?;
            out.raw(",\"authorization\":")?;
            out.exact_string(value.authorization())?;
            out.raw(",\"base_agreement\":")?;
            optional(value.base_agreement(), out)?;
            out.raw(",\"seller_sequence\":")?;
            out.exact_string(value.seller_sequence())?;
            out.raw(",\"terms_digest\":")?;
            out.exact_string(value.terms_digest())?;
        }
        CommercePayload::WithdrawalRequested(value) => {
            out.raw("\"authorization\":")?;
            out.exact_string(value.target())?;
            out.raw(",\"reason\":")?;
            out.string(value.reason())?;
        }
        CommercePayload::WithdrawalAcknowledged(value) => {
            out.raw("\"seller_sequence\":")?;
            out.exact_string(value.seller_sequence())?;
            out.raw(",\"withdrawal\":")?;
            out.exact_string(value.withdrawal())?;
        }
        CommercePayload::CancellationProposed(value) => {
            out.raw("\"agreement\":")?;
            out.exact_string(value.agreement())?;
            out.raw(",\"expires_at\":")?;
            out.exact_string(value.expires_at())?;
            out.raw(",\"reason\":")?;
            out.string(value.reason())?;
            out.raw(",\"residual\":")?;
            lines(value.residual(), out)?;
        }
        CommercePayload::CancellationAuthorized(value)
        | CommercePayload::ResolutionAuthorized(value) => {
            out.raw("\"expires_at\":")?;
            out.exact_string(value.expires_at())?;
            out.raw(",\"proposal\":")?;
            out.exact_string(value.proposal())?;
        }
        CommercePayload::ResidualReleased(value) => {
            out.raw("\"agreement\":")?;
            out.exact_string(value.agreement())?;
            out.raw(",\"authority\":")?;
            authority(value.authority(), out)?;
            out.raw(",\"authorization\":")?;
            out.exact_string(value.authorization())?;
            out.raw(",\"proposal\":")?;
            out.exact_string(value.proposal())?;
            out.raw(",\"residual\":")?;
            lines(value.residual(), out)?;
            out.raw(",\"seller_sequence\":")?;
            out.exact_string(value.seller_sequence())?;
        }
        CommercePayload::Ready(value) => {
            out.raw("\"agreement\":")?;
            out.exact_string(value.agreement())?;
            out.raw(",\"fulfillment_group\":")?;
            out.exact_string(value.fulfillment_group())?;
            out.raw(",\"lines\":")?;
            lines(value.lines(), out)?;
            out.raw(",\"window\":")?;
            terms::window(value.window(), out)?;
        }
        CommercePayload::Handover(value) => {
            out.raw("\"agreement\":")?;
            out.exact_string(value.agreement())?;
            out.raw(",\"authority\":")?;
            authority(value.authority(), out)?;
            out.raw(",\"fulfillment_group\":")?;
            out.exact_string(value.fulfillment_group())?;
            out.raw(",\"lines\":")?;
            lines(value.lines(), out)?;
            out.raw(",\"measurements\":[")?;
            for (index, measurement) in value.measurements().iter().enumerate() {
                if index != 0 {
                    out.raw(",")?;
                }
                out.raw("{\"line_id\":")?;
                out.exact_string(measurement.line_id())?;
                out.raw(",\"line_total\":")?;
                terms::money(measurement.line_total(), out)?;
                out.raw(",\"quantity\":{\"amount\":")?;
                out.exact_string(measurement.quantity().amount())?;
                out.raw(",\"unit\":")?;
                out.string(measurement.quantity().unit().symbol())?;
                out.raw("}}")?;
            }
            out.raw("],\"occurred_at\":")?;
            out.exact_string(value.occurred_at())?;
            out.raw(",\"seller_sequence\":")?;
            out.exact_string(value.seller_sequence())?;
        }
        CommercePayload::Receipt(value) => {
            out.raw("\"handover\":")?;
            out.exact_string(value.handover())?;
            out.raw(",\"lines\":")?;
            lines(value.lines(), out)?;
            out.raw(",\"note\":")?;
            out.string(value.note())?;
            out.raw(",\"occurred_at\":")?;
            out.exact_string(value.occurred_at())?;
            out.raw(",\"supersedes\":")?;
            optional(value.supersedes(), out)?;
        }
        CommercePayload::Issue(value) => {
            out.raw("\"code\":")?;
            out.string(value.code().as_str())?;
            out.raw(",\"description\":")?;
            out.string(value.description())?;
            out.raw(",\"lines\":")?;
            lines(value.lines(), out)?;
            out.raw(",\"source_records\":")?;
            ids(value.source_records(), out)?;
        }
        CommercePayload::ResolutionProposed(value) => {
            out.raw("\"agreement\":")?;
            out.exact_string(value.agreement())?;
            out.raw(",\"expires_at\":")?;
            out.exact_string(value.expires_at())?;
            out.raw(",\"resolution\":")?;
            resolution(value.resolution(), out)?;
        }
        CommercePayload::ResolutionCommitted(value) => {
            out.raw("\"authority\":")?;
            authority(value.authority(), out)?;
            out.raw(",\"authorization\":")?;
            out.exact_string(value.authorization())?;
            out.raw(",\"proposal\":")?;
            out.exact_string(value.proposal())?;
            out.raw(",\"seller_sequence\":")?;
            out.exact_string(value.seller_sequence())?;
        }
        _ => return Err(TermsWireError::UnsupportedProfile),
    }
    out.raw("}")
}
