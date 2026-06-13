pub use radroots_events::order::*;
pub use radroots_events::trade_validation::*;
pub use radroots_events_codec::error::EventEncodeError;
#[cfg(feature = "serde_json")]
pub use radroots_events_codec::order::{
    RadrootsOrderEnvelopeParseError, RadrootsOrderEventContext, RadrootsOrderListingAddress,
    RadrootsOrderListingAddressError,
};
pub use radroots_trade::listing::validation::RadrootsTradeListing as TradeListingValidateResult;

use crate::{RadrootsNostrEvent, RadrootsNostrEventPtr, WireEventParts};
use radroots_events::ids::RadrootsEventId;

#[derive(Debug, Clone)]
pub struct RadrootsOrderRequestDraft {
    parts: WireEventParts,
}

#[derive(Debug, Clone)]
pub struct RadrootsOrderDecisionDraft {
    parts: WireEventParts,
}

#[derive(Debug, Clone)]
pub struct RadrootsOrderRevisionProposalDraft {
    parts: WireEventParts,
}

#[derive(Debug, Clone)]
pub struct RadrootsOrderRevisionDecisionDraft {
    parts: WireEventParts,
}

#[derive(Debug, Clone)]
pub struct RadrootsOrderFulfillmentUpdateDraft {
    parts: WireEventParts,
}

#[derive(Debug, Clone)]
pub struct RadrootsOrderCancellationDraft {
    parts: WireEventParts,
}

#[derive(Debug, Clone)]
pub struct RadrootsOrderReceiptDraft {
    parts: WireEventParts,
}

impl RadrootsOrderRequestDraft {
    pub fn as_wire_parts(&self) -> &WireEventParts {
        &self.parts
    }

    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

impl RadrootsOrderDecisionDraft {
    pub fn as_wire_parts(&self) -> &WireEventParts {
        &self.parts
    }

    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

impl RadrootsOrderRevisionProposalDraft {
    pub fn as_wire_parts(&self) -> &WireEventParts {
        &self.parts
    }

    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

impl RadrootsOrderRevisionDecisionDraft {
    pub fn as_wire_parts(&self) -> &WireEventParts {
        &self.parts
    }

    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

impl RadrootsOrderFulfillmentUpdateDraft {
    pub fn as_wire_parts(&self) -> &WireEventParts {
        &self.parts
    }

    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

impl RadrootsOrderCancellationDraft {
    pub fn as_wire_parts(&self) -> &WireEventParts {
        &self.parts
    }

    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

impl RadrootsOrderReceiptDraft {
    pub fn as_wire_parts(&self) -> &WireEventParts {
        &self.parts
    }

    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

#[cfg(feature = "serde_json")]
pub fn build_order_request_draft(
    listing_event: &RadrootsNostrEventPtr,
    payload: &RadrootsOrderRequest,
) -> Result<RadrootsOrderRequestDraft, EventEncodeError> {
    Ok(RadrootsOrderRequestDraft {
        parts: radroots_events_codec::order::order_request_event_build(listing_event, payload)?,
    })
}

#[cfg(feature = "serde_json")]
pub fn build_order_decision_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderDecision,
) -> Result<RadrootsOrderDecisionDraft, EventEncodeError> {
    Ok(RadrootsOrderDecisionDraft {
        parts: radroots_events_codec::order::order_decision_event_build(
            root_event_id,
            prev_event_id,
            payload,
        )?,
    })
}

#[cfg(feature = "serde_json")]
pub fn build_order_revision_proposal_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderRevisionProposal,
) -> Result<RadrootsOrderRevisionProposalDraft, EventEncodeError> {
    Ok(RadrootsOrderRevisionProposalDraft {
        parts: radroots_events_codec::order::order_revision_proposal_event_build(
            root_event_id,
            prev_event_id,
            payload,
        )?,
    })
}

#[cfg(feature = "serde_json")]
pub fn build_order_revision_decision_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderRevisionDecision,
) -> Result<RadrootsOrderRevisionDecisionDraft, EventEncodeError> {
    Ok(RadrootsOrderRevisionDecisionDraft {
        parts: radroots_events_codec::order::order_revision_decision_event_build(
            root_event_id,
            prev_event_id,
            payload,
        )?,
    })
}

#[cfg(feature = "serde_json")]
pub fn build_fulfillment_update_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderFulfillmentUpdate,
) -> Result<RadrootsOrderFulfillmentUpdateDraft, EventEncodeError> {
    Ok(RadrootsOrderFulfillmentUpdateDraft {
        parts: radroots_events_codec::order::order_fulfillment_update_event_build(
            root_event_id,
            prev_event_id,
            payload,
        )?,
    })
}

#[cfg(feature = "serde_json")]
pub fn build_order_cancellation_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderCancellation,
) -> Result<RadrootsOrderCancellationDraft, EventEncodeError> {
    Ok(RadrootsOrderCancellationDraft {
        parts: radroots_events_codec::order::order_cancellation_event_build(
            root_event_id,
            prev_event_id,
            payload,
        )?,
    })
}

#[cfg(feature = "serde_json")]
pub fn build_buyer_receipt_draft(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderReceipt,
) -> Result<RadrootsOrderReceiptDraft, EventEncodeError> {
    Ok(RadrootsOrderReceiptDraft {
        parts: radroots_events_codec::order::order_receipt_event_build(
            root_event_id,
            prev_event_id,
            payload,
        )?,
    })
}

#[cfg(feature = "serde_json")]
pub fn parse_order_request(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderRequest>, RadrootsOrderEnvelopeParseError> {
    radroots_events_codec::order::order_request_from_event(event)
}

#[cfg(feature = "serde_json")]
pub fn parse_order_decision(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderDecision>, RadrootsOrderEnvelopeParseError> {
    radroots_events_codec::order::order_decision_from_event(event)
}

#[cfg(feature = "serde_json")]
pub fn parse_order_revision_proposal(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderRevisionProposal>, RadrootsOrderEnvelopeParseError> {
    radroots_events_codec::order::order_revision_proposal_from_event(event)
}

#[cfg(feature = "serde_json")]
pub fn parse_order_revision_decision(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderRevisionDecision>, RadrootsOrderEnvelopeParseError> {
    radroots_events_codec::order::order_revision_decision_from_event(event)
}

#[cfg(feature = "serde_json")]
pub fn parse_fulfillment_update(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderFulfillmentUpdate>, RadrootsOrderEnvelopeParseError>
{
    radroots_events_codec::order::order_fulfillment_update_from_event(event)
}

#[cfg(feature = "serde_json")]
pub fn parse_order_cancellation(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderCancellation>, RadrootsOrderEnvelopeParseError> {
    radroots_events_codec::order::order_cancellation_from_event(event)
}

#[cfg(feature = "serde_json")]
pub fn parse_buyer_receipt(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderReceipt>, RadrootsOrderEnvelopeParseError> {
    radroots_events_codec::order::order_receipt_from_event(event)
}

#[cfg(feature = "serde_json")]
pub fn parse_listing_address(
    listing_addr: &str,
) -> Result<RadrootsOrderListingAddress, RadrootsOrderListingAddressError> {
    RadrootsOrderListingAddress::parse(listing_addr)
}

#[cfg(feature = "serde_json")]
pub fn validate_listing_event(
    event: &RadrootsNostrEvent,
) -> Result<TradeListingValidateResult, RadrootsTradeValidationListingError> {
    radroots_trade::listing::validation::validate_listing_event(event)
}
