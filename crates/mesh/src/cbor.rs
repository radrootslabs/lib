use crate::{
    RADROOTS_MESH_FRAME_VERSION, RadrootsMeshError, RadrootsMeshFrame, RadrootsMeshFrameType,
    RadrootsMeshPayload, RadrootsMeshScope,
};
use alloc::string::String;
use alloc::vec::Vec;

pub fn encode_mesh_frame_cbor(frame: &RadrootsMeshFrame) -> Result<Vec<u8>, RadrootsMeshError> {
    frame.validate()?;
    let mut output = Vec::new();
    encode_map_len(&mut output, 7);
    encode_uint(&mut output, 0);
    encode_uint(&mut output, u64::from(frame.version));
    encode_uint(&mut output, 1);
    encode_uint(&mut output, frame.frame_type.code());
    encode_uint(&mut output, 2);
    encode_text(&mut output, &frame.scope_id.cbor_label())?;
    encode_uint(&mut output, 3);
    encode_text(&mut output, &frame.message_id)?;
    encode_uint(&mut output, 4);
    encode_uint(&mut output, frame.created_at_ms);
    encode_uint(&mut output, 5);
    encode_uint(&mut output, frame.ttl);
    encode_uint(&mut output, 6);
    encode_payload(&mut output, &frame.payload)?;
    Ok(output)
}

pub fn decode_mesh_frame_cbor(bytes: &[u8]) -> Result<RadrootsMeshFrame, RadrootsMeshError> {
    let mut cursor = Cursor::new(bytes);
    cursor.expect_map_len(7)?;
    cursor.expect_uint(0)?;
    let version = cursor.read_uint_u16()?;
    if version != RADROOTS_MESH_FRAME_VERSION {
        return Err(RadrootsMeshError::UnsupportedVersion);
    }
    cursor.expect_uint(1)?;
    let frame_type = RadrootsMeshFrameType::parse_code(cursor.read_uint()?)?;
    cursor.expect_uint(2)?;
    let scope_id = RadrootsMeshScope::parse(&cursor.read_text()?)?;
    cursor.expect_uint(3)?;
    let message_id = cursor.read_text()?;
    cursor.expect_uint(4)?;
    let created_at_ms = cursor.read_uint()?;
    cursor.expect_uint(5)?;
    let ttl = cursor.read_uint()?;
    cursor.expect_uint(6)?;
    let payload = cursor.read_payload()?;
    cursor.finish()?;
    let frame = RadrootsMeshFrame {
        version,
        frame_type,
        scope_id,
        message_id,
        created_at_ms,
        ttl,
        payload,
    };
    frame.validate()?;
    Ok(frame)
}

fn encode_payload(
    output: &mut Vec<u8>,
    payload: &RadrootsMeshPayload,
) -> Result<(), RadrootsMeshError> {
    payload.validate()?;
    encode_map_len(output, 0);
    Ok(())
}

fn encode_uint(output: &mut Vec<u8>, value: u64) {
    encode_major(output, 0, value);
}

fn encode_text(output: &mut Vec<u8>, value: &str) -> Result<(), RadrootsMeshError> {
    let len = u64::try_from(value.len()).map_err(|_| RadrootsMeshError::InvalidCbor)?;
    encode_major(output, 3, len);
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn encode_map_len(output: &mut Vec<u8>, len: u64) {
    encode_major(output, 5, len);
}

fn encode_major(output: &mut Vec<u8>, major: u8, value: u64) {
    let prefix = major << 5;
    match value {
        0..=23 => output.push(prefix | value as u8),
        24..=0xff => output.extend_from_slice(&[prefix | 24, value as u8]),
        0x100..=0xffff => {
            output.push(prefix | 25);
            output.extend_from_slice(&(value as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            output.push(prefix | 26);
            output.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            output.push(prefix | 27);
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_byte(&mut self) -> Result<u8, RadrootsMeshError> {
        let byte = self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or(RadrootsMeshError::InvalidCbor)?;
        self.offset += 1;
        Ok(byte)
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], RadrootsMeshError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(RadrootsMeshError::InvalidCbor)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(RadrootsMeshError::InvalidCbor)?;
        self.offset = end;
        Ok(slice)
    }

    fn read_major(&mut self, expected_major: u8) -> Result<u64, RadrootsMeshError> {
        let initial = self.read_byte()?;
        if initial >> 5 != expected_major {
            return Err(RadrootsMeshError::InvalidCbor);
        }
        match initial & 0x1f {
            value @ 0..=23 => Ok(u64::from(value)),
            24 => {
                let value = u64::from(self.read_byte()?);
                if value < 24 {
                    return Err(RadrootsMeshError::InvalidCbor);
                }
                Ok(value)
            }
            25 => {
                let bytes = self.read_exact(2)?;
                let value = u64::from(u16::from_be_bytes([bytes[0], bytes[1]]));
                if value < 0x100 {
                    return Err(RadrootsMeshError::InvalidCbor);
                }
                Ok(value)
            }
            26 => {
                let bytes = self.read_exact(4)?;
                let value = u64::from(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                if value < 0x1_0000 {
                    return Err(RadrootsMeshError::InvalidCbor);
                }
                Ok(value)
            }
            27 => {
                let bytes = self.read_exact(8)?;
                let value = u64::from_be_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                if value < 0x1_0000_0000 {
                    return Err(RadrootsMeshError::InvalidCbor);
                }
                Ok(value)
            }
            _ => Err(RadrootsMeshError::InvalidCbor),
        }
    }

    fn read_uint(&mut self) -> Result<u64, RadrootsMeshError> {
        self.read_major(0)
    }

    fn read_uint_u16(&mut self) -> Result<u16, RadrootsMeshError> {
        u16::try_from(self.read_uint()?).map_err(|_| RadrootsMeshError::InvalidCbor)
    }

    fn expect_uint(&mut self, expected: u64) -> Result<(), RadrootsMeshError> {
        if self.read_uint()? == expected {
            Ok(())
        } else {
            Err(RadrootsMeshError::InvalidCbor)
        }
    }

    fn read_text(&mut self) -> Result<String, RadrootsMeshError> {
        let len =
            usize::try_from(self.read_major(3)?).map_err(|_| RadrootsMeshError::InvalidCbor)?;
        let bytes = self.read_exact(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| RadrootsMeshError::InvalidUtf8)
    }

    fn skip_bytes(&mut self) -> Result<(), RadrootsMeshError> {
        let len =
            usize::try_from(self.read_major(2)?).map_err(|_| RadrootsMeshError::InvalidCbor)?;
        self.read_exact(len)?;
        Ok(())
    }

    fn read_payload(&mut self) -> Result<RadrootsMeshPayload, RadrootsMeshError> {
        let initial = self.read_byte()?;
        match initial >> 5 {
            2 => {
                self.offset -= 1;
                self.skip_bytes()?;
                Err(RadrootsMeshError::PayloadTransmissionForbidden)
            }
            5 => {
                self.offset -= 1;
                if self.read_major(5)? == 0 {
                    Ok(RadrootsMeshPayload::EmptyMap)
                } else {
                    Err(RadrootsMeshError::PayloadTransmissionForbidden)
                }
            }
            _ => Err(RadrootsMeshError::InvalidCbor),
        }
    }

    fn expect_map_len(&mut self, expected: u64) -> Result<(), RadrootsMeshError> {
        if self.read_major(5)? == expected {
            Ok(())
        } else {
            Err(RadrootsMeshError::InvalidCbor)
        }
    }

    fn finish(&self) -> Result<(), RadrootsMeshError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(RadrootsMeshError::InvalidCbor)
        }
    }
}
