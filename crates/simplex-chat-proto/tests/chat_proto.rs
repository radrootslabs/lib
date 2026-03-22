use radroots_simplex_chat_proto::prelude::{
    RadrootsSimplexChatContainerKind, RadrootsSimplexChatContent, RadrootsSimplexChatEvent,
    RadrootsSimplexChatForwardMarker, RadrootsSimplexChatMessage, RadrootsSimplexChatScope,
    RadrootsSimplexChatVersionRange, decode_messages, encode_compressed_batch, encode_message,
};
use serde_json::{Value, json};

fn decode_one(value: Value) -> RadrootsSimplexChatMessage {
    let bytes = serde_json::to_vec(&value).expect("serialize synthetic test value");
    let mut messages = decode_messages(&bytes).expect("decode synthetic test value");
    assert_eq!(messages.len(), 1, "expected exactly one decoded message");
    messages.pop().expect("single decoded message")
}

fn encode_value(message: &RadrootsSimplexChatMessage) -> Value {
    serde_json::from_slice(&encode_message(message).expect("encode message"))
        .expect("parse encoded message json")
}

#[test]
fn roundtrips_ok_message_with_top_level_version() {
    let expected = json!({
        "v": "1-16",
        "msgId": "AQ",
        "event": "x.ok",
        "params": {},
    });

    let message = decode_one(expected.clone());
    assert_eq!(
        message.version,
        Some(RadrootsSimplexChatVersionRange::new(1, 16).unwrap())
    );
    assert!(matches!(message.event, RadrootsSimplexChatEvent::Ok(_)));
    assert_eq!(encode_value(&message), expected);
}

#[test]
fn roundtrips_contact_event_with_request_message_fields() {
    let expected = json!({
        "v": "1-16",
        "event": "x.contact",
        "params": {
            "profile": {
                "displayName": "rr",
                "fullName": "Rad Roots",
                "peerType": "human"
            },
            "contactReqId": "AQ",
            "welcomeMsgId": "Ag",
            "msgId": "Aw",
            "content": {
                "type": "text",
                "text": "hello from rr"
            },
            "nickname": "roots"
        }
    });

    let message = decode_one(expected.clone());
    let RadrootsSimplexChatEvent::Contact(event) = &message.event else {
        panic!("expected contact event");
    };
    assert_eq!(event.profile.display_name, "rr");
    assert!(event.request_message.is_some());
    assert_eq!(
        event.extra.get("nickname"),
        Some(&Value::String("roots".into()))
    );
    assert_eq!(encode_value(&message), expected);
}

#[test]
fn roundtrips_msg_new_with_object_forward_marker() {
    let expected = json!({
        "event": "x.msg.new",
        "params": {
            "forward": {
                "groupLinkId": "AQ",
                "mode": "public"
            },
            "content": {
                "type": "text",
                "text": "forwarded text"
            },
            "ttl": 60,
            "live": true
        }
    });

    let message = decode_one(expected.clone());
    let RadrootsSimplexChatEvent::MsgNew(event) = &message.event else {
        panic!("expected x.msg.new");
    };
    assert!(matches!(
        event.container.kind,
        RadrootsSimplexChatContainerKind::Forward(RadrootsSimplexChatForwardMarker::Object(_))
    ));
    assert_eq!(event.container.ttl, Some(60));
    assert_eq!(event.container.live, Some(true));
    assert_eq!(encode_value(&message), expected);
}

#[test]
fn roundtrips_msg_update_with_mentions_and_scope() {
    let expected = json!({
        "event": "x.msg.update",
        "params": {
            "msgId": "AQ",
            "content": {
                "type": "text",
                "text": "edited"
            },
            "mentions": {
                "lead": {
                    "memberId": "Ag",
                    "label": "Lead"
                }
            },
            "scope": {
                "type": "member",
                "data": {
                    "memberId": "Aw",
                    "role": "writer"
                }
            },
            "ttl": 90,
            "live": false
        }
    });

    let message = decode_one(expected.clone());
    let RadrootsSimplexChatEvent::MsgUpdate(event) = &message.event else {
        panic!("expected x.msg.update");
    };
    assert_eq!(event.mentions.len(), 1);
    assert!(matches!(
        event.scope,
        Some(RadrootsSimplexChatScope::Member { .. })
    ));
    assert_eq!(encode_value(&message), expected);
}

#[test]
fn roundtrips_file_description_and_accept_invitation_events() {
    let file_descr = json!({
        "event": "x.msg.file.descr",
        "params": {
            "msgId": "AQ",
            "fileDescr": {
                "fileDescrText": "part 1",
                "fileDescrPartNo": 1,
                "fileDescrComplete": true
            },
            "label": "intro"
        }
    });

    let file_accept_inv = json!({
        "event": "x.file.acpt.inv",
        "params": {
            "msgId": "Ag",
            "fileConnReq": "smp://example",
            "fileName": "hello.txt",
            "label": "doc"
        }
    });

    let descr_message = decode_one(file_descr.clone());
    let acpt_inv_message = decode_one(file_accept_inv.clone());
    assert_eq!(encode_value(&descr_message), file_descr);
    assert_eq!(encode_value(&acpt_inv_message), file_accept_inv);
}

#[test]
fn preserves_unknown_event_params() {
    let expected = json!({
        "v": "8",
        "event": "x.future.dm",
        "params": {
            "flag": true,
            "nested": {
                "kind": "preview"
            }
        }
    });

    let message = decode_one(expected.clone());
    let RadrootsSimplexChatEvent::Unknown { event, params } = &message.event else {
        panic!("expected unknown event");
    };
    assert_eq!(event, "x.future.dm");
    assert_eq!(params.get("flag"), Some(&Value::Bool(true)));
    assert_eq!(encode_value(&message), expected);
}

#[test]
fn preserves_unknown_content_types_inside_direct_messages() {
    let expected = json!({
        "event": "x.msg.new",
        "params": {
            "content": {
                "type": "chat",
                "text": "join us",
                "chatLink": "https://radroots.example/simplex"
            }
        }
    });

    let message = decode_one(expected.clone());
    let RadrootsSimplexChatEvent::MsgNew(event) = &message.event else {
        panic!("expected x.msg.new");
    };
    match &event.container.content {
        RadrootsSimplexChatContent::Unknown {
            content_type,
            text,
            raw,
        } => {
            assert_eq!(content_type, "chat");
            assert_eq!(text.as_deref(), Some("join us"));
            assert_eq!(
                raw.get("chatLink"),
                Some(&Value::String("https://radroots.example/simplex".into()))
            );
        }
        other => panic!("expected unknown content, got {other:?}"),
    }
    assert_eq!(encode_value(&message), expected);
}

#[test]
fn roundtrips_official_compressed_envelope_batches() {
    let message = decode_one(json!({
        "v": "1-16",
        "msgId": "AQ",
        "event": "x.msg.new",
        "params": {
            "content": {
                "type": "text",
                "text": "x".repeat(256)
            }
        }
    }));

    let encoded = encode_compressed_batch(&[message.clone()]).expect("encode compressed batch");
    assert_eq!(encoded.first(), Some(&b'X'));

    let decoded = decode_messages(&encoded).expect("decode compressed envelope");
    assert_eq!(decoded, vec![message]);
}
