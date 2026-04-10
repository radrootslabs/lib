use crate::error::RadrootsSimplexChatProtoError;
use crate::model::{
    RadrootsSimplexChatBase64Url, RadrootsSimplexChatContactEvent,
    RadrootsSimplexChatContainerKind, RadrootsSimplexChatContent, RadrootsSimplexChatDeleteEvent,
    RadrootsSimplexChatEvent, RadrootsSimplexChatFileAcceptEvent,
    RadrootsSimplexChatFileAcceptInvitationEvent, RadrootsSimplexChatFileCancelEvent,
    RadrootsSimplexChatFileDescription, RadrootsSimplexChatFileDescriptionEvent,
    RadrootsSimplexChatFileInvitation, RadrootsSimplexChatForwardMarker,
    RadrootsSimplexChatInfoEvent, RadrootsSimplexChatMention, RadrootsSimplexChatMessage,
    RadrootsSimplexChatMessageContainer, RadrootsSimplexChatMessageContentReference,
    RadrootsSimplexChatMessageRef, RadrootsSimplexChatMsgNewEvent,
    RadrootsSimplexChatMsgUpdateEvent, RadrootsSimplexChatNoParamsEvent, RadrootsSimplexChatObject,
    RadrootsSimplexChatProbeCheckEvent, RadrootsSimplexChatProbeEvent, RadrootsSimplexChatProfile,
    RadrootsSimplexChatQuotedMessage, RadrootsSimplexChatScope,
};
use crate::version::RadrootsSimplexChatVersionRange;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const RADROOTS_SIMPLEX_CHAT_MAX_PASSTHROUGH_LENGTH: usize = 180;
pub const RADROOTS_SIMPLEX_CHAT_COMPRESSION_LEVEL: i32 = 3;
pub const RADROOTS_SIMPLEX_CHAT_MAX_COMPRESSED_LENGTH: usize = 13_380;
pub const RADROOTS_SIMPLEX_CHAT_MAX_DECOMPRESSED_LENGTH: usize = 65_536;

const COMPRESSED_ENVELOPE_PREFIX: u8 = b'X';
const PASSTHROUGH_TAG: u8 = b'0';
const COMPRESSED_TAG: u8 = b'1';

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct WireMessage {
    #[serde(rename = "v", skip_serializing_if = "Option::is_none")]
    version: Option<RadrootsSimplexChatVersionRange>,
    #[serde(rename = "msgId", skip_serializing_if = "Option::is_none")]
    msg_id: Option<RadrootsSimplexChatBase64Url>,
    event: String,
    params: RadrootsSimplexChatObject,
}

pub fn decode_messages(
    input: &[u8],
) -> Result<Vec<RadrootsSimplexChatMessage>, RadrootsSimplexChatProtoError> {
    let Some(first) = input.first() else {
        return Err(RadrootsSimplexChatProtoError::EmptyInput);
    };

    match *first {
        COMPRESSED_ENVELOPE_PREFIX => decode_compressed_messages(&input[1..]),
        b'{' => {
            let wire = serde_json::from_slice::<WireMessage>(input).map_err(
                |source: serde_json::Error| {
                    RadrootsSimplexChatProtoError::InvalidJson(source.to_string())
                },
            )?;
            Ok(vec![decode_wire_message(wire)?])
        }
        b'[' => {
            let wires = serde_json::from_slice::<Vec<WireMessage>>(input).map_err(
                |source: serde_json::Error| {
                    RadrootsSimplexChatProtoError::InvalidJson(source.to_string())
                },
            )?;
            wires.into_iter().map(decode_wire_message).collect()
        }
        _ => Err(RadrootsSimplexChatProtoError::UnsupportedBinaryMessage),
    }
}

pub fn encode_message(
    message: &RadrootsSimplexChatMessage,
) -> Result<Vec<u8>, RadrootsSimplexChatProtoError> {
    let wire = encode_wire_message(message)?;
    serde_json::to_vec(&wire)
        .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string()))
}

pub fn encode_batch(
    messages: &[RadrootsSimplexChatMessage],
) -> Result<Vec<u8>, RadrootsSimplexChatProtoError> {
    if messages.len() == 1 {
        return encode_message(&messages[0]);
    }

    let wires = messages
        .iter()
        .map(encode_wire_message)
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_vec(&wires)
        .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string()))
}

pub fn encode_compressed_batch(
    messages: &[RadrootsSimplexChatMessage],
) -> Result<Vec<u8>, RadrootsSimplexChatProtoError> {
    let body = encode_batch(messages)?;
    let mut encoded = Vec::new();
    encoded.push(COMPRESSED_ENVELOPE_PREFIX);
    encoded.push(1);
    if body.len() <= RADROOTS_SIMPLEX_CHAT_MAX_PASSTHROUGH_LENGTH {
        encoded.push(PASSTHROUGH_TAG);
        encoded.push(u8::try_from(body.len()).map_err(|_| {
            RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
                "passthrough payload exceeds one-byte length".to_string(),
            )
        })?);
        encoded.extend_from_slice(&body);
    } else {
        #[cfg(feature = "std")]
        {
            let compressed = zstd::bulk::compress(&body, RADROOTS_SIMPLEX_CHAT_COMPRESSION_LEVEL)
                .map_err(|source| {
                RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(source.to_string())
            })?;
            encoded.push(COMPRESSED_TAG);
            let length = u16::try_from(compressed.len()).map_err(|_| {
                RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
                    "compressed payload exceeds two-byte length".to_string(),
                )
            })?;
            encoded.extend_from_slice(&length.to_be_bytes());
            encoded.extend_from_slice(&compressed);
        }
        #[cfg(not(feature = "std"))]
        {
            let _ = body;
            return Err(RadrootsSimplexChatProtoError::CompressionUnavailable);
        }
    }

    if encoded.len().saturating_sub(1) > RADROOTS_SIMPLEX_CHAT_MAX_COMPRESSED_LENGTH {
        return Err(RadrootsSimplexChatProtoError::CompressedMessageTooLarge(
            encoded.len() - 1,
        ));
    }

    Ok(encoded)
}

fn decode_compressed_messages(
    input: &[u8],
) -> Result<Vec<RadrootsSimplexChatMessage>, RadrootsSimplexChatProtoError> {
    let mut cursor = 0;
    let Some(&count) = input.get(cursor) else {
        return Err(RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
            "missing compressed chunk count".to_string(),
        ));
    };
    cursor += 1;
    if count == 0 {
        return Err(RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
            "compressed envelope must contain at least one chunk".to_string(),
        ));
    }

    let mut messages = Vec::new();
    for _ in 0..count {
        let Some(&tag) = input.get(cursor) else {
            return Err(RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
                "missing compressed chunk tag".to_string(),
            ));
        };
        cursor += 1;
        let payload = match tag {
            PASSTHROUGH_TAG => {
                let Some(&length) = input.get(cursor) else {
                    return Err(RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
                        "missing passthrough length".to_string(),
                    ));
                };
                cursor += 1;
                read_exact(input, &mut cursor, usize::from(length))?.to_vec()
            }
            COMPRESSED_TAG => {
                let length_bytes = read_exact(input, &mut cursor, 2)?;
                let length = usize::from(u16::from_be_bytes([length_bytes[0], length_bytes[1]]));
                let compressed = read_exact(input, &mut cursor, length)?;
                #[cfg(feature = "std")]
                {
                    decompress_compressed_chunk(compressed)?
                }
                #[cfg(not(feature = "std"))]
                {
                    let _ = compressed;
                    return Err(RadrootsSimplexChatProtoError::CompressionUnavailable);
                }
            }
            _ => {
                return Err(RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
                    alloc::format!("unknown compressed chunk tag `{}`", char::from(tag)),
                ));
            }
        };
        messages.extend(decode_messages(&payload)?);
    }

    if cursor != input.len() {
        return Err(RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
            "trailing bytes after compressed envelope".to_string(),
        ));
    }

    Ok(messages)
}

#[cfg(feature = "std")]
fn decompress_compressed_chunk(
    compressed: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexChatProtoError> {
    let declared_size = zstd::zstd_safe::get_frame_content_size(compressed)
        .map_err(|_| {
            RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
                "compressed size not specified or corrupted".to_string(),
            )
        })?
        .ok_or_else(|| {
            RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
                "compressed size not specified or exceeds limit".to_string(),
            )
        })?;
    if declared_size > RADROOTS_SIMPLEX_CHAT_MAX_DECOMPRESSED_LENGTH as u64 {
        return Err(RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
            "compressed size not specified or exceeds limit".to_string(),
        ));
    }

    zstd::bulk::decompress(compressed, RADROOTS_SIMPLEX_CHAT_MAX_DECOMPRESSED_LENGTH).map_err(
        |source| RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(source.to_string()),
    )
}

fn read_exact<'a>(
    input: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], RadrootsSimplexChatProtoError> {
    let end = cursor.saturating_add(length);
    if end > input.len() {
        return Err(RadrootsSimplexChatProtoError::InvalidCompressedEnvelope(
            "unexpected end of compressed envelope".to_string(),
        ));
    }
    let slice = &input[*cursor..end];
    *cursor = end;
    Ok(slice)
}

fn decode_wire_message(
    wire: WireMessage,
) -> Result<RadrootsSimplexChatMessage, RadrootsSimplexChatProtoError> {
    let event = match wire.event.as_str() {
        "x.contact" => RadrootsSimplexChatEvent::Contact(decode_contact_event(wire.params)?),
        "x.info" => RadrootsSimplexChatEvent::Info(decode_info_event(wire.params)?),
        "x.info.probe" => RadrootsSimplexChatEvent::InfoProbe(decode_probe_event(wire.params)?),
        "x.info.probe.check" => {
            RadrootsSimplexChatEvent::InfoProbeCheck(decode_probe_check_event(wire.params)?)
        }
        "x.info.probe.ok" => {
            RadrootsSimplexChatEvent::InfoProbeOk(decode_probe_event(wire.params)?)
        }
        "x.msg.new" => RadrootsSimplexChatEvent::MsgNew(RadrootsSimplexChatMsgNewEvent {
            container: decode_message_container(wire.params)?,
        }),
        "x.msg.file.descr" => {
            RadrootsSimplexChatEvent::MsgFileDescr(decode_file_description_event(wire.params)?)
        }
        "x.msg.update" => RadrootsSimplexChatEvent::MsgUpdate(decode_update_event(wire.params)?),
        "x.msg.del" => RadrootsSimplexChatEvent::MsgDel(decode_delete_event(wire.params)?),
        "x.file.acpt" => RadrootsSimplexChatEvent::FileAcpt(decode_file_accept_event(wire.params)?),
        "x.file.acpt.inv" => {
            RadrootsSimplexChatEvent::FileAcptInv(decode_file_accept_inv_event(wire.params)?)
        }
        "x.file.cancel" => {
            RadrootsSimplexChatEvent::FileCancel(decode_file_cancel_event(wire.params)?)
        }
        "x.direct.del" => RadrootsSimplexChatEvent::DirectDel(RadrootsSimplexChatNoParamsEvent {
            extra: wire.params,
        }),
        "x.ok" => {
            RadrootsSimplexChatEvent::Ok(RadrootsSimplexChatNoParamsEvent { extra: wire.params })
        }
        _ => RadrootsSimplexChatEvent::Unknown {
            event: wire.event,
            params: wire.params,
        },
    };

    Ok(RadrootsSimplexChatMessage {
        version: wire.version,
        msg_id: wire.msg_id,
        event,
    })
}

fn encode_wire_message(
    message: &RadrootsSimplexChatMessage,
) -> Result<WireMessage, RadrootsSimplexChatProtoError> {
    let (event, params) = match &message.event {
        RadrootsSimplexChatEvent::Contact(event) => {
            (String::from("x.contact"), encode_contact_event(event)?)
        }
        RadrootsSimplexChatEvent::Info(event) => {
            (String::from("x.info"), encode_info_event(event)?)
        }
        RadrootsSimplexChatEvent::InfoProbe(event) => (
            String::from("x.info.probe"),
            encode_probe_event("probe", &event.probe, &event.extra),
        ),
        RadrootsSimplexChatEvent::InfoProbeCheck(event) => (
            String::from("x.info.probe.check"),
            encode_probe_event("probeHash", &event.probe_hash, &event.extra),
        ),
        RadrootsSimplexChatEvent::InfoProbeOk(event) => (
            String::from("x.info.probe.ok"),
            encode_probe_event("probe", &event.probe, &event.extra),
        ),
        RadrootsSimplexChatEvent::MsgNew(event) => (
            String::from("x.msg.new"),
            encode_message_container(&event.container)?,
        ),
        RadrootsSimplexChatEvent::MsgFileDescr(event) => (
            String::from("x.msg.file.descr"),
            encode_file_description_event(event)?,
        ),
        RadrootsSimplexChatEvent::MsgUpdate(event) => {
            (String::from("x.msg.update"), encode_update_event(event)?)
        }
        RadrootsSimplexChatEvent::MsgDel(event) => {
            (String::from("x.msg.del"), encode_delete_event(event))
        }
        RadrootsSimplexChatEvent::FileAcpt(event) => {
            (String::from("x.file.acpt"), encode_file_accept_event(event))
        }
        RadrootsSimplexChatEvent::FileAcptInv(event) => (
            String::from("x.file.acpt.inv"),
            encode_file_accept_inv_event(event),
        ),
        RadrootsSimplexChatEvent::FileCancel(event) => (
            String::from("x.file.cancel"),
            encode_file_cancel_event(event),
        ),
        RadrootsSimplexChatEvent::DirectDel(event) => {
            (String::from("x.direct.del"), event.extra.clone())
        }
        RadrootsSimplexChatEvent::Ok(event) => (String::from("x.ok"), event.extra.clone()),
        RadrootsSimplexChatEvent::Unknown { event, params } => (event.clone(), params.clone()),
    };

    Ok(WireMessage {
        version: message.version,
        msg_id: message.msg_id.clone(),
        event,
        params,
    })
}

fn decode_contact_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatContactEvent, RadrootsSimplexChatProtoError> {
    let profile = parse_from_map::<RadrootsSimplexChatProfile>(&mut params, "profile")?;
    let contact_req_id = take_optional_base64url(&mut params, "contactReqId")?;
    let welcome_msg_id = take_optional_base64url(&mut params, "welcomeMsgId")?;
    let req_msg_id = take_optional_base64url(&mut params, "msgId")?;
    let req_content = take_optional_content(&mut params, "content")?;
    let request_message = match (req_msg_id, req_content) {
        (Some(msg_id), Some(content)) => {
            Some(RadrootsSimplexChatMessageContentReference { msg_id, content })
        }
        (Some(msg_id), None) => {
            params.insert(
                String::from("msgId"),
                Value::String(msg_id.as_str().to_string()),
            );
            None
        }
        (None, Some(content)) => {
            params.insert(
                String::from("content"),
                serde_json::to_value(content).map_err(|source| {
                    RadrootsSimplexChatProtoError::InvalidJson(source.to_string())
                })?,
            );
            None
        }
        (None, None) => None,
    };

    Ok(RadrootsSimplexChatContactEvent {
        profile,
        contact_req_id,
        welcome_msg_id,
        request_message,
        extra: params,
    })
}

fn encode_contact_event(
    event: &RadrootsSimplexChatContactEvent,
) -> Result<RadrootsSimplexChatObject, RadrootsSimplexChatProtoError> {
    let mut params = event.extra.clone();
    params.insert(String::from("profile"), to_value(&event.profile)?);
    insert_optional_base64url(&mut params, "contactReqId", event.contact_req_id.as_ref());
    insert_optional_base64url(&mut params, "welcomeMsgId", event.welcome_msg_id.as_ref());
    if let Some(request_message) = &event.request_message {
        insert_optional_base64url(&mut params, "msgId", Some(&request_message.msg_id));
        params.insert(String::from("content"), to_value(&request_message.content)?);
    } else {
        params.remove("msgId");
        params.remove("content");
    }
    Ok(params)
}

fn decode_info_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatInfoEvent, RadrootsSimplexChatProtoError> {
    Ok(RadrootsSimplexChatInfoEvent {
        profile: parse_from_map::<RadrootsSimplexChatProfile>(&mut params, "profile")?,
        extra: params,
    })
}

fn encode_info_event(
    event: &RadrootsSimplexChatInfoEvent,
) -> Result<RadrootsSimplexChatObject, RadrootsSimplexChatProtoError> {
    let mut params = event.extra.clone();
    params.insert(String::from("profile"), to_value(&event.profile)?);
    Ok(params)
}

fn decode_probe_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatProbeEvent, RadrootsSimplexChatProtoError> {
    Ok(RadrootsSimplexChatProbeEvent {
        probe: take_required_base64url(&mut params, "probe")?,
        extra: params,
    })
}

fn decode_probe_check_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatProbeCheckEvent, RadrootsSimplexChatProtoError> {
    Ok(RadrootsSimplexChatProbeCheckEvent {
        probe_hash: take_required_base64url(&mut params, "probeHash")?,
        extra: params,
    })
}

fn encode_probe_event(
    field: &str,
    value: &RadrootsSimplexChatBase64Url,
    extra: &RadrootsSimplexChatObject,
) -> RadrootsSimplexChatObject {
    let mut params = extra.clone();
    params.insert(
        String::from(field),
        Value::String(value.as_str().to_string()),
    );
    params
}

fn decode_file_description_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatFileDescriptionEvent, RadrootsSimplexChatProtoError> {
    Ok(RadrootsSimplexChatFileDescriptionEvent {
        msg_id: take_required_base64url(&mut params, "msgId")?,
        file_descr: parse_from_map::<RadrootsSimplexChatFileDescription>(&mut params, "fileDescr")?,
        extra: params,
    })
}

fn encode_file_description_event(
    event: &RadrootsSimplexChatFileDescriptionEvent,
) -> Result<RadrootsSimplexChatObject, RadrootsSimplexChatProtoError> {
    let mut params = event.extra.clone();
    params.insert(
        String::from("msgId"),
        Value::String(event.msg_id.as_str().to_string()),
    );
    params.insert(String::from("fileDescr"), to_value(&event.file_descr)?);
    Ok(params)
}

fn decode_update_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatMsgUpdateEvent, RadrootsSimplexChatProtoError> {
    Ok(RadrootsSimplexChatMsgUpdateEvent {
        msg_id: take_required_base64url(&mut params, "msgId")?,
        content: take_required_content(&mut params, "content")?,
        mentions: take_optional_mentions(&mut params)?,
        ttl: take_optional_i64(&mut params, "ttl")?,
        live: take_optional_bool(&mut params, "live")?,
        scope: take_optional_scope(&mut params)?,
        extra: params,
    })
}

fn encode_update_event(
    event: &RadrootsSimplexChatMsgUpdateEvent,
) -> Result<RadrootsSimplexChatObject, RadrootsSimplexChatProtoError> {
    let mut params = event.extra.clone();
    params.insert(
        String::from("msgId"),
        Value::String(event.msg_id.as_str().to_string()),
    );
    params.insert(String::from("content"), to_value(&event.content)?);
    insert_optional_mentions(&mut params, &event.mentions)?;
    insert_optional_i64(&mut params, "ttl", event.ttl);
    insert_optional_bool(&mut params, "live", event.live);
    insert_optional_scope(&mut params, "scope", event.scope.as_ref())?;
    Ok(params)
}

fn decode_delete_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatDeleteEvent, RadrootsSimplexChatProtoError> {
    Ok(RadrootsSimplexChatDeleteEvent {
        msg_id: take_required_base64url(&mut params, "msgId")?,
        member_id: take_optional_base64url(&mut params, "memberId")?,
        scope: take_optional_scope(&mut params)?,
        extra: params,
    })
}

fn encode_delete_event(event: &RadrootsSimplexChatDeleteEvent) -> RadrootsSimplexChatObject {
    let mut params = event.extra.clone();
    params.insert(
        String::from("msgId"),
        Value::String(event.msg_id.as_str().to_string()),
    );
    insert_optional_base64url(&mut params, "memberId", event.member_id.as_ref());
    if let Some(scope) = &event.scope {
        params.insert(
            String::from("scope"),
            serde_json::to_value(scope).unwrap_or(Value::Null),
        );
    } else {
        params.remove("scope");
    }
    params
}

fn decode_file_accept_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatFileAcceptEvent, RadrootsSimplexChatProtoError> {
    Ok(RadrootsSimplexChatFileAcceptEvent {
        file_name: take_required_string(&mut params, "fileName")?,
        extra: params,
    })
}

fn encode_file_accept_event(
    event: &RadrootsSimplexChatFileAcceptEvent,
) -> RadrootsSimplexChatObject {
    let mut params = event.extra.clone();
    params.insert(
        String::from("fileName"),
        Value::String(event.file_name.clone()),
    );
    params
}

fn decode_file_accept_inv_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatFileAcceptInvitationEvent, RadrootsSimplexChatProtoError> {
    Ok(RadrootsSimplexChatFileAcceptInvitationEvent {
        msg_id: take_required_base64url(&mut params, "msgId")?,
        file_conn_req: take_optional_string(&mut params, "fileConnReq")?,
        file_name: take_required_string(&mut params, "fileName")?,
        extra: params,
    })
}

fn encode_file_accept_inv_event(
    event: &RadrootsSimplexChatFileAcceptInvitationEvent,
) -> RadrootsSimplexChatObject {
    let mut params = event.extra.clone();
    params.insert(
        String::from("msgId"),
        Value::String(event.msg_id.as_str().to_string()),
    );
    insert_optional_string(&mut params, "fileConnReq", event.file_conn_req.as_ref());
    params.insert(
        String::from("fileName"),
        Value::String(event.file_name.clone()),
    );
    params
}

fn decode_file_cancel_event(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatFileCancelEvent, RadrootsSimplexChatProtoError> {
    Ok(RadrootsSimplexChatFileCancelEvent {
        msg_id: take_required_base64url(&mut params, "msgId")?,
        extra: params,
    })
}

fn encode_file_cancel_event(
    event: &RadrootsSimplexChatFileCancelEvent,
) -> RadrootsSimplexChatObject {
    let mut params = event.extra.clone();
    params.insert(
        String::from("msgId"),
        Value::String(event.msg_id.as_str().to_string()),
    );
    params
}

fn decode_message_container(
    mut params: RadrootsSimplexChatObject,
) -> Result<RadrootsSimplexChatMessageContainer, RadrootsSimplexChatProtoError> {
    let kind = if let Some(value) = params.remove("quote") {
        RadrootsSimplexChatContainerKind::Quote(
            serde_json::from_value::<RadrootsSimplexChatQuotedMessage>(value)
                .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string()))?,
        )
    } else if let Some(value) = params.remove("parent") {
        RadrootsSimplexChatContainerKind::Comment(
            serde_json::from_value::<RadrootsSimplexChatMessageRef>(value)
                .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string()))?,
        )
    } else if let Some(value) = params.remove("forward") {
        match value {
            Value::Bool(false) => RadrootsSimplexChatContainerKind::Simple,
            Value::Bool(true) => {
                RadrootsSimplexChatContainerKind::Forward(RadrootsSimplexChatForwardMarker::Flag)
            }
            Value::Object(object) => RadrootsSimplexChatContainerKind::Forward(
                RadrootsSimplexChatForwardMarker::Object(object),
            ),
            _ => return Err(RadrootsSimplexChatProtoError::InvalidField("forward")),
        }
    } else {
        RadrootsSimplexChatContainerKind::Simple
    };

    Ok(RadrootsSimplexChatMessageContainer {
        kind,
        content: take_required_content(&mut params, "content")?,
        mentions: take_optional_mentions(&mut params)?,
        file: take_optional_from_map::<RadrootsSimplexChatFileInvitation>(&mut params, "file")?,
        ttl: take_optional_i64(&mut params, "ttl")?,
        live: take_optional_bool(&mut params, "live")?,
        scope: take_optional_scope(&mut params)?,
        extra: params,
    })
}

fn encode_message_container(
    container: &RadrootsSimplexChatMessageContainer,
) -> Result<RadrootsSimplexChatObject, RadrootsSimplexChatProtoError> {
    let mut params = container.extra.clone();
    params.insert(String::from("content"), to_value(&container.content)?);
    insert_optional_mentions(&mut params, &container.mentions)?;
    insert_optional_to_map(&mut params, "file", container.file.as_ref())?;
    insert_optional_i64(&mut params, "ttl", container.ttl);
    insert_optional_bool(&mut params, "live", container.live);
    insert_optional_scope(&mut params, "scope", container.scope.as_ref())?;

    match &container.kind {
        RadrootsSimplexChatContainerKind::Simple => {
            params.remove("quote");
            params.remove("parent");
            params.remove("forward");
        }
        RadrootsSimplexChatContainerKind::Quote(quoted) => {
            params.insert(String::from("quote"), to_value(quoted)?);
            params.remove("parent");
            params.remove("forward");
        }
        RadrootsSimplexChatContainerKind::Comment(reference) => {
            params.insert(String::from("parent"), to_value(reference)?);
            params.remove("quote");
            params.remove("forward");
        }
        RadrootsSimplexChatContainerKind::Forward(RadrootsSimplexChatForwardMarker::Flag) => {
            params.insert(String::from("forward"), Value::Bool(true));
            params.remove("quote");
            params.remove("parent");
        }
        RadrootsSimplexChatContainerKind::Forward(RadrootsSimplexChatForwardMarker::Object(
            object,
        )) => {
            params.insert(String::from("forward"), Value::Object(object.clone()));
            params.remove("quote");
            params.remove("parent");
        }
    }

    Ok(params)
}

fn parse_from_map<T>(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<T, RadrootsSimplexChatProtoError>
where
    T: serde::de::DeserializeOwned,
{
    let value = params
        .remove(field)
        .ok_or(RadrootsSimplexChatProtoError::MissingField(field))?;
    serde_json::from_value(value)
        .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string()))
}

fn take_required_string(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<String, RadrootsSimplexChatProtoError> {
    match params.remove(field) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(RadrootsSimplexChatProtoError::InvalidField(field)),
        None => Err(RadrootsSimplexChatProtoError::MissingField(field)),
    }
}

fn take_optional_string(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<Option<String>, RadrootsSimplexChatProtoError> {
    match params.remove(field) {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(RadrootsSimplexChatProtoError::InvalidField(field)),
        None => Ok(None),
    }
}

fn take_required_base64url(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<RadrootsSimplexChatBase64Url, RadrootsSimplexChatProtoError> {
    let value = take_required_string(params, field)?;
    RadrootsSimplexChatBase64Url::parse_field(value, field)
}

fn take_optional_base64url(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<Option<RadrootsSimplexChatBase64Url>, RadrootsSimplexChatProtoError> {
    let value = take_optional_string(params, field)?;
    value
        .map(|value| RadrootsSimplexChatBase64Url::parse_field(value, field))
        .transpose()
}

fn take_optional_i64(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<Option<i64>, RadrootsSimplexChatProtoError> {
    match params.remove(field) {
        Some(Value::Number(value)) => value
            .as_i64()
            .map(Some)
            .ok_or(RadrootsSimplexChatProtoError::InvalidField(field)),
        Some(_) => Err(RadrootsSimplexChatProtoError::InvalidField(field)),
        None => Ok(None),
    }
}

fn take_optional_bool(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<Option<bool>, RadrootsSimplexChatProtoError> {
    match params.remove(field) {
        Some(Value::Bool(value)) => Ok(Some(value)),
        Some(_) => Err(RadrootsSimplexChatProtoError::InvalidField(field)),
        None => Ok(None),
    }
}

fn take_optional_scope(
    params: &mut RadrootsSimplexChatObject,
) -> Result<Option<RadrootsSimplexChatScope>, RadrootsSimplexChatProtoError> {
    match params.remove("scope") {
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string())),
        None => Ok(None),
    }
}

fn take_optional_mentions(
    params: &mut RadrootsSimplexChatObject,
) -> Result<BTreeMap<String, RadrootsSimplexChatMention>, RadrootsSimplexChatProtoError> {
    match params.remove("mentions") {
        Some(value) => serde_json::from_value(value)
            .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string())),
        None => Ok(BTreeMap::new()),
    }
}

fn take_required_content(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<RadrootsSimplexChatContent, RadrootsSimplexChatProtoError> {
    let value = params
        .remove(field)
        .ok_or(RadrootsSimplexChatProtoError::MissingField(field))?;
    serde_json::from_value(value)
        .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string()))
}

fn take_optional_content(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<Option<RadrootsSimplexChatContent>, RadrootsSimplexChatProtoError> {
    match params.remove(field) {
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string())),
        None => Ok(None),
    }
}

fn take_optional_from_map<T>(
    params: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<Option<T>, RadrootsSimplexChatProtoError>
where
    T: serde::de::DeserializeOwned,
{
    match params.remove(field) {
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string())),
        None => Ok(None),
    }
}

fn insert_optional_base64url(
    params: &mut RadrootsSimplexChatObject,
    field: &str,
    value: Option<&RadrootsSimplexChatBase64Url>,
) {
    if let Some(value) = value {
        params.insert(
            String::from(field),
            Value::String(value.as_str().to_string()),
        );
    } else {
        params.remove(field);
    }
}

fn insert_optional_string(
    params: &mut RadrootsSimplexChatObject,
    field: &str,
    value: Option<&String>,
) {
    if let Some(value) = value {
        params.insert(String::from(field), Value::String(value.clone()));
    } else {
        params.remove(field);
    }
}

fn insert_optional_i64(params: &mut RadrootsSimplexChatObject, field: &str, value: Option<i64>) {
    if let Some(value) = value {
        params.insert(String::from(field), Value::from(value));
    } else {
        params.remove(field);
    }
}

fn insert_optional_bool(params: &mut RadrootsSimplexChatObject, field: &str, value: Option<bool>) {
    if let Some(value) = value {
        params.insert(String::from(field), Value::Bool(value));
    } else {
        params.remove(field);
    }
}

fn insert_optional_scope(
    params: &mut RadrootsSimplexChatObject,
    field: &str,
    value: Option<&RadrootsSimplexChatScope>,
) -> Result<(), RadrootsSimplexChatProtoError> {
    if let Some(value) = value {
        params.insert(String::from(field), to_value(value)?);
    } else {
        params.remove(field);
    }
    Ok(())
}

fn insert_optional_mentions(
    params: &mut RadrootsSimplexChatObject,
    mentions: &BTreeMap<String, RadrootsSimplexChatMention>,
) -> Result<(), RadrootsSimplexChatProtoError> {
    if mentions.is_empty() {
        params.remove("mentions");
    } else {
        params.insert(String::from("mentions"), to_value(mentions)?);
    }
    Ok(())
}

fn insert_optional_to_map<T>(
    params: &mut RadrootsSimplexChatObject,
    field: &str,
    value: Option<&T>,
) -> Result<(), RadrootsSimplexChatProtoError>
where
    T: Serialize,
{
    if let Some(value) = value {
        params.insert(String::from(field), to_value(value)?);
    } else {
        params.remove(field);
    }
    Ok(())
}

fn to_value<T>(value: &T) -> Result<Value, RadrootsSimplexChatProtoError>
where
    T: Serialize,
{
    serde_json::to_value(value)
        .map_err(|source| RadrootsSimplexChatProtoError::InvalidJson(source.to_string()))
}
