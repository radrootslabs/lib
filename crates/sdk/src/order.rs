pub use radroots_events::order::*;
#[cfg(any(feature = "signer-adapters", test))]
pub use radroots_events_codec::error::EventEncodeError;
pub use radroots_events_codec::order::RadrootsOrderEnvelopeParseError;

use radroots_events::RadrootsEventEnvelope;
#[cfg(any(feature = "signer-adapters", test))]
use radroots_events::{RadrootsEventPtr, ids::RadrootsEventId, tags::TAG_E};
#[cfg(any(feature = "signer-adapters", test))]
use radroots_events_codec::wire::WireEventParts;

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderRequestDraft {
    parts: WireEventParts,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderDecisionDraft {
    parts: WireEventParts,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderRevisionProposalDraft {
    parts: WireEventParts,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderRevisionDecisionDraft {
    parts: WireEventParts,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Debug, Clone)]
pub struct RadrootsOrderCancellationDraft {
    parts: WireEventParts,
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderRequestDraft {
    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderDecisionDraft {
    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderRevisionProposalDraft {
    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderRevisionDecisionDraft {
    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl RadrootsOrderCancellationDraft {
    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

#[cfg(any(feature = "signer-adapters", test))]
fn with_contract_root_event_tag(
    mut parts: WireEventParts,
    root_event_id: &RadrootsEventId,
) -> WireEventParts {
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
        parts: radroots_events_codec::order::order_request_event_build(listing_event, payload)?,
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
            radroots_events_codec::order::order_decision_event_build(
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
            radroots_events_codec::order::order_revision_proposal_event_build(
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
            radroots_events_codec::order::order_revision_decision_event_build(
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
            radroots_events_codec::order::order_cancellation_event_build(
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
    radroots_events_codec::order::order_request_from_event(event)
}
