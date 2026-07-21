use super::ReconciledEvent;
use crate::model::reconciliation_v1::{RadrootsEventAdmissionStatus, StoredEventClass};
use crate::{RadrootsEventStoreError, RadrootsEventStoreRawSourceRebuildDriftV1};
use radroots_event::envelope::RadrootsEventEnvelope;
use radroots_event::event_head::v1::{
    RadrootsCurrentEventHead, RadrootsEventHeadCandidate, RadrootsEventHeadCandidateResult,
    RadrootsEventHeadCoordinate, RadrootsEventHeadDecision,
    event_head_candidate_for_nip01_event_v1, select_event_head_v1,
};
use radroots_event::ids::RadrootsNip01Coordinate;
use radroots_event_codec::deletion::reconciliation_v1::admission::{
    RadrootsAdmittedNip09DeletionRequestEventV1, admit_verified_nip09_deletion_request_event_v1,
};
#[cfg(test)]
use radroots_event_codec::deletion::reconciliation_v1::evaluator::evaluate_nip09_suppression_from_borrowed_requests_v1;
use radroots_event_codec::deletion::reconciliation_v1::evaluator::{
    RadrootsNip09SuppressionOutcome, RadrootsNip09SuppressionReason,
};
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct VisibilityOracleFactV1 {
    pub(super) event_id: String,
    pub(super) admission_status: String,
    pub(super) contract_id: Option<String>,
    pub(super) event_class: String,
    pub(super) raw_d_tag: Option<String>,
    pub(super) is_raw_head: i64,
    pub(super) raw_head_event_id: Option<String>,
    pub(super) suppression_outcome: Option<String>,
    pub(super) suppression_reason: Option<String>,
    pub(super) event_reference_request_id: Option<String>,
    pub(super) address_reference_request_id: Option<String>,
    pub(super) address_reference_cutoff: Option<i64>,
    pub(super) current_visibility: String,
}

pub(super) async fn audit_current_visibility_from_raw_v1(
    events: &[ReconciledEvent],
    actual: Vec<VisibilityOracleFactV1>,
) -> Result<(), RadrootsEventStoreError> {
    let expected = expected_visibility(events)?;
    if actual != expected {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::DerivedProductStateAuthority,
            "current visibility does not equal the independent immutable-raw oracle",
        );
    }
    Ok(())
}

fn expected_visibility(
    events: &[ReconciledEvent],
) -> Result<Vec<VisibilityOracleFactV1>, RadrootsEventStoreError> {
    let winners = oracle_head_winners(events);
    let requests = oracle_deletion_requests(events)?;
    let request_index = OracleRequestIndexV1::new(&requests);
    let mut expected = Vec::with_capacity(events.len());
    for event in events {
        let envelope = event.verified_event.event();
        let event_class = StoredEventClass::from_event_kind_class(envelope.kind_class());
        let raw_d_tag = oracle_raw_d_tag(envelope, event_class);
        let raw_head_event_id = match event_class {
            StoredEventClass::Regular => None,
            StoredEventClass::Replaceable => winners
                .get(&RadrootsEventHeadCoordinate::Replaceable {
                    kind: envelope.kind_u32(),
                    pubkey: envelope.author().clone(),
                })
                .map(|winner| winner.event_id.to_string()),
            StoredEventClass::Addressable => winners
                .get(&RadrootsEventHeadCoordinate::Addressable {
                    kind: envelope.kind_u32(),
                    pubkey: envelope.author().clone(),
                    d_tag: raw_d_tag.clone().unwrap_or_default(),
                })
                .map(|winner| winner.event_id.to_string()),
            StoredEventClass::Ephemeral => {
                return rebuild_drift(
                    RadrootsEventStoreRawSourceRebuildDriftV1::ImmutableRawAuthority,
                    "the immutable-raw visibility oracle found an ephemeral row",
                );
            }
        };
        let is_raw_head = event_class == StoredEventClass::Regular
            || raw_head_event_id.as_deref() == Some(envelope.id_str());
        let admitted = event.admission.status == RadrootsEventAdmissionStatus::Admitted;
        let (
            suppression_outcome,
            suppression_reason,
            event_reference_request_id,
            address_reference_request_id,
            address_reference_cutoff,
        ) = if admitted {
            let decision = request_index.decision(envelope);
            (
                Some(decision.outcome.code().to_owned()),
                Some(decision.reason.code().to_owned()),
                decision.event_reference_request_id,
                decision.address_reference_request_id,
                decision
                    .address_reference_cutoff
                    .map(i64::try_from)
                    .transpose()
                    .map_err(|_| RadrootsEventStoreError::RawSourceRebuildStateDrift {
                        kind:
                            RadrootsEventStoreRawSourceRebuildDriftV1::DerivedProductStateAuthority,
                        detail: format!(
                            "deletion cutoff for `{}` exceeds SQLite integer range",
                            envelope.id_str()
                        ),
                    })?,
            )
        } else {
            (None, None, None, None, None)
        };
        let current_visibility = if !admitted {
            "not_admitted"
        } else if !is_raw_head {
            "not_current"
        } else if suppression_outcome.as_deref()
            == Some(RadrootsNip09SuppressionOutcome::Suppressed.code())
        {
            "suppressed"
        } else {
            "visible"
        };
        expected.push(VisibilityOracleFactV1 {
            event_id: envelope.id_str().to_owned(),
            admission_status: event.admission.status.as_str().to_owned(),
            contract_id: event
                .admission
                .contract
                .map(|contract| contract.id.to_owned()),
            event_class: event_class.as_str().to_owned(),
            raw_d_tag,
            is_raw_head: i64::from(is_raw_head),
            raw_head_event_id,
            suppression_outcome,
            suppression_reason,
            event_reference_request_id,
            address_reference_request_id,
            address_reference_cutoff,
            current_visibility: current_visibility.to_owned(),
        });
    }
    Ok(expected)
}

fn oracle_head_winners(
    events: &[ReconciledEvent],
) -> BTreeMap<RadrootsEventHeadCoordinate, RadrootsEventHeadCandidate> {
    let mut winners = BTreeMap::new();
    for event in events {
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_nip01_event_v1(event.verified_event.event())
        else {
            continue;
        };
        let current =
            winners
                .get(&candidate.coordinate)
                .map(
                    |winner: &RadrootsEventHeadCandidate| RadrootsCurrentEventHead {
                        coordinate: winner.coordinate.clone(),
                        event_id: winner.event_id.clone(),
                        created_at: winner.created_at,
                    },
                );
        if matches!(
            select_event_head_v1(candidate.clone(), current.as_ref()),
            RadrootsEventHeadDecision::Applied(_)
        ) {
            winners.insert(candidate.coordinate.clone(), candidate);
        }
    }
    winners
}

fn oracle_raw_d_tag(
    event: &RadrootsEventEnvelope,
    event_class: StoredEventClass,
) -> Option<String> {
    match event_class {
        StoredEventClass::Regular | StoredEventClass::Ephemeral => None,
        StoredEventClass::Replaceable => Some(String::new()),
        StoredEventClass::Addressable => Some(
            event
                .tag_slices()
                .iter()
                .find(|tag| tag.as_slice().first().is_some_and(|name| name == "d"))
                .and_then(|tag| tag.as_slice().get(1))
                .cloned()
                .unwrap_or_default(),
        ),
    }
}

fn oracle_deletion_requests(
    events: &[ReconciledEvent],
) -> Result<Vec<RadrootsAdmittedNip09DeletionRequestEventV1>, RadrootsEventStoreError> {
    let mut requests = events
        .iter()
        .filter(|event| {
            event.admission.status == RadrootsEventAdmissionStatus::Admitted
                && event.verified_event.event().kind_u32() == 5
        })
        .map(|event| {
            admit_verified_nip09_deletion_request_event_v1(event.verified_event.clone()).map_err(
                |error| RadrootsEventStoreError::RawSourceRebuildStateDrift {
                    kind: RadrootsEventStoreRawSourceRebuildDriftV1::DerivedProductStateAuthority,
                    detail: format!(
                        "oracle could not type admitted deletion request `{}`: {error}",
                        event.verified_event.event().id_str()
                    ),
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    requests.sort_by(|left, right| left.event().id().cmp(right.event().id()));
    Ok(requests)
}

struct OracleRequestIndexV1<'a> {
    requests: &'a [RadrootsAdmittedNip09DeletionRequestEventV1],
    event_targets: BTreeMap<String, BTreeMap<String, usize>>,
    address_targets: BTreeMap<(u32, String, String), OracleAddressRequestEvidenceV1>,
}

#[derive(Debug, PartialEq, Eq)]
struct OracleSuppressionDecisionV1 {
    outcome: RadrootsNip09SuppressionOutcome,
    reason: RadrootsNip09SuppressionReason,
    event_reference_request_id: Option<String>,
    address_reference_request_id: Option<String>,
    address_reference_cutoff: Option<u64>,
}

#[derive(Default)]
struct OracleAddressRequestEvidenceV1 {
    authorized: Option<usize>,
    unauthorized: Option<usize>,
}

impl<'a> OracleRequestIndexV1<'a> {
    fn new(requests: &'a [RadrootsAdmittedNip09DeletionRequestEventV1]) -> Self {
        let mut event_targets = BTreeMap::<String, BTreeMap<String, usize>>::new();
        let mut address_targets =
            BTreeMap::<(u32, String, String), OracleAddressRequestEvidenceV1>::new();
        for (index, request) in requests.iter().enumerate() {
            let request_author = request.event().author_str();
            for target in request.projection().event_targets() {
                event_targets
                    .entry(target.event_id().as_str().to_owned())
                    .or_default()
                    .entry(request_author.to_owned())
                    .and_modify(|current| {
                        if request.event().id() < requests[*current].event().id() {
                            *current = index;
                        }
                    })
                    .or_insert(index);
            }
            for target in request.projection().address_targets() {
                let coordinate = (
                    target.coordinate().kind(),
                    target.coordinate().pubkey().as_str().to_owned(),
                    target.coordinate().identifier().to_owned(),
                );
                let evidence = address_targets.entry(coordinate.clone()).or_default();
                if request_author == coordinate.1 {
                    let replace = evidence.authorized.is_none_or(|current| {
                        let current = requests[current].event();
                        request.event().created_at_u64() > current.created_at_u64()
                            || (request.event().created_at_u64() == current.created_at_u64()
                                && request.event().id() < current.id())
                    });
                    if replace {
                        evidence.authorized = Some(index);
                    }
                } else if evidence.unauthorized.is_none() {
                    evidence.unauthorized = Some(index);
                }
            }
        }
        Self {
            requests,
            event_targets,
            address_targets,
        }
    }

    fn decision(&self, event: &RadrootsEventEnvelope) -> OracleSuppressionDecisionV1 {
        if event.kind_u32() == 5 {
            return OracleSuppressionDecisionV1 {
                outcome: RadrootsNip09SuppressionOutcome::Visible,
                reason: RadrootsNip09SuppressionReason::DeletionRequestImmune,
                event_reference_request_id: None,
                address_reference_request_id: None,
                address_reference_cutoff: None,
            };
        }

        let (event_reference, unauthorized_event_reference) = self
            .event_targets
            .get(event.id_str())
            .map_or((None, false), |by_author| {
                let authorized = by_author.get(event.author_str()).copied();
                (
                    authorized,
                    by_author.len() > usize::from(authorized.is_some()),
                )
            });
        let address_evidence = oracle_nip01_coordinate_key(event)
            .as_ref()
            .and_then(|coordinate| self.address_targets.get(coordinate));
        let address_reference = address_evidence.and_then(|evidence| evidence.authorized);
        let has_unauthorized_reference = unauthorized_event_reference
            || address_evidence.is_some_and(|evidence| evidence.unauthorized.is_some());

        let event_reference_request_id =
            event_reference.map(|index| self.requests[index].event().id().as_str().to_owned());
        let (address_reference_request_id, address_reference_cutoff) = address_reference
            .map(|index| {
                let request = self.requests[index].event();
                (
                    Some(request.id().as_str().to_owned()),
                    Some(request.created_at_u64()),
                )
            })
            .unwrap_or((None, None));
        let address_applies =
            address_reference_cutoff.is_some_and(|cutoff| event.created_at_u64() <= cutoff);
        let (outcome, reason) = match (event_reference.is_some(), address_applies) {
            (true, true) => (
                RadrootsNip09SuppressionOutcome::Suppressed,
                RadrootsNip09SuppressionReason::EventIdAndAddressReference,
            ),
            (true, false) => (
                RadrootsNip09SuppressionOutcome::Suppressed,
                RadrootsNip09SuppressionReason::EventIdReference,
            ),
            (false, true) => (
                RadrootsNip09SuppressionOutcome::Suppressed,
                RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff,
            ),
            (false, false) if address_reference.is_some() => (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget,
            ),
            (false, false) if has_unauthorized_reference => (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::RequestAuthorMismatch,
            ),
            (false, false) => (
                RadrootsNip09SuppressionOutcome::Visible,
                RadrootsNip09SuppressionReason::NoAuthorizedReference,
            ),
        };
        OracleSuppressionDecisionV1 {
            outcome,
            reason,
            event_reference_request_id,
            address_reference_request_id,
            address_reference_cutoff,
        }
    }
}

fn oracle_nip01_coordinate_key(event: &RadrootsEventEnvelope) -> Option<(u32, String, String)> {
    let kind = event.kind_u32();
    let identifier = match kind {
        0 | 3 | 10_000..=19_999 => String::new(),
        30_000..=39_999 => event
            .tag_slices()
            .iter()
            .find(|tag| tag.as_slice().first().is_some_and(|name| name == "d"))?
            .as_slice()
            .get(1)?
            .clone(),
        _ => return None,
    };
    let coordinate =
        RadrootsNip01Coordinate::parse(format!("{kind}:{}:{identifier}", event.author_str()))
            .ok()?;
    Some((
        coordinate.kind(),
        coordinate.pubkey().as_str().to_owned(),
        coordinate.identifier().to_owned(),
    ))
}

fn rebuild_drift<T>(
    kind: RadrootsEventStoreRawSourceRebuildDriftV1,
    detail: impl Into<String>,
) -> Result<T, RadrootsEventStoreError> {
    Err(RadrootsEventStoreError::RawSourceRebuildStateDrift {
        kind,
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip09::reconciliation_v1::{
        ReconciliationCapacityLimits, load_reconciliation_snapshot,
    };
    use crate::{RadrootsEventIngest, RadrootsEventStore};
    use nostr::{EventBuilder, Keys, Kind, SecretKey, Tag, TagKind, Timestamp};
    use radroots_event_codec::verification::v1::RadrootsSignatureVerifiedEvent;
    use serde_json::Value;

    const FOOD_FIXTURE: &[u8] =
        include_bytes!("../../../tests/fixtures/food_availability_projection.v1.json");
    const FIXTURE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const OTHER_SECRET_KEY_HEX: &str =
        "0000000000000000000000000000000000000000000000000000000000000002";

    fn signed_ingest(kind: u16, created_at: u64, content: &str) -> RadrootsEventIngest {
        signed_ingest_with_tags(kind, created_at, content, Vec::new())
    }

    fn signed_ingest_with_tags(
        kind: u16,
        created_at: u64,
        content: &str,
        tags: Vec<Vec<String>>,
    ) -> RadrootsEventIngest {
        signed_ingest_with_tags_and_key(kind, created_at, content, tags, FIXTURE_SECRET_KEY_HEX)
    }

    fn signed_ingest_with_tags_and_key(
        kind: u16,
        created_at: u64,
        content: &str,
        tags: Vec<Vec<String>>,
        secret_key_hex: &str,
    ) -> RadrootsEventIngest {
        let keys = Keys::new(SecretKey::from_hex(secret_key_hex).expect("fixture secret key"));
        let event = EventBuilder::new(Kind::Custom(kind), content)
            .tags(
                tags.into_iter()
                    .map(|mut values| {
                        let name = values.remove(0);
                        Tag::custom(TagKind::Custom(name.into()), values)
                    })
                    .collect::<Vec<_>>(),
            )
            .custom_created_at(Timestamp::from_secs(created_at))
            .sign_with_keys(&keys)
            .expect("signed oracle event");
        RadrootsEventIngest::from_raw_json(
            serde_json::to_string(&event).expect("oracle event JSON"),
            i64::try_from(created_at * 1_000).expect("oracle observed time"),
        )
        .expect("verified oracle ingest")
    }

    fn fixture_ingests(case_id: &str) -> Vec<RadrootsEventIngest> {
        let fixture: Value = serde_json::from_slice(FOOD_FIXTURE).expect("Food fixture JSON");
        let case = fixture["cases"]
            .as_array()
            .expect("Food fixture cases")
            .iter()
            .find(|case| case["id"].as_str() == Some(case_id))
            .expect("oracle fixture case");
        case["events"]
            .as_array()
            .expect("oracle fixture events")
            .iter()
            .map(|observed| {
                RadrootsEventIngest::from_raw_json(
                    serde_json::to_string(&observed["event"]).expect("fixture event JSON"),
                    observed["observed_at_ms"]
                        .as_i64()
                        .expect("fixture observed time"),
                )
                .expect("verified fixture ingest")
            })
            .collect()
    }

    fn assert_indexed_decision_matches_protocol_v1(
        target: &RadrootsSignatureVerifiedEvent,
        requests: &[RadrootsAdmittedNip09DeletionRequestEventV1],
        index: &OracleRequestIndexV1<'_>,
    ) -> OracleSuppressionDecisionV1 {
        let expected = evaluate_nip09_suppression_from_borrowed_requests_v1(target, requests);
        let actual = index.decision(target.event());
        assert_eq!(actual.outcome, expected.outcome());
        assert_eq!(actual.reason, expected.reason());
        assert_eq!(
            actual.event_reference_request_id.as_deref(),
            expected
                .event_reference()
                .map(|evidence| evidence.request_id().as_str())
        );
        assert_eq!(
            actual.address_reference_request_id.as_deref(),
            expected
                .address_reference()
                .map(|evidence| evidence.request_id().as_str())
        );
        assert_eq!(
            actual.address_reference_cutoff,
            expected
                .address_reference()
                .map(|evidence| evidence.inclusive_cutoff())
        );
        actual
    }

    fn admitted_request(
        ingest: RadrootsEventIngest,
    ) -> RadrootsAdmittedNip09DeletionRequestEventV1 {
        admit_verified_nip09_deletion_request_event_v1(ingest.verified_event().clone())
            .expect("admitted deletion request")
    }

    #[test]
    fn raw_snapshot_visibility_oracle_bounds_high_fan_in_evidence_v1() {
        const REQUEST_COUNT: usize = 512;
        let target = signed_ingest_with_tags(
            30_402,
            1_700_000_000,
            "{}",
            vec![vec!["d".to_owned(), "high-fan-in".to_owned()]],
        );
        let coordinate = format!("30402:{}:high-fan-in", target.event().author_str());
        let mut requests = (0..REQUEST_COUNT)
            .map(|index| {
                let ingest = signed_ingest_with_tags(
                    5,
                    1_700_001_000 + u64::try_from(index).expect("request timestamp"),
                    "high fan-in",
                    vec![vec!["a".to_owned(), coordinate.clone()]],
                );
                admit_verified_nip09_deletion_request_event_v1(ingest.verified_event().clone())
                    .expect("admitted deletion request")
            })
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| left.event().id().cmp(right.event().id()));

        let index = OracleRequestIndexV1::new(&requests);
        let actual = index.decision(target.event());
        assert_eq!(
            actual.address_reference_cutoff,
            Some(1_700_001_000 + u64::try_from(REQUEST_COUNT - 1).expect("request count"))
        );
        assert_indexed_decision_matches_protocol_v1(target.verified_event(), &requests, &index);
    }

    #[test]
    fn raw_snapshot_visibility_oracle_matches_wide_event_and_address_requests_v1() {
        const TARGETS_PER_REFERENCE_KIND: usize = 128;
        let mut targets = Vec::with_capacity(TARGETS_PER_REFERENCE_KIND * 2);
        let mut request_tags = Vec::with_capacity(TARGETS_PER_REFERENCE_KIND * 2);
        for index in 0..TARGETS_PER_REFERENCE_KIND {
            let offset = u64::try_from(index).expect("target timestamp");
            let event_target = signed_ingest(
                1,
                1_700_010_000 + offset,
                &format!("wide event target {index}"),
            );
            request_tags.push(vec![
                "e".to_owned(),
                event_target.event().id_str().to_owned(),
            ]);
            targets.push(event_target);

            let identifier = format!("wide-address-{index}");
            let address_target = signed_ingest_with_tags(
                30_402,
                1_700_020_000 + offset,
                "{}",
                vec![vec!["d".to_owned(), identifier.clone()]],
            );
            request_tags.push(vec![
                "a".to_owned(),
                format!("30402:{}:{identifier}", address_target.event().author_str()),
            ]);
            targets.push(address_target);
        }
        let request = signed_ingest_with_tags(5, 1_700_030_000, "wide request", request_tags);
        let requests = vec![
            admit_verified_nip09_deletion_request_event_v1(request.verified_event().clone())
                .expect("admitted wide deletion request"),
        ];
        let index = OracleRequestIndexV1::new(&requests);
        for target in &targets {
            assert_indexed_decision_matches_protocol_v1(target.verified_event(), &requests, &index);
        }
    }

    #[test]
    fn raw_snapshot_visibility_oracle_matches_all_protocol_decision_branches_v1() {
        let no_reference = signed_ingest(1, 1_700_100_000, "no reference");
        let no_reference_requests = Vec::new();
        let no_reference_index = OracleRequestIndexV1::new(&no_reference_requests);
        assert_eq!(
            assert_indexed_decision_matches_protocol_v1(
                no_reference.verified_event(),
                &no_reference_requests,
                &no_reference_index,
            )
            .reason,
            RadrootsNip09SuppressionReason::NoAuthorizedReference
        );

        let immune = signed_ingest(5, 1_700_100_010, "immune");
        let immune_requests = vec![admitted_request(signed_ingest_with_tags(
            5,
            1_700_100_020,
            "references deletion request",
            vec![vec!["e".to_owned(), immune.event().id_str().to_owned()]],
        ))];
        let immune_index = OracleRequestIndexV1::new(&immune_requests);
        assert_eq!(
            assert_indexed_decision_matches_protocol_v1(
                immune.verified_event(),
                &immune_requests,
                &immune_index,
            )
            .reason,
            RadrootsNip09SuppressionReason::DeletionRequestImmune
        );

        let unauthorized = signed_ingest(1, 1_700_100_030, "unauthorized target");
        let unauthorized_requests = vec![admitted_request(signed_ingest_with_tags_and_key(
            5,
            1_700_100_040,
            "wrong author",
            vec![vec![
                "e".to_owned(),
                unauthorized.event().id_str().to_owned(),
            ]],
            OTHER_SECRET_KEY_HEX,
        ))];
        let unauthorized_index = OracleRequestIndexV1::new(&unauthorized_requests);
        assert_eq!(
            assert_indexed_decision_matches_protocol_v1(
                unauthorized.verified_event(),
                &unauthorized_requests,
                &unauthorized_index,
            )
            .reason,
            RadrootsNip09SuppressionReason::RequestAuthorMismatch
        );

        let stale = signed_ingest_with_tags(
            30_402,
            1_700_100_100,
            "{}",
            vec![vec!["d".to_owned(), "stale".to_owned()]],
        );
        let stale_requests = vec![
            admitted_request(signed_ingest_with_tags(
                5,
                1_700_100_090,
                "stale address",
                vec![vec![
                    "a".to_owned(),
                    format!("30402:{}:stale", stale.event().author_str()),
                ]],
            )),
            admitted_request(signed_ingest_with_tags_and_key(
                5,
                1_700_100_110,
                "unauthorized exact reference",
                vec![vec!["e".to_owned(), stale.event().id_str().to_owned()]],
                OTHER_SECRET_KEY_HEX,
            )),
        ];
        let stale_index = OracleRequestIndexV1::new(&stale_requests);
        let stale_decision = assert_indexed_decision_matches_protocol_v1(
            stale.verified_event(),
            &stale_requests,
            &stale_index,
        );
        assert_eq!(
            stale_decision.reason,
            RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget
        );
        assert!(stale_decision.address_reference_request_id.is_some());

        let exact = signed_ingest_with_tags(
            30_402,
            1_700_100_200,
            "{}",
            vec![vec!["d".to_owned(), "exact".to_owned()]],
        );
        let exact_requests = vec![
            admitted_request(signed_ingest_with_tags(
                5,
                1_700_100_190,
                "stale address",
                vec![vec![
                    "a".to_owned(),
                    format!("30402:{}:exact", exact.event().author_str()),
                ]],
            )),
            admitted_request(signed_ingest_with_tags(
                5,
                1_700_100_210,
                "exact event",
                vec![vec!["e".to_owned(), exact.event().id_str().to_owned()]],
            )),
        ];
        let exact_index = OracleRequestIndexV1::new(&exact_requests);
        let exact_decision = assert_indexed_decision_matches_protocol_v1(
            exact.verified_event(),
            &exact_requests,
            &exact_index,
        );
        assert_eq!(
            exact_decision.reason,
            RadrootsNip09SuppressionReason::EventIdReference
        );
        assert!(exact_decision.event_reference_request_id.is_some());
        assert!(exact_decision.address_reference_request_id.is_some());

        let address = signed_ingest_with_tags(
            30_402,
            1_700_100_300,
            "{}",
            vec![vec!["d".to_owned(), "address".to_owned()]],
        );
        let address_requests = vec![admitted_request(signed_ingest_with_tags(
            5,
            1_700_100_310,
            "address reference",
            vec![vec![
                "a".to_owned(),
                format!("30402:{}:address", address.event().author_str()),
            ]],
        ))];
        let address_index = OracleRequestIndexV1::new(&address_requests);
        assert_eq!(
            assert_indexed_decision_matches_protocol_v1(
                address.verified_event(),
                &address_requests,
                &address_index,
            )
            .reason,
            RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff
        );

        let both = signed_ingest_with_tags(
            30_402,
            1_700_100_400,
            "{}",
            vec![vec!["d".to_owned(), "both".to_owned()]],
        );
        let both_requests = vec![admitted_request(signed_ingest_with_tags(
            5,
            1_700_100_410,
            "both references",
            vec![
                vec!["e".to_owned(), both.event().id_str().to_owned()],
                vec![
                    "a".to_owned(),
                    format!("30402:{}:both", both.event().author_str()),
                ],
            ],
        ))];
        let both_index = OracleRequestIndexV1::new(&both_requests);
        let both_decision = assert_indexed_decision_matches_protocol_v1(
            both.verified_event(),
            &both_requests,
            &both_index,
        );
        assert_eq!(
            both_decision.reason,
            RadrootsNip09SuppressionReason::EventIdAndAddressReference
        );
        assert!(both_decision.event_reference_request_id.is_some());
        assert!(both_decision.address_reference_request_id.is_some());
    }

    #[test]
    fn raw_snapshot_visibility_oracle_is_order_and_repeat_invariant_v1() {
        let target = signed_ingest_with_tags(
            30_402,
            1_700_200_000,
            "{}",
            vec![vec!["d".to_owned(), "invariant".to_owned()]],
        );
        let coordinate = format!("30402:{}:invariant", target.event().author_str());
        let first = admitted_request(signed_ingest_with_tags(
            5,
            1_700_200_010,
            "first exact",
            vec![vec!["e".to_owned(), target.event().id_str().to_owned()]],
        ));
        let second = admitted_request(signed_ingest_with_tags(
            5,
            1_700_200_020,
            "second exact and address",
            vec![
                vec!["e".to_owned(), target.event().id_str().to_owned()],
                vec!["a".to_owned(), coordinate.clone()],
            ],
        ));
        let third = admitted_request(signed_ingest_with_tags(
            5,
            1_700_200_020,
            "address tie",
            vec![vec!["a".to_owned(), coordinate]],
        ));
        let canonical_requests = vec![first.clone(), second.clone(), third.clone()];
        let repeated_reverse_requests = vec![
            third.clone(),
            second.clone(),
            first.clone(),
            third,
            second,
            first,
        ];
        let canonical_index = OracleRequestIndexV1::new(&canonical_requests);
        let repeated_reverse_index = OracleRequestIndexV1::new(&repeated_reverse_requests);
        let canonical = assert_indexed_decision_matches_protocol_v1(
            target.verified_event(),
            &canonical_requests,
            &canonical_index,
        );
        let repeated_reverse = assert_indexed_decision_matches_protocol_v1(
            target.verified_event(),
            &repeated_reverse_requests,
            &repeated_reverse_index,
        );
        assert_eq!(repeated_reverse, canonical);
    }

    #[tokio::test]
    async fn raw_snapshot_visibility_oracle_covers_regular_replaceable_addressable_and_deletion_v1()
    {
        let store = RadrootsEventStore::open_memory().await.expect("open store");
        let fixture_pubkey = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
        for ingest in [
            signed_ingest(1, 1_700_000_010, "Victoria harvest update"),
            signed_ingest(0, 1_700_000_020, "{}"),
            signed_ingest_with_tags(
                5,
                1_700_000_030,
                "Profile withdrawn",
                vec![vec!["a".to_owned(), format!("0:{fixture_pubkey}:")]],
            ),
        ]
        .into_iter()
        .chain(fixture_ingests(
            "authorized_address_deletion_retracts_projection",
        )) {
            store.ingest_event(ingest).await.expect("ingest oracle row");
        }

        let mut connection = store.pool().acquire().await.expect("connection");
        let snapshot = load_reconciliation_snapshot(
            &mut connection,
            ReconciliationCapacityLimits::production(),
        )
        .await
        .expect("load immutable raw snapshot");
        let requests = oracle_deletion_requests(&snapshot.events).expect("oracle requests");
        let request_index = OracleRequestIndexV1::new(&requests);
        for event in &snapshot.events {
            assert_indexed_decision_matches_protocol_v1(
                &event.verified_event,
                &requests,
                &request_index,
            );
        }
        let expected = expected_visibility(&snapshot.events).expect("oracle facts");
        let classes = expected
            .iter()
            .map(|fact| fact.event_class.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            classes,
            BTreeSet::from(["regular", "replaceable", "addressable"])
        );

        let deletion_id = snapshot
            .events
            .iter()
            .find(|event| event.verified_event.event().kind_u32() == 5)
            .map(|event| event.verified_event.event().id_str())
            .expect("deletion request");
        let deletion_fact = expected
            .iter()
            .find(|fact| fact.event_id == deletion_id)
            .expect("deletion oracle fact");
        assert_eq!(deletion_fact.current_visibility, "visible");
        assert_eq!(
            deletion_fact.suppression_reason.as_deref(),
            Some("deletion_request_immune")
        );

        let addressable_id = snapshot
            .events
            .iter()
            .find(|event| event.verified_event.event().kind_u32() == 30_402)
            .map(|event| event.verified_event.event().id_str())
            .expect("addressable Food event");
        assert_eq!(
            expected
                .iter()
                .find(|fact| fact.event_id == addressable_id)
                .expect("addressable oracle fact")
                .current_visibility,
            "suppressed"
        );
        let replaceable_id = snapshot
            .events
            .iter()
            .find(|event| event.verified_event.event().kind_u32() == 0)
            .map(|event| event.verified_event.event().id_str())
            .expect("replaceable profile event");
        let replaceable_fact = expected
            .iter()
            .find(|fact| fact.event_id == replaceable_id)
            .expect("replaceable oracle fact");
        assert_eq!(replaceable_fact.current_visibility, "suppressed");
        assert!(replaceable_fact.address_reference_request_id.is_some());
        audit_current_visibility_from_raw_v1(&snapshot.events, expected)
            .await
            .expect("matching oracle audit");

        let mut drift = expected_visibility(&snapshot.events).expect("second oracle facts");
        drift[0].current_visibility = "forged".to_owned();
        assert!(
            audit_current_visibility_from_raw_v1(&snapshot.events, drift)
                .await
                .is_err()
        );
    }
}
