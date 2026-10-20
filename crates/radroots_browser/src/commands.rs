use core::num::NonZeroU64;
use radroots_domain::{PublicKey, UnixSeconds};
use radroots_execution::session::{
    HostFuture,
    command::{CommandIo, capture_dm_route, selected_dm_targets},
};
use radroots_nostr::signing::gift_wrap::{randomized_past_time, wrap_retained_seal};
use radroots_protocol::chat::CapturedDmRoute;
use radroots_signing::provider::{NonceSource, ProviderError};
use radroots_storage::command::{EnvelopeCandidate, OperationError, PublicationWork};
use radroots_transport::{
    EventSink, EventSource, TargetSet,
    policy::{SatisfactionClass, SatisfactionPolicy},
    source::{FetchBounds, FetchRequest, FetchSelector},
};
use radroots_transport_nostr::{Config, NostrTransport};
use radroots_transport_nostr_browser::BrowserConnector;
use std::{
    sync::Arc,
    sync::atomic::{AtomicU8, Ordering},
};
use wasm_bindgen::JsCast;

pub(crate) struct BrowserCommands {
    transport: NostrTransport,
    connector: Arc<BrowserConnector>,
    read: TargetSet,
    write: TargetSet,
    policy: SatisfactionPolicy,
    mode: AtomicU8,
}
impl BrowserCommands {
    pub(crate) fn new(config: Config, policy: SatisfactionPolicy) -> Result<Self, OperationError> {
        let read = TargetSet::new(
            config
                .read_relays()
                .map(|relay| relay.to_target())
                .collect::<Result<_, _>>()
                .map_err(|_| OperationError::InvalidCommand)?,
        )
        .map_err(|_| OperationError::InvalidCommand)?;
        let write = TargetSet::new(
            config
                .write_relays()
                .map(|relay| relay.to_target())
                .collect::<Result<_, _>>()
                .map_err(|_| OperationError::InvalidCommand)?,
        )
        .map_err(|_| OperationError::InvalidCommand)?;
        if policy.class() != SatisfactionClass::Accepted {
            return Err(OperationError::InvalidCommand);
        }
        policy
            .validate_for(&write)
            .map_err(|_| OperationError::InvalidCommand)?;
        let connector = Arc::new(BrowserConnector::default());
        Ok(Self {
            transport: NostrTransport::new(config, connector.clone()),
            connector,
            read,
            write,
            policy,
            mode: AtomicU8::new(0),
        })
    }
    pub(crate) fn select(&self, online: bool) -> Result<(), OperationError> {
        let mode = if online { 2 } else { 1 };
        match self
            .mode
            .compare_exchange(0, mode, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(()),
            Err(current) if current == mode => Ok(()),
            Err(_) => Err(OperationError::Unauthorized),
        }
    }
    pub(crate) async fn disconnect(&self) {
        self.transport.shutdown().await;
    }
    pub(crate) async fn dispose(&self) -> Result<(), OperationError> {
        self.connector
            .close()
            .await
            .map_err(|_| OperationError::RecoveryRequired)
    }
    fn capture_route<'a>(
        &self,
        recipient: PublicKey,
        raw_sources: impl IntoIterator<Item = &'a str>,
    ) -> Result<CapturedDmRoute, OperationError> {
        capture_dm_route(recipient, raw_sources, &self.write)
    }
}
impl CommandIo for BrowserCommands {
    fn enabled(&self) -> bool {
        self.mode.load(Ordering::Acquire) == 2
    }
    fn now(&self) -> Result<UnixSeconds, OperationError> {
        unix_seconds()
    }
    fn seal_time(&self, now: UnixSeconds) -> Result<UnixSeconds, OperationError> {
        let time = randomized_past_time(
            NonZeroU64::new(now.get()).ok_or(OperationError::RecoveryRequired)?,
            &BrowserNonceSource,
        )
        .map_err(|_| OperationError::RecoveryRequired)?;
        UnixSeconds::new(time.get()).map_err(|_| OperationError::RecoveryRequired)
    }
    fn route(
        &self,
        recipient: PublicKey,
        deadline: NonZeroU64,
    ) -> HostFuture<'_, Result<CapturedDmRoute, OperationError>> {
        Box::pin(async move {
            if !self.enabled() {
                return Err(OperationError::Unauthorized);
            }
            let selector = FetchSelector::all()
                .with_kinds(vec![10050])
                .and_then(|selector| selector.with_authors(vec![recipient]))
                .map_err(|_| OperationError::InvalidCommand)?;
            let bounds =
                FetchBounds::new(2, deadline.get()).map_err(|_| OperationError::InvalidCommand)?;
            let request = FetchRequest::new(self.delivery_id()?, self.read.clone(), bounds)
                .map_err(|_| OperationError::InvalidCommand)?
                .with_selector(selector);
            let page = self
                .transport
                .fetch(request)
                .await
                .map_err(|_| OperationError::MissingHistory)?;
            self.capture_route(
                recipient,
                page.events()
                    .iter()
                    .map(|observed| observed.event().raw_json()),
            )
        })
    }
    fn targets(&self, route: &CapturedDmRoute) -> Result<TargetSet, OperationError> {
        selected_dm_targets(route, &self.write)
    }
    fn publication_policy(&self) -> SatisfactionPolicy {
        self.policy.clone()
    }
    fn envelope(
        &self,
        work: PublicationWork,
        raw_seal: &str,
    ) -> Result<EnvelopeCandidate, OperationError> {
        let now = NonZeroU64::new(self.now()?.get()).ok_or(OperationError::RecoveryRequired)?;
        let recipient = work.recipient();
        let wrapped = wrap_retained_seal(raw_seal, recipient, now, &BrowserNonceSource)
            .map_err(|_| OperationError::InvalidEvidence)?;
        wrapped.with_wrapping_key(|key| {
            EnvelopeCandidate::from_obtained(work, wrapped.signed_json(), key)
        })
    }
    fn delivery_id(&self) -> Result<String, OperationError> {
        let bytes = BrowserNonceSource
            .next_nonce()
            .map_err(|_| OperationError::RecoveryRequired)?;
        let mut output = String::with_capacity(64);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in bytes {
            output.push(HEX[usize::from(byte >> 4)] as char);
            output.push(HEX[usize::from(byte & 15)] as char);
        }
        Ok(output)
    }
    fn publish(
        &self,
        request: radroots_transport::sink::DeliveryRequest,
        selected: TargetSet,
    ) -> HostFuture<'_, Result<radroots_transport::DeliveryReceipt, radroots_transport::SinkFailure>>
    {
        self.transport.deliver_selected(request, selected)
    }
}

pub(crate) fn unix_seconds() -> Result<UnixSeconds, OperationError> {
    let millis = js_sys::Date::now();
    if !millis.is_finite() || millis < 1000.0 {
        return Err(OperationError::RecoveryRequired);
    }
    UnixSeconds::new((millis / 1000.0) as u64).map_err(|_| OperationError::RecoveryRequired)
}

/// Explicit origin-thread WebCrypto CSPRNG. Construction is inert; failed
/// entropy acquisition never returns a partial nonce or falls back to a clock.
pub struct BrowserNonceSource;
impl NonceSource for BrowserNonceSource {
    fn next_nonce(&self) -> Result<[u8; 32], ProviderError> {
        let crypto = js_sys::Reflect::get(&js_sys::global(), &"crypto".into())
            .map_err(|_| ProviderError::CapabilityUnavailable)?
            .dyn_into::<web_sys::Crypto>()
            .map_err(|_| ProviderError::CapabilityUnavailable)?;
        let mut nonce = [0; 32];
        crypto
            .get_random_values_with_u8_array(&mut nonce)
            .map_err(|_| ProviderError::CapabilityUnavailable)?;
        Ok(nonce)
    }
}
