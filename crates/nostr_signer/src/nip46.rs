use nostr::UnsignedEvent;
use radroots_nostr::prelude::{
    RadrootsNostrEvent, RadrootsNostrEventBuilder, RadrootsNostrFilter, RadrootsNostrKind,
    RadrootsNostrPublicKey, RadrootsNostrTag, RadrootsNostrTimestamp, radroots_nostr_filter_tag,
};
use radroots_nostr_connect::prelude::{
    RADROOTS_NOSTR_CONNECT_RPC_KIND, RadrootsNostrConnectRequest,
    RadrootsNostrConnectRequestMessage, RadrootsNostrConnectResponse,
};

use crate::error::RadrootsNostrSignerError;
use crate::evaluation::{RadrootsNostrSignerRequestAction, RadrootsNostrSignerRequestResponseHint};
use crate::model::{RadrootsNostrSignerConnectionId, RadrootsNostrSignerConnectionRecord};

pub trait RadrootsNostrSignerNip46Signer: Clone + Send + Sync {
    fn signer_public_key_hex(&self) -> String;
    fn decrypt_request(
        &self,
        client_public_key: &RadrootsNostrPublicKey,
        ciphertext: &str,
    ) -> Result<String, RadrootsNostrSignerError>;
    fn encrypt_response(
        &self,
        client_public_key: &RadrootsNostrPublicKey,
        payload: &str,
    ) -> Result<String, RadrootsNostrSignerError>;
    fn user_public_key(&self) -> RadrootsNostrPublicKey;
    fn sign_user_event(
        &self,
        unsigned_event: UnsignedEvent,
    ) -> Result<RadrootsNostrEvent, RadrootsNostrSignerError>;
    fn nip04_encrypt(
        &self,
        public_key: &RadrootsNostrPublicKey,
        plaintext: &str,
    ) -> Result<String, RadrootsNostrSignerError>;
    fn nip04_decrypt(
        &self,
        public_key: &RadrootsNostrPublicKey,
        ciphertext: &str,
    ) -> Result<String, RadrootsNostrSignerError>;
    fn nip44_encrypt(
        &self,
        public_key: &RadrootsNostrPublicKey,
        plaintext: &str,
    ) -> Result<String, RadrootsNostrSignerError>;
    fn nip44_decrypt(
        &self,
        public_key: &RadrootsNostrPublicKey,
        ciphertext: &str,
    ) -> Result<String, RadrootsNostrSignerError>;
}

#[derive(Clone)]
pub struct RadrootsNostrSignerNip46Codec<S> {
    signer: S,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsNostrSignerHandledRequest {
    Respond {
        response: RadrootsNostrConnectResponse,
        connection_id: Option<RadrootsNostrSignerConnectionId>,
        consume_connect_secret_for: Option<RadrootsNostrSignerConnectionId>,
    },
    Ignore,
}

impl<S: RadrootsNostrSignerNip46Signer> RadrootsNostrSignerNip46Codec<S> {
    pub fn new(signer: S) -> Self {
        Self { signer }
    }

    pub fn filter(&self) -> Result<RadrootsNostrFilter, RadrootsNostrSignerError> {
        let filter = RadrootsNostrFilter::new()
            .kind(RadrootsNostrKind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND))
            .since(RadrootsNostrTimestamp::now());
        radroots_nostr_filter_tag(filter, "p", vec![self.signer.signer_public_key_hex()])
            .map_err(|error| RadrootsNostrSignerError::InvalidState(error.to_string()))
    }

    pub fn parse_request_event(
        &self,
        event: &RadrootsNostrEvent,
    ) -> Result<RadrootsNostrConnectRequestMessage, RadrootsNostrSignerError> {
        let decrypted = self.signer.decrypt_request(&event.pubkey, &event.content)?;
        serde_json::from_str(&decrypted)
            .map_err(radroots_nostr_connect::prelude::RadrootsNostrConnectError::from)
            .map_err(|error| RadrootsNostrSignerError::InvalidState(error.to_string()))
    }

    pub fn build_response_event(
        &self,
        client_public_key: RadrootsNostrPublicKey,
        request_id: impl Into<String>,
        response: RadrootsNostrConnectResponse,
    ) -> Result<RadrootsNostrEventBuilder, RadrootsNostrSignerError> {
        let envelope = response
            .into_envelope(request_id.into())
            .map_err(|error| RadrootsNostrSignerError::InvalidState(error.to_string()))?;
        let payload = serde_json::to_string(&envelope)
            .map_err(|error| RadrootsNostrSignerError::InvalidState(error.to_string()))?;
        let ciphertext = self.signer.encrypt_response(&client_public_key, &payload)?;

        Ok(RadrootsNostrEventBuilder::new(
            RadrootsNostrKind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND),
            ciphertext,
        )
        .tags(vec![RadrootsNostrTag::public_key(client_public_key)]))
    }

    pub fn sign_event_response(
        &self,
        unsigned_event: UnsignedEvent,
    ) -> Result<RadrootsNostrConnectResponse, RadrootsNostrSignerError> {
        let user_public_key = self.signer.user_public_key();
        if unsigned_event.pubkey != user_public_key {
            return Ok(RadrootsNostrConnectResponse::Error {
                result: None,
                error: "sign_event pubkey does not match the managed user identity".to_owned(),
            });
        }

        match self.signer.sign_user_event(unsigned_event) {
            Ok(event) => Ok(RadrootsNostrConnectResponse::SignedEvent(event)),
            Err(error) => Ok(RadrootsNostrConnectResponse::Error {
                result: None,
                error: format!("failed to sign event: {error}"),
            }),
        }
    }

    pub fn crypto_response(
        &self,
        request: RadrootsNostrConnectRequest,
    ) -> Result<RadrootsNostrConnectResponse, RadrootsNostrSignerError> {
        Ok(match request {
            RadrootsNostrConnectRequest::Nip04Encrypt {
                public_key,
                plaintext,
            } => match self.signer.nip04_encrypt(&public_key, &plaintext) {
                Ok(ciphertext) => RadrootsNostrConnectResponse::Nip04Encrypt(ciphertext),
                Err(error) => RadrootsNostrConnectResponse::Error {
                    result: None,
                    error: format!("nip04 encrypt failed: {error}"),
                },
            },
            RadrootsNostrConnectRequest::Nip04Decrypt {
                public_key,
                ciphertext,
            } => match self.signer.nip04_decrypt(&public_key, &ciphertext) {
                Ok(plaintext) => RadrootsNostrConnectResponse::Nip04Decrypt(plaintext),
                Err(error) => RadrootsNostrConnectResponse::Error {
                    result: None,
                    error: format!("nip04 decrypt failed: {error}"),
                },
            },
            RadrootsNostrConnectRequest::Nip44Encrypt {
                public_key,
                plaintext,
            } => match self.signer.nip44_encrypt(&public_key, &plaintext) {
                Ok(ciphertext) => RadrootsNostrConnectResponse::Nip44Encrypt(ciphertext),
                Err(error) => RadrootsNostrConnectResponse::Error {
                    result: None,
                    error: format!("nip44 encrypt failed: {error}"),
                },
            },
            RadrootsNostrConnectRequest::Nip44Decrypt {
                public_key,
                ciphertext,
            } => match self.signer.nip44_decrypt(&public_key, &ciphertext) {
                Ok(plaintext) => RadrootsNostrConnectResponse::Nip44Decrypt(plaintext),
                Err(error) => RadrootsNostrConnectResponse::Error {
                    result: None,
                    error: format!("nip44 decrypt failed: {error}"),
                },
            },
            other => RadrootsNostrConnectResponse::Error {
                result: None,
                error: format!("request `{}` is not a crypto method", other.method()),
            },
        })
    }
}

impl RadrootsNostrSignerHandledRequest {
    pub fn respond(response: RadrootsNostrConnectResponse) -> Self {
        Self::respond_for_connection(None, response)
    }

    pub fn respond_for_connection(
        connection_id: Option<RadrootsNostrSignerConnectionId>,
        response: RadrootsNostrConnectResponse,
    ) -> Self {
        Self::Respond {
            response,
            connection_id,
            consume_connect_secret_for: None,
        }
    }

    pub fn into_publish_parts(
        self,
    ) -> Option<(
        RadrootsNostrConnectResponse,
        Option<RadrootsNostrSignerConnectionId>,
        Option<RadrootsNostrSignerConnectionId>,
    )> {
        match self {
            Self::Respond {
                response,
                connection_id,
                consume_connect_secret_for,
            } => Some((response, connection_id, consume_connect_secret_for)),
            Self::Ignore => None,
        }
    }
}

pub fn connect_response_outcome(
    connection: &RadrootsNostrSignerConnectionRecord,
    secret: Option<String>,
) -> RadrootsNostrSignerHandledRequest {
    let consume_connect_secret_for = secret.as_ref().map(|_| connection.connection_id.clone());
    RadrootsNostrSignerHandledRequest::Respond {
        response: match secret {
            Some(secret) => RadrootsNostrConnectResponse::ConnectSecretEcho(secret),
            None => RadrootsNostrConnectResponse::ConnectAcknowledged,
        },
        connection_id: Some(connection.connection_id.clone()),
        consume_connect_secret_for,
    }
}

pub fn response_from_hint(
    connection: &RadrootsNostrSignerConnectionRecord,
    hint: RadrootsNostrSignerRequestResponseHint,
) -> RadrootsNostrConnectResponse {
    match hint {
        RadrootsNostrSignerRequestResponseHint::Pong => RadrootsNostrConnectResponse::Pong,
        RadrootsNostrSignerRequestResponseHint::UserPublicKey(public_key) => {
            RadrootsNostrConnectResponse::UserPublicKey(public_key)
        }
        RadrootsNostrSignerRequestResponseHint::RemoteSessionCapability(capability) => {
            RadrootsNostrConnectResponse::RemoteSessionCapability(capability)
        }
        RadrootsNostrSignerRequestResponseHint::RelayList(relays) => {
            if relays == connection.relays {
                RadrootsNostrConnectResponse::RelayList(relays)
            } else {
                RadrootsNostrConnectResponse::RelayList(connection.relays.clone())
            }
        }
        RadrootsNostrSignerRequestResponseHint::None => RadrootsNostrConnectResponse::Error {
            result: None,
            error: "request evaluation did not provide a response hint".to_owned(),
        },
    }
}

pub fn handled_request_for_action<F>(
    connection: &RadrootsNostrSignerConnectionRecord,
    action: RadrootsNostrSignerRequestAction,
    on_allowed: F,
) -> Result<RadrootsNostrSignerHandledRequest, RadrootsNostrSignerError>
where
    F: FnOnce() -> Result<RadrootsNostrConnectResponse, RadrootsNostrSignerError>,
{
    Ok(match action {
        RadrootsNostrSignerRequestAction::Denied { reason } => {
            RadrootsNostrSignerHandledRequest::respond_for_connection(
                Some(connection.connection_id.clone()),
                RadrootsNostrConnectResponse::Error {
                    result: None,
                    error: reason,
                },
            )
        }
        RadrootsNostrSignerRequestAction::Challenged { auth_challenge, .. } => {
            RadrootsNostrSignerHandledRequest::respond_for_connection(
                Some(connection.connection_id.clone()),
                RadrootsNostrConnectResponse::AuthUrl(auth_challenge.auth_url),
            )
        }
        RadrootsNostrSignerRequestAction::Allowed { .. } => {
            RadrootsNostrSignerHandledRequest::respond_for_connection(
                Some(connection.connection_id.clone()),
                on_allowed()?,
            )
        }
    })
}
