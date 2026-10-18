//! Canonical NIP-01 bytes. These data codecs authenticate no event or command.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;
use radroots_identity::PublicKey;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Fixed codec failures, without private input in diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireError {
    /// Data does not fit the selected profile.
    InvalidInput,
    /// Complete serialized output exceeds its selected byte budget.
    LimitExceeded,
}
impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid event profile",
            Self::LimitExceeded => "event byte limit exceeded",
        })
    }
}
impl core::error::Error for WireError {}

/// Borrowed exact fields. No registry, clock, participant or grant is inferred.
pub struct EventFields<'a> {
    /// Validated x-only author key.
    pub author: PublicKey,
    /// Exact NIP-01 integer timestamp; business profiles apply their own bounds.
    pub created_at: u64,
    /// Exact NIP-01 kind.
    pub kind: u32,
    /// Ordered tags and fields, without sorting or normalization.
    pub tags: &'a [Vec<String>],
    /// Exact UTF-8 content.
    pub content: &'a str,
}
impl fmt::Debug for EventFields<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EventFields(<redacted>)")
    }
}

fn add(total: &mut usize, amount: usize, limit: usize) -> Result<(), WireError> {
    *total = total
        .checked_add(amount)
        .filter(|n| *n <= limit)
        .ok_or(WireError::LimitExceeded)?;
    Ok(())
}
fn string_len(value: &str, total: &mut usize, limit: usize) -> Result<(), WireError> {
    add(total, 2, limit)?;
    for byte in value.bytes() {
        add(
            total,
            match byte {
                b'"' | b'\\' | b'\n' | b'\r' | b'\t' | 8 | 12 => 2,
                0..=31 => 6,
                _ => 1,
            },
            limit,
        )?;
    }
    Ok(())
}
/// Counts complete canonical preimage bytes before allocating the output.
pub fn preimage_len(fields: &EventFields<'_>, limit: usize) -> Result<usize, WireError> {
    let mut size = 3 + 66 + 1;
    add(
        &mut size,
        fields.created_at.checked_ilog10().unwrap_or(0) as usize + 1,
        limit,
    )?;
    add(
        &mut size,
        1 + fields.kind.checked_ilog10().unwrap_or(0) as usize + 1 + 2,
        limit,
    )?;
    for (index, tag) in fields.tags.iter().enumerate() {
        add(&mut size, 2 + usize::from(index != 0), limit)?;
        for (index, value) in tag.iter().enumerate() {
            add(&mut size, usize::from(index != 0), limit)?;
            string_len(value, &mut size, limit)?;
        }
    }
    add(&mut size, 2, limit)?;
    string_len(fields.content, &mut size, limit)?;
    add(&mut size, 1, limit)?;
    Ok(size)
}

fn push_string(target: &mut String, value: &str) {
    target.push('"');
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for character in value.chars() {
        match character {
            '"' => target.push_str("\\\""),
            '\\' => target.push_str("\\\\"),
            '\n' => target.push_str("\\n"),
            '\r' => target.push_str("\\r"),
            '\t' => target.push_str("\\t"),
            '\u{08}' => target.push_str("\\b"),
            '\u{0c}' => target.push_str("\\f"),
            '\u{00}'..='\u{1f}' => {
                let value = character as usize;
                target.push_str("\\u00");
                target.push(HEX[value >> 4] as char);
                target.push(HEX[value & 15] as char);
            }
            _ => target.push(character),
        }
    }
    target.push('"');
}
fn push_tags(target: &mut String, tags: &[Vec<String>]) {
    target.push('[');
    for (index, tag) in tags.iter().enumerate() {
        if index != 0 {
            target.push(',');
        }
        target.push('[');
        for (index, value) in tag.iter().enumerate() {
            if index != 0 {
                target.push(',');
            }
            push_string(target, value);
        }
        target.push(']');
    }
    target.push(']');
}
fn append_preimage(target: &mut String, fields: &EventFields<'_>) {
    target.push_str("[0,");
    push_string(target, &fields.author.to_hex());
    target.push(',');
    target.push_str(&fields.created_at.to_string());
    target.push(',');
    target.push_str(&fields.kind.to_string());
    target.push(',');
    push_tags(target, fields.tags);
    target.push(',');
    push_string(target, fields.content);
    target.push(']');
}
/// Retains the original wire-v1 serialization semantics, including unrestricted
/// NIP-01 integer timestamps. Callers must bound their input; new private
/// profiles use the explicitly bounded encoder instead.
pub fn canonical_preimage(fields: &EventFields<'_>) -> String {
    let mut target = String::new();
    append_preimage(&mut target, fields);
    target
}
/// Checks the complete preimage before allocating private serialized bytes.
pub(crate) fn bounded_preimage(
    fields: &EventFields<'_>,
    limit: usize,
) -> Result<Zeroizing<String>, WireError> {
    let size = preimage_len(fields, limit)?;
    let mut target = Zeroizing::new(String::with_capacity(size));
    append_preimage(&mut target, fields);
    Ok(target)
}
/// Counts complete signed JSON, including the exact ID and 64-byte signature.
pub fn signed_json_len(fields: &EventFields<'_>, limit: usize) -> Result<usize, WireError> {
    let mut size = preimage_len(fields, limit)?;
    add(&mut size, 253, limit)?;
    Ok(size)
}
fn hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 15) as usize] as char);
    }
    output
}
/// Owned exact unsigned JSON and its computed event ID; no signature or
/// authenticated participant claim is supplied by this value.
pub struct UnsignedEvent {
    id: [u8; 32],
    json: Zeroizing<String>,
}
impl UnsignedEvent {
    /// Checks the complete rumor/object size before creating private output.
    pub fn encode(fields: &EventFields<'_>, limit: usize) -> Result<Self, WireError> {
        let preimage = bounded_preimage(
            fields,
            limit.checked_sub(116).ok_or(WireError::LimitExceeded)?,
        )?;
        let id = Sha256::digest(preimage.as_bytes()).into();
        let mut json = Zeroizing::new(String::with_capacity(preimage.len() + 116));
        json.push_str("{\"id\":");
        push_string(&mut json, &hex(&id));
        json.push_str(",\"pubkey\":");
        push_string(&mut json, &fields.author.to_hex());
        json.push_str(",\"created_at\":");
        json.push_str(&fields.created_at.to_string());
        json.push_str(",\"kind\":");
        json.push_str(&fields.kind.to_string());
        json.push_str(",\"tags\":");
        push_tags(&mut json, fields.tags);
        json.push_str(",\"content\":");
        push_string(&mut json, fields.content);
        json.push('}');
        Ok(Self { id, json })
    }
    /// Returns the exact SHA-256 event ID.
    pub const fn event_id(&self) -> [u8; 32] {
        self.id
    }
    /// Borrows the exact unsigned JSON, without cloning private content.
    pub fn json(&self) -> &str {
        &self.json
    }
}
impl fmt::Debug for UnsignedEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UnsignedEvent(<redacted>)")
    }
}
