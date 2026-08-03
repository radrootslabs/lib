//! Bounded source pagination and ingestion.

use radroots_transport::{
    FetchRequest,
    outcome::FetchTargetOutcome,
    source::{FETCH_PAGE_MAX_EVENTS, FetchBounds, FetchCursor, NextPage},
    target::TargetSet,
};

use crate::{
    Engine,
    ingest::{AdmissionPolicy, IngestReceipt},
    policy::{Error, OperationKind, SyncId},
};

/// Hard maximum number of source pages in one explicit pull call.
pub const PULL_MAX_PAGES: u16 = 1_000;

/// Caller-owned bounds and continuation state for one pull operation.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequest {
    targets: TargetSet,
    page_limit: u16,
    max_pages: u16,
    cursor: Option<FetchCursor>,
}

impl PullRequest {
    pub fn new(targets: TargetSet, page_limit: u16, max_pages: u16) -> Result<Self, Error> {
        if page_limit == 0
            || page_limit > FETCH_PAGE_MAX_EVENTS
            || max_pages == 0
            || max_pages > PULL_MAX_PAGES
        {
            return Err(Error::InvalidPullRequest);
        }
        Ok(Self {
            targets,
            page_limit,
            max_pages,
            cursor: None,
        })
    }

    #[must_use]
    pub fn with_cursor(mut self, cursor: FetchCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    pub const fn targets(&self) -> &TargetSet {
        &self.targets
    }

    pub const fn page_limit(&self) -> u16 {
        self.page_limit
    }

    pub const fn max_pages(&self) -> u16 {
        self.max_pages
    }

    pub const fn cursor(&self) -> Option<&FetchCursor> {
        self.cursor.as_ref()
    }
}

/// Deterministic reason why a bounded pull returned control to its caller.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullTermination {
    Complete,
    PageLimit,
    Deadline,
    Cancelled,
    SourceFailed,
}

/// Normalized receipt retaining every ingest outcome and final target state.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullReceipt {
    sync_id: SyncId,
    deadline_unix_ms: u64,
    pages_fetched: u16,
    events_observed: usize,
    ingest_outcomes: Vec<Result<IngestReceipt, Error>>,
    target_outcomes: Vec<FetchTargetOutcome>,
    termination: PullTermination,
    resume_from: Option<FetchCursor>,
}

impl PullReceipt {
    pub const fn sync_id(&self) -> SyncId {
        self.sync_id
    }

    pub const fn deadline_unix_ms(&self) -> u64 {
        self.deadline_unix_ms
    }

    pub const fn pages_fetched(&self) -> u16 {
        self.pages_fetched
    }

    pub const fn events_observed(&self) -> usize {
        self.events_observed
    }

    pub fn ingest_outcomes(&self) -> &[Result<IngestReceipt, Error>] {
        self.ingest_outcomes.as_slice()
    }

    pub fn target_outcomes(&self) -> &[FetchTargetOutcome] {
        self.target_outcomes.as_slice()
    }

    pub const fn termination(&self) -> PullTermination {
        self.termination
    }

    pub const fn resume_from(&self) -> Option<&FetchCursor> {
        self.resume_from.as_ref()
    }
}

impl Engine {
    /// Fetches and ingests at most the caller-bounded number of source pages.
    pub async fn pull(
        &self,
        request: PullRequest,
        admission: &dyn AdmissionPolicy,
    ) -> Result<PullReceipt, Error> {
        let source = self.source.as_deref().ok_or(Error::MissingSource)?;
        let sync_id = self.ids.next_id(OperationKind::Pull)?;
        let started_at = self.clock.now_unix_ms()?;
        let deadline_unix_ms = self
            .deadlines
            .deadline_unix_ms(OperationKind::Pull, started_at)?;
        let bounds = FetchBounds::new(request.page_limit, deadline_unix_ms)
            .map_err(|_| Error::InvalidPullRequest)?;
        let mut receipt = PullReceipt {
            sync_id,
            deadline_unix_ms,
            pages_fetched: 0,
            events_observed: 0,
            ingest_outcomes: Vec::new(),
            target_outcomes: Vec::new(),
            termination: PullTermination::Complete,
            resume_from: request.cursor.clone(),
        };
        let mut cursor = request.cursor;

        for page_index in 0..request.max_pages {
            if page_index != 0 && self.clock.now_unix_ms()? >= deadline_unix_ms {
                receipt.termination = PullTermination::Deadline;
                receipt.resume_from = cursor;
                return Ok(receipt);
            }
            let mut fetch = FetchRequest::new(
                fetch_request_id(sync_id, page_index),
                request.targets.clone(),
                bounds,
            )
            .map_err(|_| Error::InvalidPullRequest)?;
            if let Some(current) = cursor.clone() {
                fetch = fetch.with_cursor(current);
            }
            let page = match source.fetch(fetch.clone()).await {
                Ok(page) => page,
                Err(_) => {
                    receipt.termination = PullTermination::SourceFailed;
                    receipt.resume_from = cursor;
                    return Ok(receipt);
                }
            };
            page.validate_for_request(&fetch)
                .map_err(|_| Error::InvalidSourcePage)?;
            receipt.pages_fetched += 1;
            receipt.events_observed += page.events().len();
            merge_target_outcomes(&mut receipt.target_outcomes, page.target_outcomes());
            let outcomes = self.ingest_batch(page.events().to_vec(), admission).await;
            receipt
                .ingest_outcomes
                .extend_from_slice(outcomes.outcomes());

            match page.next_page() {
                NextPage::Complete => {
                    receipt.termination = PullTermination::Complete;
                    receipt.resume_from = None;
                    return Ok(receipt);
                }
                NextPage::Cancelled { resume_from } => {
                    receipt.termination = PullTermination::Cancelled;
                    receipt.resume_from = resume_from.clone();
                    return Ok(receipt);
                }
                NextPage::Cursor(next) => {
                    cursor = Some(next.clone());
                    if page_index + 1 == request.max_pages {
                        receipt.termination = PullTermination::PageLimit;
                        receipt.resume_from = cursor;
                        return Ok(receipt);
                    }
                }
            }
        }
        unreachable!("validated pull requests execute at least one page")
    }
}

fn merge_target_outcomes(current: &mut Vec<FetchTargetOutcome>, page: &[FetchTargetOutcome]) {
    for outcome in page {
        if let Some(existing) = current
            .iter_mut()
            .find(|existing| existing.target() == outcome.target())
        {
            *existing = outcome.clone();
        } else {
            current.push(outcome.clone());
        }
    }
}

fn fetch_request_id(sync_id: SyncId, page_index: u16) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(48);
    value.push_str("sync-");
    for byte in sync_id.as_bytes() {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value.push('-');
    value.push_str(page_index.to_string().as_str());
    value
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PullRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            targets: TargetSet,
            page_limit: u16,
            max_pages: u16,
            cursor: Option<FetchCursor>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let mut request = Self::new(wire.targets, wire.page_limit, wire.max_pages)
            .map_err(serde::de::Error::custom)?;
        if let Some(cursor) = wire.cursor {
            request = request.with_cursor(cursor);
        }
        Ok(request)
    }
}
