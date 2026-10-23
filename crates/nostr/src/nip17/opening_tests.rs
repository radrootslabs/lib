use super::*;
use nostr::JsonUtil;
use nostr::{
    signer::{SignerBackend, SignerError},
    util::BoxedFuture,
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

struct ObservedSigner {
    keys: nostr::Keys,
    switch_at: usize,
    observations: AtomicUsize,
    decryptions: AtomicUsize,
}
impl core::fmt::Debug for ObservedSigner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ObservedSigner")
    }
}
impl NostrSigner for ObservedSigner {
    fn backend(&self) -> SignerBackend<'_> {
        SignerBackend::Keys
    }
    fn get_public_key(&self) -> BoxedFuture<'_, Result<PublicKey, SignerError>> {
        Box::pin(async move {
            let n = self.observations.fetch_add(1, Ordering::SeqCst);
            Ok(if n >= self.switch_at {
                keys(4).public_key()
            } else {
                self.keys.public_key()
            })
        })
    }
    fn sign_event(&self, value: UnsignedEvent) -> BoxedFuture<'_, Result<Event, SignerError>> {
        self.keys.sign_event(value)
    }
    fn nip04_encrypt<'a>(
        &'a self,
        key: &'a PublicKey,
        text: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        self.keys.nip04_encrypt(key, text)
    }
    fn nip04_decrypt<'a>(
        &'a self,
        key: &'a PublicKey,
        text: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        self.keys.nip04_decrypt(key, text)
    }
    fn nip44_encrypt<'a>(
        &'a self,
        key: &'a PublicKey,
        text: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        self.keys.nip44_encrypt(key, text)
    }
    fn nip44_decrypt<'a>(
        &'a self,
        key: &'a PublicKey,
        text: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        self.decryptions.fetch_add(1, Ordering::SeqCst);
        self.keys.nip44_decrypt(key, text)
    }
}

fn keys(value: u8) -> nostr::Keys {
    let mut secret = [0; 32];
    secret[31] = value;
    nostr::Keys::new(nostr::SecretKey::from_slice(&secret).unwrap())
}
async fn envelope(sender: &nostr::Keys, recipient: &nostr::Keys, raw: &str) -> Event {
    let encrypted = sender
        .nip44_encrypt(&recipient.public_key(), raw)
        .await
        .unwrap();
    let seal = EventBuilder::new(Kind::Seal, encrypted)
        .custom_created_at(Timestamp::from_secs(1_700_000_001))
        .sign_with_keys(sender)
        .unwrap();
    EventBuilder::gift_wrap_from_seal(&recipient.public_key(), &seal, []).unwrap()
}

#[tokio::test]
async fn raw_rumor_kinds_and_identity_never_normalize_into_an_accepted_message() {
    let sender = keys(1);
    let recipient = keys(2);
    let message = Message {
        recipients: vec![radroots_event::social::message::MessageRecipient {
            public_key: recipient.public_key().to_hex(),
            relay_url: None,
        }],
        content: "Synthetic bounded opening regression".into(),
        reply_to: None,
        subject: None,
    };
    let parts = message_encode::to_wire_parts(&message).unwrap();
    let mut rumor = rumor_from_parts(parts, sender.public_key(), Some(1_700_000_000)).unwrap();
    let _ = rumor.id();
    let original: Value = serde_json::from_str(&rumor.as_json()).unwrap();
    let valid = envelope(&sender, &recipient, &original.to_string()).await;
    assert!(unwrap_gift_wrap(&recipient, &valid).await.is_ok());
    let mut accepted = Vec::new();
    for (name, field, value) in [
        ("u16 overflow kind", "kind", json!(65_550)),
        ("u32 overflow kind", "kind", json!(4_294_967_310u64)),
        ("wrong rumor identity", "id", json!("00".repeat(32))),
        ("unknown rumor field", "future_private_field", json!(true)),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        let outer = envelope(&sender, &recipient, &changed.to_string()).await;
        if unwrap_gift_wrap(&recipient, &outer).await.is_ok() {
            accepted.push(name);
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted malformed representations: {accepted:?}"
    );
}

fn rumor(sender: &nostr::Keys, recipient: &nostr::Keys, content: &str) -> UnsignedEvent {
    let mut rumor = UnsignedEvent {
        id: None,
        pubkey: sender.public_key(),
        created_at: Timestamp::from_secs(1_700_000_000),
        kind: Kind::PrivateDirectMessage,
        tags: nostr::Tags::from_list(vec![Tag::public_key(recipient.public_key())]),
        content: content.into(),
    };
    let _ = rumor.id();
    rumor
}

async fn wrap_raw_seal(recipient: &nostr::Keys, raw: &str, tags: Vec<Tag>) -> Event {
    let ephemeral = keys(3);
    let content = ephemeral
        .nip44_encrypt(&recipient.public_key(), raw)
        .await
        .unwrap();
    EventBuilder::new(Kind::GiftWrap, content)
        .tags(tags)
        .custom_created_at(Timestamp::from_secs(1_700_000_002))
        .sign_with_keys(&ephemeral)
        .unwrap()
}

#[tokio::test]
async fn authenticated_outer_recipient_and_closed_seal_are_required() {
    let sender = keys(1);
    let recipient = keys(2);
    let original = rumor(&sender, &recipient, "Synthetic carrier validation");
    let content = sender
        .nip44_encrypt(&recipient.public_key(), &original.as_json())
        .await
        .unwrap();
    let seal = EventBuilder::new(Kind::Seal, content.clone())
        .custom_created_at(Timestamp::from_secs(1_700_000_001))
        .sign_with_keys(&sender)
        .unwrap();
    let valid = wrap_raw_seal(
        &recipient,
        &seal.as_json(),
        vec![Tag::public_key(recipient.public_key())],
    )
    .await;
    assert!(unwrap_gift_wrap(&recipient, &valid).await.is_ok());
    for tags in [
        vec![],
        vec![Tag::public_key(sender.public_key())],
        vec![
            Tag::public_key(recipient.public_key()),
            Tag::public_key(sender.public_key()),
        ],
    ] {
        let outer = wrap_raw_seal(&recipient, &seal.as_json(), tags).await;
        assert!(matches!(
            unwrap_gift_wrap(&recipient, &outer).await,
            Err(Error::InvalidRecipient)
        ));
    }
    let mut invalid_id = valid.clone();
    invalid_id.id = nostr::EventId::from_hex(&"00".repeat(32)).unwrap();
    assert!(matches!(
        unwrap_gift_wrap(&recipient, &invalid_id).await,
        Err(Error::GiftWrap)
    ));
    let mut invalid_sig = valid;
    invalid_sig.sig = nostr::secp256k1::schnorr::Signature::from_slice(&[0; 64]).unwrap();
    assert!(matches!(
        unwrap_gift_wrap(&recipient, &invalid_sig).await,
        Err(Error::GiftWrap)
    ));
    for (kind, tags) in [
        (Kind::TextNote, vec![]),
        (Kind::Seal, vec![Tag::public_key(recipient.public_key())]),
    ] {
        let wrong = EventBuilder::new(kind, content.clone())
            .tags(tags)
            .custom_created_at(Timestamp::from_secs(1_700_000_001))
            .sign_with_keys(&sender)
            .unwrap();
        let outer = wrap_raw_seal(
            &recipient,
            &wrong.as_json(),
            vec![Tag::public_key(recipient.public_key())],
        )
        .await;
        assert!(matches!(
            unwrap_gift_wrap(&recipient, &outer).await,
            Err(Error::GiftWrap)
        ));
    }
    for changed in [format!("{{\"kind\":13,{}", &seal.as_json()[1..]), {
        let mut value: Value = serde_json::from_str(&seal.as_json()).unwrap();
        value["extra"] = json!(true);
        value.to_string()
    }] {
        let outer = wrap_raw_seal(
            &recipient,
            &changed,
            vec![Tag::public_key(recipient.public_key())],
        )
        .await;
        assert!(matches!(
            unwrap_gift_wrap(&recipient, &outer).await,
            Err(Error::GiftWrap)
        ));
    }
}

#[tokio::test]
async fn known_rumor_shape_author_id_recipient_and_complete_size_are_checked() {
    let sender = keys(1);
    let recipient = keys(2);
    let other = keys(4);
    let original = rumor(&sender, &recipient, "Synthetic rumor validation");
    let raw = original.as_json();
    let mut missing: Value = serde_json::from_str(&raw).unwrap();
    missing.as_object_mut().unwrap().remove("id");
    for raw in [
        missing.to_string(),
        format!("{{\"kind\":14,{}", &raw[1..]),
        "[]".into(),
        rumor(&other, &recipient, "Wrong author").as_json(),
        rumor(&sender, &other, "Wrong recipient").as_json(),
        rumor(
            &sender,
            &recipient,
            &"a".repeat(radroots_protocol::chat::RUMOR_MAX_BYTES),
        )
        .as_json(),
    ] {
        let outer = envelope(&sender, &recipient, &raw).await;
        assert!(unwrap_gift_wrap(&recipient, &outer).await.is_err());
    }
    let peer = envelope(&sender, &recipient, &original.as_json()).await;
    let self_copy = envelope(&sender, &sender, &original.as_json()).await;
    for (participant, event) in [(&recipient, peer), (&sender, self_copy)] {
        let Rumor::Message(message) = unwrap_gift_wrap(participant, &event).await.unwrap() else {
            panic!("expected chat")
        };
        assert_eq!(message.id, original.id.unwrap().to_hex());
        assert_eq!(message.data.content, original.content);
        assert_eq!(
            message.data.recipients[0].public_key,
            recipient.public_key().to_hex()
        );
    }
}

#[tokio::test]
async fn identity_changes_stop_opening_and_invalid_outer_never_invokes_the_signer() {
    let sender = keys(1);
    let recipient = keys(2);
    let original = rumor(&sender, &recipient, "Synthetic scoped identity");
    let outer = envelope(&sender, &recipient, &original.as_json()).await;
    for switch_at in [1, 2, usize::MAX] {
        let observed = ObservedSigner {
            keys: keys(2),
            switch_at,
            observations: AtomicUsize::new(0),
            decryptions: AtomicUsize::new(0),
        };
        let result = unwrap_gift_wrap(&observed, &outer).await;
        if switch_at == usize::MAX {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(Error::Signer)));
        }
        assert_eq!(
            observed.decryptions.load(Ordering::SeqCst),
            switch_at.min(2)
        );
    }
    let observed = ObservedSigner {
        keys: keys(2),
        switch_at: usize::MAX,
        observations: AtomicUsize::new(0),
        decryptions: AtomicUsize::new(0),
    };
    let mut invalid = outer;
    invalid.id = nostr::EventId::from_hex(&"00".repeat(32)).unwrap();
    assert!(matches!(
        unwrap_gift_wrap(&observed, &invalid).await,
        Err(Error::GiftWrap)
    ));
    assert_eq!(observed.observations.load(Ordering::SeqCst), 0);
    assert_eq!(observed.decryptions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn authentic_unknown_kind_is_unsupported_before_chat_tag_interpretation() {
    let sender = keys(1);
    let recipient = keys(2);
    let mut value = rumor(&sender, &recipient, "Future private protocol content");
    value.kind = Kind::Custom(u16::MAX);
    value.tags = nostr::Tags::default();
    value.id = None;
    let _ = value.id();
    let outer = envelope(&sender, &recipient, &value.as_json()).await;
    assert!(matches!(
        unwrap_gift_wrap(&recipient, &outer).await,
        Err(Error::UnsupportedRumorKind { kind: 65535 })
    ));
}

#[tokio::test]
async fn invalid_outer_tag_structure_is_rejected_before_provider_callbacks() {
    let sender = keys(1);
    let recipient = keys(2);
    let original = rumor(&sender, &recipient, "Synthetic structural admission");
    let content = sender
        .nip44_encrypt(&recipient.public_key(), &original.as_json())
        .await
        .unwrap();
    let seal = EventBuilder::new(Kind::Seal, content)
        .custom_created_at(Timestamp::from_secs(1_700_000_001))
        .sign_with_keys(&sender)
        .unwrap();
    let outer = wrap_raw_seal(
        &recipient,
        &seal.as_json(),
        vec![
            Tag::public_key(recipient.public_key()),
            Tag::parse(["", "invalid tag key"]).unwrap(),
        ],
    )
    .await;
    outer.verify().unwrap();
    let observed = ObservedSigner {
        keys: keys(2),
        switch_at: usize::MAX,
        observations: AtomicUsize::new(0),
        decryptions: AtomicUsize::new(0),
    };
    assert!(matches!(
        unwrap_gift_wrap(&observed, &outer).await,
        Err(Error::GiftWrap)
    ));
    assert_eq!(observed.observations.load(Ordering::SeqCst), 0);
    assert_eq!(observed.decryptions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn opening_admission_scalar_and_participant_negatives_are_not_normalized() {
    let sender = keys(1);
    let recipient = keys(2);
    let original = rumor(&sender, &recipient, "Synthetic exact scalar checks");
    let original_value: Value = serde_json::from_str(&original.as_json()).unwrap();
    for (field, value) in [
        ("created_at", json!(253_402_300_800u64)),
        ("tags", json!({})),
        ("content", json!(0)),
        ("id", json!(original.id.unwrap().to_hex().to_uppercase())),
    ] {
        let mut changed = original_value.clone();
        changed[field] = value;
        let outer = envelope(&sender, &recipient, &changed.to_string()).await;
        assert!(
            matches!(
                unwrap_gift_wrap(&recipient, &outer).await,
                Err(Error::GiftWrap)
            ),
            "{field}"
        );
    }
    for tags in [
        vec![Tag::parse(["p"]).unwrap()],
        vec![Tag::parse(["p", &recipient.public_key().to_hex(), "relay", "extra"]).unwrap()],
        vec![Tag::parse(["p", &recipient.public_key().to_hex().to_uppercase()]).unwrap()],
        vec![
            Tag::public_key(recipient.public_key()),
            Tag::public_key(recipient.public_key()),
        ],
        vec![],
    ] {
        let mut changed = original.clone();
        changed.tags = nostr::Tags::from_list(tags);
        changed.id = None;
        let _ = changed.id();
        let outer = envelope(&sender, &recipient, &changed.as_json()).await;
        assert!(matches!(
            unwrap_gift_wrap(&recipient, &outer).await,
            Err(Error::InvalidRecipient)
        ));
    }
    let mut self_without_peers = original;
    self_without_peers.tags = nostr::Tags::default();
    self_without_peers.id = None;
    let _ = self_without_peers.id();
    let outer = envelope(&sender, &sender, &self_without_peers.as_json()).await;
    assert!(matches!(
        unwrap_gift_wrap(&sender, &outer).await,
        Err(Error::InvalidRecipient)
    ));
}

#[tokio::test]
async fn opening_admission_outer_budgets_reject_before_provider_callbacks() {
    let recipient = keys(2);
    let ephemeral = keys(3);
    let limits = radroots_event::wire::EventWireLimits::default();
    let empty = String::new();
    let large_content = "a".repeat(radroots_protocol::chat::SIGNED_MAX_BYTES + 1);
    let too_many_tags = vec![Tag::parse(["x"]).unwrap(); limits.max_tag_count + 1];
    let too_many_fields = vec![Tag::parse(vec!["x"; limits.max_total_tag_elements + 1]).unwrap()];
    let too_many_bytes = vec![
        Tag::parse(vec![
            "x".repeat(limits.max_tag_element_bytes);
            limits.max_total_tag_bytes
                / limits.max_tag_element_bytes
                + 1
        ])
        .unwrap(),
    ];
    let large_field =
        vec![Tag::parse(["x".into(), "a".repeat(limits.max_tag_element_bytes + 1)]).unwrap()];
    let mut long_p = vec!["p".into(), recipient.public_key().to_hex()];
    long_p.extend(["relay".into(), "extra".into()]);
    let cases = [
        (
            Kind::TextNote,
            &empty,
            vec![Tag::public_key(recipient.public_key())],
        ),
        (
            Kind::GiftWrap,
            &large_content,
            vec![Tag::public_key(recipient.public_key())],
        ),
        (Kind::GiftWrap, &empty, too_many_tags),
        (Kind::GiftWrap, &empty, too_many_fields),
        (Kind::GiftWrap, &empty, too_many_bytes),
        (Kind::GiftWrap, &empty, large_field),
    ];
    for (kind, content, tags) in cases {
        let outer = EventBuilder::new(kind, content)
            .tags(tags)
            .custom_created_at(Timestamp::from_secs(1_700_000_002))
            .sign_with_keys(&ephemeral)
            .unwrap();
        outer.verify().unwrap();
        let observed = ObservedSigner {
            keys: keys(2),
            switch_at: usize::MAX,
            observations: AtomicUsize::new(0),
            decryptions: AtomicUsize::new(0),
        };
        assert!(matches!(
            unwrap_gift_wrap(&observed, &outer).await,
            Err(Error::GiftWrap)
        ));
        assert_eq!(observed.observations.load(Ordering::SeqCst), 0);
        assert_eq!(observed.decryptions.load(Ordering::SeqCst), 0);
    }
    let outer = EventBuilder::new(Kind::GiftWrap, empty)
        .tags([Tag::parse(long_p).unwrap()])
        .custom_created_at(Timestamp::from_secs(1_700_000_002))
        .sign_with_keys(&ephemeral)
        .unwrap();
    let observed = ObservedSigner {
        keys: keys(2),
        switch_at: usize::MAX,
        observations: AtomicUsize::new(0),
        decryptions: AtomicUsize::new(0),
    };
    assert!(matches!(
        unwrap_gift_wrap(&observed, &outer).await,
        Err(Error::InvalidRecipient)
    ));
    assert_eq!(observed.decryptions.load(Ordering::SeqCst), 0);
}
