//! Relay-independent client state and the host transport SPI.

use crate::error::RadrootsNostrConnectError;
use crate::message::{
    RPC_KIND, Request, RequestId, RequestMessage, Response, ResponseEnvelope, ResponseValidator,
};
use crate::method::Method;
use crate::uri::{RELAY_COUNT_MAX, RelayUrl as ConnectRelayUrl};
use nostr::nips::nip44::{self, Version};
use nostr::{Event, EventBuilder, JsonUtil, Keys, Kind, PublicKey, RelayUrl, SecretKey, Tag};
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub const CLIENT_EVENT_MAX_BYTES: usize = 524_288;

/// A relay-independent NIP-46 client with single-owner session key material.
///
/// The client owns request encryption, event signing, response selection, and
/// protocol state. A host-provided [`Transport`] owns publication, waiting,
/// timeout policy, and cancellation wakeups. Dropping an execution future after
/// publication does not retract the remote request; use [`CancellationToken`]
/// so the resulting [`CancellationPhase`] is observed explicitly.
pub struct Client {
    keys: Keys,
    target: Target,
    target_nostr_public_key: PublicKey,
}

impl Client {
    /// Generates fresh single-owner client key material for this session.
    pub fn generate(target: Target) -> Result<Self, RadrootsNostrConnectError> {
        Self::from_keys(Keys::generate(), target)
    }

    /// Builds a client from a persisted hexadecimal or NIP-19 secret.
    ///
    /// Invalid inputs are normalized and never retained in diagnostics.
    pub fn from_secret(secret: &str, target: Target) -> Result<Self, RadrootsNostrConnectError> {
        let secret =
            SecretKey::parse(secret).map_err(|_| RadrootsNostrConnectError::InvalidClientKey)?;
        Self::from_keys(Keys::new(secret), target)
    }

    fn from_keys(keys: Keys, target: Target) -> Result<Self, RadrootsNostrConnectError> {
        let target_nostr_public_key = radroots_nostr::key::public_key_to_nostr(
            target.remote_signer_public_key,
        )
        .map_err(|_| RadrootsNostrConnectError::InvalidClientTarget {
            reason: "remote signer public key is not a valid Nostr key",
        })?;
        Ok(Self {
            keys,
            target,
            target_nostr_public_key,
        })
    }

    /// Returns the public identity of this client session.
    pub fn public_key(&self) -> Result<radroots_identity::PublicKey, RadrootsNostrConnectError> {
        radroots_nostr::key::public_key_from_nostr(self.keys.public_key()).map_err(|_| {
            RadrootsNostrConnectError::InvalidClientState {
                reason: "client public key is invalid",
            }
        })
    }

    #[must_use]
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// Constructs a validated, encrypted request in the prepared state.
    pub fn prepare(
        &self,
        request_id: RequestId,
        request: Request,
    ) -> Result<Operation<'_>, RadrootsNostrConnectError> {
        let method = request.method();
        let message = RequestMessage::try_new(request_id.to_string(), request)?;
        let event = build_request_event_for(&self.keys, self.target_nostr_public_key, message)?;
        Ok(Operation {
            client: self,
            request_id: request_id.clone(),
            method,
            publication: ClientEvent(event),
            phase: OperationPhase::Prepared,
            validator: ResponseValidator::new(request_id, self.target.remote_signer_public_key),
        })
    }

    /// Publishes and drives one request to response or explicit cancellation.
    ///
    /// Timeout policy belongs to `transport`, which reports [`Receive::TimedOut`].
    /// The transport must observe `cancellation` while waiting and return
    /// [`Receive::Cancelled`] promptly when cancellation wins its host-level
    /// wait. Cancellation, progress-observer errors, or future drops after
    /// publication stop local waiting without retracting signer-side work.
    pub async fn execute<T, F>(
        &self,
        request_id: RequestId,
        request: Request,
        transport: &mut T,
        cancellation: &CancellationToken,
        mut on_progress: F,
    ) -> Result<Completion, RadrootsNostrConnectError>
    where
        T: Transport + ?Sized,
        F: FnMut(Progress) -> Result<(), RadrootsNostrConnectError>,
    {
        let mut operation = self.prepare(request_id, request)?;
        if cancellation.is_cancelled() {
            return operation.cancel().map(Completion::Cancelled);
        }

        transport.publish(operation.publication()?.clone()).await?;
        operation.mark_published()?;
        if cancellation.is_cancelled() {
            return operation.cancel().map(Completion::Cancelled);
        }

        loop {
            match transport.receive(cancellation).await? {
                Receive::Event(event) => match operation.select(&event)? {
                    EventOutcome::Ignore => {}
                    EventOutcome::Progress(progress) => on_progress(progress)?,
                    EventOutcome::Complete(response) => {
                        return Ok(Completion::Response(response));
                    }
                },
                Receive::TimedOut => return Err(RadrootsNostrConnectError::RequestTimedOut),
                Receive::Cancelled => return operation.cancel().map(Completion::Cancelled),
            }
        }
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Client")
            .field("key_material", &"<redacted>")
            .field("target", &self.target)
            .finish()
    }
}

/// A validated remote-signer target independent of relay implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    remote_signer_public_key: radroots_identity::PublicKey,
    relays: Vec<ConnectRelayUrl>,
}

impl Target {
    pub fn try_new(
        remote_signer_public_key: radroots_identity::PublicKey,
        relays: Vec<ConnectRelayUrl>,
    ) -> Result<Self, RadrootsNostrConnectError> {
        if relays.len() > RELAY_COUNT_MAX {
            return Err(RadrootsNostrConnectError::InvalidClientTarget {
                reason: "relay count exceeds its limit",
            });
        }
        let mut normalized = Vec::with_capacity(relays.len());
        for relay in relays {
            if !normalized.contains(&relay) {
                normalized.push(relay);
            }
        }
        Ok(Self {
            remote_signer_public_key,
            relays: normalized,
        })
    }

    #[must_use]
    pub const fn remote_signer_public_key(&self) -> radroots_identity::PublicKey {
        self.remote_signer_public_key
    }

    #[must_use]
    pub fn relays(&self) -> &[ConnectRelayUrl] {
        &self.relays
    }
}

/// A signed NIP-46 protocol event with a package-owned representation.
#[derive(Clone, PartialEq, Eq)]
pub struct ClientEvent(Event);

impl ClientEvent {
    pub fn from_json(value: &str) -> Result<Self, RadrootsNostrConnectError> {
        if value.len() > CLIENT_EVENT_MAX_BYTES {
            return Err(RadrootsNostrConnectError::InvalidClientEvent);
        }
        Event::from_json(value)
            .map(Self)
            .map_err(|_| RadrootsNostrConnectError::InvalidClientEvent)
    }

    #[must_use]
    pub fn as_json(&self) -> String {
        self.0.as_json()
    }
}

impl std::fmt::Debug for ClientEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ClientEvent(<redacted>)")
    }
}

#[derive(PartialEq, Eq)]
pub enum Progress {
    AuthChallenge { url: String },
}

impl std::fmt::Debug for Progress {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Progress::AuthChallenge(<redacted>)")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EventOutcome {
    Ignore,
    Progress(Progress),
    Complete(Box<Response>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Completion {
    Response(Box<Response>),
    Cancelled(CancellationPhase),
}

impl Completion {
    #[must_use]
    pub fn response(response: Response) -> Self {
        Self::Response(Box::new(response))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationPhase {
    BeforePublication,
    AfterPublication,
}

/// A cloneable host cancellation signal containing no runtime dependency.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Receive {
    Event(Box<ClientEvent>),
    TimedOut,
    Cancelled,
}

impl Receive {
    #[must_use]
    pub fn event(event: ClientEvent) -> Self {
        Self::Event(Box::new(event))
    }
}

pub type TransportFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RadrootsNostrConnectError>> + Send + 'a>>;

/// Host SPI for publishing and receiving package-owned NIP-46 events.
///
/// External implementations are supported. The trait is dyn-compatible,
/// `Send`, and returns `Send` futures. Implementations normalize backend
/// failures into [`RadrootsNostrConnectError::Transport`]. The host owns
/// deadlines and reports them as [`Receive::TimedOut`]. A successful `publish`
/// may durably expose the request to a remote signer. Once that call returns,
/// cancellation only stops local waiting and cannot retract remote work.
/// `receive` must observe its token and return [`Receive::Cancelled`] when host
/// cancellation wins.
pub trait Transport: Send {
    fn publish<'a>(&'a mut self, event: ClientEvent) -> TransportFuture<'a, ()>;

    fn receive<'a>(
        &'a mut self,
        cancellation: &'a CancellationToken,
    ) -> TransportFuture<'a, Receive>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationPhase {
    Prepared,
    Published,
    Completed,
    Cancelled,
}

/// One request's explicit prepared/published/completed state machine.
pub struct Operation<'a> {
    client: &'a Client,
    request_id: RequestId,
    method: Method,
    publication: ClientEvent,
    phase: OperationPhase,
    validator: ResponseValidator,
}

impl Operation<'_> {
    pub fn publication(&self) -> Result<&ClientEvent, RadrootsNostrConnectError> {
        if self.phase != OperationPhase::Prepared {
            return Err(RadrootsNostrConnectError::InvalidClientState {
                reason: "publication is only available while prepared",
            });
        }
        Ok(&self.publication)
    }

    pub fn mark_published(&mut self) -> Result<(), RadrootsNostrConnectError> {
        if self.phase != OperationPhase::Prepared {
            return Err(RadrootsNostrConnectError::InvalidClientState {
                reason: "only a prepared request can be marked published",
            });
        }
        self.phase = OperationPhase::Published;
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<CancellationPhase, RadrootsNostrConnectError> {
        let cancellation = match self.phase {
            OperationPhase::Prepared => CancellationPhase::BeforePublication,
            OperationPhase::Published => CancellationPhase::AfterPublication,
            OperationPhase::Completed => {
                return Err(RadrootsNostrConnectError::InvalidClientState {
                    reason: "completed request cannot be cancelled",
                });
            }
            OperationPhase::Cancelled => {
                return Err(RadrootsNostrConnectError::InvalidClientState {
                    reason: "request is already cancelled",
                });
            }
        };
        self.phase = OperationPhase::Cancelled;
        Ok(cancellation)
    }

    pub fn select(
        &mut self,
        event: &ClientEvent,
    ) -> Result<EventOutcome, RadrootsNostrConnectError> {
        if matches!(
            self.phase,
            OperationPhase::Prepared | OperationPhase::Cancelled
        ) {
            return Err(RadrootsNostrConnectError::InvalidClientState {
                reason: "responses require a published active request",
            });
        }

        let event = &event.0;
        if event.kind != Kind::Custom(RPC_KIND)
            || event.pubkey != self.client.target_nostr_public_key
            || !event
                .tags
                .public_keys()
                .any(|public_key| *public_key == self.client.keys.public_key())
        {
            return Ok(EventOutcome::Ignore);
        }
        event
            .verify()
            .map_err(|_| RadrootsNostrConnectError::InvalidClientEvent)?;

        let decrypted = nip44::decrypt(
            self.client.keys.secret_key(),
            &self.client.target_nostr_public_key,
            &event.content,
        )
        .map_err(|error| RadrootsNostrConnectError::Decrypt {
            reason: error.to_string(),
        })?;
        let envelope: ResponseEnvelope =
            serde_json::from_str(&decrypted).map_err(RadrootsNostrConnectError::from)?;
        if envelope.request_id()? != self.request_id {
            return Ok(EventOutcome::Ignore);
        }
        self.validator.validate(
            self.client.target.remote_signer_public_key,
            event.id.to_hex(),
            &envelope,
        )?;
        if self.phase == OperationPhase::Completed {
            return Err(RadrootsNostrConnectError::InvalidClientState {
                reason: "request already completed",
            });
        }

        match Response::from_envelope(&self.method, envelope)? {
            Response::AuthUrl(url) => Ok(EventOutcome::Progress(Progress::AuthChallenge { url })),
            response => {
                self.phase = OperationPhase::Completed;
                Ok(EventOutcome::Complete(Box::new(response)))
            }
        }
    }
}

impl std::fmt::Debug for Operation<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Operation")
            .field("request_id", &self.request_id)
            .field("method", &self.method)
            .field("phase", &self.phase)
            .field("publication", &"<redacted>")
            .finish()
    }
}

fn build_request_event_for(
    client_keys: &Keys,
    remote_signer_public_key: PublicKey,
    message: RequestMessage,
) -> Result<Event, RadrootsNostrConnectError> {
    let payload = serde_json::to_string(&message).map_err(RadrootsNostrConnectError::from)?;
    let ciphertext = nip44::encrypt(
        client_keys.secret_key(),
        &remote_signer_public_key,
        payload,
        Version::V2,
    )
    .map_err(encrypt_error)?;

    EventBuilder::new(Kind::Custom(RPC_KIND), ciphertext)
        .tag(Tag::public_key(remote_signer_public_key))
        .sign_with_keys(client_keys)
        .map_err(sign_error)
}

#[doc(hidden)]
pub type RadrootsNostrConnectClientTransportFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RadrootsNostrConnectError>> + Send + 'a>>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsNostrConnectClientTarget {
    pub remote_signer_public_key: PublicKey,
    pub relays: Vec<RelayUrl>,
}

#[doc(hidden)]
impl RadrootsNostrConnectClientTarget {
    pub fn new(remote_signer_public_key: PublicKey, relays: Vec<RelayUrl>) -> Self {
        Self {
            remote_signer_public_key,
            relays,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsNostrConnectClientRequest {
    pub request_id: String,
    pub request: Request,
}

#[doc(hidden)]
impl RadrootsNostrConnectClientRequest {
    pub fn new(request_id: impl Into<String>, request: Request) -> Self {
        Self {
            request_id: request_id.into(),
            request,
        }
    }

    pub fn method(&self) -> Method {
        self.request.method()
    }

    pub fn into_message(self) -> RequestMessage {
        RequestMessage::new(self.request_id, self.request)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectClientProgress {
    AuthChallenge { url: String },
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectClientEventOutcome {
    Ignore,
    Progress(RadrootsNostrConnectClientProgress),
    Response(Response),
}

#[doc(hidden)]
pub trait RadrootsNostrConnectClientTransport {
    fn publish_request_event<'a>(
        &'a mut self,
        event: Event,
    ) -> RadrootsNostrConnectClientTransportFuture<'a, ()>;

    fn next_response_event<'a>(
        &'a mut self,
    ) -> RadrootsNostrConnectClientTransportFuture<'a, Event>;
}

#[doc(hidden)]
pub fn build_request_event(
    client_keys: &Keys,
    target: &RadrootsNostrConnectClientTarget,
    message: RequestMessage,
) -> Result<Event, RadrootsNostrConnectError> {
    let payload = serde_json::to_string(&message).map_err(RadrootsNostrConnectError::from)?;
    let ciphertext = nip44::encrypt(
        client_keys.secret_key(),
        &target.remote_signer_public_key,
        payload,
        Version::V2,
    )
    .map_err(encrypt_error)?;

    EventBuilder::new(Kind::Custom(RPC_KIND), ciphertext)
        .tag(Tag::public_key(target.remote_signer_public_key))
        .sign_with_keys(client_keys)
        .map_err(sign_error)
}

#[doc(hidden)]
pub fn parse_response_event(
    client_keys: &Keys,
    target: &RadrootsNostrConnectClientTarget,
    request_id: &str,
    method: &Method,
    event: &Event,
) -> Result<RadrootsNostrConnectClientEventOutcome, RadrootsNostrConnectError> {
    if event.kind != Kind::Custom(RPC_KIND) {
        return Ok(RadrootsNostrConnectClientEventOutcome::Ignore);
    }
    if event.pubkey != target.remote_signer_public_key {
        return Ok(RadrootsNostrConnectClientEventOutcome::Ignore);
    }
    let client_public_key = client_keys.public_key();
    if !event
        .tags
        .public_keys()
        .any(|public_key| *public_key == client_public_key)
    {
        return Ok(RadrootsNostrConnectClientEventOutcome::Ignore);
    }
    event
        .verify()
        .map_err(|_| RadrootsNostrConnectError::InvalidClientEvent)?;
    let decrypted = nip44::decrypt(
        client_keys.secret_key(),
        &target.remote_signer_public_key,
        &event.content,
    )
    .map_err(|error| RadrootsNostrConnectError::Decrypt {
        reason: error.to_string(),
    })?;
    let envelope: ResponseEnvelope =
        serde_json::from_str(&decrypted).map_err(RadrootsNostrConnectError::from)?;
    if envelope.id != request_id {
        return Ok(RadrootsNostrConnectClientEventOutcome::Ignore);
    }
    let response = Response::from_envelope(method, envelope)?;
    Ok(match response {
        Response::AuthUrl(url) => RadrootsNostrConnectClientEventOutcome::Progress(
            RadrootsNostrConnectClientProgress::AuthChallenge { url },
        ),
        response => RadrootsNostrConnectClientEventOutcome::Response(response),
    })
}

#[doc(hidden)]
pub async fn execute_request_with_transport<T, F>(
    client_keys: &Keys,
    target: &RadrootsNostrConnectClientTarget,
    request: RadrootsNostrConnectClientRequest,
    transport: &mut T,
    mut on_progress: F,
) -> Result<Response, RadrootsNostrConnectError>
where
    T: RadrootsNostrConnectClientTransport,
    F: FnMut(RadrootsNostrConnectClientProgress) -> Result<(), RadrootsNostrConnectError>,
{
    let method = request.method();
    let request_id = request.request_id.clone();
    let event = build_request_event(client_keys, target, request.into_message())?;
    transport.publish_request_event(event).await?;
    loop {
        let event = transport.next_response_event().await?;
        match parse_response_event(client_keys, target, &request_id, &method, &event)? {
            RadrootsNostrConnectClientEventOutcome::Ignore => {}
            RadrootsNostrConnectClientEventOutcome::Progress(progress) => on_progress(progress)?,
            RadrootsNostrConnectClientEventOutcome::Response(response) => return Ok(response),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn encrypt_error(error: impl ToString) -> RadrootsNostrConnectError {
    RadrootsNostrConnectError::Encrypt {
        reason: error.to_string(),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn sign_error(error: impl ToString) -> RadrootsNostrConnectError {
    RadrootsNostrConnectError::Sign {
        reason: error.to_string(),
    }
}
