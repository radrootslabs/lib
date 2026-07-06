use crate::{
    RADROOTS_MESH_FRAME_VERSION, RadrootsMeshError, RadrootsMeshEventHead, RadrootsMeshFrame,
    RadrootsMeshPayloadPolicy, RadrootsMeshScope,
};
use alloc::string::String;
use alloc::vec::Vec;

pub fn encode_mesh_frame_cbor(frame: &RadrootsMeshFrame) -> Result<Vec<u8>, RadrootsMeshError> {
    frame.validate()?;
    let mut output = Vec::new();
    encode_map_len(&mut output, 5);
    encode_uint(&mut output, 1);
    encode_uint(&mut output, u64::from(frame.version));
    encode_uint(&mut output, 2);
    encode_text(&mut output, &frame.scope.cbor_label())?;
    encode_uint(&mut output, 3);
    encode_text(&mut output, frame.payload_policy.label())?;
    encode_uint(&mut output, 4);
    encode_array_len(&mut output, frame.event_heads.len() as u64);
    for head in &frame.event_heads {
        encode_event_head(&mut output, head)?;
    }
    encode_uint(&mut output, 5);
    output.push(0xf6);
    Ok(output)
}

pub fn decode_mesh_frame_cbor(bytes: &[u8]) -> Result<RadrootsMeshFrame, RadrootsMeshError> {
    let mut cursor = Cursor::new(bytes);
    cursor.expect_map_len(5)?;
    cursor.expect_uint(1)?;
    let version = cursor.read_uint()? as u16;
    if version != RADROOTS_MESH_FRAME_VERSION {
        return Err(RadrootsMeshError::UnsupportedVersion);
    }
    cursor.expect_uint(2)?;
    let scope = RadrootsMeshScope::parse(&cursor.read_text()?)?;
    cursor.expect_uint(3)?;
    let payload_policy = RadrootsMeshPayloadPolicy::parse(&cursor.read_text()?)?;
    cursor.expect_uint(4)?;
    let head_count = cursor.read_array_len()? as usize;
    let mut event_heads = Vec::with_capacity(head_count);
    for _ in 0..head_count {
        event_heads.push(decode_event_head(&mut cursor)?);
    }
    cursor.expect_uint(5)?;
    cursor.expect_null()?;
    cursor.finish()?;
    let frame = RadrootsMeshFrame {
        version,
        scope,
        payload_policy,
        event_heads,
        payload: None,
    };
    frame.validate()?;
    Ok(frame)
}

fn encode_event_head(
    output: &mut Vec<u8>,
    head: &RadrootsMeshEventHead,
) -> Result<(), RadrootsMeshError> {
    encode_map_len(output, 4);
    encode_uint(output, 1);
    encode_text(output, &head.event_id)?;
    encode_uint(output, 2);
    encode_text(output, &head.author)?;
    encode_uint(output, 3);
    encode_uint(output, u64::from(head.kind));
    encode_uint(output, 4);
    encode_uint(output, head.created_at);
    Ok(())
}

fn decode_event_head(cursor: &mut Cursor<'_>) -> Result<RadrootsMeshEventHead, RadrootsMeshError> {
    cursor.expect_map_len(4)?;
    cursor.expect_uint(1)?;
    let event_id = cursor.read_text()?;
    cursor.expect_uint(2)?;
    let author = cursor.read_text()?;
    cursor.expect_uint(3)?;
    let kind = cursor.read_uint()? as u32;
    cursor.expect_uint(4)?;
    let created_at = cursor.read_uint()?;
    Ok(RadrootsMeshEventHead {
        event_id,
        author,
        kind,
        created_at,
    })
}

fn encode_uint(output: &mut Vec<u8>, value: u64) {
    encode_major(output, 0, value);
}

fn encode_text(output: &mut Vec<u8>, value: &str) -> Result<(), RadrootsMeshError> {
    encode_major(output, 3, value.len() as u64);
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn encode_array_len(output: &mut Vec<u8>, len: u64) {
    encode_major(output, 4, len);
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
            24 => Ok(u64::from(self.read_byte()?)),
            25 => {
                let bytes = self.read_exact(2)?;
                Ok(u64::from(u16::from_be_bytes([bytes[0], bytes[1]])))
            }
            26 => {
                let bytes = self.read_exact(4)?;
                Ok(u64::from(u32::from_be_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3],
                ])))
            }
            27 => {
                let bytes = self.read_exact(8)?;
                Ok(u64::from_be_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]))
            }
            _ => Err(RadrootsMeshError::InvalidCbor),
        }
    }

    fn read_uint(&mut self) -> Result<u64, RadrootsMeshError> {
        self.read_major(0)
    }

    fn expect_uint(&mut self, expected: u64) -> Result<(), RadrootsMeshError> {
        if self.read_uint()? == expected {
            Ok(())
        } else {
            Err(RadrootsMeshError::InvalidCbor)
        }
    }

    fn read_text(&mut self) -> Result<String, RadrootsMeshError> {
        let len = self.read_major(3)? as usize;
        let bytes = self.read_exact(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| RadrootsMeshError::InvalidUtf8)
    }

    fn read_array_len(&mut self) -> Result<u64, RadrootsMeshError> {
        self.read_major(4)
    }

    fn expect_map_len(&mut self, expected: u64) -> Result<(), RadrootsMeshError> {
        if self.read_major(5)? == expected {
            Ok(())
        } else {
            Err(RadrootsMeshError::InvalidCbor)
        }
    }

    fn expect_null(&mut self) -> Result<(), RadrootsMeshError> {
        if self.read_byte()? == 0xf6 {
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
