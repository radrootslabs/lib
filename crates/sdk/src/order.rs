pub use radroots_event::order::*;
#[cfg(feature = "signer-adapters")]
pub use radroots_event_codec::error::EventEncodeError;
pub use radroots_event_codec::order::RadrootsOrderEnvelopeParseError;

use radroots_event::RadrootsEventEnvelope;
#[cfg(feature = "signer-adapters")]
use radroots_event::wire::RadrootsNip01EventWireParts;
#[cfg(feature = "signer-adapters")]
use radroots_event::{RadrootsEventPtr, ids::RadrootsEventId, tags::TAG_E};

#[cfg(feature = "signer-adapters")]
#[derive(Debug, Clone)]
pub struct RadrootsOrderRequestDraft {
    parts: RadrootsNip01EventWireParts,
}

#[cfg(feature = "signer-adapters")]
#[derive(Debug, Clone)]
pub struct RadrootsOrderDecisionDraft {
    parts: RadrootsNip01EventWireParts,
}

#[cfg(feature = "signer-adapters")]
#[derive(Debug, Clone)]
pub struct RadrootsOrderCancellationDraft {
    parts: RadrootsNip01EventWireParts,
}

#[cfg(feature = "signer-adapters")]
impl RadrootsOrderRequestDraft {
    pub fn into_wire_parts(self) -> RadrootsNip01EventWireParts {
        self.parts
    }
}

#[cfg(feature = "signer-adapters")]
impl RadrootsOrderDecisionDraft {
    pub fn into_wire_parts(self) -> RadrootsNip01EventWireParts {
        self.parts
    }
}

#[cfg(feature = "signer-adapters")]
impl RadrootsOrderCancellationDraft {
    pub fn into_wire_parts(self) -> RadrootsNip01EventWireParts {
        self.parts
    }
}

#[cfg(feature = "signer-adapters")]
fn with_contract_root_event_tag(
    mut parts: RadrootsNip01EventWireParts,
    root_event_id: &RadrootsEventId,
) -> RadrootsNip01EventWireParts {
    parts
        .tags
        .push(vec![TAG_E.to_owned(), root_event_id.as_str().to_owned()]);
    parts
}

#[cfg(feature = "signer-adapters")]
pub fn build_order_request_draft(
    listing_event: &RadrootsEventPtr,
    payload: &RadrootsOrderRequest,
) -> Result<RadrootsOrderRequestDraft, EventEncodeError> {
    Ok(RadrootsOrderRequestDraft {
        parts: radroots_event_codec::order::order_request_event_build(listing_event, payload)?,
    })
}

#[cfg(feature = "signer-adapters")]
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

#[cfg(feature = "signer-adapters")]
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
