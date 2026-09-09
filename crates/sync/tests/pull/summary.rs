use super::*;
use radroots_sync::PullReceipt;
use radroots_transport::target::TARGET_SET_MAX_ITEMS;

struct Source {
    pages: Mutex<VecDeque<Option<Vec<Option<FetchTargetState>>>>>,
    finish: NextPage,
}

impl EventSource for Source {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, TransportError>> {
        Box::pin(async { unreachable!("explicit pull only") })
    }

    fn fetch(
        &self,
        request: FetchRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<FetchPage, TransportError>> {
        Box::pin(async move {
            let mut pages = self.pages.lock().expect("pages");
            let states = pages
                .pop_front()
                .expect("bounded scripted fetch")
                .ok_or(TransportError::UnsupportedOperation)?;
            assert_eq!(states.len(), request.target_set().len());
            let outcomes = request
                .target_set()
                .targets()
                .iter()
                .zip(states)
                .filter_map(|(target, state)| {
                    state.map(|state| FetchTargetOutcome::new(target.fingerprint().clone(), state))
                })
                .collect();
            let next = if pages.is_empty() {
                self.finish.clone()
            } else {
                NextPage::Cursor(
                    FetchCursor::parse(format!("remaining-{}", pages.len())).expect("cursor"),
                )
            };
            FetchPage::for_request(&request, vec![], outcomes, next)
        })
    }
}

fn target_set(count: usize) -> TargetSet {
    TargetSet::new(
        (0..count)
            .rev()
            .map(|index| {
                Target::nostr_relay(format!("wss://relay-{index}.example")).expect("target")
            })
            .collect(),
    )
    .expect("targets")
}

fn pull(
    pages: Vec<Option<Vec<Option<FetchTargetState>>>>,
    targets: TargetSet,
    max_pages: u16,
    finish: NextPage,
    clock: Arc<dyn Clock>,
) -> PullReceipt {
    let source = Arc::new(Source {
        pages: Mutex::new(pages.into()),
        finish,
    });
    block_on(engine(source, clock, 50).pull(
        PullRequest::new(targets, 1, max_pages).expect("request"),
        &RegistryPolicy::visible(),
    ))
    .expect("receipt")
}

fn ordered(states: &[Option<FetchTargetState>]) -> PullReceipt {
    pull(
        states.iter().map(|state| Some(vec![*state])).collect(),
        target_set(1),
        states.len() as u16,
        NextPage::Complete,
        Arc::new(FixedClock(100)),
    )
}

#[test]
fn every_incomplete_state_survives_later_success_and_missing_outcomes() {
    use FetchTargetState::*;
    for state in [
        Partial,
        Unavailable,
        FailedRetryable,
        FailedTerminal,
        Cancelled,
    ] {
        for states in [
            vec![Some(state), Some(Complete)],
            vec![Some(Complete), Some(state)],
            vec![Some(state), None, Some(Complete)],
            vec![Some(state), Some(Complete), None],
        ] {
            let receipt = ordered(&states);
            let summaries = receipt.target_summaries().expect("measured");
            assert_eq!(summaries.len(), 1);
            let summary = &summaries[0];
            assert_eq!(summary.target(), target_set(1).targets()[0].fingerprint());
            assert_eq!(summary.pages_observed(), states.len() as u16);
            assert_eq!(summary.incomplete_pages(), 1);
            assert_eq!(
                summary.missing_outcome_pages(),
                states.iter().filter(|value| value.is_none()).count() as u16
            );
            assert_eq!(summary.last_incomplete(), Some(state));
            assert!(!summary.all_pages_complete());
            assert_eq!(
                receipt.target_outcomes()[0].state(),
                states.iter().rev().flatten().next().copied().unwrap()
            );
            #[cfg(feature = "serde")]
            assert_eq!(
                serde_json::from_str::<PullReceipt>(&serde_json::to_string(&receipt).unwrap())
                    .unwrap(),
                receipt
            );
        }
    }
    let receipt = ordered(&[Some(Partial), Some(FailedTerminal), Some(Complete)]);
    let summary = &receipt.target_summaries().unwrap()[0];
    assert_eq!(summary.incomplete_pages(), 2);
    assert_eq!(summary.last_incomplete(), Some(FailedTerminal));
}

#[test]
fn omitted_outcomes_never_become_positive_evidence() {
    use FetchTargetState::Complete;
    for states in [
        vec![None],
        vec![None, Some(Complete)],
        vec![Some(Complete), None],
    ] {
        let receipt = ordered(&states);
        assert_eq!(receipt.termination(), PullTermination::Complete);
        let summary = &receipt.target_summaries().unwrap()[0];
        assert_eq!(summary.missing_outcome_pages(), 1);
        assert_eq!(summary.incomplete_pages(), 0);
        assert_eq!(summary.last_incomplete(), None);
        assert!(!summary.all_pages_complete());
        #[cfg(feature = "serde")]
        assert_eq!(
            serde_json::from_value::<PullReceipt>(serde_json::to_value(&receipt).unwrap()).unwrap(),
            receipt
        );
    }
}

#[test]
fn maximum_inventory_preserves_request_order_and_counts_without_page_history() {
    let targets = target_set(TARGET_SET_MAX_ITEMS);
    let receipt = pull(
        vec![
            Some(vec![Some(FetchTargetState::Complete); TARGET_SET_MAX_ITEMS]);
            usize::from(PULL_MAX_PAGES)
        ],
        targets.clone(),
        PULL_MAX_PAGES,
        NextPage::Complete,
        Arc::new(FixedClock(100)),
    );
    assert_eq!(receipt.pages_fetched(), PULL_MAX_PAGES);
    let summaries = receipt.target_summaries().unwrap();
    assert_eq!(summaries.len(), TARGET_SET_MAX_ITEMS);
    for (summary, target) in summaries.iter().zip(targets.targets()) {
        assert_eq!(summary.target(), target.fingerprint());
        assert_eq!(summary.pages_observed(), PULL_MAX_PAGES);
        assert!(summary.all_pages_complete());
    }
    #[cfg(feature = "serde")]
    {
        let encoded = serde_json::to_string(&receipt).unwrap();
        assert!(
            encoded.len() < 32_768,
            "bounded evidence excludes per-page history"
        );
        assert_eq!(
            serde_json::from_str::<PullReceipt>(&encoded).unwrap(),
            receipt
        );
    }
}

#[test]
fn multiple_targets_retain_independent_evidence() {
    use FetchTargetState::*;
    let receipt = pull(
        vec![
            Some(vec![Some(Partial), Some(Complete), None]),
            Some(vec![Some(Complete), Some(FailedRetryable), Some(Complete)]),
        ],
        target_set(3),
        2,
        NextPage::Complete,
        Arc::new(FixedClock(100)),
    );
    let summaries = receipt.target_summaries().unwrap();
    assert_eq!(
        summaries
            .iter()
            .map(|s| s.incomplete_pages())
            .collect::<Vec<_>>(),
        [1, 1, 0]
    );
    assert_eq!(
        summaries
            .iter()
            .map(|s| s.missing_outcome_pages())
            .collect::<Vec<_>>(),
        [0, 0, 1]
    );
    assert_eq!(
        summaries
            .iter()
            .map(|s| s.last_incomplete())
            .collect::<Vec<_>>(),
        [Some(Partial), Some(FailedRetryable), None]
    );
    assert!(summaries.iter().all(|s| !s.all_pages_complete()));
}

#[test]
fn termination_and_zero_returned_pages_remain_explicit() {
    use FetchTargetState::*;
    let failed = pull(
        vec![None],
        target_set(1),
        1,
        NextPage::Complete,
        Arc::new(FixedClock(100)),
    );
    assert_eq!(failed.termination(), PullTermination::SourceFailed);
    assert_eq!(failed.target_summaries().unwrap()[0].pages_observed(), 0);
    assert!(!failed.target_summaries().unwrap()[0].all_pages_complete());
    let later_failure = pull(
        vec![Some(vec![Some(Complete)]), None],
        target_set(1),
        2,
        NextPage::Complete,
        Arc::new(FixedClock(100)),
    );
    assert_eq!(later_failure.termination(), PullTermination::SourceFailed);
    assert!(later_failure.target_summaries().unwrap()[0].all_pages_complete());
    assert_eq!(
        later_failure.target_summaries().unwrap()[0].pages_observed(),
        1
    );
    let limited = pull(
        vec![Some(vec![Some(Partial)]); 2],
        target_set(1),
        1,
        NextPage::Complete,
        Arc::new(FixedClock(100)),
    );
    assert_eq!(limited.termination(), PullTermination::PageLimit);
    let deadline = pull(
        vec![Some(vec![Some(Partial)]); 2],
        target_set(1),
        2,
        NextPage::Complete,
        Arc::new(DeadlineClock(Mutex::new(VecDeque::from([100, 150])))),
    );
    assert_eq!(deadline.termination(), PullTermination::Deadline);
    let cancelled = pull(
        vec![Some(vec![Some(Cancelled)])],
        target_set(1),
        1,
        NextPage::Cancelled { resume_from: None },
        Arc::new(FixedClock(100)),
    );
    assert_eq!(cancelled.termination(), PullTermination::Cancelled);
    for receipt in [failed, later_failure, limited, deadline, cancelled] {
        #[cfg(feature = "serde")]
        assert_eq!(
            serde_json::from_value::<PullReceipt>(serde_json::to_value(&receipt).unwrap()).unwrap(),
            receipt
        );
        assert_eq!(
            receipt.target_summaries().unwrap()[0].pages_observed(),
            receipt.pages_fetched()
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn legacy_receipts_remain_unknown_and_new_receipts_reject_inconsistent_evidence() {
    use serde_json::json;
    let receipt = ordered(&[Some(FetchTargetState::Complete)]);
    let original = serde_json::to_value(&receipt).unwrap();
    let mut legacy = original.clone();
    legacy.as_object_mut().unwrap().remove("target_summaries");
    assert!(
        serde_json::from_value::<PullReceipt>(legacy.clone())
            .unwrap()
            .target_summaries()
            .is_none()
    );
    legacy["target_summaries"] = json!(null);
    assert!(
        serde_json::from_value::<PullReceipt>(legacy)
            .unwrap()
            .target_summaries()
            .is_none()
    );
    for replacement in [json!([]), json!({}), json!(42)] {
        let mut changed = original.clone();
        changed["target_summaries"] = replacement;
        assert!(serde_json::from_value::<PullReceipt>(changed).is_err());
    }
    let mut duplicate = original.clone();
    duplicate["target_summaries"]
        .as_array_mut()
        .unwrap()
        .push(original["target_summaries"][0].clone());
    assert!(serde_json::from_value::<PullReceipt>(duplicate).is_err());
    for (field, value) in [
        ("pages_observed", json!(1001)),
        ("pages_observed", json!(2)),
        ("incomplete_pages", json!(1)),
        ("incomplete_pages", json!(65535)),
        ("missing_outcome_pages", json!(2)),
        ("missing_outcome_pages", json!(1)),
        ("last_incomplete", json!("partial")),
        ("target", json!("bad")),
        ("extra", json!(true)),
    ] {
        let mut changed = original.clone();
        changed["target_summaries"][0][field] = value;
        assert!(
            serde_json::from_value::<PullReceipt>(changed).is_err(),
            "{field}"
        );
    }
    let mut complete_as_incomplete = original.clone();
    complete_as_incomplete["target_summaries"][0]["incomplete_pages"] = json!(1);
    complete_as_incomplete["target_summaries"][0]["last_incomplete"] = json!("complete");
    assert!(serde_json::from_value::<PullReceipt>(complete_as_incomplete).is_err());
    for change in 0..5 {
        let mut changed = original.clone();
        match change {
            0 => {
                changed["target_outcomes"] = json!([]);
            }
            1 => {
                changed["target_outcomes"]
                    .as_array_mut()
                    .unwrap()
                    .push(original["target_outcomes"][0].clone());
            }
            2 => {
                changed["target_outcomes"][0]["target"] =
                    json!(target_set(2).targets()[0].fingerprint());
            }
            3 => {
                changed["target_outcomes"][0]["state"] = json!("partial");
            }
            _ => {
                changed["target_summaries"][0]["incomplete_pages"] = json!(1);
                changed["target_summaries"][0]["last_incomplete"] = json!("partial");
            }
        }
        assert!(
            serde_json::from_value::<PullReceipt>(changed).is_err(),
            "case {change}"
        );
    }
    let mut all_missing = serde_json::to_value(ordered(&[None])).unwrap();
    all_missing["target_summaries"][0]["missing_outcome_pages"] = json!(0);
    assert!(serde_json::from_value::<PullReceipt>(all_missing).is_err());
    let maximum = pull(
        vec![Some(vec![
            Some(FetchTargetState::Complete);
            TARGET_SET_MAX_ITEMS
        ])],
        target_set(TARGET_SET_MAX_ITEMS),
        1,
        NextPage::Complete,
        Arc::new(FixedClock(100)),
    );
    let mut too_many = serde_json::to_value(maximum).unwrap();
    too_many["target_summaries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"unparsed_extra": [1, 2, 3]}));
    let error = serde_json::from_str::<PullReceipt>(&serde_json::to_string(&too_many).unwrap())
        .unwrap_err();
    assert!(error.to_string().contains("too many pull target summaries"));
}
