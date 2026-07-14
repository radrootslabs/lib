pub use radroots_event::order::*;
#[cfg(any(feature = "signer-adapters", test))]
pub use radroots_event_codec::error::EventEncodeError;
pub use radroots_event_codec::order::RadrootsOrderEnvelopeParseError;

use radroots_event::RadrootsEventEnvelope;
#[cfg(any(feature = "signer-adapters", test))]
use radroots_event::wire::RadrootsNip01EventWireParts;
#[cfg(any(feature = "signer-adapters", test))]
use radroots_event::{RadrootsEventPtr, ids::RadrootsEventId, tags::TAG_E};

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderRequestDraft {
    parts: RadrootsNip01EventWireParts,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderDecisionDraft {
    parts: RadrootsNip01EventWireParts,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderRevisionProposalDraft {
    parts: RadrootsNip01EventWireParts,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderRevisionDecisionDraft {
    parts: RadrootsNip01EventWireParts,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderCancellationDraft {
    parts: RadrootsNip01EventWireParts,
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderRequestDraft {
    pub fn into_wire_parts(self) -> RadrootsNip01EventWireParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderDecisionDraft {
    pub fn into_wire_parts(self) -> RadrootsNip01EventWireParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderRevisionProposalDraft {
    pub fn into_wire_parts(self) -> RadrootsNip01EventWireParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderRevisionDecisionDraft {
    pub fn into_wire_parts(self) -> RadrootsNip01EventWireParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderCancellationDraft {
    pub fn into_wire_parts(self) -> RadrootsNip01EventWireParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
fn with_contract_root_event_tag(
    mut parts: RadrootsNip01EventWireParts,
    root_event_id: &RadrootsEventId,
) -> RadrootsNip01EventWireParts {
    parts
        .tags
        .push(vec![TAG_E.to_owned(), root_event_id.as_str().to_owned()]);
    parts
}

#[cfg(any(feature = "signer-adapters", test))]
pub fn build_order_request_draft(
    listing_event: &RadrootsEventPtr,
    payload: &RadrootsOrderRequest,
) -> Result<RadrootsOrderRequestDraft, EventEncodeError> {
    Ok(RadrootsOrderRequestDraft {
        parts: radroots_event_codec::order::order_request_event_build(listing_event, payload)?,
    })
}

#[cfg(any(feature = "signer-adapters", test))]
pub fn build_order_decision_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderDecision,
) -> Result<RadrootsOrderDecisionDraft, EventEncodeError> {
    Ok(RadrootsOrderDecisionDraft {
        parts: with_contract_root_event_tag(
            radroots_event_codec::order::order_decision_event_build(
                root_event_id,
                prev_event_id,
                payload,
            )?,
            root_event_id,
        ),
    })
}

#[cfg(any(feature = "signer-adapters", test))]
pub fn build_order_revision_proposal_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderRevisionProposal,
) -> Result<RadrootsOrderRevisionProposalDraft, EventEncodeError> {
    Ok(RadrootsOrderRevisionProposalDraft {
        parts: with_contract_root_event_tag(
            radroots_event_codec::order::order_revision_proposal_event_build(
                root_event_id,
                prev_event_id,
                payload,
            )?,
            root_event_id,
        ),
    })
}

#[cfg(any(feature = "signer-adapters", test))]
pub fn build_order_revision_decision_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderRevisionDecision,
) -> Result<RadrootsOrderRevisionDecisionDraft, EventEncodeError> {
    Ok(RadrootsOrderRevisionDecisionDraft {
        parts: with_contract_root_event_tag(
            radroots_event_codec::order::order_revision_decision_event_build(
                root_event_id,
                prev_event_id,
                payload,
            )?,
            root_event_id,
        ),
    })
}

#[cfg(any(feature = "signer-adapters", test))]
pub fn build_order_cancellation_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderCancellation,
) -> Result<RadrootsOrderCancellationDraft, EventEncodeError> {
    Ok(RadrootsOrderCancellationDraft {
        parts: with_contract_root_event_tag(
            radroots_event_codec::order::order_cancellation_event_build(
                root_event_id,
                prev_event_id,
                payload,
            )?,
            root_event_id,
        ),
    })
}

pub fn parse_order_request(
    event: &RadrootsEventEnvelope,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderRequest>, RadrootsOrderEnvelopeParseError> {
    radroots_event_codec::order::order_request_from_event(event)
}
