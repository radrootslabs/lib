use crate::error::RadrootsSimplexAgentProtoError;
use crate::model::{
    RADROOTS_SIMPLEX_AGENT_CURRENT_VERSION, RadrootsSimplexAgentConnectionLink,
    RadrootsSimplexAgentDecryptedMessage, RadrootsSimplexAgentEncryptedPayload,
    RadrootsSimplexAgentEnvelope, RadrootsSimplexAgentMessage, RadrootsSimplexAgentMessageFrame,
    RadrootsSimplexAgentMessageHeader, RadrootsSimplexAgentMessageReceipt,
    RadrootsSimplexAgentQueueAddress, RadrootsSimplexAgentQueueDescriptor,
    RadrootsSimplexAgentQueueUseDecision,
};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use radroots_simplex_smp_crypto::prelude::RadrootsSimplexSmpRatchetHeader;
use radroots_simplex_smp_proto::prelude::{
    RadrootsSimplexSmpQueueUri, RadrootsSimplexSmpServerAddress,
};

pub fn encode_connection_link(
    link: &RadrootsSimplexAgentConnectionLink,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    let mut buffer = Vec::new();
    push_short_bytes(&mut buffer, link.invitation_queue.to_string().as_bytes())?;
    push_short_bytes(&mut buffer, &link.connection_id)?;
    push_short_bytes(&mut buffer, &link.e2e_public_key)?;
    buffer.push(encode_bool(link.contact_address));
    Ok(buffer)
}

pub fn decode_connection_link(
    bytes: &[u8],
) -> Result<RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentProtoError> {
    let mut cursor = Cursor::new(bytes);
    let invitation_queue = String::from_utf8(cursor.read_short_bytes()?)
        .map_err(|error| RadrootsSimplexAgentProtoError::InvalidUtf8(error.to_string()))?;
    let link = RadrootsSimplexAgentConnectionLink {
        invitation_queue: RadrootsSimplexSmpQueueUri::parse(&invitation_queue)?,
        connection_id: cursor.read_short_bytes()?,
        e2e_public_key: cursor.read_short_bytes()?,
        contact_address: decode_bool(cursor.read_byte()?)?,
    };
    cursor.finish()?;
    Ok(link)
}

pub fn encode_agent_message_frame(
    frame: &RadrootsSimplexAgentMessageFrame,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    let mut buffer = Vec::new();
    buffer.push(b'M');
    buffer.extend_from_slice(&frame.header.message_id.to_be_bytes());
    push_short_bytes(&mut buffer, &frame.header.previous_message_hash)?;
    buffer.extend_from_slice(&encode_agent_message(&frame.message)?);
    buffer.extend_from_slice(&frame.padding);
    Ok(buffer)
}

pub fn decode_agent_message_frame(
    bytes: &[u8],
) -> Result<RadrootsSimplexAgentMessageFrame, RadrootsSimplexAgentProtoError> {
    let mut cursor = Cursor::new(bytes);
    cursor.expect_tag(b"M")?;
    let header = RadrootsSimplexAgentMessageHeader {
        message_id: cursor.read_u64()?,
        previous_message_hash: cursor.read_short_bytes()?,
    };
    let message = decode_agent_message(&mut cursor)?;
    let padding = cursor.read_remaining().to_vec();
    Ok(RadrootsSimplexAgentMessageFrame {
        header,
        message,
        padding,
    })
}

pub fn encode_decrypted_message(
    message: &RadrootsSimplexAgentDecryptedMessage,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    let mut buffer = Vec::new();
    match message {
        RadrootsSimplexAgentDecryptedMessage::ConnectionInfo(info) => {
            buffer.push(b'I');
            buffer.extend_from_slice(info);
        }
        RadrootsSimplexAgentDecryptedMessage::ConnectionInfoReply { reply_queues, info } => {
            buffer.push(b'D');
            push_list(&mut buffer, reply_queues, encode_queue_descriptor)?;
            push_large_bytes(&mut buffer, info)?;
        }
        RadrootsSimplexAgentDecryptedMessage::RatchetInfo(info) => {
            buffer.push(b'R');
            push_large_bytes(&mut buffer, info)?;
        }
        RadrootsSimplexAgentDecryptedMessage::Message(frame) => {
            buffer.extend_from_slice(&encode_agent_message_frame(frame)?);
        }
    }
    Ok(buffer)
}

pub fn decode_decrypted_message(
    bytes: &[u8],
) -> Result<RadrootsSimplexAgentDecryptedMessage, RadrootsSimplexAgentProtoError> {
    let mut cursor = Cursor::new(bytes);
    match cursor.read_byte()? {
        b'I' => Ok(RadrootsSimplexAgentDecryptedMessage::ConnectionInfo(
            cursor.read_remaining().to_vec(),
        )),
        b'D' => {
            let reply_queues = cursor.read_list(decode_queue_descriptor)?;
            let info = cursor.read_large_bytes()?;
            cursor.finish()?;
            Ok(RadrootsSimplexAgentDecryptedMessage::ConnectionInfoReply { reply_queues, info })
        }
        b'R' => {
            let info = cursor.read_large_bytes()?;
            cursor.finish()?;
            Ok(RadrootsSimplexAgentDecryptedMessage::RatchetInfo(info))
        }
        b'M' => {
            decode_agent_message_frame(bytes).map(RadrootsSimplexAgentDecryptedMessage::Message)
        }
        tag => Err(RadrootsSimplexAgentProtoError::InvalidTag(
            String::from_utf8_lossy(&[tag]).into_owned(),
        )),
    }
}

pub fn encode_envelope(
    envelope: &RadrootsSimplexAgentEnvelope,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&RADROOTS_SIMPLEX_AGENT_CURRENT_VERSION.to_be_bytes());
    match envelope {
        RadrootsSimplexAgentEnvelope::Confirmation {
            reply_queue,
            encrypted,
        } => {
            buffer.push(b'C');
            buffer.push(encode_bool(*reply_queue));
            encode_encrypted_payload(&mut buffer, encrypted)?;
        }
        RadrootsSimplexAgentEnvelope::Message(encrypted) => {
            buffer.push(b'M');
            encode_encrypted_payload(&mut buffer, encrypted)?;
        }
        RadrootsSimplexAgentEnvelope::Invitation {
            request,
            connection_info,
        } => {
            buffer.push(b'I');
            push_large_bytes(&mut buffer, request)?;
            push_large_bytes(&mut buffer, connection_info)?;
        }
        RadrootsSimplexAgentEnvelope::RatchetKey { info, encrypted } => {
            buffer.push(b'R');
            push_large_bytes(&mut buffer, info)?;
            encode_encrypted_payload(&mut buffer, encrypted)?;
        }
    }
    Ok(buffer)
}

pub fn decode_envelope(
    bytes: &[u8],
) -> Result<RadrootsSimplexAgentEnvelope, RadrootsSimplexAgentProtoError> {
    let mut cursor = Cursor::new(bytes);
    let _version = cursor.read_u16()?;
    match cursor.read_byte()? {
        b'C' => {
            let reply_queue = decode_bool(cursor.read_byte()?)?;
            let encrypted = decode_encrypted_payload(&mut cursor)?;
            cursor.finish()?;
            Ok(RadrootsSimplexAgentEnvelope::Confirmation {
                reply_queue,
                encrypted,
            })
        }
        b'M' => {
            let encrypted = decode_encrypted_payload(&mut cursor)?;
            cursor.finish()?;
            Ok(RadrootsSimplexAgentEnvelope::Message(encrypted))
        }
        b'I' => {
            let request = cursor.read_large_bytes()?;
            let connection_info = cursor.read_large_bytes()?;
            cursor.finish()?;
            Ok(RadrootsSimplexAgentEnvelope::Invitation {
                request,
                connection_info,
            })
        }
        b'R' => {
            let info = cursor.read_large_bytes()?;
            let encrypted = decode_encrypted_payload(&mut cursor)?;
            cursor.finish()?;
            Ok(RadrootsSimplexAgentEnvelope::RatchetKey { info, encrypted })
        }
        tag => Err(RadrootsSimplexAgentProtoError::InvalidTag(
            String::from_utf8_lossy(&[tag]).into_owned(),
        )),
    }
}

fn encode_agent_message(
    message: &RadrootsSimplexAgentMessage,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    let mut buffer = Vec::new();
    match message {
        RadrootsSimplexAgentMessage::Hello => buffer.push(b'H'),
        RadrootsSimplexAgentMessage::UserMessage(body) => {
            buffer.push(b'M');
            buffer.extend_from_slice(body);
        }
        RadrootsSimplexAgentMessage::Receipt(receipt) => {
            buffer.push(b'V');
            buffer.extend_from_slice(&receipt.message_id.to_be_bytes());
            push_short_bytes(&mut buffer, &receipt.message_hash)?;
            push_large_bytes(&mut buffer, &receipt.receipt_info)?;
        }
        RadrootsSimplexAgentMessage::EncryptionReady { up_to_message_id } => {
            buffer.push(b'E');
            buffer.extend_from_slice(&up_to_message_id.to_be_bytes());
        }
        RadrootsSimplexAgentMessage::QueueContinue(queue) => {
            buffer.extend_from_slice(b"QC");
            encode_queue_address(&mut buffer, queue)?;
        }
        RadrootsSimplexAgentMessage::QueueAdd(queues) => {
            buffer.extend_from_slice(b"QA");
            push_list(&mut buffer, queues, encode_queue_descriptor)?;
        }
        RadrootsSimplexAgentMessage::QueueKey(queues) => {
            buffer.extend_from_slice(b"QK");
            push_list(&mut buffer, queues, encode_queue_descriptor)?;
        }
        RadrootsSimplexAgentMessage::QueueUse(queues) => {
            buffer.extend_from_slice(b"QU");
            push_list(&mut buffer, queues, encode_queue_use_decision)?;
        }
        RadrootsSimplexAgentMessage::QueueTest(queues) => {
            buffer.extend_from_slice(b"QT");
            push_list(&mut buffer, queues, encode_queue_address)?;
        }
    }
    Ok(buffer)
}

fn decode_agent_message(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexAgentMessage, RadrootsSimplexAgentProtoError> {
    let first = cursor.read_byte()?;
    match first {
        b'H' => Ok(RadrootsSimplexAgentMessage::Hello),
        b'M' => Ok(RadrootsSimplexAgentMessage::UserMessage(
            cursor.read_remaining().to_vec(),
        )),
        b'V' => Ok(RadrootsSimplexAgentMessage::Receipt(
            RadrootsSimplexAgentMessageReceipt {
                message_id: cursor.read_u64()?,
                message_hash: cursor.read_short_bytes()?,
                receipt_info: cursor.read_large_bytes()?,
            },
        )),
        b'E' => Ok(RadrootsSimplexAgentMessage::EncryptionReady {
            up_to_message_id: cursor.read_u64()?,
        }),
        b'Q' => match cursor.read_byte()? {
            b'C' => Ok(RadrootsSimplexAgentMessage::QueueContinue(
                decode_queue_address(cursor)?,
            )),
            b'A' => Ok(RadrootsSimplexAgentMessage::QueueAdd(
                cursor.read_list(decode_queue_descriptor)?,
            )),
            b'K' => Ok(RadrootsSimplexAgentMessage::QueueKey(
                cursor.read_list(decode_queue_descriptor)?,
            )),
            b'U' => Ok(RadrootsSimplexAgentMessage::QueueUse(
                cursor.read_list(decode_queue_use_decision)?,
            )),
            b'T' => Ok(RadrootsSimplexAgentMessage::QueueTest(
                cursor.read_list(decode_queue_address)?,
            )),
            tag => Err(RadrootsSimplexAgentProtoError::InvalidTag(alloc::format!(
                "Q{}",
                tag as char
            ))),
        },
        tag => Err(RadrootsSimplexAgentProtoError::InvalidTag(
            String::from_utf8_lossy(&[tag]).into_owned(),
        )),
    }
}

fn encode_encrypted_payload(
    buffer: &mut Vec<u8>,
    encrypted: &RadrootsSimplexAgentEncryptedPayload,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    match &encrypted.ratchet_header {
        Some(header) => {
            buffer.push(1);
            let ratchet = encode_ratchet_header(header)?;
            push_large_bytes(buffer, &ratchet)?;
        }
        None => buffer.push(0),
    }
    push_large_bytes(buffer, &encrypted.ciphertext)
}

fn decode_encrypted_payload(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexAgentEncryptedPayload, RadrootsSimplexAgentProtoError> {
    let has_header = decode_bool(cursor.read_byte()?)?;
    let ratchet_header = if has_header {
        Some(decode_ratchet_header(&cursor.read_large_bytes()?)?)
    } else {
        None
    };
    Ok(RadrootsSimplexAgentEncryptedPayload {
        ratchet_header,
        ciphertext: cursor.read_large_bytes()?,
    })
}

fn encode_queue_descriptor(
    buffer: &mut Vec<u8>,
    descriptor: &RadrootsSimplexAgentQueueDescriptor,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    push_short_bytes(buffer, descriptor.queue_uri.to_string().as_bytes())?;
    push_maybe(
        buffer,
        descriptor.replaced_queue.as_ref(),
        encode_queue_address,
    )?;
    buffer.push(encode_bool(descriptor.primary));
    push_maybe_short_bytes(buffer, descriptor.sender_key.as_deref())?;
    Ok(())
}

fn decode_queue_descriptor(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexAgentQueueDescriptor, RadrootsSimplexAgentProtoError> {
    let queue_uri = String::from_utf8(cursor.read_short_bytes()?)
        .map_err(|error| RadrootsSimplexAgentProtoError::InvalidUtf8(error.to_string()))?;
    Ok(RadrootsSimplexAgentQueueDescriptor {
        queue_uri: RadrootsSimplexSmpQueueUri::parse(&queue_uri)?,
        replaced_queue: cursor.read_maybe(decode_queue_address)?,
        primary: decode_bool(cursor.read_byte()?)?,
        sender_key: cursor.read_maybe(decode_short_bytes)?,
    })
}

fn encode_queue_use_decision(
    buffer: &mut Vec<u8>,
    decision: &RadrootsSimplexAgentQueueUseDecision,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    encode_queue_address(buffer, &decision.queue_address)?;
    buffer.push(encode_bool(decision.primary));
    Ok(())
}

fn decode_queue_use_decision(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexAgentQueueUseDecision, RadrootsSimplexAgentProtoError> {
    Ok(RadrootsSimplexAgentQueueUseDecision {
        queue_address: decode_queue_address(cursor)?,
        primary: decode_bool(cursor.read_byte()?)?,
    })
}

fn encode_queue_address(
    buffer: &mut Vec<u8>,
    queue: &RadrootsSimplexAgentQueueAddress,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    push_short_bytes(buffer, queue.server.server_identity.as_bytes())?;
    push_list(buffer, &queue.server.hosts, |buffer, host| {
        push_short_bytes(buffer, host.as_bytes())
    })?;
    let port = queue
        .server
        .port
        .map(|value| value.to_string())
        .unwrap_or_default();
    push_short_bytes(buffer, port.as_bytes())?;
    push_short_bytes(buffer, &queue.sender_id)?;
    Ok(())
}

fn decode_queue_address(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexAgentQueueAddress, RadrootsSimplexAgentProtoError> {
    let server_identity = String::from_utf8(cursor.read_short_bytes()?)
        .map_err(|error| RadrootsSimplexAgentProtoError::InvalidUtf8(error.to_string()))?;
    let hosts = cursor.read_list(|cursor| {
        let host = String::from_utf8(cursor.read_short_bytes()?)
            .map_err(|error| RadrootsSimplexAgentProtoError::InvalidUtf8(error.to_string()))?;
        Ok(host)
    })?;
    let port_raw = String::from_utf8(cursor.read_short_bytes()?)
        .map_err(|error| RadrootsSimplexAgentProtoError::InvalidUtf8(error.to_string()))?;
    let port = if port_raw.is_empty() {
        None
    } else {
        Some(
            port_raw
                .parse::<u16>()
                .map_err(|_| RadrootsSimplexAgentProtoError::InvalidUtf8(port_raw.clone()))?,
        )
    };
    Ok(RadrootsSimplexAgentQueueAddress {
        server: RadrootsSimplexSmpServerAddress {
            server_identity,
            hosts,
            port,
        },
        sender_id: cursor.read_short_bytes()?,
    })
}

fn encode_ratchet_header(
    header: &RadrootsSimplexSmpRatchetHeader,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&header.previous_sending_chain_length.to_be_bytes());
    buffer.extend_from_slice(&header.message_number.to_be_bytes());
    push_short_bytes(&mut buffer, &header.dh_public_key)?;
    push_maybe_short_bytes(&mut buffer, header.pq_public_key.as_deref())?;
    push_maybe_short_bytes(&mut buffer, header.pq_ciphertext.as_deref())?;
    Ok(buffer)
}

fn decode_ratchet_header(
    bytes: &[u8],
) -> Result<RadrootsSimplexSmpRatchetHeader, RadrootsSimplexAgentProtoError> {
    let mut cursor = Cursor::new(bytes);
    let header = RadrootsSimplexSmpRatchetHeader {
        previous_sending_chain_length: cursor.read_u32()?,
        message_number: cursor.read_u32()?,
        dh_public_key: cursor.read_short_bytes()?,
        pq_public_key: cursor.read_maybe(decode_short_bytes)?,
        pq_ciphertext: cursor.read_maybe(decode_short_bytes)?,
    };
    cursor.finish()?;
    Ok(header)
}

fn decode_short_bytes(cursor: &mut Cursor<'_>) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    cursor.read_short_bytes()
}

fn push_short_bytes(
    buffer: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RadrootsSimplexAgentProtoError> {
    if value.len() > u8::MAX as usize {
        return Err(RadrootsSimplexAgentProtoError::InvalidShortFieldLength(
            value.len(),
        ));
    }
    buffer.push(value.len() as u8);
    buffer.extend_from_slice(value);
    Ok(())
}

fn push_large_bytes(
    buffer: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RadrootsSimplexAgentProtoError> {
    if value.len() > u16::MAX as usize {
        return Err(RadrootsSimplexAgentProtoError::InvalidLargeFieldLength(
            value.len(),
        ));
    }
    buffer.extend_from_slice(&(value.len() as u16).to_be_bytes());
    buffer.extend_from_slice(value);
    Ok(())
}

fn push_maybe_short_bytes(
    buffer: &mut Vec<u8>,
    value: Option<&[u8]>,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    match value {
        Some(value) => {
            buffer.push(1);
            push_short_bytes(buffer, value)
        }
        None => {
            buffer.push(0);
            Ok(())
        }
    }
}

fn push_maybe<T>(
    buffer: &mut Vec<u8>,
    value: Option<&T>,
    encode: fn(&mut Vec<u8>, &T) -> Result<(), RadrootsSimplexAgentProtoError>,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    match value {
        Some(value) => {
            buffer.push(1);
            encode(buffer, value)
        }
        None => {
            buffer.push(0);
            Ok(())
        }
    }
}

fn push_list<T>(
    buffer: &mut Vec<u8>,
    values: &[T],
    encode: fn(&mut Vec<u8>, &T) -> Result<(), RadrootsSimplexAgentProtoError>,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    if values.len() > u8::MAX as usize {
        return Err(RadrootsSimplexAgentProtoError::InvalidShortFieldLength(
            values.len(),
        ));
    }
    buffer.push(values.len() as u8);
    for value in values {
        encode(buffer, value)?;
    }
    Ok(())
}

const fn encode_bool(value: bool) -> u8 {
    if value { 1 } else { 0 }
}

fn decode_bool(value: u8) -> Result<bool, RadrootsSimplexAgentProtoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(RadrootsSimplexAgentProtoError::InvalidBoolEncoding(other)),
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn finish(&self) -> Result<(), RadrootsSimplexAgentProtoError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(RadrootsSimplexAgentProtoError::TrailingBytes)
        }
    }

    fn read_byte(&mut self) -> Result<u8, RadrootsSimplexAgentProtoError> {
        let Some(value) = self.bytes.get(self.position) else {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        };
        self.position += 1;
        Ok(*value)
    }

    fn expect_tag(&mut self, tag: &[u8]) -> Result<(), RadrootsSimplexAgentProtoError> {
        let Some(value) = self.bytes.get(self.position..self.position + tag.len()) else {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        };
        if value != tag {
            return Err(RadrootsSimplexAgentProtoError::InvalidTag(
                String::from_utf8_lossy(value).into_owned(),
            ));
        }
        self.position += tag.len();
        Ok(())
    }

    fn read_u16(&mut self) -> Result<u16, RadrootsSimplexAgentProtoError> {
        let Some(value) = self.bytes.get(self.position..self.position + 2) else {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        };
        self.position += 2;
        Ok(u16::from_be_bytes([value[0], value[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, RadrootsSimplexAgentProtoError> {
        let Some(value) = self.bytes.get(self.position..self.position + 4) else {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        };
        self.position += 4;
        Ok(u32::from_be_bytes([value[0], value[1], value[2], value[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, RadrootsSimplexAgentProtoError> {
        let Some(value) = self.bytes.get(self.position..self.position + 8) else {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        };
        self.position += 8;
        Ok(u64::from_be_bytes([
            value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
        ]))
    }

    fn read_short_bytes(&mut self) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
        let length = self.read_byte()? as usize;
        let Some(value) = self.bytes.get(self.position..self.position + length) else {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        };
        self.position += length;
        Ok(value.to_vec())
    }

    fn read_large_bytes(&mut self) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
        let length = self.read_u16()? as usize;
        let Some(value) = self.bytes.get(self.position..self.position + length) else {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        };
        self.position += length;
        Ok(value.to_vec())
    }

    fn read_maybe<T>(
        &mut self,
        decode: fn(&mut Cursor<'_>) -> Result<T, RadrootsSimplexAgentProtoError>,
    ) -> Result<Option<T>, RadrootsSimplexAgentProtoError> {
        match self.read_byte()? {
            0 => Ok(None),
            1 => decode(self).map(Some),
            other => Err(RadrootsSimplexAgentProtoError::InvalidBoolEncoding(other)),
        }
    }

    fn read_list<T>(
        &mut self,
        decode: fn(&mut Cursor<'_>) -> Result<T, RadrootsSimplexAgentProtoError>,
    ) -> Result<Vec<T>, RadrootsSimplexAgentProtoError> {
        let len = self.read_byte()? as usize;
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            values.push(decode(self)?);
        }
        Ok(values)
    }

    fn read_remaining(&self) -> &'a [u8] {
        &self.bytes[self.position..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_simplex_smp_proto::prelude::{
        RadrootsSimplexSmpQueueMode, RadrootsSimplexSmpQueueUri, RadrootsSimplexSmpVersionRange,
    };

    fn sample_queue_uri() -> RadrootsSimplexSmpQueueUri {
        RadrootsSimplexSmpQueueUri::parse(
            "smp://aGVsbG8@relay.example/cXVldWU#/?v=4&dh=Zm9vYmFy&q=m",
        )
        .unwrap()
    }

    #[test]
    fn roundtrips_connection_link() {
        let link = RadrootsSimplexAgentConnectionLink {
            invitation_queue: sample_queue_uri(),
            connection_id: b"conn-1".to_vec(),
            e2e_public_key: b"e2e".to_vec(),
            contact_address: true,
        };
        let encoded = encode_connection_link(&link).unwrap();
        let decoded = decode_connection_link(&encoded).unwrap();
        assert_eq!(decoded, link);
    }

    #[test]
    fn roundtrips_message_frame_and_envelope() {
        let descriptor = RadrootsSimplexAgentQueueDescriptor {
            queue_uri: sample_queue_uri(),
            replaced_queue: None,
            primary: true,
            sender_key: Some(b"sender-key".to_vec()),
        };
        let frame = RadrootsSimplexAgentMessageFrame {
            header: RadrootsSimplexAgentMessageHeader {
                message_id: 7,
                previous_message_hash: b"hash".to_vec(),
            },
            message: RadrootsSimplexAgentMessage::QueueAdd(vec![descriptor.clone()]),
            padding: b"pad".to_vec(),
        };
        let decrypted = RadrootsSimplexAgentDecryptedMessage::Message(frame);
        let encoded_decrypted = encode_decrypted_message(&decrypted).unwrap();
        let decoded_decrypted = decode_decrypted_message(&encoded_decrypted).unwrap();
        assert_eq!(decoded_decrypted, decrypted);

        let envelope =
            RadrootsSimplexAgentEnvelope::Message(RadrootsSimplexAgentEncryptedPayload {
                ratchet_header: Some(RadrootsSimplexSmpRatchetHeader {
                    previous_sending_chain_length: 1,
                    message_number: 2,
                    dh_public_key: b"dh".to_vec(),
                    pq_public_key: Some(b"pq".to_vec()),
                    pq_ciphertext: Some(b"ct".to_vec()),
                }),
                ciphertext: encoded_decrypted,
            });
        let encoded_envelope = encode_envelope(&envelope).unwrap();
        let decoded_envelope = decode_envelope(&encoded_envelope).unwrap();
        assert_eq!(decoded_envelope, envelope);

        assert_eq!(
            descriptor.client_version_range(),
            RadrootsSimplexSmpVersionRange::single(4)
        );
        assert_eq!(
            descriptor.queue_uri.queue_mode,
            Some(RadrootsSimplexSmpQueueMode::Messaging)
        );
    }
}
