#![forbid(unsafe_code)]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use nostr::nips::nip59;
use nostr::{
    Event, EventBuilder, Kind, NostrSigner, PublicKey, Tag, TagKind, Timestamp, UnsignedEvent,
};
use thiserror::Error;

use radroots_event::envelope::kind::{KIND_MESSAGE, KIND_MESSAGE_FILE};
use radroots_event::social::message::Message;
use radroots_event::social::message_file::MessageFile;
use radroots_event::wire::Nip01EventWireParts;
use radroots_event_codec::decode::RadrootsParsedData;
use radroots_event_codec::decode::message_file as message_file_decode;
use radroots_event_codec::decode::{EventParseError, message as message_decode};
use radroots_event_codec::encode::message_file as message_file_encode;
use radroots_event_codec::encode::{EventEncodeError, message as message_encode};

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
    Message(RadrootsParsedData<Message>),
    MessageFile(Box<RadrootsParsedData<MessageFile>>),
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
        .map_err(|_| RadrootsNip17Error::UnsupportedRumorKind(parts.kind))?;
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
        other => Err(RadrootsNip17Error::UnsupportedRumorKind(other)),
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
            Err(RadrootsNip17Error::UnsupportedRumorKind(kind)) if kind == overflow
        ));
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
        match rumor {
            RadrootsNip17Rumor::MessageFile(metadata) => {
                assert_eq!(metadata.data.file_url, message.file_url);
                assert_eq!(metadata.data.encrypted_hash, message.encrypted_hash);
            }
            other => panic!("expected message file rumor, got {other:?}"),
        }
    }
}
