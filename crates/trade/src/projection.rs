#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use radroots_event_store::{
    RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX, RadrootsEventStore, RadrootsEventStoreError,
    RadrootsProjectionCursor, RadrootsStoredEvent,
};
use radroots_events::{
    RadrootsNostrEvent,
    ids::{RadrootsEventId, RadrootsIdParseError, RadrootsListingAddress, RadrootsOrderId},
    kinds::{KIND_TRADE_VALIDATION_RECEIPT, is_listing_kind, is_order_event_kind},
    listing::{RadrootsListingAvailability, RadrootsListingDeliveryMethod, RadrootsListingStatus},
    order::RadrootsOrderEventType,
    tags::TAG_D,
};
use radroots_events_codec::order::{
    RadrootsOrderEnvelopeParseError, order_event_context_from_tags,
};
use sqlx::Row;
use thiserror::Error;

use crate::{
    listing::validation::{RadrootsTradeListing, validate_listing_event},
    order::{
        RadrootsGroupedOrderEventRecords, RadrootsOrderEventDecodeError, RadrootsOrderEventRecord,
        RadrootsOrderProjectionQueryResult, order_event_record_from_event,
    },
    validation_receipt::{RadrootsValidationReceiptError, validation_receipt_from_event},
    workflow::{
        RadrootsTradeWorkflowRecords, RadrootsTradeWorkflowState,
        RadrootsTradeWorkflowValidationReceiptRecord, reduce_trade_workflow_records,
    },
};

pub const RADROOTS_PRODUCT_PROJECTION_ID: &str = "radroots.product_projection.v1";
pub const RADROOTS_PRODUCT_PROJECTION_VERSION: u32 = 1;
pub const RADROOTS_TRADE_VALIDATION_RECEIPT_CONTRACT_ID: &str =
    "radroots.trade.validation_receipt.v1";

const PRODUCT_PROJECTION_CONTRACT_IDS: [&str; 6] = [
    "radroots.order.request.v1",
    "radroots.order.decision.v1",
    "radroots.order.revision_proposal.v1",
    "radroots.order.revision_decision.v1",
    "radroots.order.cancellation.v1",
    RADROOTS_TRADE_VALIDATION_RECEIPT_CONTRACT_ID,
];

#[derive(Debug, Error)]
pub enum RadrootsTradeProjectionError {
    #[error("{0}")]
    Store(#[from] RadrootsEventStoreError),
    #[error("projection sqlite query failed: {0}")]
    Sqlite(#[from] sqlx::Error),
    #[error("stored event {event_id} contains invalid tags_json: {source}")]
    InvalidStoredTagsJson {
        event_id: String,
        source: serde_json::Error,
    },
    #[error("stored listing event {event_id} failed validation: {source}")]
    ListingValidation {
        event_id: String,
        source: radroots_events::trade_validation::RadrootsTradeValidationListingError,
    },
    #[error("stored order event {event_id} could not decode as an order record: {source}")]
    OrderDecode {
        event_id: String,
        source: RadrootsOrderEventDecodeError,
    },
    #[error("stored order event {event_id} has invalid context tags: {source}")]
    OrderContext {
        event_id: String,
        source: RadrootsOrderEnvelopeParseError,
    },
    #[error("stored validation receipt event {event_id} failed validation: {source}")]
    ValidationReceipt {
        event_id: String,
        source: RadrootsValidationReceiptError,
    },
    #[error("stored validation receipt event {event_id} has invalid order id: {source}")]
    ValidationReceiptOrderId {
        event_id: String,
        source: RadrootsIdParseError,
    },
    #[error("stored validation receipt event {event_id} has invalid event id: {source}")]
    ValidationReceiptEventId {
        event_id: String,
        source: RadrootsIdParseError,
    },
    #[error("stored listing projection has invalid listing_addr {listing_addr}: {source}")]
    ListingAddress {
        listing_addr: String,
        source: RadrootsIdParseError,
    },
    #[error("projection serialization failed for {model}: {source}")]
    Serialize {
        model: &'static str,
        source: serde_json::Error,
    },
    #[error("projection query limit must be between 1 and {max}")]
    InvalidLimit { max: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsProjectionRefreshRequest {
    pub limit: u32,
}

impl Default for RadrootsProjectionRefreshRequest {
    fn default() -> Self {
        Self {
            limit: RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
        }
    }
}

impl RadrootsProjectionRefreshRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    fn validate(self) -> Result<Self, RadrootsTradeProjectionError> {
        if self.limit == 0 || self.limit > RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX {
            return Err(RadrootsTradeProjectionError::InvalidLimit {
                max: RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
            });
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsProjectionRefreshReceipt {
    pub scanned_events: usize,
    pub listing_upserts: usize,
    pub trade_upserts: usize,
    pub validation_receipts: usize,
    pub relay_observations: i64,
    pub last_event_seq: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingProjectionRow {
    pub listing_addr: RadrootsListingAddress,
    pub listing_event_id: String,
    pub seller_pubkey: String,
    pub title: String,
    pub description: String,
    pub product_type: String,
    pub price_amount: String,
    pub price_currency: String,
    pub inventory_available: String,
    pub delivery_method: String,
    pub locality_primary: String,
    pub locality_city: Option<String>,
    pub locality_region: Option<String>,
    pub locality_country: Option<String>,
    pub geohash5: String,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingSearchRequest {
    pub query: String,
    pub limit: u32,
}

impl RadrootsListingSearchRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            limit: 50,
        }
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    fn validate(&self) -> Result<(), RadrootsTradeProjectionError> {
        if self.limit == 0 || self.limit > RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX {
            return Err(RadrootsTradeProjectionError::InvalidLimit {
                max: RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
            });
        }
        Ok(())
    }
}

pub async fn refresh_product_projections(
    store: &RadrootsEventStore,
    request: RadrootsProjectionRefreshRequest,
    updated_at_ms: i64,
) -> Result<RadrootsProjectionRefreshReceipt, RadrootsTradeProjectionError> {
    let request = request.validate()?;
    let events = store
        .events_since_cursor(RADROOTS_PRODUCT_PROJECTION_ID, request.limit)
        .await?;
    let mut receipt = RadrootsProjectionRefreshReceipt {
        scanned_events: events.len(),
        ..RadrootsProjectionRefreshReceipt::default()
    };
    let mut affected_orders = BTreeSet::new();

    for stored_event in &events {
        receipt.last_event_seq = Some(stored_event.seq);
        receipt.relay_observations +=
            relay_observation_count_for_event(store, &stored_event.event_id).await?;
        if is_listing_kind(stored_event.kind) {
            let event = stored_event_to_nostr_event(stored_event)?;
            let listing = validate_listing_event(&event).map_err(|source| {
                RadrootsTradeProjectionError::ListingValidation {
                    event_id: stored_event.event_id.clone(),
                    source,
                }
            })?;
            upsert_listing_projection(store, stored_event, &listing, updated_at_ms).await?;
            receipt.listing_upserts += 1;
        } else if is_order_event_kind(stored_event.kind) {
            let event = stored_event_to_nostr_event(stored_event)?;
            let record = order_event_record_from_event(&event).map_err(|source| {
                RadrootsTradeProjectionError::OrderDecode {
                    event_id: stored_event.event_id.clone(),
                    source,
                }
            })?;
            affected_orders.insert(record.order_id().clone());
        } else if stored_event.kind == KIND_TRADE_VALIDATION_RECEIPT {
            let event = stored_event_to_nostr_event(stored_event)?;
            let verified = validation_receipt_from_event(&event).map_err(|source| {
                RadrootsTradeProjectionError::ValidationReceipt {
                    event_id: stored_event.event_id.clone(),
                    source,
                }
            })?;
            let order_id =
                RadrootsOrderId::parse(verified.tags.order_id.as_str()).map_err(|source| {
                    RadrootsTradeProjectionError::ValidationReceiptOrderId {
                        event_id: stored_event.event_id.clone(),
                        source,
                    }
                })?;
            affected_orders.insert(order_id);
            receipt.validation_receipts += 1;
        }
    }

    for order_id in affected_orders {
        upsert_trade_projection(store, &order_id, request.limit, updated_at_ms).await?;
        receipt.trade_upserts += 1;
    }

    if let Some(last_event_seq) = receipt.last_event_seq {
        store
            .update_projection_cursor(&RadrootsProjectionCursor {
                projection_id: RADROOTS_PRODUCT_PROJECTION_ID.to_owned(),
                projection_version: RADROOTS_PRODUCT_PROJECTION_VERSION,
                last_event_seq,
                updated_at_ms,
            })
            .await?;
    }

    Ok(receipt)
}

pub async fn search_listing_projection(
    store: &RadrootsEventStore,
    request: &RadrootsListingSearchRequest,
) -> Result<Vec<RadrootsListingProjectionRow>, RadrootsTradeProjectionError> {
    request.validate()?;
    let rows = if let Some(query) = listing_fts_query(&request.query) {
        sqlx::query(
            "SELECT p.listing_addr, p.listing_event_id, p.seller_pubkey, p.title, p.description, p.product_type, p.price_amount, p.price_currency, p.inventory_available, p.delivery_method, p.locality_primary, p.locality_city, p.locality_region, p.locality_country, p.geohash5, p.updated_at_ms FROM listing_projection p JOIN listing_search_fts f ON f.listing_addr = p.listing_addr WHERE listing_search_fts MATCH ? ORDER BY bm25(listing_search_fts), p.updated_at_ms DESC, p.listing_addr LIMIT ?",
        )
        .bind(query)
        .bind(i64::from(request.limit))
        .fetch_all(store.pool())
        .await?
    } else {
        sqlx::query(
            "SELECT listing_addr, listing_event_id, seller_pubkey, title, description, product_type, price_amount, price_currency, inventory_available, delivery_method, locality_primary, locality_city, locality_region, locality_country, geohash5, updated_at_ms FROM listing_projection ORDER BY updated_at_ms DESC, listing_addr LIMIT ?",
        )
        .bind(i64::from(request.limit))
        .fetch_all(store.pool())
        .await?
    };
    rows.into_iter().map(listing_projection_row).collect()
}

pub async fn trade_projection_query_for_order_id(
    store: &RadrootsEventStore,
    order_id: &RadrootsOrderId,
    limit: u32,
) -> Result<RadrootsOrderProjectionQueryResult, RadrootsTradeProjectionError> {
    if limit == 0 || limit > RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX {
        return Err(RadrootsTradeProjectionError::InvalidLimit {
            max: RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
        });
    }
    let inputs = trade_projection_inputs_for_order_id(store, order_id, limit).await?;
    let projection = reduce_trade_workflow_records(order_id, inputs.workflow_records);
    Ok(RadrootsOrderProjectionQueryResult {
        projection,
        event_count: inputs.event_ids.len(),
        limit_applied: limit,
        event_ids: inputs.event_ids,
    })
}

async fn upsert_listing_projection(
    store: &RadrootsEventStore,
    stored_event: &RadrootsStoredEvent,
    listing: &RadrootsTradeListing,
    updated_at_ms: i64,
) -> Result<(), RadrootsTradeProjectionError> {
    let listing_json = serde_json::to_string(&listing.listing).map_err(|source| {
        RadrootsTradeProjectionError::Serialize {
            model: "listing",
            source,
        }
    })?;
    let location = &listing.location;
    let locality = listing_locality_search_text(listing);
    sqlx::query(
        "INSERT INTO listing_projection(listing_addr, listing_event_id, seller_pubkey, farm_pubkey, farm_d_tag, listing_d_tag, title, description, product_type, primary_bin_id, quantity_amount, quantity_unit, price_amount, price_currency, inventory_available, availability_status, delivery_method, locality_primary, locality_city, locality_region, locality_country, geohash5, listing_json, source_event_seq, created_at, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(listing_addr) DO UPDATE SET listing_event_id = excluded.listing_event_id, seller_pubkey = excluded.seller_pubkey, farm_pubkey = excluded.farm_pubkey, farm_d_tag = excluded.farm_d_tag, listing_d_tag = excluded.listing_d_tag, title = excluded.title, description = excluded.description, product_type = excluded.product_type, primary_bin_id = excluded.primary_bin_id, quantity_amount = excluded.quantity_amount, quantity_unit = excluded.quantity_unit, price_amount = excluded.price_amount, price_currency = excluded.price_currency, inventory_available = excluded.inventory_available, availability_status = excluded.availability_status, delivery_method = excluded.delivery_method, locality_primary = excluded.locality_primary, locality_city = excluded.locality_city, locality_region = excluded.locality_region, locality_country = excluded.locality_country, geohash5 = excluded.geohash5, listing_json = excluded.listing_json, source_event_seq = excluded.source_event_seq, created_at = excluded.created_at, updated_at_ms = excluded.updated_at_ms",
    )
    .bind(listing.listing_addr.as_str())
    .bind(stored_event.event_id.as_str())
    .bind(listing.seller_pubkey.as_str())
    .bind(listing.listing.farm.pubkey.as_str())
    .bind(listing.listing.farm.d_tag.as_str())
    .bind(listing.listing.d_tag.as_str())
    .bind(listing.title.as_str())
    .bind(listing.description.as_str())
    .bind(listing.product_type.as_str())
    .bind(listing.primary_bin_id.as_str())
    .bind(listing.bin_quantity.amount.to_string())
    .bind(listing.unit.to_string())
    .bind(listing.unit_price.amount.to_string())
    .bind(listing.unit_price.currency.to_string())
    .bind(listing.inventory_available.to_string())
    .bind(listing_availability_label(&listing.availability))
    .bind(listing_delivery_method_label(&listing.delivery_method))
    .bind(location.primary.as_str())
    .bind(location.city.as_deref())
    .bind(location.region.as_deref())
    .bind(location.country.as_deref())
    .bind(location.geohash.as_str())
    .bind(listing_json)
    .bind(stored_event.seq)
    .bind(i64::from(stored_event.created_at))
    .bind(updated_at_ms)
    .execute(store.pool())
    .await?;

    sqlx::query("DELETE FROM listing_search_fts WHERE listing_addr = ?")
        .bind(listing.listing_addr.as_str())
        .execute(store.pool())
        .await?;
    sqlx::query(
        "INSERT INTO listing_search_fts(listing_addr, title, description, product_type, locality, seller_pubkey) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(listing.listing_addr.as_str())
    .bind(listing.title.as_str())
    .bind(listing.description.as_str())
    .bind(listing.product_type.as_str())
    .bind(locality)
    .bind(listing.seller_pubkey.as_str())
    .execute(store.pool())
    .await?;
    Ok(())
}

async fn upsert_trade_projection(
    store: &RadrootsEventStore,
    order_id: &RadrootsOrderId,
    limit: u32,
    updated_at_ms: i64,
) -> Result<(), RadrootsTradeProjectionError> {
    let inputs = trade_projection_inputs_for_order_id(store, order_id, limit).await?;
    let event_ids = inputs.event_ids.clone();
    let source_event_count = event_ids.len();
    let relay_observation_count = relay_observation_count_for_events(store, &event_ids).await?;
    let expected_listing_event_id = inputs.expected_listing_event_id.clone();
    let current_listing_event_id = inputs.current_listing_event_id.clone();
    let projection = reduce_trade_workflow_records(order_id, inputs.workflow_records);
    let economics_json = projection
        .economics
        .as_ref()
        .map(|economics| {
            serde_json::to_string(economics).map_err(|source| {
                RadrootsTradeProjectionError::Serialize {
                    model: "trade_economics",
                    source,
                }
            })
        })
        .transpose()?;
    let pending_inventory_json = serde_json::to_string(&projection.pending_inventory_reservations)
        .map_err(|source| RadrootsTradeProjectionError::Serialize {
            model: "pending_inventory",
            source,
        })?;
    let committed_inventory_json =
        serde_json::to_string(&projection.committed_inventory_reservations).map_err(|source| {
            RadrootsTradeProjectionError::Serialize {
                model: "committed_inventory",
                source,
            }
        })?;
    let issue_labels = projection
        .issues
        .iter()
        .map(|issue| format!("{issue:?}"))
        .collect::<Vec<_>>();
    let issues_json = serde_json::to_string(&issue_labels).map_err(|source| {
        RadrootsTradeProjectionError::Serialize {
            model: "trade_issues",
            source,
        }
    })?;

    sqlx::query(
        "INSERT INTO trade_projection(order_id, status, lifecycle_terminal, rhi_state, listing_addr, buyer_pubkey, seller_pubkey, request_event_id, decision_event_id, agreement_event_id, pending_revision_event_id, cancellation_event_id, validation_receipt_event_id, last_event_id, expected_listing_event_id, current_listing_event_id, economics_json, pending_inventory_json, committed_inventory_json, issues_json, issue_count, source_event_count, relay_observation_count, last_source_event_seq, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(order_id) DO UPDATE SET status = excluded.status, lifecycle_terminal = excluded.lifecycle_terminal, rhi_state = excluded.rhi_state, listing_addr = excluded.listing_addr, buyer_pubkey = excluded.buyer_pubkey, seller_pubkey = excluded.seller_pubkey, request_event_id = excluded.request_event_id, decision_event_id = excluded.decision_event_id, agreement_event_id = excluded.agreement_event_id, pending_revision_event_id = excluded.pending_revision_event_id, cancellation_event_id = excluded.cancellation_event_id, validation_receipt_event_id = excluded.validation_receipt_event_id, last_event_id = excluded.last_event_id, expected_listing_event_id = excluded.expected_listing_event_id, current_listing_event_id = excluded.current_listing_event_id, economics_json = excluded.economics_json, pending_inventory_json = excluded.pending_inventory_json, committed_inventory_json = excluded.committed_inventory_json, issues_json = excluded.issues_json, issue_count = excluded.issue_count, source_event_count = excluded.source_event_count, relay_observation_count = excluded.relay_observation_count, last_source_event_seq = excluded.last_source_event_seq, updated_at_ms = excluded.updated_at_ms",
    )
    .bind(order_id.as_str())
    .bind(trade_workflow_status_label(&projection.status))
    .bind(bool_i64(projection.lifecycle_terminal))
    .bind(trade_rhi_state_label(&projection.status, projection.validation_receipt_event_id.as_ref()))
    .bind(projection.listing_addr.as_ref().map(RadrootsListingAddress::as_str))
    .bind(projection.buyer_pubkey.as_ref().map(|value| value.as_str()))
    .bind(projection.seller_pubkey.as_ref().map(|value| value.as_str()))
    .bind(projection.request_event_id.as_ref().map(|value| value.as_str()))
    .bind(projection.decision_event_id.as_ref().map(|value| value.as_str()))
    .bind(projection.agreement_event_id.as_ref().map(|value| value.as_str()))
    .bind(projection.pending_revision_event_id.as_ref().map(|value| value.as_str()))
    .bind(projection.cancellation_event_id.as_ref().map(|value| value.as_str()))
    .bind(projection.validation_receipt_event_id.as_ref().map(|value| value.as_str()))
    .bind(projection.last_event_id.as_ref().map(|value| value.as_str()))
    .bind(expected_listing_event_id.as_ref().map(|value| value.as_str()))
    .bind(current_listing_event_id.as_ref().map(|value| value.as_str()))
    .bind(economics_json)
    .bind(pending_inventory_json)
    .bind(committed_inventory_json)
    .bind(issues_json)
    .bind(i64::try_from(projection.issues.len()).unwrap_or(i64::MAX))
    .bind(i64::try_from(source_event_count).unwrap_or(i64::MAX))
    .bind(relay_observation_count)
    .bind(inputs.last_source_event_seq)
    .bind(updated_at_ms)
    .execute(store.pool())
    .await?;
    Ok(())
}

struct TradeProjectionInputs {
    workflow_records: RadrootsTradeWorkflowRecords,
    event_ids: Vec<RadrootsEventId>,
    expected_listing_event_id: Option<RadrootsEventId>,
    current_listing_event_id: Option<RadrootsEventId>,
    last_source_event_seq: Option<i64>,
}

async fn trade_projection_inputs_for_order_id(
    store: &RadrootsEventStore,
    order_id: &RadrootsOrderId,
    limit: u32,
) -> Result<TradeProjectionInputs, RadrootsTradeProjectionError> {
    let stored_events = store
        .events_by_contract_and_tag(
            &PRODUCT_PROJECTION_CONTRACT_IDS,
            TAG_D,
            order_id.as_str(),
            limit,
        )
        .await?;
    let mut workflow_records = RadrootsTradeWorkflowRecords::default();
    let mut event_ids = Vec::with_capacity(stored_events.len());
    let mut expected_listing_event_id = None;
    let mut listing_addr = None;
    let mut last_source_event_seq = None;

    for stored_event in stored_events {
        last_source_event_seq = Some(stored_event.seq);
        let event = stored_event_to_nostr_event(&stored_event)?;
        if stored_event.kind == KIND_TRADE_VALIDATION_RECEIPT {
            let verified = validation_receipt_from_event(&event).map_err(|source| {
                RadrootsTradeProjectionError::ValidationReceipt {
                    event_id: stored_event.event_id.clone(),
                    source,
                }
            })?;
            let receipt_order_id = RadrootsOrderId::parse(verified.tags.order_id.as_str())
                .map_err(
                    |source| RadrootsTradeProjectionError::ValidationReceiptOrderId {
                        event_id: stored_event.event_id.clone(),
                        source,
                    },
                )?;
            let receipt_event_id =
                RadrootsEventId::parse(stored_event.event_id.as_str()).map_err(|source| {
                    RadrootsTradeProjectionError::ValidationReceiptEventId {
                        event_id: stored_event.event_id.clone(),
                        source,
                    }
                })?;
            event_ids.push(receipt_event_id.clone());
            workflow_records.validation_receipts.push(
                RadrootsTradeWorkflowValidationReceiptRecord {
                    event_id: receipt_event_id,
                    order_id: receipt_order_id,
                    receipt: verified.receipt,
                    tags: verified.tags,
                },
            );
            continue;
        }

        let record = order_event_record_from_event(&event).map_err(|source| {
            RadrootsTradeProjectionError::OrderDecode {
                event_id: stored_event.event_id.clone(),
                source,
            }
        })?;
        event_ids.push(record.event_id().clone());
        if let RadrootsOrderEventRecord::Request(request) = &record {
            listing_addr.get_or_insert_with(|| request.payload.listing_addr.clone());
            if expected_listing_event_id.is_none() {
                expected_listing_event_id = request_listing_event_id(&event)?;
            }
        }
        push_order_record(&mut workflow_records.order_events, record);
    }

    workflow_records.expected_listing_event_id = expected_listing_event_id.clone();
    let current_listing_event_id = match listing_addr.as_ref() {
        Some(listing_addr) => current_listing_event_id(store, listing_addr.as_str()).await?,
        None => None,
    };
    workflow_records.current_listing_event_id = current_listing_event_id.clone();

    Ok(TradeProjectionInputs {
        workflow_records,
        event_ids,
        expected_listing_event_id,
        current_listing_event_id,
        last_source_event_seq,
    })
}

fn push_order_record(
    records: &mut RadrootsGroupedOrderEventRecords,
    record: RadrootsOrderEventRecord,
) {
    match record {
        RadrootsOrderEventRecord::Request(record) => records.requests.push(record),
        RadrootsOrderEventRecord::Decision(record) => records.decisions.push(record),
        RadrootsOrderEventRecord::RevisionProposal(record) => {
            records.revision_proposals.push(record)
        }
        RadrootsOrderEventRecord::RevisionDecision(record) => {
            records.revision_decisions.push(record)
        }
        RadrootsOrderEventRecord::Cancellation(record) => records.cancellations.push(record),
    }
}

fn request_listing_event_id(
    event: &RadrootsNostrEvent,
) -> Result<Option<RadrootsEventId>, RadrootsTradeProjectionError> {
    let context =
        order_event_context_from_tags(RadrootsOrderEventType::OrderRequested, &event.tags)
            .map_err(|source| RadrootsTradeProjectionError::OrderContext {
                event_id: event.id.clone(),
                source,
            })?;
    context
        .listing_event
        .map(|listing_event| {
            RadrootsEventId::parse(listing_event.id.as_str()).map_err(|source| {
                RadrootsTradeProjectionError::ValidationReceiptEventId {
                    event_id: event.id.clone(),
                    source,
                }
            })
        })
        .transpose()
}

async fn current_listing_event_id(
    store: &RadrootsEventStore,
    listing_addr: &str,
) -> Result<Option<RadrootsEventId>, RadrootsTradeProjectionError> {
    let row = sqlx::query("SELECT listing_event_id FROM listing_projection WHERE listing_addr = ?")
        .bind(listing_addr)
        .fetch_optional(store.pool())
        .await?;
    row.map(|row| {
        let value: String = row.try_get("listing_event_id")?;
        RadrootsEventId::parse(value).map_err(|source| {
            RadrootsTradeProjectionError::ValidationReceiptEventId {
                event_id: listing_addr.to_owned(),
                source,
            }
        })
    })
    .transpose()
}

fn stored_event_to_nostr_event(
    stored_event: &RadrootsStoredEvent,
) -> Result<RadrootsNostrEvent, RadrootsTradeProjectionError> {
    let tags = serde_json::from_str(&stored_event.tags_json).map_err(|source| {
        RadrootsTradeProjectionError::InvalidStoredTagsJson {
            event_id: stored_event.event_id.clone(),
            source,
        }
    })?;
    Ok(RadrootsNostrEvent {
        id: stored_event.event_id.clone(),
        author: stored_event.pubkey.clone(),
        created_at: stored_event.created_at,
        kind: stored_event.kind,
        tags,
        content: stored_event.content.clone(),
        sig: stored_event.sig.clone(),
    })
}

async fn relay_observation_count_for_events(
    store: &RadrootsEventStore,
    event_ids: &[RadrootsEventId],
) -> Result<i64, RadrootsTradeProjectionError> {
    let mut count = 0;
    for event_id in event_ids {
        count += relay_observation_count_for_event(store, event_id.as_str()).await?;
    }
    Ok(count)
}

async fn relay_observation_count_for_event(
    store: &RadrootsEventStore,
    event_id: &str,
) -> Result<i64, RadrootsTradeProjectionError> {
    let row = sqlx::query("SELECT COUNT(*) AS count FROM relay_event_seen WHERE event_id = ?")
        .bind(event_id)
        .fetch_one(store.pool())
        .await?;
    Ok(row.try_get("count")?)
}

fn listing_projection_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsListingProjectionRow, RadrootsTradeProjectionError> {
    let listing_addr = row.try_get::<String, _>("listing_addr")?;
    let listing_addr = RadrootsListingAddress::parse(&listing_addr).map_err(|source| {
        RadrootsTradeProjectionError::ListingAddress {
            listing_addr: listing_addr.clone(),
            source,
        }
    })?;
    Ok(RadrootsListingProjectionRow {
        listing_addr,
        listing_event_id: row.try_get("listing_event_id")?,
        seller_pubkey: row.try_get("seller_pubkey")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        product_type: row.try_get("product_type")?,
        price_amount: row.try_get("price_amount")?,
        price_currency: row.try_get("price_currency")?,
        inventory_available: row.try_get("inventory_available")?,
        delivery_method: row.try_get("delivery_method")?,
        locality_primary: row.try_get("locality_primary")?,
        locality_city: row.try_get("locality_city")?,
        locality_region: row.try_get("locality_region")?,
        locality_country: row.try_get("locality_country")?,
        geohash5: row.try_get("geohash5")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

fn listing_availability_label(availability: &RadrootsListingAvailability) -> String {
    match availability {
        RadrootsListingAvailability::Window { .. } => "window".to_owned(),
        RadrootsListingAvailability::Status { status } => match status {
            RadrootsListingStatus::Active => "active".to_owned(),
            RadrootsListingStatus::Sold => "sold".to_owned(),
            RadrootsListingStatus::Other { value } => value.trim().to_owned(),
        },
    }
}

fn listing_delivery_method_label(delivery_method: &RadrootsListingDeliveryMethod) -> String {
    match delivery_method {
        RadrootsListingDeliveryMethod::Pickup => "pickup".to_owned(),
        RadrootsListingDeliveryMethod::LocalDelivery => "local_delivery".to_owned(),
        RadrootsListingDeliveryMethod::Shipping => "shipping".to_owned(),
        RadrootsListingDeliveryMethod::Other { method } => method.trim().to_owned(),
    }
}

fn listing_locality_search_text(listing: &RadrootsTradeListing) -> String {
    [
        Some(listing.location.primary.as_str()),
        listing.location.city.as_deref(),
        listing.location.region.as_deref(),
        listing.location.country.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.trim().is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

fn listing_fts_query(query: &str) -> Option<String> {
    let terms = query
        .split(|character: char| !character.is_alphanumeric())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

fn trade_workflow_status_label(status: &RadrootsTradeWorkflowState) -> &'static str {
    match status {
        RadrootsTradeWorkflowState::Missing => "missing",
        RadrootsTradeWorkflowState::Requested => "requested",
        RadrootsTradeWorkflowState::RevisionProposed => "revision_proposed",
        RadrootsTradeWorkflowState::AgreedPendingRhi => "agreed_pending_rhi",
        RadrootsTradeWorkflowState::Committed => "committed",
        RadrootsTradeWorkflowState::Declined => "declined",
        RadrootsTradeWorkflowState::Cancelled => "cancelled",
        RadrootsTradeWorkflowState::Invalid => "invalid",
    }
}

fn trade_rhi_state_label(
    status: &RadrootsTradeWorkflowState,
    validation_receipt_event_id: Option<&RadrootsEventId>,
) -> &'static str {
    match status {
        RadrootsTradeWorkflowState::AgreedPendingRhi => "pending",
        RadrootsTradeWorkflowState::Committed => "final",
        RadrootsTradeWorkflowState::Invalid if validation_receipt_event_id.is_some() => "invalid",
        RadrootsTradeWorkflowState::Invalid => "invalid",
        _ => "not_required",
    }
}

fn bool_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_event_store::{RadrootsEventIngest, RadrootsRelayObservation};
    use radroots_events::{
        RadrootsNostrEventPtr,
        farm::RadrootsFarmRef,
        ids::RadrootsOrderQuoteId,
        kinds::KIND_LISTING,
        listing::{
            RadrootsListing, RadrootsListingBin, RadrootsListingProduct,
            RadrootsListingPublicLocation,
        },
        order::{
            RadrootsOrderDecision, RadrootsOrderDecisionOutcome, RadrootsOrderEconomicItem,
            RadrootsOrderEconomics, RadrootsOrderInventoryCommitment, RadrootsOrderItem,
            RadrootsOrderPricingBasis, RadrootsOrderRequest,
        },
    };
    use radroots_events_codec::order::{order_decision_event_build, order_request_event_build};
    use radroots_nostr::prelude::{
        RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp,
        radroots_event_from_nostr, radroots_nostr_build_event,
    };

    use crate::validation_receipt::{
        RadrootsTradeValidationReceipt, RadrootsValidationReceiptProof,
        RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
        RadrootsValidationReceiptStatement, RadrootsValidationReceiptType,
        validation_receipt_event_build, validation_receipt_public_values_hash_hex,
    };

    const SELLER_SECRET: &str = "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const SELLER: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const BUYER_SECRET: &str = "59392e9068f66431b12f70218fb61281cb6b433d7f27c55d61f1a63fe1a96ff8";
    const BUYER: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    fn keys(secret: &str) -> RadrootsNostrKeys {
        RadrootsNostrKeys::new(RadrootsNostrSecretKey::from_hex(secret).expect("secret"))
    }

    fn decimal(raw: &str) -> RadrootsCoreDecimal {
        raw.parse().expect("decimal")
    }

    fn listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".parse().expect("d tag"),
            published_at: Some(1_700_000_000),
            farm: RadrootsFarmRef {
                pubkey: SELLER.to_owned(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_owned(),
            },
            product: RadrootsListingProduct {
                key: "pea-shoots".to_owned(),
                title: "Pea shoots".to_owned(),
                category: "greens".to_owned(),
                summary: Some("Tender early greens".to_owned()),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: "bin-1".parse().expect("bin"),
            bins: vec![RadrootsListingBin {
                bin_id: "bin-1".parse().expect("bin"),
                quantity: RadrootsCoreQuantity::new(decimal("1"), RadrootsCoreUnit::Each),
                price_per_canonical_unit: RadrootsCoreQuantityPrice {
                    amount: RadrootsCoreMoney::new(decimal("5"), RadrootsCoreCurrency::USD),
                    quantity: RadrootsCoreQuantity::new(decimal("1"), RadrootsCoreUnit::Each),
                },
                display_amount: None,
                display_unit: None,
                display_label: None,
                display_price: None,
                display_price_unit: None,
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: Some(decimal("9")),
            availability: Some(RadrootsListingAvailability::Status {
                status: RadrootsListingStatus::Active,
            }),
            delivery_method: Some(RadrootsListingDeliveryMethod::Pickup),
            location: Some(RadrootsListingPublicLocation {
                primary: "Old Town".to_owned(),
                city: Some("Victoria".to_owned()),
                region: Some("BC".to_owned()),
                country: Some("CA".to_owned()),
                geohash: "c2b2q".to_owned(),
            }),
            images: None,
        }
    }

    fn signed_listing_event() -> RadrootsNostrEvent {
        let parts = radroots_events_codec::listing::encode::to_wire_parts(&listing())
            .expect("listing parts");
        sign_parts(
            parts.kind,
            parts.content,
            parts.tags,
            1_700_000_000,
            &keys(SELLER_SECRET),
        )
    }

    fn listing_addr(event: &RadrootsNostrEvent) -> RadrootsListingAddress {
        RadrootsListingAddress::parse(format!(
            "{}:{}:{}",
            KIND_LISTING,
            event.author,
            listing().d_tag
        ))
        .expect("listing address")
    }

    fn order_id() -> RadrootsOrderId {
        RadrootsOrderId::parse("projection-order").expect("order id")
    }

    fn economics() -> RadrootsOrderEconomics {
        let currency = RadrootsCoreCurrency::USD;
        RadrootsOrderEconomics {
            quote_id: RadrootsOrderQuoteId::parse("quote-1").expect("quote"),
            quote_version: 1,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency,
            items: vec![RadrootsOrderEconomicItem {
                bin_id: "bin-1".parse().expect("bin"),
                bin_count: 1,
                quantity_amount: decimal("1"),
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: decimal("5"),
                unit_price_currency: currency,
                line_subtotal: RadrootsCoreMoney::new(decimal("5"), currency),
            }],
            discounts: Vec::new(),
            adjustments: Vec::new(),
            subtotal: RadrootsCoreMoney::new(decimal("5"), currency),
            discount_total: RadrootsCoreMoney::zero(currency),
            adjustment_total: RadrootsCoreMoney::zero(currency),
            total: RadrootsCoreMoney::new(decimal("5"), currency),
        }
    }

    fn order_request(listing_event: &RadrootsNostrEvent) -> RadrootsOrderRequest {
        RadrootsOrderRequest {
            order_id: order_id(),
            listing_addr: listing_addr(listing_event),
            buyer_pubkey: BUYER.parse().expect("buyer"),
            seller_pubkey: SELLER.parse().expect("seller"),
            items: vec![RadrootsOrderItem {
                bin_id: "bin-1".parse().expect("bin"),
                bin_count: 1,
            }],
            economics: economics(),
        }
    }

    fn signed_order_request_event(listing_event: &RadrootsNostrEvent) -> RadrootsNostrEvent {
        let parts = order_request_event_build(
            &RadrootsNostrEventPtr {
                id: listing_event.id.clone(),
                relays: Some("wss://relay.example.test".to_owned()),
            },
            &order_request(listing_event),
        )
        .expect("request parts");
        sign_parts(
            parts.kind,
            parts.content,
            parts.tags,
            1_700_000_010,
            &keys(BUYER_SECRET),
        )
    }

    fn signed_order_decision_event(
        request: &RadrootsNostrEvent,
        listing_event: &RadrootsNostrEvent,
    ) -> RadrootsNostrEvent {
        let decision = RadrootsOrderDecision {
            order_id: order_id(),
            listing_addr: listing_addr(listing_event),
            buyer_pubkey: BUYER.parse().expect("buyer"),
            seller_pubkey: SELLER.parse().expect("seller"),
            decision: RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: "bin-1".parse().expect("bin"),
                    bin_count: 1,
                }],
            },
        };
        let root = RadrootsEventId::parse(request.id.as_str()).expect("root");
        let parts = order_decision_event_build(&root, &root, &decision).expect("decision parts");
        sign_parts(
            parts.kind,
            parts.content,
            parts.tags,
            1_700_000_020,
            &keys(SELLER_SECRET),
        )
    }

    fn signed_receipt_event(
        listing_event: &RadrootsNostrEvent,
        request: &RadrootsNostrEvent,
        decision: &RadrootsNostrEvent,
        result: RadrootsValidationReceiptResult,
    ) -> RadrootsNostrEvent {
        let request_id = RadrootsEventId::parse(request.id.as_str()).expect("request");
        let listing_event_id = RadrootsEventId::parse(listing_event.id.as_str()).expect("listing");
        let decision_id = RadrootsEventId::parse(decision.id.as_str()).expect("decision");
        let receipt = RadrootsTradeValidationReceipt {
            changed_records_root: hash32('a'),
            domain: "radroots.receipt".to_owned(),
            error_bitmap: match result {
                RadrootsValidationReceiptResult::Valid => {
                    "0x00000000000000000000000000000000".to_owned()
                }
                RadrootsValidationReceiptResult::Invalid => {
                    "0x00000000000000000000000000000001".to_owned()
                }
            },
            event_set_root: hash32('b'),
            new_state_root: hash32('c'),
            previous_state_root: hash32('d'),
            proof: RadrootsValidationReceiptProof {
                inline_proof_base64: None,
                mode: None,
                program_hash: None,
                proof_reference: None,
                system: RadrootsValidationReceiptProofSystem::None,
                verifying_key_hash: None,
            },
            public_values_hash: validation_receipt_public_values_hash_hex(
                format!("{}:{}", request_id.as_str(), decision_id.as_str()).as_bytes(),
            ),
            receipt_type: RadrootsValidationReceiptType::TradeTransition,
            result,
            statement: RadrootsValidationReceiptStatement {
                listing_event_id: listing_event_id.into_string(),
                root_event_id: request_id.into_string(),
                target_event_id: decision_id.into_string(),
                statement_type: RadrootsValidationReceiptType::TradeTransition,
            },
            version: 1,
        };
        let parts = validation_receipt_event_build(order_id().as_str(), &receipt).expect("receipt");
        sign_parts(
            parts.kind,
            parts.content,
            parts.tags,
            1_700_000_030,
            &keys(SELLER_SECRET),
        )
    }

    fn sign_parts(
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
        created_at: u32,
        keys: &RadrootsNostrKeys,
    ) -> RadrootsNostrEvent {
        let raw_event = radroots_nostr_build_event(kind, content, tags)
            .expect("builder")
            .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(created_at)))
            .sign_with_keys(keys)
            .expect("signed");
        radroots_event_from_nostr(&raw_event)
    }

    fn hash32(character: char) -> String {
        format!(
            "0x{}",
            core::iter::repeat_n(character, 64).collect::<String>()
        )
    }

    #[tokio::test]
    async fn refresh_materializes_listing_search_and_receipt_aware_trade_projection() {
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let listing_event = signed_listing_event();
        let request_event = signed_order_request_event(&listing_event);
        let decision_event = signed_order_decision_event(&request_event, &listing_event);
        let receipt_event = signed_receipt_event(
            &listing_event,
            &request_event,
            &decision_event,
            RadrootsValidationReceiptResult::Valid,
        );

        store
            .ingest_event(RadrootsEventIngest::new(listing_event.clone(), 10))
            .await
            .expect("listing");
        store
            .ingest_event(
                RadrootsEventIngest::new(request_event.clone(), 20).with_observation(
                    RadrootsRelayObservation::new(
                        "wss://relay.example.test",
                        radroots_event_store::RadrootsRelayObservationType::Import,
                        20,
                    ),
                ),
            )
            .await
            .expect("request");
        store
            .ingest_event(RadrootsEventIngest::new(decision_event.clone(), 30))
            .await
            .expect("decision");
        store
            .ingest_event(RadrootsEventIngest::new(receipt_event.clone(), 40))
            .await
            .expect("receipt");

        let refresh =
            refresh_product_projections(&store, RadrootsProjectionRefreshRequest::new(), 50)
                .await
                .expect("refresh");
        assert_eq!(refresh.scanned_events, 4);
        assert_eq!(refresh.listing_upserts, 1);
        assert_eq!(refresh.trade_upserts, 1);
        assert_eq!(refresh.validation_receipts, 1);
        assert_eq!(refresh.relay_observations, 1);

        let rows = search_listing_projection(
            &store,
            &RadrootsListingSearchRequest::new("pea victoria").with_limit(10),
        )
        .await
        .expect("search");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Pea shoots");
        assert_eq!(rows[0].geohash5, "c2b2q");

        let status = trade_projection_query_for_order_id(&store, &order_id(), 100)
            .await
            .expect("status");
        assert_eq!(
            status.projection.status,
            RadrootsTradeWorkflowState::Committed
        );
        assert_eq!(
            status.projection.validation_receipt_event_id,
            Some(RadrootsEventId::parse(receipt_event.id).expect("receipt"))
        );

        let trade_row = sqlx::query(
            "SELECT status, rhi_state, relay_observation_count FROM trade_projection WHERE order_id = ?",
        )
        .bind(order_id().as_str())
        .fetch_one(store.pool())
        .await
        .expect("trade row");
        assert_eq!(
            trade_row.try_get::<String, _>("status").unwrap(),
            "committed"
        );
        assert_eq!(
            trade_row.try_get::<String, _>("rhi_state").unwrap(),
            "final"
        );
        assert_eq!(
            trade_row
                .try_get::<i64, _>("relay_observation_count")
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn refresh_rejects_out_of_range_limits_without_advancing_cursor() {
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let error = refresh_product_projections(
            &store,
            RadrootsProjectionRefreshRequest::new().with_limit(0),
            1,
        )
        .await
        .expect_err("limit");
        assert!(matches!(
            error,
            RadrootsTradeProjectionError::InvalidLimit { .. }
        ));
        assert!(
            store
                .get_projection_cursor(RADROOTS_PRODUCT_PROJECTION_ID)
                .await
                .expect("cursor")
                .is_none()
        );
    }
}
