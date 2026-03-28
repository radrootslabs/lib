use crate::error::RadrootsSimplexSmpCryptoError;
use alloc::vec::Vec;
use radroots_simplex_smp_proto::prelude::{
    RadrootsSimplexSmpBrokerMessage, RadrootsSimplexSmpCommand, RadrootsSimplexSmpCorrelationId,
};
use sha2::{Digest, Sha512};

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
    pub authorized_digest: [u8; 64],
    pub nonce: [u8; 24],
    pub queue_key_material: Vec<u8>,
    pub server_session_key: Vec<u8>,
}

impl RadrootsSimplexSmpQueueAuthorizationMaterial {
    pub fn for_command(
        scope: &RadrootsSimplexSmpQueueAuthorizationScope,
        command: &RadrootsSimplexSmpCommand,
        transport_version: u16,
        queue_key_material: Vec<u8>,
        server_session_key: Vec<u8>,
    ) -> Result<Self, RadrootsSimplexSmpCryptoError> {
        let authorized_body = scope.authorized_command_body(command, transport_version)?;
        Ok(Self::new(
            authorized_body,
            scope.correlation_id,
            queue_key_material,
            server_session_key,
        ))
    }

    pub fn for_broker_message(
        scope: &RadrootsSimplexSmpQueueAuthorizationScope,
        message: &RadrootsSimplexSmpBrokerMessage,
        transport_version: u16,
        queue_key_material: Vec<u8>,
        server_session_key: Vec<u8>,
    ) -> Result<Self, RadrootsSimplexSmpCryptoError> {
        let authorized_body = scope.authorized_broker_body(message, transport_version)?;
        Ok(Self::new(
            authorized_body,
            scope.correlation_id,
            queue_key_material,
            server_session_key,
        ))
    }

    fn new(
        authorized_body: Vec<u8>,
        correlation_id: RadrootsSimplexSmpCorrelationId,
        queue_key_material: Vec<u8>,
        server_session_key: Vec<u8>,
    ) -> Self {
        let digest = Sha512::digest(&authorized_body);
        let mut authorized_digest = [0_u8; 64];
        authorized_digest.copy_from_slice(&digest);

        Self {
            authorized_body,
            authorized_digest,
            nonce: *correlation_id.as_bytes(),
            queue_key_material,
            server_session_key,
        }
    }
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
    fn builds_authorization_material_for_command_scope() {
        let scope = RadrootsSimplexSmpQueueAuthorizationScope::new(
            b"tls-unique".to_vec(),
            RadrootsSimplexSmpCorrelationId::new([5_u8; 24]),
            b"queue-id".to_vec(),
        )
        .unwrap();

        let material = RadrootsSimplexSmpQueueAuthorizationMaterial::for_command(
            &scope,
            &RadrootsSimplexSmpCommand::Ping,
            RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            b"queue-private".to_vec(),
            b"server-session".to_vec(),
        )
        .unwrap();

        assert_eq!(material.nonce, [5_u8; 24]);
        assert_eq!(material.authorized_body[0], b"tls-unique".len() as u8);
        assert_eq!(material.authorized_body[11], 24);
        assert_eq!(material.authorized_digest.len(), 64);
    }
}
