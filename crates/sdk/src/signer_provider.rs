use crate::RadrootsSdkError;
use nostr::{JsonUtil, Kind, PublicKey, Tag, Tags, Timestamp, UnsignedEvent};
#[cfg(feature = "local-signer")]
use radroots_authority::RadrootsLocalEventSigner;
use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, authorize_actor_for_draft,
    authorize_signer_for_draft, sign_authorized_draft, validate_signed_event_matches_draft,
};
use radroots_event::draft::{RadrootsEventDraft, RadrootsSignedEvent};
use radroots_event::ids::RadrootsPublicKey;
use radroots_event::kinds::{
    KIND_FARM, KIND_LISTING, KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_REQUEST,
};
use radroots_event::wire::RadrootsNip01EventWire;
use radroots_nostr::prelude::{RadrootsNostrEvent, RadrootsNostrKeys};
use radroots_nostr_connect::prelude::{
    RadrootsNostrConnectClientRequest, RadrootsNostrConnectClientTarget,
    RadrootsNostrConnectClientTransport, RadrootsNostrConnectClientTransportFuture,
    RadrootsNostrConnectError, RadrootsNostrConnectMethod, RadrootsNostrConnectPermission,
    RadrootsNostrConnectPermissions, RadrootsNostrConnectRequest, RadrootsNostrConnectResponse,
    execute_request_with_transport,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

pub type RadrootsSdkNip46TransportFuture<'a, T> = RadrootsNostrConnectClientTransportFuture<'a, T>;

pub const RADROOTS_SDK_MYC_NIP46_PRODUCT_SIGN_EVENT_KINDS: [u32; 5] = [
    KIND_FARM,
    KIND_LISTING,
    KIND_ORDER_REQUEST,
    KIND_ORDER_DECISION,
    KIND_ORDER_CANCELLATION,
];
pub const RADROOTS_SDK_MYC_NIP46_DEFAULT_REQUEST_TIMEOUT_MS: u64 = 30_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RadrootsSdkSignerMode {
    #[cfg(feature = "local-signer")]
    LocalKey,
    MycNip46,
}

impl RadrootsSdkSignerMode {
    pub fn as_str(self) -> &'static str {
        match self {
            #[cfg(feature = "local-signer")]
            Self::LocalKey => "local_key",
            Self::MycNip46 => "myc_nip46",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RadrootsSdkSignerState {
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RadrootsSdkSignerStatus {
    pub mode: RadrootsSdkSignerMode,
    pub state: RadrootsSdkSignerState,
    pub signer_pubkey: String,
    pub remote_signer_pubkey: Option<String>,
    pub relay_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RadrootsSdkSignerCapability {
    pub mode: RadrootsSdkSignerMode,
    pub signer_pubkey: String,
    pub remote_signer_pubkey: Option<String>,
    pub relays: Vec<String>,
    pub can_sign_events: bool,
    pub nip46_permissions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RadrootsSdkSignerProgress {
    RequestStarted {
        mode: RadrootsSdkSignerMode,
    },
    AuthChallenge {
        mode: RadrootsSdkSignerMode,
        url: String,
    },
    RequestCompleted {
        mode: RadrootsSdkSignerMode,
    },
}

pub trait RadrootsSdkSignerProgressSink {
    fn on_signer_progress(
        &mut self,
        progress: RadrootsSdkSignerProgress,
    ) -> Result<(), RadrootsSdkError>;
}

impl<F> RadrootsSdkSignerProgressSink for F
where
    F: FnMut(RadrootsSdkSignerProgress) -> Result<(), RadrootsSdkError>,
{
    fn on_signer_progress(
        &mut self,
        progress: RadrootsSdkSignerProgress,
    ) -> Result<(), RadrootsSdkError> {
        self(progress)
    }
}

pub struct RadrootsSdkSignRequest<'a> {
    pub operation_kind: &'a str,
    pub actor: &'a RadrootsActorContext,
    pub frozen_draft: &'a RadrootsEventDraft,
    progress_sink: Option<&'a mut dyn RadrootsSdkSignerProgressSink>,
}

impl<'a> RadrootsSdkSignRequest<'a> {
    pub fn new(
        operation_kind: &'a str,
        actor: &'a RadrootsActorContext,
        frozen_draft: &'a RadrootsEventDraft,
    ) -> Self {
        Self {
            operation_kind,
            actor,
            frozen_draft,
            progress_sink: None,
        }
    }

    pub fn with_progress_sink(
        mut self,
        progress_sink: &'a mut dyn RadrootsSdkSignerProgressSink,
    ) -> Self {
        self.progress_sink = Some(progress_sink);
        self
    }

    fn emit_progress(
        &mut self,
        progress: RadrootsSdkSignerProgress,
    ) -> Result<(), RadrootsSdkError> {
        match self.progress_sink.as_deref_mut() {
            Some(progress_sink) => progress_sink.on_signer_progress(progress),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RadrootsSdkSignReceipt {
    pub operation_kind: String,
    pub mode: RadrootsSdkSignerMode,
    pub signer_pubkey: String,
    pub remote_signer_pubkey: Option<String>,
    pub signed_event_id: String,
    pub signed_event: RadrootsSignedEvent,
}

#[derive(Clone)]
pub enum RadrootsSdkSignerProvider {
    #[cfg(feature = "local-signer")]
    LocalKey(RadrootsSdkLocalKeySigner),
    MycNip46(Box<RadrootsSdkMycNip46Signer>),
}

impl RadrootsSdkSignerProvider {
    pub fn mode(&self) -> RadrootsSdkSignerMode {
        match self {
            #[cfg(feature = "local-signer")]
            Self::LocalKey(_) => RadrootsSdkSignerMode::LocalKey,
            Self::MycNip46(_) => RadrootsSdkSignerMode::MycNip46,
        }
    }

    pub fn status(&self) -> RadrootsSdkSignerStatus {
        match self {
            #[cfg(feature = "local-signer")]
            Self::LocalKey(signer) => signer.status(),
            Self::MycNip46(signer) => signer.status(),
        }
    }

    pub fn capability(&self) -> RadrootsSdkSignerCapability {
        match self {
            #[cfg(feature = "local-signer")]
            Self::LocalKey(signer) => signer.capability(),
            Self::MycNip46(signer) => signer.capability(),
        }
    }

    pub async fn sign(
        &self,
        request: RadrootsSdkSignRequest<'_>,
    ) -> Result<RadrootsSdkSignReceipt, RadrootsSdkError> {
        match self {
            #[cfg(feature = "local-signer")]
            Self::LocalKey(signer) => signer.sign(request).await,
            Self::MycNip46(signer) => signer.sign(request).await,
        }
    }
}

#[cfg(feature = "local-signer")]
#[derive(Clone)]
pub struct RadrootsSdkLocalKeySigner {
    signer: Arc<RadrootsLocalEventSigner>,
    signer_pubkey: String,
}

#[cfg(feature = "local-signer")]
impl RadrootsSdkLocalKeySigner {
    pub fn new(keys: RadrootsNostrKeys) -> Result<Self, RadrootsSdkError> {
        let signer = RadrootsLocalEventSigner::new(keys)?;
        let signer_pubkey = signer.pubkey().as_str().to_owned();
        Ok(Self {
            signer: Arc::new(signer),
            signer_pubkey,
        })
    }

    pub fn status(&self) -> RadrootsSdkSignerStatus {
        RadrootsSdkSignerStatus {
            mode: RadrootsSdkSignerMode::LocalKey,
            state: RadrootsSdkSignerState::Ready,
            signer_pubkey: self.signer_pubkey.clone(),
            remote_signer_pubkey: None,
            relay_count: 0,
        }
    }

    pub fn capability(&self) -> RadrootsSdkSignerCapability {
        RadrootsSdkSignerCapability {
            mode: RadrootsSdkSignerMode::LocalKey,
            signer_pubkey: self.signer_pubkey.clone(),
            remote_signer_pubkey: None,
            relays: Vec::new(),
            can_sign_events: true,
            nip46_permissions: Vec::new(),
        }
    }

    pub async fn sign(
        &self,
        mut request: RadrootsSdkSignRequest<'_>,
    ) -> Result<RadrootsSdkSignReceipt, RadrootsSdkError> {
        request.emit_progress(RadrootsSdkSignerProgress::RequestStarted {
            mode: RadrootsSdkSignerMode::LocalKey,
        })?;
        let signed_event =
            sign_authorized_draft(request.actor, self.signer.as_ref(), request.frozen_draft)?;
        request.emit_progress(RadrootsSdkSignerProgress::RequestCompleted {
            mode: RadrootsSdkSignerMode::LocalKey,
        })?;
        Ok(sign_receipt(
            request.operation_kind,
            RadrootsSdkSignerMode::LocalKey,
            self.signer_pubkey.clone(),
            None,
            signed_event,
        ))
    }
}

pub trait RadrootsSdkNip46Transport: Send + Sync {
    fn publish_request_event<'a>(
        &'a self,
        event: RadrootsNostrEvent,
    ) -> RadrootsSdkNip46TransportFuture<'a, ()>;

    fn next_response_event<'a>(&'a self)
    -> RadrootsSdkNip46TransportFuture<'a, RadrootsNostrEvent>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsSdkMycNip46RequestPolicy {
    request_timeout: Duration,
}

impl RadrootsSdkMycNip46RequestPolicy {
    pub fn new(request_timeout: Duration) -> Result<Self, RadrootsSdkError> {
        if request_timeout.is_zero() {
            return Err(RadrootsSdkError::SignerUnavailable {
                mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
                reason: "myc_nip46 request timeout must be greater than zero".to_owned(),
            });
        }
        Ok(Self { request_timeout })
    }

    pub fn request_timeout(self) -> Duration {
        self.request_timeout
    }
}

impl Default for RadrootsSdkMycNip46RequestPolicy {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_millis(
                RADROOTS_SDK_MYC_NIP46_DEFAULT_REQUEST_TIMEOUT_MS,
            ),
        }
    }
}

#[derive(Clone)]
pub struct RadrootsSdkMycNip46Signer {
    client_keys: RadrootsNostrKeys,
    target: RadrootsNostrConnectClientTarget,
    user_pubkey: RadrootsPublicKey,
    transport: Arc<dyn RadrootsSdkNip46Transport>,
    request_policy: RadrootsSdkMycNip46RequestPolicy,
    request_id_generator: Arc<dyn RadrootsSdkMycNip46RequestIdGenerator>,
}

impl RadrootsSdkMycNip46Signer {
    pub fn new(
        client_keys: RadrootsNostrKeys,
        target: RadrootsNostrConnectClientTarget,
        user_pubkey: impl AsRef<str>,
        transport: Arc<dyn RadrootsSdkNip46Transport>,
    ) -> Result<Self, RadrootsSdkError> {
        Self::new_with_request_policy(
            client_keys,
            target,
            user_pubkey,
            transport,
            RadrootsSdkMycNip46RequestPolicy::default(),
        )
    }

    pub fn new_with_request_policy(
        client_keys: RadrootsNostrKeys,
        target: RadrootsNostrConnectClientTarget,
        user_pubkey: impl AsRef<str>,
        transport: Arc<dyn RadrootsSdkNip46Transport>,
        request_policy: RadrootsSdkMycNip46RequestPolicy,
    ) -> Result<Self, RadrootsSdkError> {
        Self::new_with_request_id_generator(
            client_keys,
            target,
            user_pubkey,
            transport,
            request_policy,
            Arc::new(RadrootsSdkUuidNip46RequestIdGenerator),
        )
    }

    fn new_with_request_id_generator(
        client_keys: RadrootsNostrKeys,
        target: RadrootsNostrConnectClientTarget,
        user_pubkey: impl AsRef<str>,
        transport: Arc<dyn RadrootsSdkNip46Transport>,
        request_policy: RadrootsSdkMycNip46RequestPolicy,
        request_id_generator: Arc<dyn RadrootsSdkMycNip46RequestIdGenerator>,
    ) -> Result<Self, RadrootsSdkError> {
        RadrootsSdkMycNip46RequestPolicy::new(request_policy.request_timeout())?;
        let user_pubkey = RadrootsPublicKey::parse(user_pubkey.as_ref()).map_err(|error| {
            RadrootsSdkError::InvalidRequest {
                message: format!("myc_nip46 user pubkey is invalid: {error}"),
            }
        })?;
        Ok(Self {
            client_keys,
            target,
            user_pubkey,
            transport,
            request_policy,
            request_id_generator,
        })
    }

    pub fn status(&self) -> RadrootsSdkSignerStatus {
        RadrootsSdkSignerStatus {
            mode: RadrootsSdkSignerMode::MycNip46,
            state: RadrootsSdkSignerState::Ready,
            signer_pubkey: self.user_pubkey.as_str().to_owned(),
            remote_signer_pubkey: Some(self.target.remote_signer_public_key.to_hex()),
            relay_count: self.target.relays.len(),
        }
    }

    pub fn capability(&self) -> RadrootsSdkSignerCapability {
        RadrootsSdkSignerCapability {
            mode: RadrootsSdkSignerMode::MycNip46,
            signer_pubkey: self.user_pubkey.as_str().to_owned(),
            remote_signer_pubkey: Some(self.target.remote_signer_public_key.to_hex()),
            relays: self.target.relays.iter().map(ToString::to_string).collect(),
            can_sign_events: true,
            nip46_permissions: radroots_sdk_myc_nip46_product_permission_strings(),
        }
    }

    pub async fn sign(
        &self,
        mut request: RadrootsSdkSignRequest<'_>,
    ) -> Result<RadrootsSdkSignReceipt, RadrootsSdkError> {
        request.emit_progress(RadrootsSdkSignerProgress::RequestStarted {
            mode: RadrootsSdkSignerMode::MycNip46,
        })?;
        authorize_actor_for_draft(request.actor, request.frozen_draft)?;
        let signer_identity = RadrootsSdkSignerIdentityOnly {
            pubkey: self.user_pubkey.clone(),
        };
        authorize_signer_for_draft(&signer_identity, request.frozen_draft)?;
        let sign_event_request = sign_event_request_from_frozen_draft(request.frozen_draft)?;
        let request_id = self.next_request_id();
        let mut adapter = RadrootsSdkNip46TransportAdapter {
            transport: self.transport.as_ref(),
        };
        let mut progress_error = None;
        let request_future = execute_request_with_transport(
            &self.client_keys,
            &self.target,
            RadrootsNostrConnectClientRequest::new(request_id, sign_event_request),
            &mut adapter,
            |progress| {
                let sdk_progress = match progress {
                    radroots_nostr_connect::prelude::RadrootsNostrConnectClientProgress::AuthChallenge {
                        url,
                    } => RadrootsSdkSignerProgress::AuthChallenge {
                        mode: RadrootsSdkSignerMode::MycNip46,
                        url,
                    },
                };
                if let Err(error) = request.emit_progress(sdk_progress) {
                    progress_error = Some(error);
                    return Err(RadrootsNostrConnectError::Transport {
                        reason: "SDK signer progress sink failed".to_owned(),
                    });
                }
                Ok(())
            },
        );
        let response = timeout(self.request_policy.request_timeout(), request_future)
            .await
            .map_err(|_| RadrootsNostrConnectError::RequestTimedOut)
            .and_then(|response| response);
        if let Some(error) = progress_error {
            return Err(error);
        }
        let response = response.map_err(sdk_error_from_nip46_error)?;
        let signed_event = signed_event_from_nip46_response(request.operation_kind, response)?;
        validate_signed_event_matches_draft(&signed_event, request.frozen_draft).map_err(
            |error| RadrootsSdkError::SignerReturnedEventDrift {
                operation: request.operation_kind.to_owned(),
                reason: error.to_string(),
            },
        )?;
        request.emit_progress(RadrootsSdkSignerProgress::RequestCompleted {
            mode: RadrootsSdkSignerMode::MycNip46,
        })?;
        Ok(sign_receipt(
            request.operation_kind,
            RadrootsSdkSignerMode::MycNip46,
            self.user_pubkey.as_str().to_owned(),
            Some(self.target.remote_signer_public_key.to_hex()),
            signed_event,
        ))
    }

    fn next_request_id(&self) -> String {
        self.request_id_generator.next_request_id()
    }
}

trait RadrootsSdkMycNip46RequestIdGenerator: Send + Sync {
    fn next_request_id(&self) -> String;
}

struct RadrootsSdkUuidNip46RequestIdGenerator;

impl RadrootsSdkMycNip46RequestIdGenerator for RadrootsSdkUuidNip46RequestIdGenerator {
    fn next_request_id(&self) -> String {
        format!("radroots-sdk-myc-nip46-sign-{}", Uuid::new_v4())
    }
}

pub fn radroots_sdk_myc_nip46_product_permissions() -> RadrootsNostrConnectPermissions {
    RADROOTS_SDK_MYC_NIP46_PRODUCT_SIGN_EVENT_KINDS
        .iter()
        .map(|kind| {
            RadrootsNostrConnectPermission::with_parameter(
                RadrootsNostrConnectMethod::SignEvent,
                kind.to_string(),
            )
        })
        .collect::<Vec<_>>()
        .into()
}

pub fn radroots_sdk_myc_nip46_product_permission_strings() -> Vec<String> {
    radroots_sdk_myc_nip46_product_permissions()
        .as_slice()
        .iter()
        .map(ToString::to_string)
        .collect()
}

struct RadrootsSdkSignerIdentityOnly {
    pubkey: RadrootsPublicKey,
}

impl RadrootsEventSigner for RadrootsSdkSignerIdentityOnly {
    fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }

    fn sign_frozen_draft(
        &self,
        _draft: &RadrootsEventDraft,
    ) -> Result<RadrootsSignedEvent, RadrootsSignerError> {
        Err(RadrootsSignerError::Unavailable)
    }
}

struct RadrootsSdkNip46TransportAdapter<'a> {
    transport: &'a dyn RadrootsSdkNip46Transport,
}

impl RadrootsNostrConnectClientTransport for RadrootsSdkNip46TransportAdapter<'_> {
    fn publish_request_event<'a>(
        &'a mut self,
        event: RadrootsNostrEvent,
    ) -> RadrootsNostrConnectClientTransportFuture<'a, ()> {
        self.transport.publish_request_event(event)
    }

    fn next_response_event<'a>(
        &'a mut self,
    ) -> RadrootsNostrConnectClientTransportFuture<'a, RadrootsNostrEvent> {
        self.transport.next_response_event()
    }
}

fn sign_event_request_from_frozen_draft(
    draft: &RadrootsEventDraft,
) -> Result<RadrootsNostrConnectRequest, RadrootsSdkError> {
    let public_key = nip46_unsigned_event_pubkey(draft)?;
    let kind = nip46_unsigned_event_kind(draft)?;
    let tags = nip46_unsigned_event_tags(draft)?;
    let unsigned_event = UnsignedEvent {
        id: None,
        pubkey: public_key,
        created_at: Timestamp::from_secs(draft.created_at_u64()),
        kind,
        tags: Tags::from_list(tags),
        content: draft.content().to_owned(),
    };
    Ok(RadrootsNostrConnectRequest::SignEvent(unsigned_event))
}

fn nip46_unsigned_event_pubkey(draft: &RadrootsEventDraft) -> Result<PublicKey, RadrootsSdkError> {
    PublicKey::parse(draft.expected_pubkey_str()).map_err(|error| {
        nip46_sign_event_protocol_error(format!(
            "failed to parse frozen draft pubkey for NIP-46 unsigned event: {error}"
        ))
    })
}

fn nip46_unsigned_event_kind(draft: &RadrootsEventDraft) -> Result<Kind, RadrootsSdkError> {
    let kind = u16::try_from(draft.kind_u32()).map_err(|error| {
        nip46_sign_event_protocol_error(format!(
            "failed to convert frozen draft kind to NIP-46 unsigned event: {error}"
        ))
    })?;
    Ok(Kind::from_u16(kind))
}

fn nip46_unsigned_event_tags(draft: &RadrootsEventDraft) -> Result<Vec<Tag>, RadrootsSdkError> {
    let raw_tags = draft.tags_as_vec();
    let mut tags = Vec::with_capacity(raw_tags.len());
    for raw_tag in raw_tags {
        let tag = Tag::parse(raw_tag).map_err(|error| {
            nip46_sign_event_protocol_error(format!(
                "failed to convert frozen draft tags to NIP-46 unsigned event: {error}"
            ))
        })?;
        tags.push(tag);
    }
    Ok(tags)
}

fn nip46_sign_event_protocol_error(reason: String) -> RadrootsSdkError {
    RadrootsSdkError::SignerProtocol {
        mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
        reason,
    }
}

fn signed_event_from_nip46_response(
    operation_kind: &str,
    response: RadrootsNostrConnectResponse,
) -> Result<RadrootsSignedEvent, RadrootsSdkError> {
    match response {
        RadrootsNostrConnectResponse::SignedEvent(event) => {
            let raw_json = event.as_json();
            let wire = RadrootsNip01EventWire::parse_json(raw_json.as_str()).map_err(|error| {
                RadrootsSdkError::SignerProtocol {
                    mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
                    reason: format!("remote signed event wire is invalid: {error}"),
                }
            })?;
            let signed_event =
                RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).map_err(|error| {
                    RadrootsSdkError::SignerProtocol {
                        mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
                        reason: format!("remote signed event is invalid: {error}"),
                    }
                })?;
            signed_event
                .verify_signature()
                .map(|verified| verified.into_signed_event())
                .map_err(|error| RadrootsSdkError::SignerProtocol {
                    mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
                    reason: format!("remote signed event signature is invalid: {error}"),
                })
        }
        RadrootsNostrConnectResponse::Error { error, .. } => {
            Err(RadrootsSdkError::SignerRequestRejected {
                mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
                reason: error,
            })
        }
        RadrootsNostrConnectResponse::PendingConnection => {
            Err(RadrootsSdkError::SignerAuthChallengePending {
                mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
                auth_url: None,
            })
        }
        other => Err(RadrootsSdkError::SignerProtocol {
            mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
            reason: format!("unexpected NIP-46 response for {operation_kind}: {other:?}"),
        }),
    }
}

fn sdk_error_from_nip46_error(error: RadrootsNostrConnectError) -> RadrootsSdkError {
    match error {
        RadrootsNostrConnectError::RequestTimedOut => RadrootsSdkError::SignerRequestTimedOut {
            mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
        },
        RadrootsNostrConnectError::Transport { reason } => RadrootsSdkError::SignerTransport {
            mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
            reason,
        },
        RadrootsNostrConnectError::Encrypt { reason }
        | RadrootsNostrConnectError::Decrypt { reason }
        | RadrootsNostrConnectError::Sign { reason }
        | RadrootsNostrConnectError::Json(reason)
        | RadrootsNostrConnectError::InvalidRequestPayload { reason, .. }
        | RadrootsNostrConnectError::InvalidResponsePayload { reason, .. } => {
            RadrootsSdkError::SignerProtocol {
                mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
                reason,
            }
        }
        error => RadrootsSdkError::SignerProtocol {
            mode: RadrootsSdkSignerMode::MycNip46.as_str().to_owned(),
            reason: error.to_string(),
        },
    }
}

fn sign_receipt(
    operation_kind: &str,
    mode: RadrootsSdkSignerMode,
    signer_pubkey: String,
    remote_signer_pubkey: Option<String>,
    signed_event: RadrootsSignedEvent,
) -> RadrootsSdkSignReceipt {
    RadrootsSdkSignReceipt {
        operation_kind: operation_kind.to_owned(),
        mode,
        signer_pubkey,
        remote_signer_pubkey,
        signed_event_id: signed_event.id_str().to_owned(),
        signed_event,
    }
}

#[cfg(test)]
#[path = "../tests/unit/signer_provider_tests.rs"]
mod tests;
