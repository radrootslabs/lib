//! Focused NIP-17/NIP-59 message wrapping and unwrapping.
//!
//! This module creates and opens protocol events only. Relay selection,
//! delivery, retries, persistence, and runtime ownership remain outside this
//! crate.

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use nostr::nips::nip59;
use nostr::{
    Event, EventBuilder, Kind, NostrSigner, PublicKey, Tag, TagKind, Timestamp, UnsignedEvent,
};
use radroots_event::envelope::kind::{KIND_MESSAGE, KIND_MESSAGE_FILE};
use radroots_event::social::message::Message;
use radroots_event::social::message_file::MessageFile;
use radroots_event::wire::Nip01EventWireParts;
use radroots_event_codec::decode::RadrootsParsedData;
use radroots_event_codec::decode::message_file as message_file_decode;
use radroots_event_codec::decode::{EventParseError, message as message_decode};
use radroots_event_codec::encode::message_file as message_file_encode;
use radroots_event_codec::encode::{EventEncodeError, message as message_encode};

/// Stable, source-redacted failures from the focused NIP-17 adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RadrootsNip17Error {
    MessageEncode,
    MessageDecode,
    GiftWrap,
    Signer,
    InvalidRecipient,
    UnsupportedRumorKind { kind: u32 },
}

impl RadrootsNip17Error {
    /// Returns a stable machine-readable failure code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MessageEncode => "message_encode",
            Self::MessageDecode => "message_decode",
            Self::GiftWrap => "gift_wrap",
            Self::Signer => "signer",
            Self::InvalidRecipient => "invalid_recipient",
            Self::UnsupportedRumorKind { .. } => "unsupported_rumor_kind",
        }
    }
}

impl core::fmt::Display for RadrootsNip17Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MessageEncode => formatter.write_str("failed to encode NIP-17 message"),
            Self::MessageDecode => formatter.write_str("failed to decode NIP-17 message"),
            Self::GiftWrap => formatter.write_str("failed to process NIP-59 gift wrap"),
            Self::Signer => formatter.write_str("NIP-17 signer failed"),
            Self::InvalidRecipient => formatter.write_str("NIP-17 recipient is invalid"),
            Self::UnsupportedRumorKind { kind } => {
                write!(formatter, "unsupported NIP-17 rumor kind {kind}")
            }
        }
    }
}

impl core::error::Error for RadrootsNip17Error {}

impl From<EventEncodeError> for RadrootsNip17Error {
    fn from(_: EventEncodeError) -> Self {
        Self::MessageEncode
    }
}

impl From<EventParseError> for RadrootsNip17Error {
    fn from(_: EventParseError) -> Self {
        Self::MessageDecode
    }
}

impl From<nip59::Error> for RadrootsNip17Error {
    fn from(_: nip59::Error) -> Self {
        Self::GiftWrap
    }
}

impl From<nostr::event::builder::Error> for RadrootsNip17Error {
    fn from(_: nostr::event::builder::Error) -> Self {
        Self::GiftWrap
    }
}

impl From<nostr::signer::SignerError> for RadrootsNip17Error {
    fn from(_: nostr::signer::SignerError) -> Self {
        Self::Signer
    }
}

impl From<nostr::key::Error> for RadrootsNip17Error {
    fn from(_: nostr::key::Error) -> Self {
        Self::InvalidRecipient
    }
}

#[derive(Clone)]
pub enum RadrootsNip17Rumor {
    Message(RadrootsParsedData<Message>),
    MessageFile(Box<RadrootsParsedData<MessageFile>>),
}

impl core::fmt::Debug for RadrootsNip17Rumor {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Message(_) => formatter
                .debug_struct("RadrootsNip17Rumor::Message")
                .finish_non_exhaustive(),
            Self::MessageFile(_) => formatter
                .debug_struct("RadrootsNip17Rumor::MessageFile")
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RadrootsNip17WrapOptions {
    pub include_sender: bool,
    pub rumor_created_at: Option<u64>,
    pub gift_wrap_tags: Vec<Vec<String>>,
}

impl Default for RadrootsNip17WrapOptions {
    fn default() -> Self {
        Self {
            include_sender: true,
            rumor_created_at: None,
            gift_wrap_tags: Vec::new(),
        }
    }
}

fn tags_from_slices(tag_slices: &[Vec<String>]) -> Vec<Tag> {
    let mut tags = Vec::with_capacity(tag_slices.len());
    for slice in tag_slices {
        if slice.is_empty() {
            continue;
        }
        let key = slice[0].clone();
        let values = slice[1..].to_vec();
        tags.push(Tag::custom(TagKind::Custom(key.into()), values));
    }
    tags
}

fn rumor_from_parts(
    parts: Nip01EventWireParts,
    author: PublicKey,
    created_at: Option<u64>,
) -> Result<UnsignedEvent, RadrootsNip17Error> {
    let kind = u16::try_from(parts.kind)
        .map_err(|_| RadrootsNip17Error::UnsupportedRumorKind { kind: parts.kind })?;
    let tags = tags_from_slices(&parts.tags);
    let timestamp = match created_at {
        Some(ts) => Timestamp::from_secs(ts),
        None => Timestamp::now(),
    };
    let mut rumor = UnsignedEvent::new(author, timestamp, Kind::Custom(kind), tags, parts.content);
    rumor.ensure_id();
    Ok(rumor)
}

fn parse_recipients(
    recipients: &[radroots_event::social::message::MessageRecipient],
) -> Result<Vec<PublicKey>, RadrootsNip17Error> {
    let mut out = Vec::with_capacity(recipients.len());
    for recipient in recipients {
        out.push(recipient.public_key.parse::<PublicKey>()?);
    }
    Ok(out)
}

fn push_unique(recipients: &mut Vec<PublicKey>, pubkey: PublicKey) {
    if recipients.iter().any(|r| r == &pubkey) {
        return;
    }
    recipients.push(pubkey);
}

async fn wrap_rumor<T>(
    signer: &T,
    rumor: UnsignedEvent,
    mut recipients: Vec<PublicKey>,
    options: &RadrootsNip17WrapOptions,
) -> Result<Vec<Event>, RadrootsNip17Error>
where
    T: NostrSigner,
{
    let sender_pubkey = signer.get_public_key().await?;
    if options.include_sender {
        push_unique(&mut recipients, sender_pubkey);
    }
    let extra_tags = tags_from_slices(&options.gift_wrap_tags);

    let mut out = Vec::with_capacity(recipients.len());
    for recipient in recipients {
        let event =
            EventBuilder::gift_wrap(signer, &recipient, rumor.clone(), extra_tags.clone()).await?;
        out.push(event);
    }
    Ok(out)
}

pub async fn radroots_nostr_wrap_message<T>(
    signer: &T,
    message: &Message,
    options: RadrootsNip17WrapOptions,
) -> Result<Vec<Event>, RadrootsNip17Error>
where
    T: NostrSigner,
{
    let parts = message_encode::to_wire_parts(message)?;
    let author = signer.get_public_key().await?;
    let rumor = rumor_from_parts(parts, author, options.rumor_created_at)?;
    let recipients = parse_recipients(&message.recipients)?;
    wrap_rumor(signer, rumor, recipients, &options).await
}

pub async fn radroots_nostr_wrap_message_file<T>(
    signer: &T,
    message: &MessageFile,
    options: RadrootsNip17WrapOptions,
) -> Result<Vec<Event>, RadrootsNip17Error>
where
    T: NostrSigner,
{
    let parts = message_file_encode::to_wire_parts(message)?;
    let author = signer.get_public_key().await?;
    let rumor = rumor_from_parts(parts, author, options.rumor_created_at)?;
    let recipients = parse_recipients(&message.recipients)?;
    wrap_rumor(signer, rumor, recipients, &options).await
}

pub async fn radroots_nostr_unwrap_gift_wrap<T>(
    signer: &T,
    gift_wrap: &Event,
) -> Result<RadrootsNip17Rumor, RadrootsNip17Error>
where
    T: NostrSigner,
{
    let unwrapped = nip59::extract_rumor(signer, gift_wrap).await?;
    let mut rumor = unwrapped.rumor;
    let id = rumor.id().to_string();
    let author = rumor.pubkey.to_string();
    let published_at = rumor.created_at.as_secs();
    let kind = rumor.kind.as_u16() as u32;
    let tags: Vec<Vec<String>> = rumor
        .tags
        .as_slice()
        .iter()
        .map(|t| t.as_slice().to_vec())
        .collect();
    let content = rumor.content.clone();

    match kind {
        KIND_MESSAGE => {
            let metadata =
                message_decode::data_from_event(id, author, published_at, kind, content, tags)?;
            Ok(RadrootsNip17Rumor::Message(metadata))
        }
        KIND_MESSAGE_FILE => {
            let metadata = message_file_decode::data_from_event(
                id,
                author,
                published_at,
                kind,
                content,
                tags,
            )?;
            Ok(RadrootsNip17Rumor::MessageFile(Box::new(metadata)))
        }
        other => Err(RadrootsNip17Error::UnsupportedRumorKind { kind: other }),
    }
}

#[cfg(all(test, feature = "nip17"))]
mod tests {
    use super::*;
    use crate::test_fixtures::{FIXTURE_ALICE, FIXTURE_BOB};
    use nostr::{Keys, SecretKey};
    use radroots_event::social::message::{Message, MessageRecipient};
    use radroots_event::social::message_file::{MessageFile, MessageFileDimensions};

    fn sender_keys() -> Keys {
        Keys::new(SecretKey::from_hex(FIXTURE_ALICE.secret_key_hex).unwrap())
    }

    fn receiver_keys() -> Keys {
        Keys::new(SecretKey::from_hex(FIXTURE_BOB.secret_key_hex).unwrap())
    }

    #[test]
    fn rumor_kind_conversion_is_range_checked() {
        let author = sender_keys().public_key();
        let max = rumor_from_parts(
            Nip01EventWireParts {
                kind: u32::from(u16::MAX),
                content: String::new(),
                tags: Vec::new(),
            },
            author,
            Some(1_700_000_000),
        )
        .expect("maximum NIP-01 kind");
        assert_eq!(max.kind.as_u16(), u16::MAX);

        let overflow = u32::from(u16::MAX) + 1;
        assert!(matches!(
            rumor_from_parts(
                Nip01EventWireParts {
                    kind: overflow,
                    content: String::new(),
                    tags: Vec::new(),
                },
                author,
                Some(1_700_000_000),
            ),
            Err(RadrootsNip17Error::UnsupportedRumorKind { kind }) if kind == overflow
        ));
    }

    #[test]
    fn adapter_error_codes_are_stable_and_source_redacted() {
        let errors = [
            RadrootsNip17Error::MessageEncode,
            RadrootsNip17Error::MessageDecode,
            RadrootsNip17Error::GiftWrap,
            RadrootsNip17Error::Signer,
            RadrootsNip17Error::InvalidRecipient,
            RadrootsNip17Error::UnsupportedRumorKind { kind: 70_000 },
        ];
        assert_eq!(
            errors
                .iter()
                .map(RadrootsNip17Error::code)
                .collect::<Vec<_>>(),
            vec![
                "message_encode",
                "message_decode",
                "gift_wrap",
                "signer",
                "invalid_recipient",
                "unsupported_rumor_kind",
            ]
        );
        for error in &errors {
            let rendered = format!("{error:?} {error}");
            assert!(!rendered.contains("nsec"));
            assert!(!rendered.contains("private"));
        }
    }

    #[tokio::test]
    async fn wrap_and_unwrap_message() {
        let sender = sender_keys();
        let receiver = receiver_keys();
        let message = Message {
            recipients: vec![MessageRecipient {
                public_key: receiver.public_key().to_string(),
                relay_url: None,
            }],
            content: "hello".to_string(),
            reply_to: None,
            subject: None,
        };
        let options = RadrootsNip17WrapOptions {
            include_sender: false,
            rumor_created_at: Some(1700000000),
            gift_wrap_tags: Vec::new(),
        };

        let events = radroots_nostr_wrap_message(&sender, &message, options)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);

        let rumor = radroots_nostr_unwrap_gift_wrap(&receiver, &events[0])
            .await
            .unwrap();
        match rumor {
            RadrootsNip17Rumor::Message(metadata) => {
                assert_eq!(metadata.data.content, "hello");
                assert_eq!(metadata.data.recipients.len(), 1);
            }
            other => panic!("expected message rumor, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn wrap_and_unwrap_message_file() {
        let sender = sender_keys();
        let receiver = receiver_keys();
        let message = MessageFile {
            recipients: vec![MessageRecipient {
                public_key: receiver.public_key().to_string(),
                relay_url: None,
            }],
            file_url: "https://files.example/encrypted.bin".to_string(),
            reply_to: None,
            subject: None,
            file_type: "image/jpeg".to_string(),
            encryption_algorithm: "aes-gcm".to_string(),
            decryption_key: "key".to_string(),
            decryption_nonce: "nonce".to_string(),
            encrypted_hash: "hash".to_string(),
            original_hash: None,
            size: Some(1200),
            dimensions: Some(MessageFileDimensions { w: 1200, h: 800 }),
            blurhash: None,
            thumb: None,
            fallbacks: Vec::new(),
        };
        let options = RadrootsNip17WrapOptions {
            include_sender: false,
            rumor_created_at: Some(1700000001),
            gift_wrap_tags: Vec::new(),
        };

        let events = radroots_nostr_wrap_message_file(&sender, &message, options)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);

        let rumor = radroots_nostr_unwrap_gift_wrap(&receiver, &events[0])
            .await
            .unwrap();
        let rendered = format!("{rumor:?}");
        for private_value in [
            message.file_url.as_str(),
            message.decryption_key.as_str(),
            message.decryption_nonce.as_str(),
            message.encrypted_hash.as_str(),
        ] {
            assert!(!rendered.contains(private_value));
        }
        match rumor {
            RadrootsNip17Rumor::MessageFile(metadata) => {
                assert_eq!(metadata.data.file_url, message.file_url);
                assert_eq!(metadata.data.encrypted_hash, message.encrypted_hash);
            }
            other => panic!("expected message file rumor, got {other:?}"),
        }
    }
}
