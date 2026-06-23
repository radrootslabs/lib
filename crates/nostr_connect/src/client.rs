use crate::error::RadrootsNostrConnectError;
use crate::message::{
    RADROOTS_NOSTR_CONNECT_RPC_KIND, RadrootsNostrConnectRequest,
    RadrootsNostrConnectRequestMessage, RadrootsNostrConnectResponse,
    RadrootsNostrConnectResponseEnvelope,
};
use crate::method::RadrootsNostrConnectMethod;
use nostr::nips::nip44::{self, Version};
use nostr::{Event, EventBuilder, Keys, Kind, PublicKey, RelayUrl, Tag};
use std::future::Future;
use std::pin::Pin;

pub type RadrootsNostrConnectClientTransportFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RadrootsNostrConnectError>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsNostrConnectClientTarget {
    pub remote_signer_public_key: PublicKey,
    pub relays: Vec<RelayUrl>,
}

impl RadrootsNostrConnectClientTarget {
    pub fn new(remote_signer_public_key: PublicKey, relays: Vec<RelayUrl>) -> Self {
        Self {
            remote_signer_public_key,
            relays,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsNostrConnectClientRequest {
    pub request_id: String,
    pub request: RadrootsNostrConnectRequest,
}

impl RadrootsNostrConnectClientRequest {
    pub fn new(request_id: impl Into<String>, request: RadrootsNostrConnectRequest) -> Self {
        Self {
            request_id: request_id.into(),
            request,
        }
    }

    pub fn method(&self) -> RadrootsNostrConnectMethod {
        self.request.method()
    }

    pub fn into_message(self) -> RadrootsNostrConnectRequestMessage {
        RadrootsNostrConnectRequestMessage::new(self.request_id, self.request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectClientProgress {
    AuthChallenge { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectClientEventOutcome {
    Ignore,
    Progress(RadrootsNostrConnectClientProgress),
    Response(RadrootsNostrConnectResponse),
}

pub trait RadrootsNostrConnectClientTransport {
    fn publish_request_event<'a>(
        &'a mut self,
        event: Event,
    ) -> RadrootsNostrConnectClientTransportFuture<'a, ()>;

    fn next_response_event<'a>(
        &'a mut self,
    ) -> RadrootsNostrConnectClientTransportFuture<'a, Event>;
}

pub fn build_request_event(
    client_keys: &Keys,
    target: &RadrootsNostrConnectClientTarget,
    message: RadrootsNostrConnectRequestMessage,
) -> Result<Event, RadrootsNostrConnectError> {
    let payload = serde_json::to_string(&message).map_err(RadrootsNostrConnectError::from)?;
    let ciphertext = nip44::encrypt(
        client_keys.secret_key(),
        &target.remote_signer_public_key,
        payload,
        Version::V2,
    )
    .map_err(|error| RadrootsNostrConnectError::Encrypt {
        reason: error.to_string(),
    })?;

    EventBuilder::new(Kind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND), ciphertext)
        .tag(Tag::public_key(target.remote_signer_public_key))
        .sign_with_keys(client_keys)
        .map_err(|error| RadrootsNostrConnectError::Sign {
            reason: error.to_string(),
        })
}

pub fn parse_response_event(
    client_keys: &Keys,
    target: &RadrootsNostrConnectClientTarget,
    request_id: &str,
    method: &RadrootsNostrConnectMethod,
    event: &Event,
) -> Result<RadrootsNostrConnectClientEventOutcome, RadrootsNostrConnectError> {
    if event.kind != Kind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND) {
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

    let decrypted = nip44::decrypt(
        client_keys.secret_key(),
        &target.remote_signer_public_key,
        &event.content,
    )
    .map_err(|error| RadrootsNostrConnectError::Decrypt {
        reason: error.to_string(),
    })?;

    let envelope: RadrootsNostrConnectResponseEnvelope =
        serde_json::from_str(&decrypted).map_err(RadrootsNostrConnectError::from)?;
    if envelope.id != request_id {
        return Ok(RadrootsNostrConnectClientEventOutcome::Ignore);
    }

    let response = RadrootsNostrConnectResponse::from_envelope(method, envelope)?;
    Ok(match response {
        RadrootsNostrConnectResponse::AuthUrl(url) => {
            RadrootsNostrConnectClientEventOutcome::Progress(
                RadrootsNostrConnectClientProgress::AuthChallenge { url },
            )
        }
        response => RadrootsNostrConnectClientEventOutcome::Response(response),
    })
}

pub async fn execute_request_with_transport<T, F>(
    client_keys: &Keys,
    target: &RadrootsNostrConnectClientTarget,
    request: RadrootsNostrConnectClientRequest,
    transport: &mut T,
    mut on_progress: F,
) -> Result<RadrootsNostrConnectResponse, RadrootsNostrConnectError>
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
