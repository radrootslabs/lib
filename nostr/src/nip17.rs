#![forbid(unsafe_code)]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use nostr::{
    Event,
    EventBuilder,
    Kind,
    NostrSigner,
    PublicKey,
    Tag,
    TagKind,
    Timestamp,
    UnsignedEvent,
};
use nostr::nips::nip59;
use thiserror::Error;

use radroots_events::kinds::{KIND_MESSAGE, KIND_MESSAGE_FILE};
use radroots_events::message::{RadrootsMessage, RadrootsMessageEventMetadata};
use radroots_events::message_file::{RadrootsMessageFile, RadrootsMessageFileEventMetadata};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::message::decode as message_decode;
use radroots_events_codec::message::encode as message_encode;
use radroots_events_codec::message_file::decode as message_file_decode;
use radroots_events_codec::message_file::encode as message_file_encode;
use radroots_events_codec::wire::WireEventParts;

use crate::util::created_at_u32_saturating;

#[derive(Debug, Error)]
pub enum RadrootsNip17Error {
    #[error("Message encode error: {0}")]
    MessageEncode(#[from] EventEncodeError),
    #[error("Message decode error: {0}")]
    MessageDecode(#[from] EventParseError),
    #[error("NIP-59 error: {0}")]
    Nip59(#[from] nip59::Error),
    #[error("Event builder error: {0}")]
    EventBuilder(#[from] nostr::event::builder::Error),
    #[error("Signer error: {0}")]
    Signer(#[from] nostr::signer::SignerError),
    #[error("Key error: {0}")]
    Key(#[from] nostr::key::Error),
    #[error("Unsupported rumor kind: {0}")]
    UnsupportedRumorKind(u32),
}

#[derive(Clone, Debug)]
pub enum RadrootsNip17Rumor {
    Message(RadrootsMessageEventMetadata),
    MessageFile(RadrootsMessageFileEventMetadata),
}

#[derive(Clone, Debug)]
pub struct RadrootsNip17WrapOptions {
    pub include_sender: bool,
    pub rumor_created_at: Option<u32>,
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
    parts: WireEventParts,
    author: PublicKey,
    created_at: Option<u32>,
) -> UnsignedEvent {
    let tags = tags_from_slices(&parts.tags);
    let timestamp = match created_at {
        Some(ts) => Timestamp::from_secs(ts as u64),
        None => Timestamp::now(),
    };
    let mut rumor = UnsignedEvent::new(
        author,
        timestamp,
        Kind::Custom(parts.kind as u16),
        tags,
        parts.content,
    );
    rumor.ensure_id();
    rumor
}

fn parse_recipients(recipients: &[radroots_events::message::RadrootsMessageRecipient]) -> Result<Vec<PublicKey>, RadrootsNip17Error> {
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
        let event = EventBuilder::gift_wrap(signer, &recipient, rumor.clone(), extra_tags.clone())
            .await?;
        out.push(event);
    }
    Ok(out)
}

pub async fn radroots_nostr_wrap_message<T>(
    signer: &T,
    message: &RadrootsMessage,
    options: RadrootsNip17WrapOptions,
) -> Result<Vec<Event>, RadrootsNip17Error>
where
    T: NostrSigner,
{
    let parts = message_encode::to_wire_parts(message)?;
    let author = signer.get_public_key().await?;
    let rumor = rumor_from_parts(parts, author, options.rumor_created_at);
    let recipients = parse_recipients(&message.recipients)?;
    wrap_rumor(signer, rumor, recipients, &options).await
}

pub async fn radroots_nostr_wrap_message_file<T>(
    signer: &T,
    message: &RadrootsMessageFile,
    options: RadrootsNip17WrapOptions,
) -> Result<Vec<Event>, RadrootsNip17Error>
where
    T: NostrSigner,
{
    let parts = message_file_encode::to_wire_parts(message)?;
    let author = signer.get_public_key().await?;
    let rumor = rumor_from_parts(parts, author, options.rumor_created_at);
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
    let published_at = created_at_u32_saturating(rumor.created_at);
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
            let metadata = message_decode::metadata_from_event(
                id,
                author,
                published_at,
                kind,
                content,
                tags,
            )?;
            Ok(RadrootsNip17Rumor::Message(metadata))
        }
        KIND_MESSAGE_FILE => {
            let metadata = message_file_decode::metadata_from_event(
                id,
                author,
                published_at,
                kind,
                content,
                tags,
            )?;
            Ok(RadrootsNip17Rumor::MessageFile(metadata))
        }
        other => Err(RadrootsNip17Error::UnsupportedRumorKind(other)),
    }
}

#[cfg(all(test, feature = "nip17"))]
mod tests {
    use super::*;
    use nostr::Keys;
    use radroots_events::message::{RadrootsMessage, RadrootsMessageRecipient};
    use radroots_events::message_file::{RadrootsMessageFile, RadrootsMessageFileDimensions};

    fn sender_keys() -> Keys {
        Keys::parse("6b911fd37cdf5c81d4c0adb1ab7fa822ed253ab0ad9aa18d77257c88b29b718e")
            .unwrap()
    }

    fn receiver_keys() -> Keys {
        Keys::parse("7b911fd37cdf5c81d4c0adb1ab7fa822ed253ab0ad9aa18d77257c88b29b718e")
            .unwrap()
    }

    #[tokio::test]
    async fn wrap_and_unwrap_message() {
        let sender = sender_keys();
        let receiver = receiver_keys();
        let message = RadrootsMessage {
            recipients: vec![RadrootsMessageRecipient {
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
                assert_eq!(metadata.message.content, "hello");
                assert_eq!(metadata.message.recipients.len(), 1);
            }
            other => panic!("expected message rumor, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn wrap_and_unwrap_message_file() {
        let sender = sender_keys();
        let receiver = receiver_keys();
        let message = RadrootsMessageFile {
            recipients: vec![RadrootsMessageRecipient {
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
            dimensions: Some(RadrootsMessageFileDimensions { w: 1200, h: 800 }),
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
        match rumor {
            RadrootsNip17Rumor::MessageFile(metadata) => {
                assert_eq!(metadata.message_file.file_url, message.file_url);
                assert_eq!(metadata.message_file.encrypted_hash, message.encrypted_hash);
            }
            other => panic!("expected message file rumor, got {other:?}"),
        }
    }
}
