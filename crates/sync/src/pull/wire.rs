use super::{PullReceipt, PullTermination, summary::PullTargetSummaries};
use crate::{
    ingest::IngestReceipt,
    policy::{Error, SyncId},
};
use radroots_transport::{
    outcome::{FetchTargetOutcome, FetchTargetState},
    source::FetchCursor,
};

impl<'de> serde::Deserialize<'de> for PullReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Preserve the existing receipt's legacy fields and unknown-field policy.
        #[derive(serde::Deserialize)]
        struct Wire {
            sync_id: SyncId,
            deadline_unix_ms: u64,
            pages_fetched: u16,
            events_observed: usize,
            ingest_outcomes: Vec<Result<IngestReceipt, Error>>,
            target_outcomes: Vec<FetchTargetOutcome>,
            #[serde(default)]
            target_summaries: Option<PullTargetSummaries>,
            termination: PullTermination,
            resume_from: Option<FetchCursor>,
        }
        let wire = Wire::deserialize(deserializer)?;
        if let Some(summaries) = &wire.target_summaries {
            let summaries = summaries.as_slice();
            if summaries
                .iter()
                .any(|summary| summary.pages_observed() != wire.pages_fetched)
            {
                return Err(serde::de::Error::custom("pull summary page count differs"));
            }
            let mut targets = std::collections::BTreeSet::new();
            for outcome in &wire.target_outcomes {
                let Some(summary) = summaries
                    .iter()
                    .find(|summary| summary.target() == outcome.target())
                else {
                    return Err(serde::de::Error::custom(
                        "pull summary target inventory differs",
                    ));
                };
                if !targets.insert(outcome.target()) {
                    return Err(serde::de::Error::custom("duplicate pull final target"));
                }
                let consistent = match outcome.state() {
                    FetchTargetState::Complete => {
                        summary.pages_observed()
                            > summary.incomplete_pages() + summary.missing_outcome_pages()
                    }
                    state => summary.last_incomplete() == Some(state),
                };
                if !consistent {
                    return Err(serde::de::Error::custom("pull summary final state differs"));
                }
            }
            for summary in summaries {
                let observed_outcome = summary.pages_observed() > summary.missing_outcome_pages();
                if targets.contains(summary.target()) != observed_outcome {
                    return Err(serde::de::Error::custom(
                        "pull summary outcome evidence differs",
                    ));
                }
            }
        }
        Ok(Self {
            sync_id: wire.sync_id,
            deadline_unix_ms: wire.deadline_unix_ms,
            pages_fetched: wire.pages_fetched,
            events_observed: wire.events_observed,
            ingest_outcomes: wire.ingest_outcomes,
            target_outcomes: wire.target_outcomes,
            target_summaries: wire.target_summaries,
            termination: wire.termination,
            resume_from: wire.resume_from,
        })
    }
}
