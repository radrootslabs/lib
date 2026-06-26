#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::ids::{RadrootsEventId, RadrootsOrderId};
use radroots_events::order::RadrootsOrderInventoryCommitment;

use crate::order::{
    RadrootsGroupedOrderEventRecords, RadrootsOrderIssue, RadrootsOrderProjection,
    reduce_grouped_order_event_records,
};

#[cfg(feature = "serde_json")]
use crate::validation_receipt::{
    RadrootsTradeValidationReceipt, RadrootsValidationReceiptResult, RadrootsValidationReceiptTags,
    RadrootsValidationReceiptType,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeWorkflowState {
    Missing,
    Requested,
    RevisionProposed,
    AgreedPendingRhi,
    Committed,
    Declined,
    Cancelled,
    Invalid,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsTradeWorkflowRecords {
    pub order_events: RadrootsGroupedOrderEventRecords,
    #[cfg(feature = "serde_json")]
    pub validation_receipts: Vec<RadrootsTradeWorkflowValidationReceiptRecord>,
    pub deterministic_failures: Vec<RadrootsTradeWorkflowDeterministicFailure>,
    pub expected_listing_event_id: Option<RadrootsEventId>,
    pub current_listing_event_id: Option<RadrootsEventId>,
}

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeWorkflowValidationReceiptRecord {
    pub event_id: RadrootsEventId,
    pub order_id: RadrootsOrderId,
    pub receipt: RadrootsTradeValidationReceipt,
    pub tags: RadrootsValidationReceiptTags,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeWorkflowDeterministicFailure {
    pub event_id: RadrootsEventId,
    pub reason: String,
}

pub fn reduce_trade_workflow_records(
    order_id: &RadrootsOrderId,
    records: RadrootsTradeWorkflowRecords,
) -> RadrootsOrderProjection {
    let mut projection = reduce_grouped_order_event_records(order_id, records.order_events);

    if let (Some(expected), Some(current)) = (
        records.expected_listing_event_id.as_ref(),
        records.current_listing_event_id.as_ref(),
    ) && expected != current
    {
        projection.status = RadrootsTradeWorkflowState::Invalid;
        projection.lifecycle_terminal = true;
        projection
            .issues
            .push(RadrootsOrderIssue::StaleListingEvent {
                expected_event_id: expected.clone(),
                current_event_id: current.clone(),
            });
        projection.finish_issue_state();
        return projection;
    }

    if !records.deterministic_failures.is_empty() {
        projection.status = RadrootsTradeWorkflowState::Invalid;
        projection.lifecycle_terminal = true;
        projection.pending_inventory_reservations.clear();
        projection
            .issues
            .extend(records.deterministic_failures.into_iter().map(|failure| {
                RadrootsOrderIssue::DeterministicValidationFailure {
                    event_id: failure.event_id,
                    reason: failure.reason,
                }
            }));
        projection.finish_issue_state();
        return projection;
    }

    #[cfg(feature = "serde_json")]
    {
        apply_validation_receipts(
            order_id,
            &mut projection,
            records.expected_listing_event_id.as_ref(),
            records.validation_receipts,
        );
    }

    projection
}

#[cfg(feature = "serde_json")]
fn apply_validation_receipts(
    order_id: &RadrootsOrderId,
    projection: &mut RadrootsOrderProjection,
    expected_listing_event_id: Option<&RadrootsEventId>,
    validation_receipts: Vec<RadrootsTradeWorkflowValidationReceiptRecord>,
) {
    if validation_receipts.is_empty() {
        return;
    }

    let mut valid_receipts = Vec::new();
    for receipt in validation_receipts {
        if validate_receipt_binding(order_id, projection, expected_listing_event_id, &receipt) {
            valid_receipts.push(receipt);
        }
    }

    if !projection.issues.is_empty() {
        projection.status = RadrootsTradeWorkflowState::Invalid;
        projection.lifecycle_terminal = true;
        projection.pending_inventory_reservations.clear();
        projection.finish_issue_state();
        return;
    }

    if valid_receipts.len() > 1 {
        let mut event_ids = valid_receipts
            .iter()
            .map(|receipt| receipt.event_id.clone())
            .collect::<Vec<_>>();
        event_ids.sort();
        event_ids.dedup();
        projection.status = RadrootsTradeWorkflowState::Invalid;
        projection.lifecycle_terminal = true;
        projection.pending_inventory_reservations.clear();
        projection
            .issues
            .push(RadrootsOrderIssue::ConflictingValidationReceipts { event_ids });
        projection.finish_issue_state();
        return;
    }

    let Some(receipt) = valid_receipts.first() else {
        return;
    };

    match receipt.receipt.result {
        RadrootsValidationReceiptResult::Valid => {
            projection.status = RadrootsTradeWorkflowState::Committed;
            projection.lifecycle_terminal = true;
            projection.validation_receipt_event_id = Some(receipt.event_id.clone());
            projection.committed_inventory_reservations =
                projection.pending_inventory_reservations.clone();
            projection.pending_inventory_reservations.clear();
            projection.last_event_id = Some(receipt.event_id.clone());
        }
        RadrootsValidationReceiptResult::Invalid => {
            projection.status = RadrootsTradeWorkflowState::Invalid;
            projection.lifecycle_terminal = true;
            projection.validation_receipt_event_id = Some(receipt.event_id.clone());
            projection.pending_inventory_reservations.clear();
            projection.last_event_id = Some(receipt.event_id.clone());
        }
    }
}

#[cfg(feature = "serde_json")]
fn validate_receipt_binding(
    order_id: &RadrootsOrderId,
    projection: &mut RadrootsOrderProjection,
    expected_listing_event_id: Option<&RadrootsEventId>,
    receipt: &RadrootsTradeWorkflowValidationReceiptRecord,
) -> bool {
    let mut valid = true;
    if projection.status != RadrootsTradeWorkflowState::AgreedPendingRhi {
        projection.issues.push(
            RadrootsOrderIssue::ValidationReceiptWithoutPendingAgreement {
                event_id: receipt.event_id.clone(),
            },
        );
        valid = false;
    }
    if &receipt.order_id != order_id || receipt.tags.order_id != order_id.as_str() {
        projection
            .issues
            .push(RadrootsOrderIssue::ValidationReceiptOrderIdMismatch {
                event_id: receipt.event_id.clone(),
            });
        valid = false;
    }
    if receipt.receipt.receipt_type != RadrootsValidationReceiptType::TradeTransition
        || receipt.tags.receipt_type != RadrootsValidationReceiptType::TradeTransition
    {
        projection
            .issues
            .push(RadrootsOrderIssue::ValidationReceiptTypeMismatch {
                event_id: receipt.event_id.clone(),
            });
        valid = false;
    }
    if projection
        .request_event_id
        .as_ref()
        .is_none_or(|root| receipt.tags.root_event_id != root.as_str())
    {
        projection
            .issues
            .push(RadrootsOrderIssue::ValidationReceiptRootMismatch {
                event_id: receipt.event_id.clone(),
            });
        valid = false;
    }
    if projection
        .agreement_event_id
        .as_ref()
        .is_none_or(|target| receipt.tags.target_event_id != target.as_str())
    {
        projection
            .issues
            .push(RadrootsOrderIssue::ValidationReceiptTargetMismatch {
                event_id: receipt.event_id.clone(),
            });
        valid = false;
    }
    if let Some(listing_event_id) = expected_listing_event_id
        && receipt.tags.listing_event_id != listing_event_id.as_str()
    {
        projection
            .issues
            .push(RadrootsOrderIssue::ValidationReceiptListingMismatch {
                event_id: receipt.event_id.clone(),
            });
        valid = false;
    }
    valid
}

pub fn inventory_reservations_from_commitments(
    commitments: &[RadrootsOrderInventoryCommitment],
) -> Vec<RadrootsOrderInventoryCommitment> {
    let mut reservations = commitments.to_vec();
    reservations.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    reservations
}

#[cfg(test)]
mod tests {
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
    };
    use radroots_events::{
        ids::{
            RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsOrderId,
            RadrootsOrderQuoteId, RadrootsOrderRevisionId, RadrootsPublicKey,
        },
        kinds::KIND_LISTING,
        order::{
            RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
            RadrootsOrderEconomicItem, RadrootsOrderEconomics, RadrootsOrderInventoryCommitment,
            RadrootsOrderItem, RadrootsOrderPricingBasis, RadrootsOrderRequest,
            RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome,
            RadrootsOrderRevisionProposal,
        },
    };

    use crate::order::{
        RadrootsGroupedOrderEventRecords, RadrootsOrderCancellationRecord,
        RadrootsOrderDecisionRecord, RadrootsOrderIssue, RadrootsOrderRequestRecord,
        RadrootsOrderRevisionDecisionRecord, RadrootsOrderRevisionProposalRecord,
    };
    use crate::validation_receipt::{
        RadrootsTradeValidationReceipt, RadrootsValidationReceiptProof,
        RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
        RadrootsValidationReceiptStatement, RadrootsValidationReceiptType,
        validation_receipt_public_values_hash_hex, validation_receipt_tags,
        validation_receipt_tags_from_tags,
    };

    use super::{
        RadrootsTradeWorkflowDeterministicFailure, RadrootsTradeWorkflowRecords,
        RadrootsTradeWorkflowState, RadrootsTradeWorkflowValidationReceiptRecord,
        reduce_trade_workflow_records,
    };

    const BUYER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn event_id(raw: u8) -> RadrootsEventId {
        RadrootsEventId::parse(format!("{raw:064x}")).expect("event id")
    }

    fn public_key(raw: &str) -> RadrootsPublicKey {
        RadrootsPublicKey::parse(raw).expect("public key")
    }

    fn order_id() -> RadrootsOrderId {
        RadrootsOrderId::parse("order-1").expect("order id")
    }

    fn revision_id() -> RadrootsOrderRevisionId {
        RadrootsOrderRevisionId::parse("revision-1").expect("revision id")
    }

    fn quote_id(raw: &str) -> RadrootsOrderQuoteId {
        RadrootsOrderQuoteId::parse(raw).expect("quote id")
    }

    fn bin_id(raw: &str) -> RadrootsInventoryBinId {
        RadrootsInventoryBinId::parse(raw).expect("bin id")
    }

    fn listing_addr() -> RadrootsListingAddress {
        RadrootsListingAddress::parse(format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"))
            .expect("listing address")
    }

    fn economics(bin_count: u32) -> RadrootsOrderEconomics {
        let currency = RadrootsCoreCurrency::USD;
        RadrootsOrderEconomics {
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency,
            items: vec![RadrootsOrderEconomicItem {
                bin_id: bin_id("bin-1"),
                bin_count,
                quantity_amount: RadrootsCoreDecimal::ONE,
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: RadrootsCoreDecimal::from(1200u32),
                unit_price_currency: currency,
                line_subtotal: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(u64::from(bin_count) * 1200),
                    currency,
                ),
            }],
            discounts: Vec::new(),
            adjustments: Vec::new(),
            subtotal: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(u64::from(bin_count) * 1200),
                currency,
            ),
            discount_total: RadrootsCoreMoney::zero(currency),
            adjustment_total: RadrootsCoreMoney::zero(currency),
            total: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(u64::from(bin_count) * 1200),
                currency,
            ),
        }
    }

    fn request_record() -> RadrootsOrderRequestRecord {
        RadrootsOrderRequestRecord {
            event_id: event_id(1),
            author_pubkey: public_key(BUYER),
            payload: RadrootsOrderRequest {
                order_id: order_id(),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                items: vec![RadrootsOrderItem {
                    bin_id: bin_id("bin-1"),
                    bin_count: 2,
                }],
                economics: economics(2),
            },
        }
    }

    fn accepted_decision() -> RadrootsOrderDecisionRecord {
        RadrootsOrderDecisionRecord {
            event_id: event_id(2),
            author_pubkey: public_key(SELLER),
            counterparty_pubkey: public_key(BUYER),
            root_event_id: event_id(1),
            prev_event_id: event_id(1),
            payload: RadrootsOrderDecision {
                order_id: order_id(),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                decision: RadrootsOrderDecisionOutcome::Accepted {
                    inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                        bin_id: bin_id("bin-1"),
                        bin_count: 2,
                    }],
                },
            },
        }
    }

    fn revision_proposal() -> RadrootsOrderRevisionProposalRecord {
        RadrootsOrderRevisionProposalRecord {
            event_id: event_id(3),
            author_pubkey: public_key(SELLER),
            counterparty_pubkey: public_key(BUYER),
            root_event_id: event_id(1),
            prev_event_id: event_id(1),
            payload: RadrootsOrderRevisionProposal {
                revision_id: revision_id(),
                order_id: order_id(),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                root_event_id: event_id(1),
                prev_event_id: event_id(1),
                items: vec![RadrootsOrderItem {
                    bin_id: bin_id("bin-1"),
                    bin_count: 1,
                }],
                economics: economics(1),
                reason: "one bin remains".to_string(),
            },
        }
    }

    fn accepted_revision_decision() -> RadrootsOrderRevisionDecisionRecord {
        RadrootsOrderRevisionDecisionRecord {
            event_id: event_id(4),
            author_pubkey: public_key(BUYER),
            counterparty_pubkey: public_key(SELLER),
            root_event_id: event_id(1),
            prev_event_id: event_id(3),
            payload: RadrootsOrderRevisionDecision {
                revision_id: revision_id(),
                order_id: order_id(),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                root_event_id: event_id(1),
                prev_event_id: event_id(3),
                decision: RadrootsOrderRevisionOutcome::Accepted,
            },
        }
    }

    fn cancellation(prev_event_id: RadrootsEventId) -> RadrootsOrderCancellationRecord {
        RadrootsOrderCancellationRecord {
            event_id: event_id(5),
            author_pubkey: public_key(BUYER),
            counterparty_pubkey: public_key(SELLER),
            root_event_id: event_id(1),
            prev_event_id,
            payload: RadrootsOrderCancellation {
                order_id: order_id(),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                reason: "changed plans".to_string(),
            },
        }
    }

    fn workflow_records() -> RadrootsTradeWorkflowRecords {
        RadrootsTradeWorkflowRecords {
            order_events: RadrootsGroupedOrderEventRecords {
                requests: vec![request_record()],
                decisions: Vec::new(),
                revision_proposals: Vec::new(),
                revision_decisions: Vec::new(),
                cancellations: Vec::new(),
            },
            validation_receipts: Vec::new(),
            deterministic_failures: Vec::new(),
            expected_listing_event_id: Some(event_id(80)),
            current_listing_event_id: Some(event_id(80)),
        }
    }

    fn receipt_record(
        event_raw: u8,
        result: RadrootsValidationReceiptResult,
        root_event_id: RadrootsEventId,
        target_event_id: RadrootsEventId,
        listing_event_id: RadrootsEventId,
    ) -> RadrootsTradeWorkflowValidationReceiptRecord {
        let error_bitmap = match result {
            RadrootsValidationReceiptResult::Valid => {
                "0x00000000000000000000000000000000".to_string()
            }
            RadrootsValidationReceiptResult::Invalid => {
                "0x00000000000000000000000000000001".to_string()
            }
        };
        let receipt = RadrootsTradeValidationReceipt {
            changed_records_root: hash32('6'),
            domain: "radroots.receipt".to_string(),
            error_bitmap,
            event_set_root: hash32('c'),
            new_state_root: hash32('4'),
            previous_state_root: hash32('3'),
            proof: RadrootsValidationReceiptProof {
                inline_proof_base64: None,
                mode: None,
                program_hash: None,
                proof_reference: None,
                system: RadrootsValidationReceiptProofSystem::None,
                verifying_key_hash: None,
            },
            public_values_hash: validation_receipt_public_values_hash_hex(
                br#"{"schema_version":1}"#,
            ),
            receipt_type: RadrootsValidationReceiptType::TradeTransition,
            result,
            statement: RadrootsValidationReceiptStatement {
                listing_event_id: listing_event_id.into_string(),
                root_event_id: root_event_id.into_string(),
                target_event_id: target_event_id.into_string(),
                statement_type: RadrootsValidationReceiptType::TradeTransition,
            },
            version: 1,
        };
        let tags = validation_receipt_tags(order_id().as_str(), &receipt).expect("receipt tags");
        RadrootsTradeWorkflowValidationReceiptRecord {
            event_id: event_id(event_raw),
            order_id: order_id(),
            receipt,
            tags: validation_receipt_tags_from_tags(&tags).expect("parsed receipt tags"),
        }
    }

    fn hash32(c: char) -> String {
        format!("0x{}", c.to_string().repeat(64))
    }

    #[test]
    fn workflow_seller_acceptance_waits_for_rhi_receipt() {
        let mut records = workflow_records();
        records.order_events.decisions.push(accepted_decision());

        let projection = reduce_trade_workflow_records(&order_id(), records);

        assert_eq!(
            projection.status,
            RadrootsTradeWorkflowState::AgreedPendingRhi
        );
        assert!(!projection.lifecycle_terminal);
        assert_eq!(projection.agreement_event_id, Some(event_id(2)));
        assert_eq!(projection.pending_inventory_reservations.len(), 1);
        assert!(projection.committed_inventory_reservations.is_empty());
    }

    #[test]
    fn workflow_valid_receipt_commits_pending_agreement() {
        let mut records = workflow_records();
        records.order_events.decisions.push(accepted_decision());
        records.validation_receipts.push(receipt_record(
            9,
            RadrootsValidationReceiptResult::Valid,
            event_id(1),
            event_id(2),
            event_id(80),
        ));

        let projection = reduce_trade_workflow_records(&order_id(), records);

        assert_eq!(projection.status, RadrootsTradeWorkflowState::Committed);
        assert!(projection.lifecycle_terminal);
        assert_eq!(projection.validation_receipt_event_id, Some(event_id(9)));
        assert!(projection.pending_inventory_reservations.is_empty());
        assert_eq!(projection.committed_inventory_reservations.len(), 1);
    }

    #[test]
    fn workflow_invalid_receipt_and_deterministic_failure_invalidate_pending_agreement() {
        let mut invalid_receipt = workflow_records();
        invalid_receipt
            .order_events
            .decisions
            .push(accepted_decision());
        invalid_receipt.validation_receipts.push(receipt_record(
            9,
            RadrootsValidationReceiptResult::Invalid,
            event_id(1),
            event_id(2),
            event_id(80),
        ));

        let projection = reduce_trade_workflow_records(&order_id(), invalid_receipt);
        assert_eq!(projection.status, RadrootsTradeWorkflowState::Invalid);
        assert_eq!(projection.validation_receipt_event_id, Some(event_id(9)));
        assert!(projection.pending_inventory_reservations.is_empty());

        let mut deterministic_failure = workflow_records();
        deterministic_failure
            .order_events
            .decisions
            .push(accepted_decision());
        deterministic_failure.deterministic_failures.push(
            RadrootsTradeWorkflowDeterministicFailure {
                event_id: event_id(10),
                reason: "inventory proof failed".to_string(),
            },
        );

        let projection = reduce_trade_workflow_records(&order_id(), deterministic_failure);
        assert_eq!(projection.status, RadrootsTradeWorkflowState::Invalid);
        assert!(matches!(
            projection.issues.as_slice(),
            [RadrootsOrderIssue::DeterministicValidationFailure { .. }]
        ));
    }

    #[test]
    fn workflow_revision_acceptance_waits_for_rhi_and_cancellation_after_agreement_is_invalid() {
        let mut records = workflow_records();
        records
            .order_events
            .revision_proposals
            .push(revision_proposal());
        records
            .order_events
            .revision_decisions
            .push(accepted_revision_decision());

        let projection = reduce_trade_workflow_records(&order_id(), records);
        assert_eq!(
            projection.status,
            RadrootsTradeWorkflowState::AgreedPendingRhi
        );
        assert_eq!(projection.agreement_event_id, Some(event_id(4)));
        assert_eq!(projection.pending_inventory_reservations[0].bin_count, 1);

        let mut cancelled = workflow_records();
        cancelled.order_events.decisions.push(accepted_decision());
        cancelled
            .order_events
            .cancellations
            .push(cancellation(event_id(2)));
        let projection = reduce_trade_workflow_records(&order_id(), cancelled);
        assert_eq!(projection.status, RadrootsTradeWorkflowState::Invalid);
        assert!(matches!(
            projection.issues.as_slice(),
            [RadrootsOrderIssue::ForkedLifecycle { .. }]
        ));
    }

    #[test]
    fn workflow_rejects_stale_listing_and_bad_receipt_bindings() {
        let mut stale = workflow_records();
        stale.order_events.decisions.push(accepted_decision());
        stale.current_listing_event_id = Some(event_id(81));

        let projection = reduce_trade_workflow_records(&order_id(), stale);
        assert_eq!(projection.status, RadrootsTradeWorkflowState::Invalid);
        assert!(matches!(
            projection.issues.as_slice(),
            [RadrootsOrderIssue::StaleListingEvent { .. }]
        ));

        let mut bad_receipt = workflow_records();
        bad_receipt.order_events.decisions.push(accepted_decision());
        bad_receipt.validation_receipts.push(receipt_record(
            9,
            RadrootsValidationReceiptResult::Valid,
            event_id(1),
            event_id(3),
            event_id(80),
        ));

        let projection = reduce_trade_workflow_records(&order_id(), bad_receipt);
        assert_eq!(projection.status, RadrootsTradeWorkflowState::Invalid);
        assert!(matches!(
            projection.issues.as_slice(),
            [RadrootsOrderIssue::ValidationReceiptTargetMismatch { .. }]
        ));
    }
}
