#![forbid(unsafe_code)]

use crate::kinds::{
    KIND_LISTING, KIND_LISTING_DRAFT, KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION,
    KIND_ORDER_FULFILLMENT_UPDATE, KIND_ORDER_PAYMENT_RECORD, KIND_ORDER_RECEIPT,
    KIND_ORDER_REQUEST, KIND_ORDER_REVISION_DECISION, KIND_ORDER_REVISION_PROPOSAL,
    KIND_ORDER_SETTLEMENT_DECISION, KIND_TRADE_LISTING_VALIDATION_REQUEST,
    KIND_TRADE_LISTING_VALIDATION_RESULT, KIND_TRADE_TRANSITION_PROOF_REQUEST,
    KIND_TRADE_TRANSITION_PROOF_RESULT, KIND_TRADE_VALIDATION_RECEIPT,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventClass {
    Regular,
    Replaceable,
    Addressable,
    Ephemeral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventPrivacy {
    Public,
    Encrypted,
    LocalOnly,
    Secret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventStability {
    Stable,
    Experimental,
    Deprecated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsActorRole {
    Any,
    Farmer,
    Seller,
    Buyer,
    Service,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsReducer {
    ListingProjection,
    MarketProjection,
    OrderProjection,
    ListingInventoryAccounting,
    TradeValidation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTagCardinality {
    RequiredOne,
    OptionalOne,
    OptionalMany,
    RequiredMany,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTagSemantic {
    Identifier,
    Counterparty,
    ListingAddress,
    RootEvent,
    PreviousEvent,
    ListingSnapshot,
    Title,
    Summary,
    PublishedAt,
    Location,
    Price,
    Status,
    Category,
    Image,
    ServiceInput,
    ServiceOutput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsTagContract {
    pub name: &'static str,
    pub cardinality: RadrootsTagCardinality,
    pub semantic: RadrootsTagSemantic,
    pub relay_indexed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsEventContract {
    pub id: &'static str,
    pub kind: u32,
    pub name: &'static str,
    pub payload_type: &'static str,
    pub class: RadrootsEventClass,
    pub stability: RadrootsEventStability,
    pub privacy: RadrootsEventPrivacy,
    pub author_role: RadrootsActorRole,
    pub tags: &'static [RadrootsTagContract],
    pub reducers: &'static [RadrootsReducer],
}

const LISTING_REDUCERS: &[RadrootsReducer] = &[
    RadrootsReducer::ListingProjection,
    RadrootsReducer::MarketProjection,
    RadrootsReducer::ListingInventoryAccounting,
];
const ORDER_REDUCERS: &[RadrootsReducer] = &[
    RadrootsReducer::OrderProjection,
    RadrootsReducer::ListingInventoryAccounting,
];
const TRADE_VALIDATION_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::TradeValidation];

const LISTING_TAGS: &[RadrootsTagContract] = &[
    RadrootsTagContract {
        name: "d",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::Identifier,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "title",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::Title,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "summary",
        cardinality: RadrootsTagCardinality::OptionalOne,
        semantic: RadrootsTagSemantic::Summary,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "published_at",
        cardinality: RadrootsTagCardinality::OptionalOne,
        semantic: RadrootsTagSemantic::PublishedAt,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "location",
        cardinality: RadrootsTagCardinality::OptionalMany,
        semantic: RadrootsTagSemantic::Location,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "price",
        cardinality: RadrootsTagCardinality::OptionalMany,
        semantic: RadrootsTagSemantic::Price,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "status",
        cardinality: RadrootsTagCardinality::OptionalOne,
        semantic: RadrootsTagSemantic::Status,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "category",
        cardinality: RadrootsTagCardinality::OptionalMany,
        semantic: RadrootsTagSemantic::Category,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "image",
        cardinality: RadrootsTagCardinality::OptionalMany,
        semantic: RadrootsTagSemantic::Image,
        relay_indexed: true,
    },
];

const ORDER_REQUEST_TAGS: &[RadrootsTagContract] = &[
    RadrootsTagContract {
        name: "d",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::Identifier,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "p",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::Counterparty,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "a",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::ListingAddress,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "listing_event",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::ListingSnapshot,
        relay_indexed: false,
    },
];

const CHAINED_ORDER_TAGS: &[RadrootsTagContract] = &[
    RadrootsTagContract {
        name: "d",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::Identifier,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "p",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::Counterparty,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "a",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::ListingAddress,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "e",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::RootEvent,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "e",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::PreviousEvent,
        relay_indexed: true,
    },
];

const TRADE_VALIDATION_REQUEST_TAGS: &[RadrootsTagContract] = &[
    RadrootsTagContract {
        name: "i",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::ServiceInput,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "a",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::ListingAddress,
        relay_indexed: true,
    },
];

const TRADE_VALIDATION_RESULT_TAGS: &[RadrootsTagContract] = &[
    RadrootsTagContract {
        name: "request",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::ServiceInput,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "output",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::ServiceOutput,
        relay_indexed: false,
    },
];

const TRADE_VALIDATION_RECEIPT_TAGS: &[RadrootsTagContract] = &[
    RadrootsTagContract {
        name: "e",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::RootEvent,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "a",
        cardinality: RadrootsTagCardinality::OptionalOne,
        semantic: RadrootsTagSemantic::ListingAddress,
        relay_indexed: true,
    },
    RadrootsTagContract {
        name: "output",
        cardinality: RadrootsTagCardinality::RequiredOne,
        semantic: RadrootsTagSemantic::ServiceOutput,
        relay_indexed: false,
    },
];

const fn contract(
    id: &'static str,
    kind: u32,
    name: &'static str,
    payload_type: &'static str,
    class: RadrootsEventClass,
    author_role: RadrootsActorRole,
    tags: &'static [RadrootsTagContract],
    reducers: &'static [RadrootsReducer],
) -> RadrootsEventContract {
    RadrootsEventContract {
        id,
        kind,
        name,
        payload_type,
        class,
        stability: RadrootsEventStability::Stable,
        privacy: RadrootsEventPrivacy::Public,
        author_role,
        tags,
        reducers,
    }
}

const LISTING_CONTRACT: RadrootsEventContract = contract(
    "listing",
    KIND_LISTING,
    "Listing",
    "RadrootsListing",
    RadrootsEventClass::Addressable,
    RadrootsActorRole::Seller,
    LISTING_TAGS,
    LISTING_REDUCERS,
);
const LISTING_DRAFT_CONTRACT: RadrootsEventContract = contract(
    "listing_draft",
    KIND_LISTING_DRAFT,
    "Listing Draft",
    "RadrootsListing",
    RadrootsEventClass::Addressable,
    RadrootsActorRole::Seller,
    LISTING_TAGS,
    LISTING_REDUCERS,
);
const ORDER_REQUEST_CONTRACT: RadrootsEventContract = contract(
    "order_request",
    KIND_ORDER_REQUEST,
    "Order Request",
    "RadrootsOrderRequest",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Buyer,
    ORDER_REQUEST_TAGS,
    ORDER_REDUCERS,
);
const ORDER_DECISION_CONTRACT: RadrootsEventContract = contract(
    "order_decision",
    KIND_ORDER_DECISION,
    "Order Decision",
    "RadrootsOrderDecision",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Seller,
    CHAINED_ORDER_TAGS,
    ORDER_REDUCERS,
);
const ORDER_REVISION_PROPOSAL_CONTRACT: RadrootsEventContract = contract(
    "order_revision_proposal",
    KIND_ORDER_REVISION_PROPOSAL,
    "Order Revision Proposal",
    "RadrootsOrderRevisionProposal",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Buyer,
    CHAINED_ORDER_TAGS,
    ORDER_REDUCERS,
);
const ORDER_REVISION_DECISION_CONTRACT: RadrootsEventContract = contract(
    "order_revision_decision",
    KIND_ORDER_REVISION_DECISION,
    "Order Revision Decision",
    "RadrootsOrderRevisionDecision",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Seller,
    CHAINED_ORDER_TAGS,
    ORDER_REDUCERS,
);
const ORDER_CANCELLATION_CONTRACT: RadrootsEventContract = contract(
    "order_cancellation",
    KIND_ORDER_CANCELLATION,
    "Order Cancellation",
    "RadrootsOrderCancellation",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Buyer,
    CHAINED_ORDER_TAGS,
    ORDER_REDUCERS,
);
const ORDER_FULFILLMENT_UPDATE_CONTRACT: RadrootsEventContract = contract(
    "order_fulfillment_update",
    KIND_ORDER_FULFILLMENT_UPDATE,
    "Order Fulfillment Update",
    "RadrootsOrderFulfillmentUpdate",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Seller,
    CHAINED_ORDER_TAGS,
    ORDER_REDUCERS,
);
const ORDER_RECEIPT_CONTRACT: RadrootsEventContract = contract(
    "order_receipt",
    KIND_ORDER_RECEIPT,
    "Order Receipt",
    "RadrootsOrderReceipt",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Buyer,
    CHAINED_ORDER_TAGS,
    ORDER_REDUCERS,
);
const ORDER_PAYMENT_RECORD_CONTRACT: RadrootsEventContract = contract(
    "order_payment_record",
    KIND_ORDER_PAYMENT_RECORD,
    "Order Payment Record",
    "RadrootsOrderPaymentRecord",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Buyer,
    CHAINED_ORDER_TAGS,
    ORDER_REDUCERS,
);
const ORDER_SETTLEMENT_DECISION_CONTRACT: RadrootsEventContract = contract(
    "order_settlement_decision",
    KIND_ORDER_SETTLEMENT_DECISION,
    "Order Settlement Decision",
    "RadrootsOrderSettlementDecision",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Seller,
    CHAINED_ORDER_TAGS,
    ORDER_REDUCERS,
);
const TRADE_LISTING_VALIDATION_REQUEST_CONTRACT: RadrootsEventContract = contract(
    "trade_listing_validation_request",
    KIND_TRADE_LISTING_VALIDATION_REQUEST,
    "Trade Listing Validation Request",
    "RadrootsTradeValidationListingRequest",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Service,
    TRADE_VALIDATION_REQUEST_TAGS,
    TRADE_VALIDATION_REDUCERS,
);
const TRADE_LISTING_VALIDATION_RESULT_CONTRACT: RadrootsEventContract = contract(
    "trade_listing_validation_result",
    KIND_TRADE_LISTING_VALIDATION_RESULT,
    "Trade Listing Validation Result",
    "RadrootsTradeValidationListingResult",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Service,
    TRADE_VALIDATION_RESULT_TAGS,
    TRADE_VALIDATION_REDUCERS,
);
const TRADE_TRANSITION_PROOF_REQUEST_CONTRACT: RadrootsEventContract = contract(
    "trade_transition_proof_request",
    KIND_TRADE_TRANSITION_PROOF_REQUEST,
    "Trade Transition Proof Request",
    "RadrootsTradeTransitionProofRequest",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Service,
    TRADE_VALIDATION_REQUEST_TAGS,
    TRADE_VALIDATION_REDUCERS,
);
const TRADE_TRANSITION_PROOF_RESULT_CONTRACT: RadrootsEventContract = contract(
    "trade_transition_proof_result",
    KIND_TRADE_TRANSITION_PROOF_RESULT,
    "Trade Transition Proof Result",
    "RadrootsTradeTransitionProofResult",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Service,
    TRADE_VALIDATION_RESULT_TAGS,
    TRADE_VALIDATION_REDUCERS,
);
const TRADE_VALIDATION_RECEIPT_CONTRACT: RadrootsEventContract = contract(
    "trade_validation_receipt",
    KIND_TRADE_VALIDATION_RECEIPT,
    "Trade Validation Receipt",
    "RadrootsTradeValidationReceipt",
    RadrootsEventClass::Regular,
    RadrootsActorRole::Service,
    TRADE_VALIDATION_RECEIPT_TAGS,
    TRADE_VALIDATION_REDUCERS,
);

static LISTING_EVENT_CONTRACTS: [RadrootsEventContract; 2] =
    [LISTING_CONTRACT, LISTING_DRAFT_CONTRACT];
static ORDER_EVENT_CONTRACTS: [RadrootsEventContract; 9] = [
    ORDER_REQUEST_CONTRACT,
    ORDER_DECISION_CONTRACT,
    ORDER_REVISION_PROPOSAL_CONTRACT,
    ORDER_REVISION_DECISION_CONTRACT,
    ORDER_CANCELLATION_CONTRACT,
    ORDER_FULFILLMENT_UPDATE_CONTRACT,
    ORDER_RECEIPT_CONTRACT,
    ORDER_PAYMENT_RECORD_CONTRACT,
    ORDER_SETTLEMENT_DECISION_CONTRACT,
];
static TRADE_VALIDATION_CONTRACTS: [RadrootsEventContract; 5] = [
    TRADE_LISTING_VALIDATION_REQUEST_CONTRACT,
    TRADE_LISTING_VALIDATION_RESULT_CONTRACT,
    TRADE_TRANSITION_PROOF_REQUEST_CONTRACT,
    TRADE_TRANSITION_PROOF_RESULT_CONTRACT,
    TRADE_VALIDATION_RECEIPT_CONTRACT,
];
static ALL_EVENT_CONTRACTS: [RadrootsEventContract; 16] = [
    LISTING_CONTRACT,
    LISTING_DRAFT_CONTRACT,
    ORDER_REQUEST_CONTRACT,
    ORDER_DECISION_CONTRACT,
    ORDER_REVISION_PROPOSAL_CONTRACT,
    ORDER_REVISION_DECISION_CONTRACT,
    ORDER_CANCELLATION_CONTRACT,
    ORDER_FULFILLMENT_UPDATE_CONTRACT,
    ORDER_RECEIPT_CONTRACT,
    ORDER_PAYMENT_RECORD_CONTRACT,
    ORDER_SETTLEMENT_DECISION_CONTRACT,
    TRADE_LISTING_VALIDATION_REQUEST_CONTRACT,
    TRADE_LISTING_VALIDATION_RESULT_CONTRACT,
    TRADE_TRANSITION_PROOF_REQUEST_CONTRACT,
    TRADE_TRANSITION_PROOF_RESULT_CONTRACT,
    TRADE_VALIDATION_RECEIPT_CONTRACT,
];

pub fn contract_for_kind(kind: u32) -> Option<&'static RadrootsEventContract> {
    ALL_EVENT_CONTRACTS
        .iter()
        .find(|contract| contract.kind == kind)
}

pub fn all_contracts() -> &'static [RadrootsEventContract] {
    &ALL_EVENT_CONTRACTS
}

pub fn order_event_contracts() -> &'static [RadrootsEventContract] {
    &ORDER_EVENT_CONTRACTS
}

pub fn listing_event_contracts() -> &'static [RadrootsEventContract] {
    &LISTING_EVENT_CONTRACTS
}

pub fn trade_validation_contracts() -> &'static [RadrootsEventContract] {
    &TRADE_VALIDATION_CONTRACTS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{COMMERCIAL_EVENT_KINDS, ORDER_EVENT_KINDS, TRADE_VALIDATION_EVENT_KINDS};

    #[test]
    fn exposes_scoped_contract_sets() {
        assert_eq!(all_contracts().len(), COMMERCIAL_EVENT_KINDS.len());
        assert_eq!(listing_event_contracts().len(), 2);
        assert_eq!(order_event_contracts().len(), ORDER_EVENT_KINDS.len());
        assert_eq!(
            trade_validation_contracts().len(),
            TRADE_VALIDATION_EVENT_KINDS.len()
        );
    }

    #[test]
    fn every_commercial_kind_has_one_contract() {
        for kind in COMMERCIAL_EVENT_KINDS {
            let matches = all_contracts()
                .iter()
                .filter(|contract| contract.kind == kind)
                .count();
            assert_eq!(matches, 1, "kind {kind}");
            assert_eq!(
                contract_for_kind(kind).map(|contract| contract.kind),
                Some(kind)
            );
        }
    }

    #[test]
    fn order_request_contract_requires_listing_snapshot_without_relay_indexing() {
        let contract = contract_for_kind(KIND_ORDER_REQUEST).expect("order request contract");
        assert_eq!(contract.class, RadrootsEventClass::Regular);
        assert_eq!(contract.author_role, RadrootsActorRole::Buyer);
        assert!(contract.tags.iter().any(|tag| {
            tag.name == "p"
                && tag.cardinality == RadrootsTagCardinality::RequiredOne
                && tag.semantic == RadrootsTagSemantic::Counterparty
        }));
        assert!(contract.tags.iter().any(|tag| {
            tag.name == "a"
                && tag.cardinality == RadrootsTagCardinality::RequiredOne
                && tag.semantic == RadrootsTagSemantic::ListingAddress
        }));
        assert!(contract.tags.iter().any(|tag| {
            tag.name == "d"
                && tag.cardinality == RadrootsTagCardinality::RequiredOne
                && tag.semantic == RadrootsTagSemantic::Identifier
        }));
        assert!(contract.tags.iter().any(|tag| {
            tag.name == "listing_event"
                && tag.cardinality == RadrootsTagCardinality::RequiredOne
                && tag.semantic == RadrootsTagSemantic::ListingSnapshot
                && !tag.relay_indexed
        }));
    }

    #[test]
    fn chained_order_contract_requires_root_and_previous_event_tags() {
        let contract = contract_for_kind(KIND_ORDER_DECISION).expect("order decision contract");
        assert_eq!(contract.class, RadrootsEventClass::Regular);
        assert!(contract.tags.iter().any(|tag| {
            tag.name == "e"
                && tag.cardinality == RadrootsTagCardinality::RequiredOne
                && tag.semantic == RadrootsTagSemantic::RootEvent
        }));
        assert!(contract.tags.iter().any(|tag| {
            tag.name == "e"
                && tag.cardinality == RadrootsTagCardinality::RequiredOne
                && tag.semantic == RadrootsTagSemantic::PreviousEvent
        }));
    }

    #[test]
    fn validation_receipt_is_trade_validation_contract() {
        let contract =
            contract_for_kind(KIND_TRADE_VALIDATION_RECEIPT).expect("validation receipt contract");
        assert_eq!(contract.id, "trade_validation_receipt");
        assert_eq!(contract.author_role, RadrootsActorRole::Service);
        assert!(
            contract
                .reducers
                .contains(&RadrootsReducer::TradeValidation)
        );
        assert!(
            trade_validation_contracts()
                .iter()
                .any(|contract| contract.kind == KIND_TRADE_VALIDATION_RECEIPT)
        );
    }
}
