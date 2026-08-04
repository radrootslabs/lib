use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use futures_executor::block_on;
use radroots_event::{SignedEvent, draft::SignedEventParts};
use radroots_storage::{event::SourceGeneration, memory::MemoryStorage};
use radroots_sync::{
    Engine, PullRequest,
    ingest::RegistryPolicy,
    policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId, SyncStorage},
    pull::{PULL_MAX_PAGES, PullTermination},
};
use radroots_transport::{
    Error as TransportError, EventSource, FetchPage, FetchRequest, SourceStatus, Target, TargetSet,
    TransportId,
    outcome::{FetchTargetOutcome, FetchTargetState},
    source::{EventProvenance, FetchCursor, NextPage, ObservedEvent},
};

const EVENT_ID: &str = "762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0";
const PUBKEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const SIGNATURE: &str = "4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109";
const CONTENT: &str = "{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}";

enum Response {
    Page {
        events: Vec<ObservedEvent>,
        state: FetchTargetState,
        next: NextPage,
    },
    Failure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RequestEvidence {
    cursor: Option<String>,
    limit: u16,
    deadline_unix_ms: u64,
}

struct ScriptedSource {
    responses: Mutex<VecDeque<Response>>,
    requests: Mutex<Vec<RequestEvidence>>,
}

impl ScriptedSource {
    fn new(responses: Vec<Response>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<RequestEvidence> {
        self.requests.lock().expect("requests").clone()
    }
}

impl EventSource for ScriptedSource {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, TransportError>> {
        Box::pin(async { unreachable!("pull does not inspect source status") })
    }

    fn fetch(
        &self,
        request: FetchRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<FetchPage, TransportError>> {
        Box::pin(async move {
            self.requests
                .lock()
                .expect("requests")
                .push(RequestEvidence {
                    cursor: request.cursor().map(|cursor| cursor.as_str().to_owned()),
                    limit: request.bounds().limit(),
                    deadline_unix_ms: request.bounds().deadline_unix_ms(),
                });
            match self
                .responses
                .lock()
                .expect("responses")
                .pop_front()
                .expect("scripted response")
            {
                Response::Failure => Err(TransportError::UnsupportedOperation),
                Response::Page {
                    events,
                    state,
                    next,
                } => {
                    let target = request.target_set().targets()[0].fingerprint().clone();
                    FetchPage::for_request(
                        &request,
                        events,
                        vec![FetchTargetOutcome::new(target, state)],
                        next,
                    )
                }
            }
        })
    }
}

struct FixedClock(u64);

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> Result<u64, Error> {
        Ok(self.0)
    }
}

struct DeadlineClock(Mutex<VecDeque<u64>>);

impl Clock for DeadlineClock {
    fn now_unix_ms(&self) -> Result<u64, Error> {
        self.0
            .lock()
            .expect("clock")
            .pop_front()
            .ok_or(Error::ClockUnavailable)
    }
}

struct SequenceIds(Mutex<u8>);

impl IdSource for SequenceIds {
    fn next_id(&self, _operation: OperationKind) -> Result<SyncId, Error> {
        let mut next = self.0.lock().expect("ids");
        let value = *next;
        *next = next.checked_add(1).ok_or(Error::InvalidSyncId)?;
        SyncId::new([value; 16])
    }
}

fn target() -> Target {
    Target::new(TransportId::NOSTR, "wss://relay.example").expect("target")
}

fn targets() -> TargetSet {
    TargetSet::new(vec![target()]).expect("target set")
}

fn signed_event(signature: &str) -> SignedEvent {
    let raw_json = format!(
        "{{\"id\":\"{EVENT_ID}\",\"pubkey\":\"{PUBKEY}\",\"created_at\":1800000100,\"kind\":0,\"tags\":[],\"content\":{content:?},\"sig\":\"{signature}\"}}",
        content = CONTENT,
    );
    SignedEvent::new(SignedEventParts {
        id: EVENT_ID.to_owned(),
        pubkey: PUBKEY.to_owned(),
        created_at: 1_800_000_100,
        kind: 0,
        tags: vec![],
        content: CONTENT.to_owned(),
        sig: signature.to_owned(),
        raw_json,
    })
    .expect("ID-valid event")
}

fn observed(signature: &str, observed_at: u64) -> ObservedEvent {
    let target = target();
    ObservedEvent::new(
        signed_event(signature),
        EventProvenance::new(
            TransportId::NOSTR,
            target.fingerprint().clone(),
            observed_at,
        )
        .expect("provenance"),
    )
}

fn engine(source: Arc<ScriptedSource>, clock: Arc<dyn Clock>, timeout_ms: u64) -> Engine {
    let storage: Arc<dyn SyncStorage> = Arc::new(MemoryStorage::new(
        SourceGeneration::new([8; 32]).expect("generation"),
    ));
    Engine::builder(
        storage,
        clock,
        Arc::new(SequenceIds(Mutex::new(1))),
        DeadlinePolicy::new(timeout_ms, 10, 10).expect("deadlines"),
    )
    .source(source)
    .build()
    .expect("engine")
}

#[test]
fn single_and_multiple_pages_propagate_cursor_deadline_and_ingest_results() {
    let single_source = Arc::new(ScriptedSource::new(vec![Response::Page {
        events: vec![observed(SIGNATURE, 1)],
        state: FetchTargetState::Complete,
        next: NextPage::Complete,
    }]));
    let single = engine(single_source.clone(), Arc::new(FixedClock(100)), 50);
    let request = PullRequest::new(targets(), 20, 1).expect("request");
    assert_eq!(request.targets().len(), 1);
    assert_eq!(request.page_limit(), 20);
    assert_eq!(request.max_pages(), 1);
    assert!(request.cursor().is_none());
    let receipt = block_on(single.pull(request, &RegistryPolicy::visible())).expect("pull");
    assert_eq!(receipt.termination(), PullTermination::Complete);
    assert_eq!(receipt.pages_fetched(), 1);
    assert_eq!(receipt.events_observed(), 1);
    assert_ne!(receipt.sync_id().as_bytes(), &[0; 16]);
    assert_eq!(receipt.deadline_unix_ms(), 150);
    assert!(receipt.ingest_outcomes()[0].is_ok());
    assert_eq!(single_source.requests()[0].deadline_unix_ms, 150);

    let next = FetchCursor::parse("page-2").expect("cursor");
    let multiple_source = Arc::new(ScriptedSource::new(vec![
        Response::Page {
            events: vec![],
            state: FetchTargetState::Partial,
            next: NextPage::Cursor(next.clone()),
        },
        Response::Page {
            events: vec![observed(SIGNATURE, 2)],
            state: FetchTargetState::Complete,
            next: NextPage::Complete,
        },
    ]));
    let multiple = engine(multiple_source.clone(), Arc::new(FixedClock(200)), 50);
    let receipt = block_on(
        multiple.pull(
            PullRequest::new(targets(), 10, 2)
                .expect("request")
                .with_cursor(FetchCursor::parse("starting").expect("initial cursor")),
            &RegistryPolicy::visible(),
        ),
    )
    .expect("pull");
    assert_eq!(receipt.pages_fetched(), 2);
    assert_eq!(receipt.termination(), PullTermination::Complete);
    assert_eq!(
        receipt.target_outcomes()[0].state(),
        FetchTargetState::Complete
    );
    let requests = multiple_source.requests();
    assert_eq!(requests[0].cursor.as_deref(), Some("starting"));
    assert_eq!(requests[1].cursor.as_deref(), Some(next.as_str()));
    assert_eq!(requests[0].deadline_unix_ms, requests[1].deadline_unix_ms);
}

#[test]
fn source_failure_and_cancelled_page_return_resumable_partial_receipts() {
    let cursor = FetchCursor::parse("resume").expect("cursor");
    let source = Arc::new(ScriptedSource::new(vec![
        Response::Page {
            events: vec![observed(SIGNATURE, 1)],
            state: FetchTargetState::Partial,
            next: NextPage::Cursor(cursor.clone()),
        },
        Response::Failure,
    ]));
    let pull = engine(source, Arc::new(FixedClock(100)), 50);
    let receipt = block_on(pull.pull(
        PullRequest::new(targets(), 10, 3).expect("request"),
        &RegistryPolicy::visible(),
    ))
    .expect("partial receipt");
    assert_eq!(receipt.termination(), PullTermination::SourceFailed);
    assert_eq!(receipt.pages_fetched(), 1);
    assert_eq!(
        receipt.resume_from().map(FetchCursor::as_str),
        Some("resume")
    );
    assert!(receipt.ingest_outcomes()[0].is_ok());

    let cancelled_from = FetchCursor::parse("cancelled-at").expect("cursor");
    let source = Arc::new(ScriptedSource::new(vec![Response::Page {
        events: vec![],
        state: FetchTargetState::Cancelled,
        next: NextPage::Cancelled {
            resume_from: Some(cancelled_from.clone()),
        },
    }]));
    let pull = engine(source, Arc::new(FixedClock(100)), 50);
    let receipt = block_on(pull.pull(
        PullRequest::new(targets(), 10, 3).expect("request"),
        &RegistryPolicy::visible(),
    ))
    .expect("cancelled receipt");
    assert_eq!(receipt.termination(), PullTermination::Cancelled);
    assert_eq!(
        receipt.resume_from().map(FetchCursor::as_str),
        Some(cancelled_from.as_str())
    );
}

#[test]
fn page_and_deadline_limits_stop_without_hidden_fetches() {
    assert_eq!(
        PullRequest::new(targets(), 0, 1),
        Err(Error::InvalidPullRequest)
    );
    assert_eq!(
        PullRequest::new(
            targets(),
            radroots_transport::source::FETCH_PAGE_MAX_EVENTS + 1,
            1
        ),
        Err(Error::InvalidPullRequest)
    );
    assert_eq!(
        PullRequest::new(targets(), 1, 0),
        Err(Error::InvalidPullRequest)
    );
    assert_eq!(
        PullRequest::new(targets(), 1, PULL_MAX_PAGES + 1),
        Err(Error::InvalidPullRequest)
    );

    let cursor = FetchCursor::parse("more").expect("cursor");
    let page_limited_source = Arc::new(ScriptedSource::new(vec![Response::Page {
        events: vec![],
        state: FetchTargetState::Partial,
        next: NextPage::Cursor(cursor.clone()),
    }]));
    let pull = engine(page_limited_source.clone(), Arc::new(FixedClock(100)), 10);
    let receipt = block_on(pull.pull(
        PullRequest::new(targets(), 1, 1).expect("request"),
        &RegistryPolicy::visible(),
    ))
    .expect("limited receipt");
    assert_eq!(receipt.termination(), PullTermination::PageLimit);
    assert_eq!(page_limited_source.requests().len(), 1);

    let deadline_source = Arc::new(ScriptedSource::new(vec![Response::Page {
        events: vec![],
        state: FetchTargetState::Partial,
        next: NextPage::Cursor(cursor),
    }]));
    let clock = Arc::new(DeadlineClock(Mutex::new(VecDeque::from([100, 110]))));
    let pull = engine(deadline_source.clone(), clock, 10);
    let receipt = block_on(pull.pull(
        PullRequest::new(targets(), 1, 2).expect("request"),
        &RegistryPolicy::visible(),
    ))
    .expect("deadline receipt");
    assert_eq!(receipt.termination(), PullTermination::Deadline);
    assert_eq!(receipt.deadline_unix_ms(), 110);
    assert_eq!(deadline_source.requests().len(), 1);
}
