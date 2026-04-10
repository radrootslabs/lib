use crate::error::RadrootsSimplexSmpTransportError;
use alloc::vec::Vec;
use radroots_simplex_smp_proto::prelude::{
    RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, RadrootsSimplexSmpBrokerTransmission,
    RadrootsSimplexSmpCommandTransmission,
};

pub const RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE: usize = 16_384;
pub const RADROOTS_SIMPLEX_SMP_TRANSPORT_PAD_BYTE: u8 = b'#';
const PADDED_PAYLOAD_PREFIX_LEN: usize = 2;
const MAX_TRANSPORT_PAYLOAD_LEN: usize =
    RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE - PADDED_PAYLOAD_PREFIX_LEN;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpTransportBlock {
    pub transmissions: Vec<Vec<u8>>,
}

impl RadrootsSimplexSmpTransportBlock {
    pub fn new(transmissions: Vec<Vec<u8>>) -> Result<Self, RadrootsSimplexSmpTransportError> {
        if transmissions.is_empty() {
            return Err(RadrootsSimplexSmpTransportError::EmptyTransportBlock);
        }
        if transmissions.len() > u8::MAX as usize {
            return Err(RadrootsSimplexSmpTransportError::TransmissionCountOverflow(
                transmissions.len(),
            ));
        }

        let mut payload_len = 1_usize;
        for transmission in &transmissions {
            if transmission.len() > u16::MAX as usize {
                return Err(RadrootsSimplexSmpTransportError::TransmissionTooLarge(
                    transmission.len(),
                ));
            }
            payload_len = payload_len.checked_add(2 + transmission.len()).ok_or(
                RadrootsSimplexSmpTransportError::TransportPayloadTooLarge(usize::MAX),
            )?;
        }
        if payload_len > MAX_TRANSPORT_PAYLOAD_LEN {
            return Err(RadrootsSimplexSmpTransportError::TransportPayloadTooLarge(
                payload_len,
            ));
        }

        Ok(Self { transmissions })
    }

    pub fn encode(&self) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
        let payload = self.encode_payload()?;
        encode_padded_bytes(
            &payload,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_PAD_BYTE,
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RadrootsSimplexSmpTransportError> {
        let payload = decode_padded_bytes(
            bytes,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE,
            RADROOTS_SIMPLEX_SMP_TRANSPORT_PAD_BYTE,
        )?;
        Self::from_payload(&payload)
    }

    pub fn encode_payload(&self) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
        encode_transport_payload(&self.transmissions)
    }

    pub fn from_payload(payload: &[u8]) -> Result<Self, RadrootsSimplexSmpTransportError> {
        let transmissions = decode_transport_payload(payload)?;
        Self::new(transmissions)
    }

    pub fn from_command_transmissions(
        transmissions: &[RadrootsSimplexSmpCommandTransmission],
        transport_version: u16,
    ) -> Result<Self, RadrootsSimplexSmpTransportError> {
        let encoded = transmissions
            .iter()
            .map(|transmission| transmission.encode_for_version(transport_version))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(encoded)
    }

    pub fn from_current_command_transmissions(
        transmissions: &[RadrootsSimplexSmpCommandTransmission],
    ) -> Result<Self, RadrootsSimplexSmpTransportError> {
        Self::from_command_transmissions(
            transmissions,
            RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
        )
    }

    pub fn decode_command_transmissions(
        &self,
        transport_version: u16,
    ) -> Result<Vec<RadrootsSimplexSmpCommandTransmission>, RadrootsSimplexSmpTransportError> {
        self.transmissions
            .iter()
            .map(|transmission| {
                RadrootsSimplexSmpCommandTransmission::decode_for_version(
                    transport_version,
                    transmission,
                )
                .map_err(Into::into)
            })
            .collect()
    }

    pub fn from_broker_transmissions(
        transmissions: &[RadrootsSimplexSmpBrokerTransmission],
        transport_version: u16,
    ) -> Result<Self, RadrootsSimplexSmpTransportError> {
        let encoded = transmissions
            .iter()
            .map(|transmission| transmission.encode_for_version(transport_version))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(encoded)
    }

    pub fn decode_broker_transmissions(
        &self,
        transport_version: u16,
    ) -> Result<Vec<RadrootsSimplexSmpBrokerTransmission>, RadrootsSimplexSmpTransportError> {
        self.transmissions
            .iter()
            .map(|transmission| {
                RadrootsSimplexSmpBrokerTransmission::decode_for_version(
                    transport_version,
                    transmission,
                )
                .map_err(Into::into)
            })
            .collect()
    }
}

pub fn encode_padded_bytes(
    payload: &[u8],
    padded_len: usize,
    pad_byte: u8,
) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
    let max_payload_len = padded_len.checked_sub(PADDED_PAYLOAD_PREFIX_LEN).ok_or(
        RadrootsSimplexSmpTransportError::TransportPayloadTooLarge(payload.len()),
    )?;
    if payload.len() > max_payload_len || payload.len() > u16::MAX as usize {
        return Err(RadrootsSimplexSmpTransportError::TransportPayloadTooLarge(
            payload.len(),
        ));
    }

    let mut buffer = Vec::with_capacity(padded_len);
    buffer.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    buffer.extend_from_slice(payload);
    buffer.resize(padded_len, pad_byte);
    Ok(buffer)
}

pub fn decode_padded_bytes(
    bytes: &[u8],
    padded_len: usize,
    pad_byte: u8,
) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
    if bytes.len() != padded_len {
        return Err(RadrootsSimplexSmpTransportError::InvalidPaddedBlockLength {
            expected: padded_len,
            actual: bytes.len(),
        });
    }
    let Some(length_bytes) = bytes.get(..2) else {
        return Err(RadrootsSimplexSmpTransportError::InvalidPaddedBlockLength {
            expected: padded_len,
            actual: bytes.len(),
        });
    };
    let payload_len = u16::from_be_bytes([length_bytes[0], length_bytes[1]]) as usize;
    let max_payload_len = padded_len - PADDED_PAYLOAD_PREFIX_LEN;
    if payload_len > max_payload_len {
        return Err(RadrootsSimplexSmpTransportError::TransportPayloadTooLarge(
            payload_len,
        ));
    }
    let end = PADDED_PAYLOAD_PREFIX_LEN + payload_len;
    for (offset, byte) in bytes[end..].iter().enumerate() {
        if *byte != pad_byte {
            return Err(RadrootsSimplexSmpTransportError::InvalidPadding {
                index: end + offset,
                value: *byte,
            });
        }
    }
    Ok(bytes[PADDED_PAYLOAD_PREFIX_LEN..end].to_vec())
}

fn encode_transport_payload(
    transmissions: &[Vec<u8>],
) -> Result<Vec<u8>, RadrootsSimplexSmpTransportError> {
    if transmissions.is_empty() {
        return Err(RadrootsSimplexSmpTransportError::EmptyTransportBlock);
    }
    if transmissions.len() > u8::MAX as usize {
        return Err(RadrootsSimplexSmpTransportError::TransmissionCountOverflow(
            transmissions.len(),
        ));
    }

    let mut buffer = Vec::new();
    buffer.push(transmissions.len() as u8);
    for transmission in transmissions {
        if transmission.len() > u16::MAX as usize {
            return Err(RadrootsSimplexSmpTransportError::TransmissionTooLarge(
                transmission.len(),
            ));
        }
        buffer.extend_from_slice(&(transmission.len() as u16).to_be_bytes());
        buffer.extend_from_slice(transmission);
    }
    if buffer.len() > MAX_TRANSPORT_PAYLOAD_LEN {
        return Err(RadrootsSimplexSmpTransportError::TransportPayloadTooLarge(
            buffer.len(),
        ));
    }
    Ok(buffer)
}

fn decode_transport_payload(
    payload: &[u8],
) -> Result<Vec<Vec<u8>>, RadrootsSimplexSmpTransportError> {
    let Some((&declared_count, mut remainder)) = payload.split_first() else {
        return Err(RadrootsSimplexSmpTransportError::EmptyTransportBlock);
    };
    if declared_count == 0 {
        return Err(RadrootsSimplexSmpTransportError::EmptyTransportBlock);
    }

    let mut transmissions = Vec::with_capacity(declared_count as usize);
    for _ in 0..declared_count {
        let Some(length_bytes) = remainder.get(..2) else {
            return Err(
                radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpProtoError::UnexpectedEof
                    .into(),
            );
        };
        let transmission_len = u16::from_be_bytes([length_bytes[0], length_bytes[1]]) as usize;
        let Some(transmission) = remainder.get(2..2 + transmission_len) else {
            return Err(
                radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpProtoError::UnexpectedEof
                    .into(),
            );
        };
        transmissions.push(transmission.to_vec());
        remainder = &remainder[2 + transmission_len..];
    }

    if !remainder.is_empty() {
        return Err(RadrootsSimplexSmpTransportError::TrailingTransportBytes(
            remainder.len(),
        ));
    }
    if transmissions.len() != declared_count as usize {
        return Err(
            RadrootsSimplexSmpTransportError::UnexpectedTransmissionCount {
                declared: declared_count,
                actual: transmissions.len(),
            },
        );
    }
    Ok(transmissions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_simplex_smp_proto::prelude::{
        RadrootsSimplexSmpCommand, RadrootsSimplexSmpCommandTransmission,
        RadrootsSimplexSmpCorrelationId,
    };

    #[test]
    fn roundtrips_command_transmissions_through_transport_block() {
        let transmissions = vec![
            RadrootsSimplexSmpCommandTransmission {
                authorization: b"sig-a".to_vec(),
                correlation_id: Some(RadrootsSimplexSmpCorrelationId::new([7_u8; 24])),
                entity_id: b"queue-a".to_vec(),
                command: RadrootsSimplexSmpCommand::Ping,
            },
            RadrootsSimplexSmpCommandTransmission {
                authorization: b"sig-b".to_vec(),
                correlation_id: Some(RadrootsSimplexSmpCorrelationId::new([9_u8; 24])),
                entity_id: b"queue-b".to_vec(),
                command: RadrootsSimplexSmpCommand::Get,
            },
        ];

        let block =
            RadrootsSimplexSmpTransportBlock::from_current_command_transmissions(&transmissions)
                .unwrap();
        let encoded = block.encode().unwrap();
        assert_eq!(encoded.len(), RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE);

        let decoded = RadrootsSimplexSmpTransportBlock::decode(&encoded).unwrap();
        let roundtrip = decoded
            .decode_command_transmissions(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION)
            .unwrap();
        assert_eq!(roundtrip, transmissions);
    }

    #[test]
    fn rejects_invalid_padding() {
        let block = RadrootsSimplexSmpTransportBlock::new(vec![b"PING".to_vec()]).unwrap();
        let mut encoded = block.encode().unwrap();
        encoded[RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE - 1] = b'!';

        let error = RadrootsSimplexSmpTransportBlock::decode(&encoded).unwrap_err();
        assert!(matches!(
            error,
            RadrootsSimplexSmpTransportError::InvalidPadding { .. }
        ));
    }

    #[test]
    fn roundtrips_transport_payload_without_padding() {
        let block =
            RadrootsSimplexSmpTransportBlock::new(vec![b"one".to_vec(), b"two".to_vec()]).unwrap();
        let payload = block.encode_payload().unwrap();
        let decoded = RadrootsSimplexSmpTransportBlock::from_payload(&payload).unwrap();
        assert_eq!(decoded, block);
    }
}
