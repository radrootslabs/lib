//! Bounded opening for the existing protocol-only NIP-17 adapter.
use super::{Error, Event, Kind, NostrSigner, PublicKey, UnsignedEvent};
use alloc::{string::String, vec::Vec};
use base64::Engine as _;
use nostr::JsonUtil;
use radroots_event::wire::EventWireLimits;
use radroots_protocol::{
    chat::{RUMOR_MAX_BYTES, SIGNED_MAX_BYTES},
    nip01::{EventFields, signed_json_len},
};
use serde::Deserialize;
use serde_json::value::RawValue;
use zeroize::{Zeroize as _, Zeroizing};

pub(super) struct OpenedRumor(pub(super) UnsignedEvent);
impl Drop for OpenedRumor {
    fn drop(&mut self) {
        self.0.content.zeroize();
        for tag in core::mem::take(&mut self.0.tags) {
            tag.to_vec().zeroize();
        }
    }
}

fn ciphertext(value: &str) -> Result<(), Error> {
    if value.len() > SIGNED_MAX_BYTES {
        return Err(Error::GiftWrap);
    }
    let raw = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| Error::GiftWrap)?;
    if raw.first() != Some(&2) || raw.len() < 99 || (raw.len() - 67) % 32 != 0 {
        return Err(Error::GiftWrap);
    }
    Ok(())
}

fn frame(raw: &str, limit: usize) -> Result<(), Error> {
    if raw.len() > limit || !raw.trim_start().starts_with('{') {
        return Err(Error::GiftWrap);
    }
    let (mut quoted, mut escaped, mut depth) = (false, false, 0u8);
    for byte in raw.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth = depth
                        .checked_add(1)
                        .filter(|n| *n <= 16)
                        .ok_or(Error::GiftWrap)?;
                }
                b'}' | b']' => {
                    depth = depth.checked_sub(1).ok_or(Error::GiftWrap)?;
                }
                _ => {}
            }
        }
    }
    if quoted || depth != 0 {
        return Err(Error::GiftWrap);
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRumor<'a> {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u16,
    #[serde(borrow)]
    tags: &'a RawValue,
    #[serde(borrow)]
    content: &'a RawValue,
}

fn rumor(raw: &str, author: PublicKey, recipient: PublicKey) -> Result<OpenedRumor, Error> {
    frame(raw, RUMOR_MAX_BYTES)?;
    let fields: RawRumor<'_> = serde_json::from_str(raw).map_err(|_| Error::GiftWrap)?;
    if fields.created_at > 253_402_300_799
        || fields.pubkey != author.to_hex()
        || !fields.tags.get().trim_start().starts_with('[')
        || !fields.content.get().trim_start().starts_with('"')
    {
        return Err(Error::GiftWrap);
    }
    let declared = nostr::EventId::from_hex(&fields.id).map_err(|_| Error::GiftWrap)?;
    if declared.to_hex() != fields.id {
        return Err(Error::GiftWrap);
    }
    let opened = OpenedRumor(UnsignedEvent::from_json(raw).map_err(|_| Error::GiftWrap)?);
    let value = &opened.0;
    if value.id != Some(declared) || value.kind.as_u16() != fields.kind {
        return Err(Error::GiftWrap);
    }
    value.verify_id().map_err(|_| Error::GiftWrap)?;
    if !matches!(fields.kind, 14 | 15) {
        return Ok(opened);
    }
    let mut addressed = author == recipient;
    let mut peers = Vec::new();
    for tag in value.tags.as_slice() {
        let fields = tag.as_slice();
        if fields.first().map(String::as_str) == Some("p") {
            if !(2..=3).contains(&fields.len()) {
                return Err(Error::InvalidRecipient);
            }
            let peer = PublicKey::from_hex(&fields[1]).map_err(|_| Error::InvalidRecipient)?;
            if peer.to_hex() != fields[1] || peers.contains(&peer) {
                return Err(Error::InvalidRecipient);
            }
            peers.push(peer);
            addressed |= peer == recipient;
        }
    }
    if !addressed || peers.is_empty() {
        return Err(Error::InvalidRecipient);
    }
    Ok(opened)
}

fn signed(raw: &str) -> Result<radroots_event::envelope::EventEnvelope, Error> {
    crate::event_verify::signed_json(raw).map_err(|_| Error::GiftWrap)
}

fn outer(event: &Event) -> Result<(), Error> {
    let limits = EventWireLimits::default();
    if event.kind != Kind::GiftWrap
        || event.content.len() > SIGNED_MAX_BYTES
        || event.tags.len() > limits.max_tag_count
    {
        return Err(Error::GiftWrap);
    }
    let mut elements = 0usize;
    let mut bytes = 0usize;
    for tag in event.tags.as_slice() {
        for field in tag.as_slice() {
            elements += 1;
            bytes = bytes.checked_add(field.len()).ok_or(Error::GiftWrap)?;
            if elements > limits.max_total_tag_elements
                || bytes > limits.max_total_tag_bytes
                || field.len() > limits.max_tag_element_bytes
            {
                return Err(Error::GiftWrap);
            }
        }
    }
    let tags = event
        .tags
        .iter()
        .map(|t| t.as_slice().to_vec())
        .collect::<Vec<_>>();
    let author = radroots_identity::PublicKey::from_hex(&event.pubkey.to_hex())
        .map_err(|_| Error::GiftWrap)?;
    signed_json_len(
        &EventFields {
            author,
            created_at: event.created_at.as_secs(),
            kind: u32::from(event.kind.as_u16()),
            tags: &tags,
            content: &event.content,
        },
        SIGNED_MAX_BYTES,
    )
    .map_err(|_| Error::GiftWrap)?;
    signed(&event.as_json())?;
    Ok(())
}

async fn identity<T: NostrSigner>(signer: &T, expected: PublicKey) -> Result<(), Error> {
    if signer.get_public_key().await? != expected {
        return Err(Error::Signer);
    }
    Ok(())
}

pub(super) async fn open<T: NostrSigner>(signer: &T, event: &Event) -> Result<OpenedRumor, Error> {
    outer(event)?;
    let recipient = signer.get_public_key().await?;
    let mut selected = None;
    for tag in event.tags.as_slice() {
        let fields = tag.as_slice();
        if fields.first().map(String::as_str) == Some("p") {
            if selected.is_some()
                || !(2..=3).contains(&fields.len())
                || fields[1] != recipient.to_hex()
            {
                return Err(Error::InvalidRecipient);
            }
            selected = Some(recipient);
        }
    }
    if selected.is_none() {
        return Err(Error::InvalidRecipient);
    }
    ciphertext(&event.content)?;
    let raw = Zeroizing::new(signer.nip44_decrypt(&event.pubkey, &event.content).await?);
    identity(signer, recipient).await?;
    frame(&raw, SIGNED_MAX_BYTES)?;
    let seal = signed(&raw)?;
    if seal.kind_u32() != 13 || !seal.tags_as_vec().is_empty() {
        return Err(Error::GiftWrap);
    }
    let author = crate::key::public_key_to_nostr(*seal.author()).map_err(|_| Error::GiftWrap)?;
    ciphertext(seal.content())?;
    let raw = Zeroizing::new(signer.nip44_decrypt(&author, seal.content()).await?);
    identity(signer, recipient).await?;
    rumor(&raw, author, recipient)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_bounds_include_escaped_string_delimiters_and_exact_depth() {
        for raw in [
            r#"{"text":"\"\\{[}]"}"#.into(),
            format!("{{\"value\":{}0{}}}", "[".repeat(15), "]".repeat(15)),
        ] {
            assert!(frame(&raw, raw.len()).is_ok());
            assert!(frame(&raw, raw.len() - 1).is_err());
        }
        for raw in [
            "[]".into(),
            "{".into(),
            "}".into(),
            "{\"unterminated".into(),
            format!("{{\"value\":{}0{}}}", "[".repeat(16), "]".repeat(16)),
        ] {
            assert!(frame(&raw, SIGNED_MAX_BYTES).is_err());
        }
    }

    #[test]
    fn only_bounded_structural_v2_ciphertext_reaches_a_signer() {
        let mut valid = vec![0; 99];
        valid[0] = 2;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&valid);
        assert!(ciphertext(&encoded).is_ok());
        for raw in [
            "#future".into(),
            "!".repeat(132),
            "a".repeat(SIGNED_MAX_BYTES + 1),
            base64::engine::general_purpose::STANDARD.encode([2; 98]),
            base64::engine::general_purpose::STANDARD.encode([3; 99]),
            base64::engine::general_purpose::STANDARD.encode([2; 100]),
        ] {
            assert!(ciphertext(&raw).is_err());
        }
    }
}
