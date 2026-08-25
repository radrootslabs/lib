//! Bounded Nostr live-subscription adapter.

use crate::{NostrTransport, RelayCursor, RelayUrl};
use nostr_sdk::prelude::{
    ClientMessage, Filter, JsonUtil, Kind, RelayMessage, RelayPoolNotification, ReqExitPolicy,
    SubscribeAutoCloseOptions, SubscribeOptions, SubscriptionId, Timestamp,
};
use radroots_transport::{
    BoxFuture, BoxSubscription, EventSubscriber, EventSubscription, SubscriptionEnd,
    SubscriptionEndReason, SubscriptionEvent, SubscriptionNext, SubscriptionRequest,
    source::{EventProvenance, FetchCursor, ObservedEvent, SubscriptionCheckpoint},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const CURSOR_PREFIX: &str = "nostr-live-v1";
const CURSOR_SCOPE_DOMAIN: &[u8] = b"radroots.transport-nostr.subscription-cursor.v1\0";
const SUBSCRIPTION_ID_DOMAIN: &[u8] = b"radroots.transport-nostr.subscription-id.v1\0";

#[derive(Clone, Debug)]
pub(crate) struct RelaySubscriptionQuery {
    id: String,
    targets: Vec<RelaySubscriptionTarget>,
    selector: radroots_transport::source::FetchSelector,
    connect_timeout: Duration,
    timeout: Duration,
}

#[derive(Clone, Debug)]
struct RelaySubscriptionTarget {
    relay: RelayUrl,
    since_unix_seconds: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelaySubscriptionItem {
    Event { relay: RelayUrl, raw: String },
    Closed { relay: RelayUrl },
    Shutdown,
}

pub(crate) trait RelaySubscriptionSession: Send {
    fn next(&mut self) -> BoxFuture<'_, Result<RelaySubscriptionItem, ()>>;
    fn cancel(&mut self) -> BoxFuture<'_, Result<(), ()>>;
}

pub(crate) trait RelaySubscriptionClient: Send + Sync {
    fn subscribe(
        &self,
        query: RelaySubscriptionQuery,
    ) -> BoxFuture<'_, Result<Box<dyn RelaySubscriptionSession>, ()>>;
}

#[derive(Clone, Debug)]
pub(crate) struct LiveRelaySubscriptionClient {
    client: nostr_sdk::Client,
}

impl LiveRelaySubscriptionClient {
    pub(crate) const fn new(client: nostr_sdk::Client) -> Self {
        Self { client }
    }

    #[cfg(test)]
    pub(crate) fn isolated() -> Self {
        let client = nostr_sdk::Client::default();
        client.automatic_authentication(false);
        Self::new(client)
    }
}

impl RelaySubscriptionClient for LiveRelaySubscriptionClient {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn subscribe(
        &self,
        query: RelaySubscriptionQuery,
    ) -> BoxFuture<'_, Result<Box<dyn RelaySubscriptionSession>, ()>> {
        Box::pin(async move {
            let notifications = self.client.notifications();
            let subscription_id = SubscriptionId::new(query.id);
            let mut targeted = Vec::with_capacity(query.targets.len());
            let mut relay_lookup = BTreeMap::new();

            for target in query.targets {
                let url = target.relay.as_str().to_owned();
                let filter = subscription_filter(&query.selector, target.since_unix_seconds)?;
                self.client.add_relay(url.as_str()).await.map_err(|_| ())?;
                self.client
                    .try_connect_relay(url.as_str(), query.connect_timeout)
                    .await
                    .map_err(|_| ())?;
                relay_lookup.insert(url.clone(), target.relay);
                targeted.push((url, filter));
            }

            let auto_close = SubscribeAutoCloseOptions::default()
                .exit_policy(ReqExitPolicy::WaitDurationAfterEOSE(query.timeout))
                .timeout(Some(query.timeout));
            let output = self
                .client
                .subscribe_targeted(
                    subscription_id.clone(),
                    targeted,
                    SubscribeOptions::default().close_on(Some(auto_close)),
                )
                .await
                .map_err(|_| ())?;
            if !output.failed.is_empty() || output.success.len() != relay_lookup.len() {
                let _ = self
                    .client
                    .send_msg_to(
                        relay_lookup.keys().map(String::as_str),
                        ClientMessage::close(subscription_id.clone()),
                    )
                    .await;
                self.client.unsubscribe(&subscription_id).await;
                return Err(());
            }

            // Keep the receiver created before REQ publication so an immediate
            // relay event cannot race ahead of local observation.
            let session = LiveRelaySubscriptionSession {
                client: self.client.clone(),
                subscription_id,
                notifications: Some(notifications),
                relay_lookup,
                cancelled: false,
            };
            Ok(Box::new(session) as Box<dyn RelaySubscriptionSession>)
        })
    }
}

struct LiveRelaySubscriptionSession {
    client: nostr_sdk::Client,
    subscription_id: SubscriptionId,
    notifications: Option<tokio::sync::broadcast::Receiver<RelayPoolNotification>>,
    relay_lookup: BTreeMap<String, RelayUrl>,
    cancelled: bool,
}

impl RelaySubscriptionSession for LiveRelaySubscriptionSession {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn next(&mut self) -> BoxFuture<'_, Result<RelaySubscriptionItem, ()>> {
        Box::pin(async move {
            let Some(notifications) = self.notifications.as_mut() else {
                return Ok(RelaySubscriptionItem::Shutdown);
            };
            loop {
                let notification = match notifications.recv().await {
                    Ok(notification) => notification,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return Err(()),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Ok(RelaySubscriptionItem::Shutdown);
                    }
                };
                match notification {
                    RelayPoolNotification::Message { relay_url, message } => {
                        let Some(relay) = self.relay_lookup.get(relay_url.as_str()).cloned() else {
                            continue;
                        };
                        match message {
                            RelayMessage::Event {
                                subscription_id,
                                event,
                            } if subscription_id.as_ref() == &self.subscription_id => {
                                return Ok(RelaySubscriptionItem::Event {
                                    relay,
                                    raw: event.as_json(),
                                });
                            }
                            RelayMessage::Closed {
                                subscription_id, ..
                            } if subscription_id.as_ref() == &self.subscription_id => {
                                return Ok(RelaySubscriptionItem::Closed { relay });
                            }
                            _ => {}
                        }
                    }
                    RelayPoolNotification::Shutdown => {
                        return Ok(RelaySubscriptionItem::Shutdown);
                    }
                    RelayPoolNotification::Event { .. } => {}
                }
            }
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn cancel(&mut self) -> BoxFuture<'_, Result<(), ()>> {
        Box::pin(async move {
            if !self.cancelled {
                let output = self
                    .client
                    .send_msg_to(
                        self.relay_lookup.keys().map(String::as_str),
                        ClientMessage::close(self.subscription_id.clone()),
                    )
                    .await
                    .map_err(|_| ())?;
                if !output.failed.is_empty() || output.success.len() != self.relay_lookup.len() {
                    return Err(());
                }
                self.client.unsubscribe(&self.subscription_id).await;
                self.cancelled = true;
                self.notifications = None;
            }
            Ok(())
        })
    }
}

struct RelayEventSubscription {
    request: SubscriptionRequest,
    session: Option<Box<dyn RelaySubscriptionSession>>,
    targets: BTreeMap<RelayUrl, radroots_transport::Target>,
    active_relays: BTreeSet<RelayUrl>,
    resume_cursors: BTreeMap<radroots_transport::target::TargetFingerprint, RelayCursor>,
    cursors: BTreeMap<radroots_transport::target::TargetFingerprint, RelayCursor>,
    checkpoints: BTreeMap<radroots_transport::target::TargetFingerprint, SubscriptionCheckpoint>,
    seen_event_ids: BTreeSet<String>,
    event_count: u16,
    terminal: Option<SubscriptionEnd>,
    cancellation_requested: Arc<AtomicBool>,
    status: Arc<crate::status::StatusTracker>,
}

impl RelayEventSubscription {
    fn ended(
        request: SubscriptionRequest,
        reason: SubscriptionEndReason,
        status: Arc<crate::status::StatusTracker>,
    ) -> Result<Self, ()> {
        let terminal = SubscriptionEnd::for_request(&request, 0, [], reason).map_err(|_| ())?;
        Ok(Self {
            request,
            session: None,
            targets: BTreeMap::new(),
            active_relays: BTreeSet::new(),
            resume_cursors: BTreeMap::new(),
            cursors: BTreeMap::new(),
            checkpoints: BTreeMap::new(),
            seen_event_ids: BTreeSet::new(),
            event_count: 0,
            terminal: Some(terminal),
            cancellation_requested: Arc::new(AtomicBool::new(false)),
            status,
        })
    }

    async fn next_inner(&mut self) -> Result<SubscriptionNext, radroots_transport::Error> {
        if let Some(terminal) = &self.terminal {
            return Ok(SubscriptionNext::End(terminal.clone()));
        }
        if self.cancellation_requested.swap(false, Ordering::SeqCst) {
            return self
                .terminate(SubscriptionEndReason::Cancelled)
                .await
                .map(SubscriptionNext::End);
        }
        if self.event_count >= self.request.bounds().event_limit() {
            return self
                .terminate(SubscriptionEndReason::EventLimit)
                .await
                .map(SubscriptionNext::End);
        }

        loop {
            let remaining = remaining_duration(self.request.bounds().deadline_unix_ms());
            if remaining.is_zero() {
                return self
                    .terminate(SubscriptionEndReason::Deadline)
                    .await
                    .map(SubscriptionNext::End);
            }
            let item = {
                let session = self
                    .session
                    .as_mut()
                    .ok_or(radroots_transport::Error::SubscriptionUnavailable)?;
                tokio::time::timeout(remaining, session.next()).await
            };
            let item = match item {
                Ok(Ok(item)) => item,
                Ok(Err(())) => return Err(radroots_transport::Error::SubscriptionUnavailable),
                Err(_) => {
                    return self
                        .terminate(SubscriptionEndReason::Deadline)
                        .await
                        .map(SubscriptionNext::End);
                }
            };
            match item {
                RelaySubscriptionItem::Event { relay, raw } => {
                    if let Some(event) = self.admit_event(&relay, raw.as_str())? {
                        return Ok(SubscriptionNext::Event(Box::new(event)));
                    }
                }
                RelaySubscriptionItem::Closed { relay } => {
                    if !self.active_relays.remove(&relay) {
                        return Err(radroots_transport::Error::SubscriptionUnavailable);
                    }
                    if self.active_relays.is_empty() {
                        return self
                            .terminate(SubscriptionEndReason::SourceClosed)
                            .await
                            .map(SubscriptionNext::End);
                    }
                }
                RelaySubscriptionItem::Shutdown => {
                    return self
                        .terminate(SubscriptionEndReason::SourceClosed)
                        .await
                        .map(SubscriptionNext::End);
                }
            }
        }
    }

    fn admit_event(
        &mut self,
        relay: &RelayUrl,
        raw: &str,
    ) -> Result<Option<SubscriptionEvent>, radroots_transport::Error> {
        let target = self
            .targets
            .get(relay)
            .ok_or(radroots_transport::Error::UnexpectedSubscriptionEvent)?;
        let event = radroots_event_codec::decode::signed_event(raw)
            .map_err(|_| radroots_transport::Error::UnexpectedSubscriptionEvent)?;
        if !self.request.selector().matches(&event) {
            return Err(radroots_transport::Error::UnexpectedSubscriptionEvent);
        }
        let event_id = event.id_str().to_owned();
        let created_at = event.created_at();
        if self.seen_event_ids.contains(event_id.as_str()) {
            return Ok(None);
        }
        if self
            .resume_cursors
            .get(target.fingerprint())
            .is_some_and(|cursor| {
                created_at < cursor.created_at_unix_s()
                    || (created_at == cursor.created_at_unix_s()
                        && event_id.as_str() == cursor.event_id())
            })
        {
            return Ok(None);
        }

        let cursor = RelayCursor::new(created_at, event_id.clone())
            .map_err(|_| radroots_transport::Error::UnexpectedSubscriptionEvent)?;
        let cursor_advances = self
            .cursors
            .get(target.fingerprint())
            .is_none_or(|current| cursor > *current);
        if cursor_advances {
            self.cursors
                .insert(target.fingerprint().clone(), cursor.clone());
            let opaque = encode_cursor(&self.request, target.fingerprint(), &cursor)?;
            self.checkpoints.insert(
                target.fingerprint().clone(),
                SubscriptionCheckpoint::new(target.fingerprint().clone(), opaque),
            );
        }
        let checkpoint = self
            .checkpoints
            .get(target.fingerprint())
            .cloned()
            .ok_or(radroots_transport::Error::UnexpectedSubscriptionEvent)?;
        let provenance = EventProvenance::new(
            radroots_transport::TransportId::NOSTR,
            target.fingerprint().clone(),
            unix_time_ms().max(1),
        )?
        .with_cursor(checkpoint.cursor().clone());
        let observed = ObservedEvent::new(event, provenance);
        let subscription_event =
            SubscriptionEvent::for_request(&self.request, observed, checkpoint.clone())?;

        self.seen_event_ids.insert(event_id);
        self.event_count = self.event_count.saturating_add(1);
        self.status
            .record_read(relay, true, false, unix_time_ms().max(1));
        Ok(Some(subscription_event))
    }

    async fn terminate(
        &mut self,
        mut reason: SubscriptionEndReason,
    ) -> Result<SubscriptionEnd, radroots_transport::Error> {
        if let Some(terminal) = &self.terminal {
            return Ok(terminal.clone());
        }
        if matches!(
            reason,
            SubscriptionEndReason::EventLimit | SubscriptionEndReason::Cancelled
        ) {
            let remaining = remaining_duration(self.request.bounds().deadline_unix_ms());
            if remaining.is_zero() {
                reason = SubscriptionEndReason::Deadline;
            } else if let Some(session) = self.session.as_mut() {
                match tokio::time::timeout(remaining, session.cancel()).await {
                    Ok(Ok(())) => {}
                    Ok(Err(())) => {
                        return Err(radroots_transport::Error::SubscriptionUnavailable);
                    }
                    Err(_) => reason = SubscriptionEndReason::Deadline,
                }
            }
        }
        self.session = None;
        let terminal = SubscriptionEnd::for_request(
            &self.request,
            self.event_count,
            self.checkpoints.values().cloned(),
            reason,
        )?;
        self.terminal = Some(terminal.clone());
        Ok(terminal)
    }
}

impl EventSubscription for RelayEventSubscription {
    fn request(&self) -> &SubscriptionRequest {
        &self.request
    }

    fn next(&mut self) -> BoxFuture<'_, Result<SubscriptionNext, radroots_transport::Error>> {
        let cancellation = CancellationOnDrop::new(Arc::clone(&self.cancellation_requested));
        Box::pin(async move {
            let result = self.next_inner().await;
            cancellation.complete();
            result
        })
    }

    fn cancel(&mut self) -> BoxFuture<'_, Result<SubscriptionEnd, radroots_transport::Error>> {
        let cancellation = CancellationOnDrop::new(Arc::clone(&self.cancellation_requested));
        Box::pin(async move {
            let result = self.terminate(SubscriptionEndReason::Cancelled).await;
            cancellation.complete();
            result
        })
    }
}

impl EventSubscriber for NostrTransport {
    fn subscribe(
        &self,
        request: SubscriptionRequest,
    ) -> BoxFuture<'_, Result<BoxSubscription, radroots_transport::Error>> {
        Box::pin(async move {
            let remaining = remaining_duration(request.bounds().deadline_unix_ms());
            if remaining.is_zero() {
                return RelayEventSubscription::ended(
                    request,
                    SubscriptionEndReason::Deadline,
                    Arc::clone(&self.status),
                )
                .map(|subscription| Box::new(subscription) as BoxSubscription)
                .map_err(|()| radroots_transport::Error::SubscriptionUnavailable);
            }
            if !selector_is_representable(request.selector()) {
                return RelayEventSubscription::ended(
                    request,
                    SubscriptionEndReason::SourceClosed,
                    Arc::clone(&self.status),
                )
                .map(|subscription| Box::new(subscription) as BoxSubscription)
                .map_err(|()| radroots_transport::Error::SubscriptionUnavailable);
            }

            let now_ms = unix_time_ms();
            let mut targets = BTreeMap::new();
            let mut active_relays = BTreeSet::new();
            for target in request.target_set().targets() {
                let endpoint = self
                    .config()
                    .endpoint_for_target(target)
                    .filter(|endpoint| endpoint.access().can_read())
                    .ok_or(radroots_transport::Error::SubscriptionUnavailable)?;
                if !self.status.may_read(endpoint.url(), now_ms)
                    || targets
                        .insert(endpoint.url().clone(), target.clone())
                        .is_some()
                {
                    return Err(radroots_transport::Error::SubscriptionUnavailable);
                }
                active_relays.insert(endpoint.url().clone());
            }

            let mut cursors = BTreeMap::new();
            let mut checkpoints = BTreeMap::new();
            let mut target_queries = Vec::with_capacity(targets.len());
            for (relay, target) in &targets {
                let checkpoint = request
                    .checkpoints()
                    .iter()
                    .find(|checkpoint| checkpoint.target() == target.fingerprint());
                let cursor = checkpoint
                    .map(|checkpoint| parse_cursor(&request, checkpoint))
                    .transpose()?;
                let since = cursor
                    .as_ref()
                    .map(RelayCursor::created_at_unix_s)
                    .or(request.selector().since_unix_seconds());
                if let Some(cursor) = cursor {
                    cursors.insert(target.fingerprint().clone(), cursor);
                }
                if let Some(checkpoint) = checkpoint {
                    checkpoints.insert(target.fingerprint().clone(), checkpoint.clone());
                }
                target_queries.push(RelaySubscriptionTarget {
                    relay: relay.clone(),
                    since_unix_seconds: since,
                });
            }

            let timeout = remaining.min(Duration::from_millis(self.config().request_timeout_ms()));
            let id = subscription_id(&request, self.next_subscription_sequence()?);
            for relay in &active_relays {
                self.status.begin_read(relay, now_ms);
            }
            let query = RelaySubscriptionQuery {
                id,
                targets: target_queries,
                selector: request.selector().clone(),
                connect_timeout: timeout
                    .min(Duration::from_millis(self.config().connect_timeout_ms())),
                timeout: remaining,
            };
            let session = match tokio::time::timeout(
                timeout,
                self.subscription_client.subscribe(query),
            )
            .await
            {
                Ok(Ok(session)) => session,
                Ok(Err(())) | Err(_) => {
                    let observed_at = unix_time_ms().max(1);
                    for relay in &active_relays {
                        self.status.record_read(relay, false, true, observed_at);
                    }
                    return Err(radroots_transport::Error::SubscriptionUnavailable);
                }
            };

            let resume_cursors = cursors.clone();
            Ok(Box::new(RelayEventSubscription {
                request,
                session: Some(session),
                targets,
                active_relays,
                resume_cursors,
                cursors,
                checkpoints,
                seen_event_ids: BTreeSet::new(),
                event_count: 0,
                terminal: None,
                cancellation_requested: Arc::new(AtomicBool::new(false)),
                status: Arc::clone(&self.status),
            }) as BoxSubscription)
        })
    }
}

struct CancellationOnDrop {
    requested: Arc<AtomicBool>,
    completed: AtomicBool,
}

impl CancellationOnDrop {
    fn new(requested: Arc<AtomicBool>) -> Arc<Self> {
        Arc::new(Self {
            requested,
            completed: AtomicBool::new(false),
        })
    }

    fn complete(&self) {
        self.completed.store(true, Ordering::SeqCst);
    }
}

impl Drop for CancellationOnDrop {
    fn drop(&mut self) {
        if !self.completed.load(Ordering::SeqCst) {
            self.requested.store(true, Ordering::SeqCst);
        }
    }
}

fn subscription_filter(
    selector: &radroots_transport::source::FetchSelector,
    since: Option<u64>,
) -> Result<Filter, ()> {
    let kinds = selector
        .kinds()
        .iter()
        .filter_map(|kind| u16::try_from(*kind).ok())
        .map(Kind::from)
        .collect::<Vec<_>>();
    if !selector.kinds().is_empty() && kinds.is_empty() {
        return Err(());
    }
    let authors = selector
        .authors()
        .iter()
        .map(|author| radroots_nostr::key::public_key_to_nostr(*author).map_err(|_| ()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut filter = Filter::new();
    if !kinds.is_empty() {
        filter = filter.kinds(kinds);
    }
    if !authors.is_empty() {
        filter = filter.authors(authors);
    }
    filter = crate::source::apply_exact_tag_filters(filter, selector)?;
    if let Some(since) = since {
        filter = filter.since(Timestamp::from_secs(since));
    }
    if let Some(until) = selector.until_unix_seconds() {
        filter = filter.until(Timestamp::from_secs(until));
    }
    Ok(filter)
}

fn selector_is_representable(selector: &radroots_transport::source::FetchSelector) -> bool {
    selector.kinds().is_empty()
        || selector
            .kinds()
            .iter()
            .any(|kind| u16::try_from(*kind).is_ok())
}

fn subscription_id(request: &SubscriptionRequest, sequence: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SUBSCRIPTION_ID_DOMAIN);
    hasher.update(request.request_id().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(sequence.to_be_bytes());
    hex_encode(&hasher.finalize())
}

fn encode_cursor(
    request: &SubscriptionRequest,
    target: &radroots_transport::target::TargetFingerprint,
    cursor: &RelayCursor,
) -> Result<FetchCursor, radroots_transport::Error> {
    FetchCursor::parse(format!(
        "{CURSOR_PREFIX}:{}:{}:{}",
        cursor.created_at_unix_s(),
        cursor.event_id(),
        cursor_scope(request, target),
    ))
}

fn parse_cursor(
    request: &SubscriptionRequest,
    checkpoint: &SubscriptionCheckpoint,
) -> Result<RelayCursor, radroots_transport::Error> {
    let mut parts = checkpoint.cursor().as_str().split(':');
    let valid = parts.next() == Some(CURSOR_PREFIX);
    let created_at = parts.next().and_then(|value| value.parse::<u64>().ok());
    let event_id = parts.next();
    let scope = parts.next();
    if !valid || parts.next().is_some() {
        return Err(radroots_transport::Error::InvalidFetchCursor);
    }
    let (Some(created_at), Some(event_id), Some(scope)) = (created_at, event_id, scope) else {
        return Err(radroots_transport::Error::InvalidFetchCursor);
    };
    if scope != cursor_scope(request, checkpoint.target()) {
        return Err(radroots_transport::Error::InvalidFetchCursor);
    }
    RelayCursor::new(created_at, event_id)
        .map_err(|_| radroots_transport::Error::InvalidFetchCursor)
}

fn cursor_scope(
    request: &SubscriptionRequest,
    target: &radroots_transport::target::TargetFingerprint,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CURSOR_SCOPE_DOMAIN);
    hasher.update(target.as_str().as_bytes());
    hasher.update([0]);
    for kind in request.selector().kinds() {
        hasher.update(kind.to_be_bytes());
    }
    hasher.update([0]);
    for author in request.selector().authors() {
        hasher.update(author.as_bytes());
    }
    hasher.update([0]);
    crate::source::hash_exact_tag_filters(&mut hasher, request.selector());
    hash_optional_u64(&mut hasher, request.selector().since_unix_seconds());
    hash_optional_u64(&mut hasher, request.selector().until_unix_seconds());
    hex_encode(&hasher.finalize())
}

fn hash_optional_u64(hasher: &mut Sha256, value: Option<u64>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.to_be_bytes());
        }
        None => hasher.update([0]),
    }
}

fn remaining_duration(deadline_unix_ms: u64) -> Duration {
    Duration::from_millis(deadline_unix_ms.saturating_sub(unix_time_ms()))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
    };
    use nostr_sdk::prelude::{EventBuilder, Keys};
    use radroots_transport::{
        EventSubscriber, Target, TargetSet,
        source::{FetchSelector, SubscriptionBounds, SubscriptionCheckpoint},
    };
    use std::{
        collections::VecDeque,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering},
        },
    };

    const FIXTURE_SECRET_KEY: &str =
        "0000000000000000000000000000000000000000000000000000000000000001";

    #[derive(Clone, Debug)]
    enum ScriptedItem {
        Item(RelaySubscriptionItem),
        Error,
    }

    #[derive(Debug)]
    struct MockSubscriptionSession {
        items: VecDeque<ScriptedItem>,
        cancellations: Arc<AtomicUsize>,
    }

    impl RelaySubscriptionSession for MockSubscriptionSession {
        fn next(&mut self) -> BoxFuture<'_, Result<RelaySubscriptionItem, ()>> {
            match self.items.pop_front() {
                Some(ScriptedItem::Item(item)) => Box::pin(async move { Ok(item) }),
                Some(ScriptedItem::Error) => Box::pin(async { Err(()) }),
                None => Box::pin(core::future::pending()),
            }
        }

        fn cancel(&mut self) -> BoxFuture<'_, Result<(), ()>> {
            self.cancellations.fetch_add(1, AtomicOrdering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }

    #[derive(Debug)]
    struct MockSubscriptionClient {
        queries: Mutex<Vec<RelaySubscriptionQuery>>,
        items: Mutex<Option<VecDeque<ScriptedItem>>>,
        cancellations: Arc<AtomicUsize>,
        fail: AtomicBool,
    }

    impl MockSubscriptionClient {
        fn new(items: impl IntoIterator<Item = ScriptedItem>) -> Self {
            Self {
                queries: Mutex::new(Vec::new()),
                items: Mutex::new(Some(items.into_iter().collect())),
                cancellations: Arc::new(AtomicUsize::new(0)),
                fail: AtomicBool::new(false),
            }
        }

        fn failing() -> Self {
            let client = Self::new([]);
            client.fail.store(true, AtomicOrdering::SeqCst);
            client
        }

        fn query(&self) -> RelaySubscriptionQuery {
            self.queries.lock().expect("queries")[0].clone()
        }
    }

    impl RelaySubscriptionClient for MockSubscriptionClient {
        fn subscribe(
            &self,
            query: RelaySubscriptionQuery,
        ) -> BoxFuture<'_, Result<Box<dyn RelaySubscriptionSession>, ()>> {
            self.queries.lock().expect("queries").push(query);
            if self.fail.load(AtomicOrdering::SeqCst) {
                return Box::pin(async { Err(()) });
            }
            let items = self.items.lock().expect("items").take().unwrap_or_default();
            let session = MockSubscriptionSession {
                items,
                cancellations: Arc::clone(&self.cancellations),
            };
            Box::pin(async move { Ok(Box::new(session) as Box<dyn RelaySubscriptionSession>) })
        }
    }

    fn configured(relays: &[&str]) -> Config {
        let endpoints = relays
            .iter()
            .map(|relay| {
                RelayEndpoint::new(relay, RelayUrlPolicy::Public, RelayAccess::ReadWrite)
                    .expect("endpoint")
            })
            .collect::<Vec<_>>();
        Config::from_profile(
            RelayProfile::explicit(RelayProfileKind::Public, endpoints).expect("profile"),
        )
    }

    fn target_set(relays: &[&str]) -> TargetSet {
        TargetSet::new(
            relays
                .iter()
                .map(|relay| Target::nostr_relay(relay).expect("target"))
                .collect::<Vec<_>>(),
        )
        .expect("target set")
    }

    fn request(relays: &[&str], event_limit: u16) -> SubscriptionRequest {
        SubscriptionRequest::new(
            "nostr-live",
            target_set(relays),
            SubscriptionBounds::new(event_limit, unix_time_ms() + 60_000).expect("bounds"),
        )
        .expect("request")
    }

    fn signed_event(content: &str, created_at: u64) -> String {
        EventBuilder::text_note(content)
            .custom_created_at(Timestamp::from_secs(created_at))
            .sign_with_keys(&Keys::parse(FIXTURE_SECRET_KEY).expect("fixture keys"))
            .expect("signed event")
            .as_json()
    }

    fn event_id(raw: &str) -> String {
        radroots_event_codec::decode::signed_event(raw)
            .expect("signed event")
            .id_str()
            .to_owned()
    }

    #[tokio::test]
    async fn subscription_translates_targets_selector_and_unique_ids() {
        let relay = "wss://one.example";
        let client = Arc::new(MockSubscriptionClient::new([ScriptedItem::Item(
            RelaySubscriptionItem::Shutdown,
        )]));
        let transport =
            NostrTransport::with_subscription_client(configured(&[relay]), client.clone());
        let selector = FetchSelector::all()
            .with_kinds(vec![1])
            .expect("kind")
            .with_authors(vec![
                *radroots_event_codec::decode::signed_event(&signed_event(
                    "subscription-author",
                    1_800_000_000,
                ))
                .expect("signed author event")
                .pubkey(),
            ])
            .expect("author")
            .with_exact_tag_value('d', "trade-1")
            .expect("tag")
            .with_since_unix_seconds(1_700_000_000)
            .expect("since")
            .with_until_unix_seconds(1_800_000_000)
            .expect("until");
        let first = transport
            .subscribe(request(&[relay], 2).with_selector(selector.clone()))
            .await
            .expect("first subscription");
        drop(first);
        let second = transport
            .subscribe(request(&[relay], 2).with_selector(selector.clone()))
            .await
            .expect("second subscription");
        drop(second);

        let queries = client.queries.lock().expect("queries");
        assert_eq!(queries.len(), 2);
        assert_ne!(queries[0].id, queries[1].id);
        assert_eq!(queries[0].targets.len(), 1);
        assert_eq!(queries[0].targets[0].relay.as_str(), relay);
        assert_eq!(
            queries[0].targets[0].since_unix_seconds,
            Some(1_700_000_000)
        );
        assert_eq!(queries[0].selector, selector);
        assert!(queries[0].connect_timeout <= queries[0].timeout);

        let filter = subscription_filter(&queries[0].selector, Some(1_700_000_000))
            .expect("filter")
            .as_json();
        assert!(filter.contains("\"kinds\":[1]"));
        assert!(filter.contains("\"#d\":[\"trade-1\"]"));
        assert!(filter.contains("\"since\":1700000000"));
        assert!(filter.contains("\"until\":1800000000"));
    }

    #[tokio::test]
    async fn reconnect_checkpoint_is_equal_timestamp_safe_and_request_bound() {
        let relay = "wss://one.example";
        let mut events = [
            signed_event("equal-a", 1_800_000_000),
            signed_event("equal-b", 1_800_000_000),
            signed_event("equal-c", 1_800_000_000),
        ];
        events.sort_by_key(|event| event_id(event));
        let base_request = request(&[relay], 3);
        let target = base_request.target_set().targets()[0].fingerprint().clone();
        let middle_cursor = RelayCursor::new(1_800_000_000, event_id(&events[1])).expect("cursor");
        let checkpoint = SubscriptionCheckpoint::new(
            target,
            encode_cursor(
                &base_request,
                base_request.target_set().targets()[0].fingerprint(),
                &middle_cursor,
            )
            .expect("opaque cursor"),
        );
        let resumed = base_request
            .with_checkpoints([checkpoint])
            .expect("checkpointed request");
        let relay_url = RelayUrl::parse(relay, RelayUrlPolicy::Public).expect("relay");
        let older = signed_event("older", 1_799_999_999);
        let later = signed_event("later", 1_800_000_001);
        let client = Arc::new(MockSubscriptionClient::new([
            ScriptedItem::Item(RelaySubscriptionItem::Event {
                relay: relay_url.clone(),
                raw: older,
            }),
            ScriptedItem::Item(RelaySubscriptionItem::Event {
                relay: relay_url.clone(),
                raw: events[0].clone(),
            }),
            ScriptedItem::Item(RelaySubscriptionItem::Event {
                relay: relay_url.clone(),
                raw: events[1].clone(),
            }),
            ScriptedItem::Item(RelaySubscriptionItem::Event {
                relay: relay_url.clone(),
                raw: events[2].clone(),
            }),
            ScriptedItem::Item(RelaySubscriptionItem::Event {
                relay: relay_url,
                raw: later.clone(),
            }),
        ]));
        let transport =
            NostrTransport::with_subscription_client(configured(&[relay]), client.clone());
        let mut subscription = transport.subscribe(resumed).await.expect("subscription");
        let SubscriptionNext::Event(event) = subscription.next().await.expect("next event") else {
            panic!("event expected");
        };
        assert_eq!(event.observed().event().id_str(), event_id(&events[0]));
        assert_eq!(
            event.checkpoint().cursor().as_str().split(':').nth(2),
            Some(event_id(&events[1]).as_str())
        );
        let SubscriptionNext::Event(event) = subscription.next().await.expect("next event") else {
            panic!("event expected");
        };
        assert_eq!(event.observed().event().id_str(), event_id(&events[2]));
        assert_eq!(
            event.checkpoint().cursor().as_str().split(':').nth(2),
            Some(event_id(&events[2]).as_str())
        );
        let SubscriptionNext::Event(event) = subscription.next().await.expect("next event") else {
            panic!("event expected");
        };
        assert_eq!(event.observed().event().id_str(), event_id(&later));
        assert_eq!(
            client.query().targets[0].since_unix_seconds,
            Some(1_800_000_000)
        );
    }

    #[tokio::test]
    async fn live_subscription_accepts_same_second_events_in_relay_arrival_order() {
        let relay = "wss://one.example";
        let relay_url = RelayUrl::parse(relay, RelayUrlPolicy::Public).expect("relay");
        let mut events = [
            signed_event("same-second-a", 1_800_000_000),
            signed_event("same-second-b", 1_800_000_000),
        ];
        events.sort_by_key(|event| event_id(event));
        let client = Arc::new(MockSubscriptionClient::new([
            ScriptedItem::Item(RelaySubscriptionItem::Event {
                relay: relay_url.clone(),
                raw: events[1].clone(),
            }),
            ScriptedItem::Item(RelaySubscriptionItem::Event {
                relay: relay_url.clone(),
                raw: events[1].clone(),
            }),
            ScriptedItem::Item(RelaySubscriptionItem::Event {
                relay: relay_url,
                raw: events[0].clone(),
            }),
        ]));
        let transport = NostrTransport::with_subscription_client(configured(&[relay]), client);
        let mut subscription = transport
            .subscribe(request(&[relay], 2))
            .await
            .expect("subscription");

        for expected in [&events[1], &events[0]] {
            let SubscriptionNext::Event(event) = subscription.next().await.expect("next event")
            else {
                panic!("event expected");
            };
            assert_eq!(event.observed().event().id_str(), event_id(expected));
            assert_eq!(
                event.checkpoint().cursor().as_str().split(':').nth(2),
                Some(event_id(&events[1]).as_str())
            );
        }
    }

    #[tokio::test]
    async fn malformed_or_mismatched_checkpoint_fails_before_backend_work() {
        let relay = "wss://one.example";
        let client = Arc::new(MockSubscriptionClient::new([]));
        let transport =
            NostrTransport::with_subscription_client(configured(&[relay]), client.clone());
        let base = request(&[relay], 1);
        let target = base.target_set().targets()[0].fingerprint().clone();
        let malformed = base
            .clone()
            .with_checkpoints([SubscriptionCheckpoint::new(
                target.clone(),
                FetchCursor::parse("not-a-live-cursor").expect("opaque cursor"),
            )])
            .expect("request checkpoint");
        assert_eq!(
            transport.subscribe(malformed).await.err(),
            Some(radroots_transport::Error::InvalidFetchCursor)
        );

        let other_selector = FetchSelector::all()
            .with_exact_tag_value('d', "trade-1")
            .expect("selector");
        let scoped = encode_cursor(
            &base,
            &target,
            &RelayCursor::new(10, "a".repeat(64)).expect("cursor"),
        )
        .expect("scoped cursor");
        for malformed_cursor in [
            FetchCursor::parse("nostr-live-v1:10").expect("opaque missing-field cursor"),
            FetchCursor::parse(format!("{}:extra", scoped.as_str()))
                .expect("opaque extra-field cursor"),
        ] {
            let checkpoint = SubscriptionCheckpoint::new(target.clone(), malformed_cursor);
            assert_eq!(
                parse_cursor(&base, &checkpoint),
                Err(radroots_transport::Error::InvalidFetchCursor)
            );
        }
        let mismatched = base
            .with_selector(other_selector)
            .with_checkpoints([SubscriptionCheckpoint::new(target, scoped)])
            .expect("request checkpoint");
        assert_eq!(
            transport.subscribe(mismatched).await.err(),
            Some(radroots_transport::Error::InvalidFetchCursor)
        );
        assert!(client.queries.lock().expect("queries").is_empty());
    }

    #[test]
    fn dropping_an_unpolled_subscription_start_performs_no_backend_work() {
        let relay = "wss://one.example";
        let client = Arc::new(MockSubscriptionClient::new([]));
        let transport =
            NostrTransport::with_subscription_client(configured(&[relay]), client.clone());
        let future = transport.subscribe(request(&[relay], 1));
        drop(future);
        assert!(client.queries.lock().expect("queries").is_empty());
    }

    #[tokio::test]
    async fn event_limit_and_explicit_cancellation_are_stable_and_idempotent() {
        let relay = "wss://one.example";
        let relay_url = RelayUrl::parse(relay, RelayUrlPolicy::Public).expect("relay");
        let client = Arc::new(MockSubscriptionClient::new([ScriptedItem::Item(
            RelaySubscriptionItem::Event {
                relay: relay_url,
                raw: signed_event("bounded", 1_800_000_001),
            },
        )]));
        let transport =
            NostrTransport::with_subscription_client(configured(&[relay]), client.clone());
        let mut subscription = transport
            .subscribe(request(&[relay], 1))
            .await
            .expect("subscription");
        assert!(matches!(
            subscription.next().await.expect("event"),
            SubscriptionNext::Event(_)
        ));
        let SubscriptionNext::End(limit) = subscription.next().await.expect("limit") else {
            panic!("terminal expected");
        };
        assert_eq!(limit.reason(), SubscriptionEndReason::EventLimit);
        assert_eq!(limit.event_count(), 1);
        assert_eq!(limit.checkpoints().len(), 1);
        assert_eq!(subscription.cancel().await.expect("stable cancel"), limit);
        assert_eq!(client.cancellations.load(AtomicOrdering::SeqCst), 1);

        let cancel_client = Arc::new(MockSubscriptionClient::new([]));
        let cancel_transport =
            NostrTransport::with_subscription_client(configured(&[relay]), cancel_client.clone());
        let mut cancelled = cancel_transport
            .subscribe(request(&[relay], 2))
            .await
            .expect("subscription");
        let terminal = cancelled.cancel().await.expect("cancel");
        assert_eq!(terminal.reason(), SubscriptionEndReason::Cancelled);
        assert_eq!(cancelled.cancel().await.expect("repeat cancel"), terminal);
        assert_eq!(cancel_client.cancellations.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dropping_pending_next_requests_cancellation_on_the_next_call() {
        let relay = "wss://one.example";
        let client = Arc::new(MockSubscriptionClient::new([]));
        let transport =
            NostrTransport::with_subscription_client(configured(&[relay]), client.clone());
        let mut subscription = transport
            .subscribe(request(&[relay], 2))
            .await
            .expect("subscription");
        let mut pending = subscription.next();
        assert!(futures::poll!(pending.as_mut()).is_pending());
        drop(pending);
        let SubscriptionNext::End(terminal) = subscription.next().await.expect("cancelled") else {
            panic!("terminal expected");
        };
        assert_eq!(terminal.reason(), SubscriptionEndReason::Cancelled);
        assert_eq!(client.cancellations.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_admitted_subscription_that_expires_before_next_is_deadline_bounded() {
        let relay = "wss://one.example";
        let client = Arc::new(MockSubscriptionClient::new([]));
        let transport = NostrTransport::with_subscription_client(configured(&[relay]), client);
        let request = SubscriptionRequest::new(
            "expires-after-admission",
            target_set(&[relay]),
            SubscriptionBounds::new(1, unix_time_ms() + 50).expect("bounds"),
        )
        .expect("request");
        let mut subscription = transport.subscribe(request).await.expect("subscription");
        tokio::time::sleep(Duration::from_millis(75)).await;

        let SubscriptionNext::End(terminal) = subscription.next().await.expect("deadline") else {
            panic!("terminal expected");
        };
        assert_eq!(terminal.reason(), SubscriptionEndReason::Deadline);
    }

    #[tokio::test]
    async fn expired_unrepresentable_closed_and_failed_sources_are_bounded() {
        let relay = "wss://one.example";
        let client = Arc::new(MockSubscriptionClient::new([]));
        let transport =
            NostrTransport::with_subscription_client(configured(&[relay]), client.clone());
        let expired = SubscriptionRequest::new(
            "expired",
            target_set(&[relay]),
            SubscriptionBounds::new(1, 1).expect("bounds"),
        )
        .expect("request");
        let mut expired = transport
            .subscribe(expired)
            .await
            .expect("expired capability");
        let SubscriptionNext::End(terminal) = expired.next().await.expect("deadline") else {
            panic!("terminal expected");
        };
        assert_eq!(terminal.reason(), SubscriptionEndReason::Deadline);

        let unsupported = request(&[relay], 1).with_selector(
            FetchSelector::all()
                .with_kinds(vec![u32::MAX])
                .expect("selector"),
        );
        let mut unsupported = transport
            .subscribe(unsupported)
            .await
            .expect("closed capability");
        let SubscriptionNext::End(terminal) = unsupported.next().await.expect("source closed")
        else {
            panic!("terminal expected");
        };
        assert_eq!(terminal.reason(), SubscriptionEndReason::SourceClosed);
        assert!(client.queries.lock().expect("queries").is_empty());

        let deadline_request = SubscriptionRequest::new(
            "pending-deadline",
            target_set(&[relay]),
            SubscriptionBounds::new(1, unix_time_ms() + 100).expect("bounds"),
        )
        .expect("request");
        let mut deadline = transport
            .subscribe(deadline_request)
            .await
            .expect("deadline capability");
        let SubscriptionNext::End(terminal) = deadline.next().await.expect("deadline") else {
            panic!("terminal expected");
        };
        assert_eq!(terminal.reason(), SubscriptionEndReason::Deadline);
        assert_eq!(client.cancellations.load(AtomicOrdering::SeqCst), 0);

        let failed = Arc::new(MockSubscriptionClient::failing());
        let failed_transport =
            NostrTransport::with_subscription_client(configured(&[relay]), failed.clone());
        assert_eq!(
            failed_transport.subscribe(request(&[relay], 1)).await.err(),
            Some(radroots_transport::Error::SubscriptionUnavailable)
        );
        assert_eq!(failed.queries.lock().expect("queries").len(), 1);
    }

    #[tokio::test]
    async fn source_closure_backend_failure_and_event_admission_are_explicit() {
        let relays = ["wss://one.example", "wss://two.example"];
        let one = RelayUrl::parse(relays[0], RelayUrlPolicy::Public).expect("one");
        let two = RelayUrl::parse(relays[1], RelayUrlPolicy::Public).expect("two");
        let client = Arc::new(MockSubscriptionClient::new([
            ScriptedItem::Item(RelaySubscriptionItem::Closed { relay: one }),
            ScriptedItem::Item(RelaySubscriptionItem::Closed { relay: two }),
        ]));
        let transport =
            NostrTransport::with_subscription_client(configured(&relays), client.clone());
        let mut subscription = transport
            .subscribe(request(&relays, 2))
            .await
            .expect("subscription");
        let SubscriptionNext::End(terminal) = subscription.next().await.expect("closed") else {
            panic!("terminal expected");
        };
        assert_eq!(terminal.reason(), SubscriptionEndReason::SourceClosed);
        assert_eq!(client.cancellations.load(AtomicOrdering::SeqCst), 0);

        let unexpected_close = Arc::new(MockSubscriptionClient::new([ScriptedItem::Item(
            RelaySubscriptionItem::Closed {
                relay: RelayUrl::parse("wss://unexpected.example", RelayUrlPolicy::Public)
                    .expect("unexpected relay"),
            },
        )]));
        let unexpected_transport =
            NostrTransport::with_subscription_client(configured(&[relays[0]]), unexpected_close);
        let mut unexpected = unexpected_transport
            .subscribe(request(&[relays[0]], 1))
            .await
            .expect("subscription");
        assert_eq!(
            unexpected.next().await,
            Err(radroots_transport::Error::SubscriptionUnavailable)
        );

        let error_client = Arc::new(MockSubscriptionClient::new([ScriptedItem::Error]));
        let error_transport =
            NostrTransport::with_subscription_client(configured(&[relays[0]]), error_client);
        let mut subscription = error_transport
            .subscribe(request(&[relays[0]], 1))
            .await
            .expect("subscription");
        assert_eq!(
            subscription.next().await,
            Err(radroots_transport::Error::SubscriptionUnavailable)
        );

        let mismatch_client = Arc::new(MockSubscriptionClient::new([ScriptedItem::Item(
            RelaySubscriptionItem::Event {
                relay: RelayUrl::parse(relays[0], RelayUrlPolicy::Public).expect("relay"),
                raw: signed_event("wrong-kind", 1_800_000_002),
            },
        )]));
        let mismatch_transport =
            NostrTransport::with_subscription_client(configured(&[relays[0]]), mismatch_client);
        let selector = FetchSelector::all().with_kinds(vec![2]).expect("selector");
        let mut subscription = mismatch_transport
            .subscribe(request(&[relays[0]], 1).with_selector(selector))
            .await
            .expect("subscription");
        assert_eq!(
            subscription.next().await,
            Err(radroots_transport::Error::UnexpectedSubscriptionEvent)
        );
    }
}
