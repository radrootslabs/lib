use crate::error::RadrootsSimplexSmpCryptoError;
use alloc::vec::Vec;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use radroots_simplex_smp_proto::prelude::{
    RadrootsSimplexSmpBrokerMessage, RadrootsSimplexSmpCommand, RadrootsSimplexSmpCorrelationId,
};
use x25519_dalek::PublicKey as X25519PublicKey;

const ED25519_SPKI_DER_PREFIX: &[u8] = &[
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];
const X25519_SPKI_DER_PREFIX: &[u8] = &[
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x03, 0x21, 0x00,
];
const X25519_SPKI_DER_PREFIX_WRAPPED: &[u8] = &[
    0x30, 0x2c, 0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x03, 0x21, 0x00,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpEd25519Keypair {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

impl RadrootsSimplexSmpEd25519Keypair {
    pub fn generate() -> Result<Self, RadrootsSimplexSmpCryptoError> {
        let mut secret = [0_u8; 32];
        getrandom::getrandom(&mut secret)
            .map_err(|_| RadrootsSimplexSmpCryptoError::EntropyUnavailable)?;
        let signing_key = SigningKey::from_bytes(&secret);
        Ok(Self {
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            private_key: secret.to_vec(),
        })
    }

    pub fn signing_key(&self) -> Result<SigningKey, RadrootsSimplexSmpCryptoError> {
        let bytes: [u8; 32] = self.private_key.as_slice().try_into().map_err(|_| {
            RadrootsSimplexSmpCryptoError::InvalidPrivateKeyLength(self.private_key.len())
        })?;
        Ok(SigningKey::from_bytes(&bytes))
    }

    pub fn verifying_key(&self) -> Result<VerifyingKey, RadrootsSimplexSmpCryptoError> {
        verifying_key_from_bytes(&self.public_key)
    }

    pub fn verify(
        &self,
        payload: &[u8],
        signature: &[u8],
    ) -> Result<(), RadrootsSimplexSmpCryptoError> {
        verify_signature(payload, &self.public_key, signature)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpCommandAuthorization {
    None,
    Ed25519(RadrootsSimplexSmpEd25519Keypair),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpQueueAuthorizationScope {
    pub session_identifier: Vec<u8>,
    pub correlation_id: RadrootsSimplexSmpCorrelationId,
    pub entity_id: Vec<u8>,
}

impl RadrootsSimplexSmpQueueAuthorizationScope {
    pub fn new(
        session_identifier: Vec<u8>,
        correlation_id: RadrootsSimplexSmpCorrelationId,
        entity_id: Vec<u8>,
    ) -> Result<Self, RadrootsSimplexSmpCryptoError> {
        validate_short_field(&session_identifier)?;
        validate_short_field(&entity_id)?;
        Ok(Self {
            session_identifier,
            correlation_id,
            entity_id,
        })
    }

    pub fn encode_authorized_frame(
        &self,
        frame: &[u8],
    ) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
        let mut buffer = Vec::new();
        push_short_bytes(&mut buffer, &self.session_identifier)?;
        push_short_bytes(&mut buffer, self.correlation_id.as_bytes())?;
        push_short_bytes(&mut buffer, &self.entity_id)?;
        buffer.extend_from_slice(frame);
        Ok(buffer)
    }

    pub fn authorized_command_body(
        &self,
        command: &RadrootsSimplexSmpCommand,
        transport_version: u16,
    ) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
        let frame = command.encode_for_version(transport_version)?;
        self.encode_authorized_frame(&frame)
    }

    pub fn authorized_broker_body(
        &self,
        message: &RadrootsSimplexSmpBrokerMessage,
        transport_version: u16,
    ) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
        let frame = message.encode_for_version(transport_version)?;
        self.encode_authorized_frame(&frame)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpQueueAuthorizationMaterial {
    pub authorized_body: Vec<u8>,
    pub authorization: Vec<u8>,
}

impl RadrootsSimplexSmpQueueAuthorizationMaterial {
    pub fn for_command(
        scope: &RadrootsSimplexSmpQueueAuthorizationScope,
        command: &RadrootsSimplexSmpCommand,
        transport_version: u16,
        authorization: &RadrootsSimplexSmpCommandAuthorization,
    ) -> Result<Self, RadrootsSimplexSmpCryptoError> {
        let authorized_body = scope.authorized_command_body(command, transport_version)?;
        Self::new(authorized_body, authorization)
    }

    pub fn for_broker_message(
        scope: &RadrootsSimplexSmpQueueAuthorizationScope,
        message: &RadrootsSimplexSmpBrokerMessage,
        transport_version: u16,
        authorization: &RadrootsSimplexSmpCommandAuthorization,
    ) -> Result<Self, RadrootsSimplexSmpCryptoError> {
        let authorized_body = scope.authorized_broker_body(message, transport_version)?;
        Self::new(authorized_body, authorization)
    }

    fn new(
        authorized_body: Vec<u8>,
        authorization: &RadrootsSimplexSmpCommandAuthorization,
    ) -> Result<Self, RadrootsSimplexSmpCryptoError> {
        let authorization = match authorization {
            RadrootsSimplexSmpCommandAuthorization::None => Vec::new(),
            RadrootsSimplexSmpCommandAuthorization::Ed25519(keypair) => {
                let signing_key = keypair.signing_key()?;
                let signature = signing_key.sign(&authorized_body);
                signature.to_bytes().to_vec()
            }
        };
        Ok(Self {
            authorized_body,
            authorization,
        })
    }
}

pub fn encode_ed25519_public_key_x509(
    public_key: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let _: [u8; 32] = public_key
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(public_key.len()))?;
    let mut encoded = Vec::with_capacity(ED25519_SPKI_DER_PREFIX.len() + public_key.len());
    encoded.extend_from_slice(ED25519_SPKI_DER_PREFIX);
    encoded.extend_from_slice(public_key);
    Ok(encoded)
}

pub fn encode_x25519_public_key_x509(
    public_key: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let _: [u8; 32] = public_key
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(public_key.len()))?;
    let mut encoded = Vec::with_capacity(X25519_SPKI_DER_PREFIX.len() + public_key.len());
    encoded.extend_from_slice(X25519_SPKI_DER_PREFIX);
    encoded.extend_from_slice(public_key);
    Ok(encoded)
}

pub fn decode_x25519_public_key_x509(
    encoded: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    if encoded.len() == 32 {
        let key: [u8; 32] = encoded
            .try_into()
            .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(encoded.len()))?;
        return Ok(X25519PublicKey::from(key).as_bytes().to_vec());
    }
    let raw = encoded
        .strip_prefix(X25519_SPKI_DER_PREFIX)
        .or_else(|| encoded.strip_prefix(X25519_SPKI_DER_PREFIX_WRAPPED))
        .ok_or_else(|| RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(encoded.len()))?;
    let key: [u8; 32] = raw
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(encoded.len()))?;
    Ok(X25519PublicKey::from(key).as_bytes().to_vec())
}

pub fn verify_signature(
    payload: &[u8],
    public_key: &[u8],
    signature: &[u8],
) -> Result<(), RadrootsSimplexSmpCryptoError> {
    let verifying_key = verifying_key_from_bytes(public_key)?;
    let signature: [u8; 64] = signature
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidSignatureLength(signature.len()))?;
    verifying_key
        .verify(payload, &Signature::from_bytes(&signature))
        .map_err(|_| RadrootsSimplexSmpCryptoError::SignatureVerificationFailed)
}

fn verifying_key_from_bytes(
    public_key: &[u8],
) -> Result<VerifyingKey, RadrootsSimplexSmpCryptoError> {
    let bytes: [u8; 32] = public_key
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(public_key.len()))?;
    VerifyingKey::from_bytes(&bytes)
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(public_key.len()))
}

fn validate_short_field(value: &[u8]) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if value.len() > u8::MAX as usize {
        return Err(RadrootsSimplexSmpCryptoError::InvalidShortFieldLength(
            value.len(),
        ));
    }
    Ok(())
}

fn push_short_bytes(
    buffer: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RadrootsSimplexSmpCryptoError> {
    validate_short_field(value)?;
    buffer.push(value.len() as u8);
    buffer.extend_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_simplex_smp_proto::prelude::{
        RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, RadrootsSimplexSmpCommand,
    };

    #[test]
    fn builds_ed25519_authorization_for_command_scope() {
        let scope = RadrootsSimplexSmpQueueAuthorizationScope::new(
            b"tls-session".to_vec(),
            RadrootsSimplexSmpCorrelationId::new([5_u8; 24]),
            b"queue-id".to_vec(),
        )
        .unwrap();
        let keypair = RadrootsSimplexSmpEd25519Keypair::generate().unwrap();

        let material = RadrootsSimplexSmpQueueAuthorizationMaterial::for_command(
            &scope,
            &RadrootsSimplexSmpCommand::Ping,
            RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            &RadrootsSimplexSmpCommandAuthorization::Ed25519(keypair.clone()),
        )
        .unwrap();

        assert_eq!(material.authorized_body[0], b"tls-session".len() as u8);
        assert_eq!(material.authorization.len(), 64);
        keypair
            .verify(&material.authorized_body, &material.authorization)
            .unwrap();
    }

    #[test]
    fn leaves_unsigned_authorization_empty() {
        let scope = RadrootsSimplexSmpQueueAuthorizationScope::new(
            b"tls-session".to_vec(),
            RadrootsSimplexSmpCorrelationId::new([3_u8; 24]),
            Vec::new(),
        )
        .unwrap();

        let material = RadrootsSimplexSmpQueueAuthorizationMaterial::for_command(
            &scope,
            &RadrootsSimplexSmpCommand::Ping,
            RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            &RadrootsSimplexSmpCommandAuthorization::None,
        )
        .unwrap();

        assert!(material.authorization.is_empty());
    }

    #[test]
    fn ed25519_public_key_x509_roundtrips_shape() {
        let keypair = RadrootsSimplexSmpEd25519Keypair::generate().unwrap();
        let encoded = encode_ed25519_public_key_x509(&keypair.public_key).unwrap();

        assert_eq!(
            &encoded[..ED25519_SPKI_DER_PREFIX.len()],
            ED25519_SPKI_DER_PREFIX
        );
        assert_eq!(
            &encoded[ED25519_SPKI_DER_PREFIX.len()..],
            keypair.public_key
        );
    }

    #[test]
    fn x25519_public_key_x509_roundtrips_and_accepts_raw() {
        let keypair = crate::message::RadrootsSimplexSmpX25519Keypair::generate().unwrap();
        let encoded = encode_x25519_public_key_x509(&keypair.public_key).unwrap();

        assert_eq!(
            decode_x25519_public_key_x509(&encoded).unwrap(),
            keypair.public_key
        );
        assert_eq!(
            decode_x25519_public_key_x509(&keypair.public_key).unwrap(),
            keypair.public_key
        );
    }
}
