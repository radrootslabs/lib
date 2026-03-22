use crate::error::RadrootsSimplexSmpProtoError;
use crate::uri::RadrootsSimplexSmpQueueMode;
use crate::version::{
    RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION,
};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

const TAG_NEW: &[u8] = b"NEW";
const TAG_SUB: &[u8] = b"SUB";
const TAG_KEY: &[u8] = b"KEY";
const TAG_NKEY: &[u8] = b"NKEY";
const TAG_NDEL: &[u8] = b"NDEL";
const TAG_GET: &[u8] = b"GET";
const TAG_ACK: &[u8] = b"ACK";
const TAG_OFF: &[u8] = b"OFF";
const TAG_DEL: &[u8] = b"DEL";
const TAG_QUE: &[u8] = b"QUE";
const TAG_SKEY: &[u8] = b"SKEY";
const TAG_SEND: &[u8] = b"SEND";
const TAG_PING: &[u8] = b"PING";
const TAG_NSUB: &[u8] = b"NSUB";

const TAG_IDS: &[u8] = b"IDS";
const TAG_NID: &[u8] = b"NID";
const TAG_MSG: &[u8] = b"MSG";
const TAG_NMSG: &[u8] = b"NMSG";
const TAG_END: &[u8] = b"END";
const TAG_DELD: &[u8] = b"DELD";
const TAG_INFO: &[u8] = b"INFO";
const TAG_OK: &[u8] = b"OK";
const TAG_ERR: &[u8] = b"ERR";
const TAG_PONG: &[u8] = b"PONG";

const COMMAND_ERR_SYNTAX: &[u8] = b"SYNTAX";
const COMMAND_ERR_PROHIBITED: &[u8] = b"PROHIBITED";
const COMMAND_ERR_NO_AUTH: &[u8] = b"NO_AUTH";
const COMMAND_ERR_HAS_AUTH: &[u8] = b"HAS_AUTH";
const COMMAND_ERR_NO_ENTITY: &[u8] = b"NO_ENTITY";
const COMMAND_ERR_NO_QUEUE: &[u8] = b"NO_QUEUE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsSimplexSmpSubscriptionMode {
    Subscribe,
    OnlyCreate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpQueueLinkData {
    pub fixed_data: Vec<u8>,
    pub user_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpMessagingQueueRequest {
    pub sender_id: Vec<u8>,
    pub link_data: RadrootsSimplexSmpQueueLinkData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpContactQueueRequest {
    pub link_id: Vec<u8>,
    pub sender_id: Vec<u8>,
    pub link_data: RadrootsSimplexSmpQueueLinkData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpQueueRequestData {
    Messaging(Option<RadrootsSimplexSmpMessagingQueueRequest>),
    Contact(Option<RadrootsSimplexSmpContactQueueRequest>),
}

impl RadrootsSimplexSmpQueueRequestData {
    pub const fn queue_mode(&self) -> RadrootsSimplexSmpQueueMode {
        match self {
            Self::Messaging(_) => RadrootsSimplexSmpQueueMode::Messaging,
            Self::Contact(_) => RadrootsSimplexSmpQueueMode::Contact,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpNewNotifierCredentials {
    pub notifier_auth_public_key: Vec<u8>,
    pub recipient_notification_dh_public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpServerNotifierCredentials {
    pub notifier_id: Vec<u8>,
    pub server_notification_dh_public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpNewQueueRequest {
    pub recipient_auth_public_key: Vec<u8>,
    pub recipient_dh_public_key: Vec<u8>,
    pub basic_auth: Option<String>,
    pub subscription_mode: RadrootsSimplexSmpSubscriptionMode,
    pub queue_request_data: Option<RadrootsSimplexSmpQueueRequestData>,
    pub notifier_credentials: Option<RadrootsSimplexSmpNewNotifierCredentials>,
}

impl RadrootsSimplexSmpNewQueueRequest {
    pub const fn sender_can_secure(&self) -> bool {
        matches!(
            self.queue_request_data,
            Some(RadrootsSimplexSmpQueueRequestData::Messaging(_))
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpMessageFlags {
    pub notification: bool,
    pub reserved: Vec<u8>,
}

impl RadrootsSimplexSmpMessageFlags {
    pub const fn notifications_enabled() -> Self {
        Self {
            notification: true,
            reserved: Vec::new(),
        }
    }

    pub const fn notifications_disabled() -> Self {
        Self {
            notification: false,
            reserved: Vec::new(),
        }
    }
}

impl Default for RadrootsSimplexSmpMessageFlags {
    fn default() -> Self {
        Self::notifications_disabled()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpSendCommand {
    pub flags: RadrootsSimplexSmpMessageFlags,
    pub message_body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpCommand {
    New(RadrootsSimplexSmpNewQueueRequest),
    Sub,
    Key(Vec<u8>),
    NKey {
        notifier_auth_public_key: Vec<u8>,
        recipient_notification_dh_public_key: Vec<u8>,
    },
    NDel,
    Get,
    Ack(Vec<u8>),
    Off,
    Del,
    Que,
    SKey(Vec<u8>),
    Send(RadrootsSimplexSmpSendCommand),
    Ping,
    NSub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpQueueIdsResponse {
    pub recipient_id: Vec<u8>,
    pub sender_id: Vec<u8>,
    pub server_dh_public_key: Vec<u8>,
    pub queue_mode: Option<RadrootsSimplexSmpQueueMode>,
    pub link_id: Option<Vec<u8>>,
    pub service_id: Option<Vec<u8>>,
    pub server_notification_credentials: Option<RadrootsSimplexSmpServerNotifierCredentials>,
}

impl RadrootsSimplexSmpQueueIdsResponse {
    pub const fn sender_can_secure(&self) -> bool {
        matches!(
            self.queue_mode,
            Some(RadrootsSimplexSmpQueueMode::Messaging)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpNotifierIdsResponse {
    pub notifier_id: Vec<u8>,
    pub server_notification_dh_public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpReceivedMessage {
    pub message_id: Vec<u8>,
    pub encrypted_body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpCommandError {
    Syntax,
    Prohibited,
    NoAuth,
    HasAuth,
    NoEntity,
    Other(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpError {
    Block,
    Session,
    Command(RadrootsSimplexSmpCommandError),
    Auth,
    Quota,
    NoMsg,
    LargeMsg,
    Internal,
    Other(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadrootsSimplexSmpCorrelationId([u8; 24]);

impl RadrootsSimplexSmpCorrelationId {
    pub const LENGTH: usize = 24;

    pub const fn new(bytes: [u8; 24]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(value: &[u8]) -> Result<Self, RadrootsSimplexSmpProtoError> {
        if value.len() != Self::LENGTH {
            return Err(RadrootsSimplexSmpProtoError::InvalidCorrelationIdLength(
                value.len(),
            ));
        }
        let mut bytes = [0_u8; Self::LENGTH];
        bytes.copy_from_slice(value);
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 24] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpBrokerMessage {
    Ids(RadrootsSimplexSmpQueueIdsResponse),
    Nid(RadrootsSimplexSmpNotifierIdsResponse),
    Msg(RadrootsSimplexSmpReceivedMessage),
    NMsg {
        nonce: [u8; 24],
        encrypted_metadata: Vec<u8>,
    },
    End,
    Deld,
    Info(Vec<u8>),
    Ok,
    Err(RadrootsSimplexSmpError),
    Pong,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpCommandTransmission {
    pub authorization: Vec<u8>,
    pub correlation_id: Option<RadrootsSimplexSmpCorrelationId>,
    pub entity_id: Vec<u8>,
    pub command: RadrootsSimplexSmpCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpBrokerTransmission {
    pub authorization: Vec<u8>,
    pub correlation_id: Option<RadrootsSimplexSmpCorrelationId>,
    pub entity_id: Vec<u8>,
    pub message: RadrootsSimplexSmpBrokerMessage,
}

impl RadrootsSimplexSmpCommand {
    pub fn encode(&self) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        self.encode_for_version(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION)
    }

    pub fn encode_for_version(
        &self,
        transport_version: u16,
    ) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        let mut buffer = Vec::new();
        match self {
            Self::New(request) => encode_new_request(&mut buffer, request, transport_version)?,
            Self::Sub => buffer.extend_from_slice(TAG_SUB),
            Self::Key(sender_auth_public_key) => {
                buffer.extend_from_slice(TAG_KEY);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, sender_auth_public_key)?;
            }
            Self::NKey {
                notifier_auth_public_key,
                recipient_notification_dh_public_key,
            } => {
                buffer.extend_from_slice(TAG_NKEY);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, notifier_auth_public_key)?;
                push_short_bytes(&mut buffer, recipient_notification_dh_public_key)?;
            }
            Self::NDel => buffer.extend_from_slice(TAG_NDEL),
            Self::Get => buffer.extend_from_slice(TAG_GET),
            Self::Ack(message_id) => {
                buffer.extend_from_slice(TAG_ACK);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, message_id)?;
            }
            Self::Off => buffer.extend_from_slice(TAG_OFF),
            Self::Del => buffer.extend_from_slice(TAG_DEL),
            Self::Que => buffer.extend_from_slice(TAG_QUE),
            Self::SKey(sender_auth_public_key) => {
                buffer.extend_from_slice(TAG_SKEY);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, sender_auth_public_key)?;
            }
            Self::Send(send) => {
                buffer.extend_from_slice(TAG_SEND);
                buffer.push(b' ');
                buffer.push(encode_bool(send.flags.notification));
                buffer.extend_from_slice(&send.flags.reserved);
                buffer.push(b' ');
                buffer.extend_from_slice(&send.message_body);
            }
            Self::Ping => buffer.extend_from_slice(TAG_PING),
            Self::NSub => buffer.extend_from_slice(TAG_NSUB),
        }
        Ok(buffer)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RadrootsSimplexSmpProtoError> {
        Self::decode_for_version(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, bytes)
    }

    pub fn decode_for_version(
        transport_version: u16,
        bytes: &[u8],
    ) -> Result<Self, RadrootsSimplexSmpProtoError> {
        let (tag, rest) = parse_tag(bytes)?;
        let mut cursor = Cursor::new(rest);
        let command = match tag.as_slice() {
            TAG_NEW => Self::New(decode_new_request(&mut cursor, transport_version)?),
            TAG_SUB => Self::Sub,
            TAG_KEY => Self::Key(cursor.read_short_bytes()?),
            TAG_NKEY => Self::NKey {
                notifier_auth_public_key: cursor.read_short_bytes()?,
                recipient_notification_dh_public_key: cursor.read_short_bytes()?,
            },
            TAG_NDEL => Self::NDel,
            TAG_GET => Self::Get,
            TAG_ACK => Self::Ack(cursor.read_short_bytes()?),
            TAG_OFF => Self::Off,
            TAG_DEL => Self::Del,
            TAG_QUE => Self::Que,
            TAG_SKEY => Self::SKey(cursor.read_short_bytes()?),
            TAG_SEND => Self::Send(decode_send_payload(rest)?),
            TAG_PING => Self::Ping,
            TAG_NSUB => Self::NSub,
            _ => {
                return Err(RadrootsSimplexSmpProtoError::UnsupportedTag(
                    String::from_utf8_lossy(&tag).into_owned(),
                ));
            }
        };
        if !matches!(command, Self::Send(_)) && !cursor.is_empty() {
            return Err(RadrootsSimplexSmpProtoError::TrailingBytes);
        }
        Ok(command)
    }
}

impl RadrootsSimplexSmpBrokerMessage {
    pub fn encode(&self) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        self.encode_for_version(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION)
    }

    pub fn encode_for_version(
        &self,
        transport_version: u16,
    ) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        let mut buffer = Vec::new();
        match self {
            Self::Ids(response) => encode_ids_response(&mut buffer, response, transport_version)?,
            Self::Nid(response) => {
                buffer.extend_from_slice(TAG_NID);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, &response.notifier_id)?;
                push_short_bytes(&mut buffer, &response.server_notification_dh_public_key)?;
            }
            Self::Msg(message) => {
                buffer.extend_from_slice(TAG_MSG);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, &message.message_id)?;
                buffer.extend_from_slice(&message.encrypted_body);
            }
            Self::NMsg {
                nonce,
                encrypted_metadata,
            } => {
                buffer.extend_from_slice(TAG_NMSG);
                buffer.push(b' ');
                buffer.extend_from_slice(nonce);
                buffer.extend_from_slice(encrypted_metadata);
            }
            Self::End => buffer.extend_from_slice(TAG_END),
            Self::Deld => buffer.extend_from_slice(TAG_DELD),
            Self::Info(info) => {
                buffer.extend_from_slice(TAG_INFO);
                buffer.push(b' ');
                buffer.extend_from_slice(info);
            }
            Self::Ok => buffer.extend_from_slice(TAG_OK),
            Self::Err(error) => {
                buffer.extend_from_slice(TAG_ERR);
                buffer.push(b' ');
                buffer.extend_from_slice(&encode_error(error));
            }
            Self::Pong => buffer.extend_from_slice(TAG_PONG),
        }
        Ok(buffer)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RadrootsSimplexSmpProtoError> {
        Self::decode_for_version(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, bytes)
    }

    pub fn decode_for_version(
        transport_version: u16,
        bytes: &[u8],
    ) -> Result<Self, RadrootsSimplexSmpProtoError> {
        let (tag, rest) = parse_tag(bytes)?;
        let mut cursor = Cursor::new(rest);
        let message = match tag.as_slice() {
            TAG_IDS => Self::Ids(decode_ids_response(&mut cursor, transport_version)?),
            TAG_NID => Self::Nid(RadrootsSimplexSmpNotifierIdsResponse {
                notifier_id: cursor.read_short_bytes()?,
                server_notification_dh_public_key: cursor.read_short_bytes()?,
            }),
            TAG_MSG => Self::Msg(RadrootsSimplexSmpReceivedMessage {
                message_id: cursor.read_short_bytes()?,
                encrypted_body: cursor.read_remaining().to_vec(),
            }),
            TAG_NMSG => {
                let nonce = cursor.read_array::<24>().map_err(|error| match error {
                    RadrootsSimplexSmpProtoError::UnexpectedEof => {
                        RadrootsSimplexSmpProtoError::InvalidNonceLength(cursor.remaining_len())
                    }
                    other => other,
                })?;
                Self::NMsg {
                    nonce,
                    encrypted_metadata: cursor.read_remaining().to_vec(),
                }
            }
            TAG_END => Self::End,
            TAG_DELD => Self::Deld,
            TAG_INFO => Self::Info(cursor.read_remaining().to_vec()),
            TAG_OK => Self::Ok,
            TAG_ERR => Self::Err(decode_error(rest)?),
            TAG_PONG => Self::Pong,
            _ => {
                return Err(RadrootsSimplexSmpProtoError::UnsupportedTag(
                    String::from_utf8_lossy(&tag).into_owned(),
                ));
            }
        };
        if !matches!(
            message,
            Self::Msg(_) | Self::NMsg { .. } | Self::Info(_) | Self::Err(_)
        ) && !cursor.is_empty()
        {
            return Err(RadrootsSimplexSmpProtoError::TrailingBytes);
        }
        Ok(message)
    }
}

impl RadrootsSimplexSmpCommandTransmission {
    pub fn encode(&self) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        self.encode_for_version(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION)
    }

    pub fn encode_for_version(
        &self,
        transport_version: u16,
    ) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        encode_transmission(
            &self.authorization,
            self.correlation_id,
            &self.entity_id,
            &self.command.encode_for_version(transport_version)?,
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RadrootsSimplexSmpProtoError> {
        Self::decode_for_version(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, bytes)
    }

    pub fn decode_for_version(
        transport_version: u16,
        bytes: &[u8],
    ) -> Result<Self, RadrootsSimplexSmpProtoError> {
        let (authorization, correlation_id, entity_id, frame) = decode_transmission(bytes)?;
        Ok(Self {
            authorization,
            correlation_id,
            entity_id,
            command: RadrootsSimplexSmpCommand::decode_for_version(transport_version, frame)?,
        })
    }
}

impl RadrootsSimplexSmpBrokerTransmission {
    pub fn encode(&self) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        self.encode_for_version(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION)
    }

    pub fn encode_for_version(
        &self,
        transport_version: u16,
    ) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        encode_transmission(
            &self.authorization,
            self.correlation_id,
            &self.entity_id,
            &self.message.encode_for_version(transport_version)?,
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RadrootsSimplexSmpProtoError> {
        Self::decode_for_version(RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, bytes)
    }

    pub fn decode_for_version(
        transport_version: u16,
        bytes: &[u8],
    ) -> Result<Self, RadrootsSimplexSmpProtoError> {
        let (authorization, correlation_id, entity_id, frame) = decode_transmission(bytes)?;
        Ok(Self {
            authorization,
            correlation_id,
            entity_id,
            message: RadrootsSimplexSmpBrokerMessage::decode_for_version(transport_version, frame)?,
        })
    }
}

fn encode_new_request(
    buffer: &mut Vec<u8>,
    request: &RadrootsSimplexSmpNewQueueRequest,
    transport_version: u16,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    if transport_version < RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION {
        return Err(RadrootsSimplexSmpProtoError::UnsupportedTransportVersion(
            transport_version,
        ));
    }

    buffer.extend_from_slice(TAG_NEW);
    buffer.push(b' ');
    push_short_bytes(buffer, &request.recipient_auth_public_key)?;
    push_short_bytes(buffer, &request.recipient_dh_public_key)?;
    push_maybe_string(buffer, request.basic_auth.as_deref())?;
    buffer.push(encode_subscription_mode(request.subscription_mode));

    if transport_version >= RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION {
        push_maybe(
            buffer,
            request.queue_request_data.as_ref(),
            encode_queue_request_data,
        )?;
        push_maybe(
            buffer,
            request.notifier_credentials.as_ref(),
            encode_new_notifier_credentials,
        )?;
    } else if transport_version >= RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION {
        push_maybe(
            buffer,
            request.queue_request_data.as_ref(),
            encode_queue_request_data,
        )?;
    } else {
        buffer.push(encode_bool(request.sender_can_secure()));
    }

    Ok(())
}

fn decode_new_request(
    cursor: &mut Cursor<'_>,
    transport_version: u16,
) -> Result<RadrootsSimplexSmpNewQueueRequest, RadrootsSimplexSmpProtoError> {
    if transport_version < RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION {
        return Err(RadrootsSimplexSmpProtoError::UnsupportedTransportVersion(
            transport_version,
        ));
    }

    let recipient_auth_public_key = cursor.read_short_bytes()?;
    let recipient_dh_public_key = cursor.read_short_bytes()?;
    let basic_auth = cursor.read_maybe_string()?;
    let subscription_mode = decode_subscription_mode(cursor.read_byte()?)?;
    let (queue_request_data, notifier_credentials) =
        if transport_version >= RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION {
            (
                cursor.read_maybe(decode_queue_request_data)?,
                cursor.read_maybe(decode_new_notifier_credentials)?,
            )
        } else if transport_version >= RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION {
            (cursor.read_maybe(decode_queue_request_data)?, None)
        } else {
            let sender_can_secure = decode_bool(cursor.read_byte()?)?;
            let queue_request_data = Some(if sender_can_secure {
                RadrootsSimplexSmpQueueRequestData::Messaging(None)
            } else {
                RadrootsSimplexSmpQueueRequestData::Contact(None)
            });
            (queue_request_data, None)
        };

    Ok(RadrootsSimplexSmpNewQueueRequest {
        recipient_auth_public_key,
        recipient_dh_public_key,
        basic_auth,
        subscription_mode,
        queue_request_data,
        notifier_credentials,
    })
}

fn encode_ids_response(
    buffer: &mut Vec<u8>,
    response: &RadrootsSimplexSmpQueueIdsResponse,
    transport_version: u16,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    buffer.extend_from_slice(TAG_IDS);
    buffer.push(b' ');
    push_short_bytes(buffer, &response.recipient_id)?;
    push_short_bytes(buffer, &response.sender_id)?;
    push_short_bytes(buffer, &response.server_dh_public_key)?;

    if transport_version >= RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION {
        push_maybe(buffer, response.queue_mode, encode_queue_mode)?;
        push_maybe_short_bytes(buffer, response.link_id.as_deref())?;
        push_maybe_short_bytes(buffer, response.service_id.as_deref())?;
        push_maybe(
            buffer,
            response.server_notification_credentials.as_ref(),
            encode_server_notifier_credentials,
        )?;
    } else if transport_version >= RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION {
        push_maybe(buffer, response.queue_mode, encode_queue_mode)?;
        push_maybe_short_bytes(buffer, response.link_id.as_deref())?;
        push_maybe_short_bytes(buffer, response.service_id.as_deref())?;
    } else if transport_version >= RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION {
        push_maybe(buffer, response.queue_mode, encode_queue_mode)?;
        push_maybe_short_bytes(buffer, response.link_id.as_deref())?;
    } else if transport_version >= RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION {
        buffer.push(encode_bool(response.sender_can_secure()));
    }

    Ok(())
}

fn decode_ids_response(
    cursor: &mut Cursor<'_>,
    transport_version: u16,
) -> Result<RadrootsSimplexSmpQueueIdsResponse, RadrootsSimplexSmpProtoError> {
    let recipient_id = cursor.read_short_bytes()?;
    let sender_id = cursor.read_short_bytes()?;
    let server_dh_public_key = cursor.read_short_bytes()?;

    let (queue_mode, link_id, service_id, server_notification_credentials) =
        if transport_version >= RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION {
            (
                cursor.read_maybe(decode_queue_mode)?,
                cursor.read_maybe(Cursor::read_short_bytes)?,
                cursor.read_maybe(Cursor::read_short_bytes)?,
                cursor.read_maybe(decode_server_notifier_credentials)?,
            )
        } else if transport_version >= RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION {
            (
                cursor.read_maybe(decode_queue_mode)?,
                cursor.read_maybe(Cursor::read_short_bytes)?,
                cursor.read_maybe(Cursor::read_short_bytes)?,
                None,
            )
        } else if transport_version >= RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION {
            (
                cursor.read_maybe(decode_queue_mode)?,
                cursor.read_maybe(Cursor::read_short_bytes)?,
                None,
                None,
            )
        } else if transport_version >= RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION {
            let sender_can_secure = decode_bool(cursor.read_byte()?)?;
            (
                Some(if sender_can_secure {
                    RadrootsSimplexSmpQueueMode::Messaging
                } else {
                    RadrootsSimplexSmpQueueMode::Contact
                }),
                None,
                None,
                None,
            )
        } else {
            (None, None, None, None)
        };

    Ok(RadrootsSimplexSmpQueueIdsResponse {
        recipient_id,
        sender_id,
        server_dh_public_key,
        queue_mode,
        link_id,
        service_id,
        server_notification_credentials,
    })
}

fn encode_queue_request_data(
    buffer: &mut Vec<u8>,
    queue_request_data: &RadrootsSimplexSmpQueueRequestData,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    match queue_request_data {
        RadrootsSimplexSmpQueueRequestData::Messaging(data) => {
            buffer.push(b'M');
            push_maybe(buffer, data.as_ref(), encode_messaging_queue_request)
        }
        RadrootsSimplexSmpQueueRequestData::Contact(data) => {
            buffer.push(b'C');
            push_maybe(buffer, data.as_ref(), encode_contact_queue_request)
        }
    }
}

fn decode_queue_request_data(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexSmpQueueRequestData, RadrootsSimplexSmpProtoError> {
    match cursor.read_byte()? {
        b'M' => Ok(RadrootsSimplexSmpQueueRequestData::Messaging(
            cursor.read_maybe(decode_messaging_queue_request)?,
        )),
        b'C' => Ok(RadrootsSimplexSmpQueueRequestData::Contact(
            cursor.read_maybe(decode_contact_queue_request)?,
        )),
        value => Err(RadrootsSimplexSmpProtoError::InvalidTag(
            String::from_utf8_lossy(&[value]).into_owned(),
        )),
    }
}

fn encode_messaging_queue_request(
    buffer: &mut Vec<u8>,
    request: &RadrootsSimplexSmpMessagingQueueRequest,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    push_short_bytes(buffer, &request.sender_id)?;
    encode_queue_link_data(buffer, &request.link_data)
}

fn decode_messaging_queue_request(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexSmpMessagingQueueRequest, RadrootsSimplexSmpProtoError> {
    Ok(RadrootsSimplexSmpMessagingQueueRequest {
        sender_id: cursor.read_short_bytes()?,
        link_data: decode_queue_link_data(cursor)?,
    })
}

fn encode_contact_queue_request(
    buffer: &mut Vec<u8>,
    request: &RadrootsSimplexSmpContactQueueRequest,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    push_short_bytes(buffer, &request.link_id)?;
    push_short_bytes(buffer, &request.sender_id)?;
    encode_queue_link_data(buffer, &request.link_data)
}

fn decode_contact_queue_request(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexSmpContactQueueRequest, RadrootsSimplexSmpProtoError> {
    Ok(RadrootsSimplexSmpContactQueueRequest {
        link_id: cursor.read_short_bytes()?,
        sender_id: cursor.read_short_bytes()?,
        link_data: decode_queue_link_data(cursor)?,
    })
}

fn encode_queue_link_data(
    buffer: &mut Vec<u8>,
    link_data: &RadrootsSimplexSmpQueueLinkData,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    push_large_bytes(buffer, &link_data.fixed_data)?;
    push_large_bytes(buffer, &link_data.user_data)
}

fn decode_queue_link_data(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexSmpQueueLinkData, RadrootsSimplexSmpProtoError> {
    Ok(RadrootsSimplexSmpQueueLinkData {
        fixed_data: cursor.read_large_bytes()?,
        user_data: cursor.read_large_bytes()?,
    })
}

fn encode_new_notifier_credentials(
    buffer: &mut Vec<u8>,
    credentials: &RadrootsSimplexSmpNewNotifierCredentials,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    push_short_bytes(buffer, &credentials.notifier_auth_public_key)?;
    push_short_bytes(buffer, &credentials.recipient_notification_dh_public_key)
}

fn decode_new_notifier_credentials(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexSmpNewNotifierCredentials, RadrootsSimplexSmpProtoError> {
    Ok(RadrootsSimplexSmpNewNotifierCredentials {
        notifier_auth_public_key: cursor.read_short_bytes()?,
        recipient_notification_dh_public_key: cursor.read_short_bytes()?,
    })
}

fn encode_server_notifier_credentials(
    buffer: &mut Vec<u8>,
    credentials: &RadrootsSimplexSmpServerNotifierCredentials,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    push_short_bytes(buffer, &credentials.notifier_id)?;
    push_short_bytes(buffer, &credentials.server_notification_dh_public_key)
}

fn decode_server_notifier_credentials(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexSmpServerNotifierCredentials, RadrootsSimplexSmpProtoError> {
    Ok(RadrootsSimplexSmpServerNotifierCredentials {
        notifier_id: cursor.read_short_bytes()?,
        server_notification_dh_public_key: cursor.read_short_bytes()?,
    })
}

fn encode_queue_mode(
    buffer: &mut Vec<u8>,
    queue_mode: RadrootsSimplexSmpQueueMode,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    buffer.push(match queue_mode {
        RadrootsSimplexSmpQueueMode::Messaging => b'M',
        RadrootsSimplexSmpQueueMode::Contact => b'C',
    });
    Ok(())
}

fn decode_queue_mode(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexSmpQueueMode, RadrootsSimplexSmpProtoError> {
    match cursor.read_byte()? {
        b'M' => Ok(RadrootsSimplexSmpQueueMode::Messaging),
        b'C' => Ok(RadrootsSimplexSmpQueueMode::Contact),
        value => Err(RadrootsSimplexSmpProtoError::InvalidTag(
            String::from_utf8_lossy(&[value]).into_owned(),
        )),
    }
}

fn encode_transmission(
    authorization: &[u8],
    correlation_id: Option<RadrootsSimplexSmpCorrelationId>,
    entity_id: &[u8],
    frame: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
    let mut buffer = Vec::new();
    push_short_bytes(&mut buffer, authorization)?;
    push_short_bytes(
        &mut buffer,
        correlation_id
            .map(|id| id.0.to_vec())
            .as_deref()
            .unwrap_or_default(),
    )?;
    push_short_bytes(&mut buffer, entity_id)?;
    buffer.extend_from_slice(frame);
    Ok(buffer)
}

fn decode_transmission(
    bytes: &[u8],
) -> Result<
    (
        Vec<u8>,
        Option<RadrootsSimplexSmpCorrelationId>,
        Vec<u8>,
        &[u8],
    ),
    RadrootsSimplexSmpProtoError,
> {
    let mut cursor = Cursor::new(bytes);
    let authorization = cursor.read_short_bytes()?;
    let correlation_id = match cursor.read_short_bytes()?.as_slice() {
        [] => None,
        value => Some(RadrootsSimplexSmpCorrelationId::from_slice(value)?),
    };
    let entity_id = cursor.read_short_bytes()?;
    let frame = cursor.read_remaining();
    if frame.is_empty() {
        return Err(RadrootsSimplexSmpProtoError::UnexpectedEof);
    }
    Ok((authorization, correlation_id, entity_id, frame))
}

fn decode_send_payload(
    payload: &[u8],
) -> Result<RadrootsSimplexSmpSendCommand, RadrootsSimplexSmpProtoError> {
    let Some(space_index) = payload.iter().position(|byte| *byte == b' ') else {
        return Err(RadrootsSimplexSmpProtoError::UnexpectedEof);
    };
    let flags_bytes = &payload[..space_index];
    if flags_bytes.is_empty() {
        return Err(RadrootsSimplexSmpProtoError::MissingField("msg_flags"));
    }
    let flags = RadrootsSimplexSmpMessageFlags {
        notification: decode_bool(flags_bytes[0])?,
        reserved: flags_bytes[1..].to_vec(),
    };
    Ok(RadrootsSimplexSmpSendCommand {
        flags,
        message_body: payload[space_index + 1..].to_vec(),
    })
}

fn encode_error(error: &RadrootsSimplexSmpError) -> Vec<u8> {
    match error {
        RadrootsSimplexSmpError::Block => b"BLOCK".to_vec(),
        RadrootsSimplexSmpError::Session => b"SESSION".to_vec(),
        RadrootsSimplexSmpError::Command(command_error) => {
            let mut bytes = b"CMD ".to_vec();
            bytes.extend_from_slice(match command_error {
                RadrootsSimplexSmpCommandError::Syntax => COMMAND_ERR_SYNTAX,
                RadrootsSimplexSmpCommandError::Prohibited => COMMAND_ERR_PROHIBITED,
                RadrootsSimplexSmpCommandError::NoAuth => COMMAND_ERR_NO_AUTH,
                RadrootsSimplexSmpCommandError::HasAuth => COMMAND_ERR_HAS_AUTH,
                RadrootsSimplexSmpCommandError::NoEntity => COMMAND_ERR_NO_ENTITY,
                RadrootsSimplexSmpCommandError::Other(raw) => raw,
            });
            bytes
        }
        RadrootsSimplexSmpError::Auth => b"AUTH".to_vec(),
        RadrootsSimplexSmpError::Quota => b"QUOTA".to_vec(),
        RadrootsSimplexSmpError::NoMsg => b"NO_MSG".to_vec(),
        RadrootsSimplexSmpError::LargeMsg => b"LARGE_MSG".to_vec(),
        RadrootsSimplexSmpError::Internal => b"INTERNAL".to_vec(),
        RadrootsSimplexSmpError::Other(raw) => raw.clone(),
    }
}

fn decode_error(bytes: &[u8]) -> Result<RadrootsSimplexSmpError, RadrootsSimplexSmpProtoError> {
    if bytes == b"BLOCK" {
        return Ok(RadrootsSimplexSmpError::Block);
    }
    if bytes == b"SESSION" {
        return Ok(RadrootsSimplexSmpError::Session);
    }
    if bytes == b"AUTH" {
        return Ok(RadrootsSimplexSmpError::Auth);
    }
    if bytes == b"QUOTA" {
        return Ok(RadrootsSimplexSmpError::Quota);
    }
    if bytes == b"NO_MSG" {
        return Ok(RadrootsSimplexSmpError::NoMsg);
    }
    if bytes == b"LARGE_MSG" {
        return Ok(RadrootsSimplexSmpError::LargeMsg);
    }
    if bytes == b"INTERNAL" {
        return Ok(RadrootsSimplexSmpError::Internal);
    }
    if let Some(command) = bytes.strip_prefix(b"CMD ") {
        let command_error = match command {
            COMMAND_ERR_SYNTAX => RadrootsSimplexSmpCommandError::Syntax,
            COMMAND_ERR_PROHIBITED => RadrootsSimplexSmpCommandError::Prohibited,
            COMMAND_ERR_NO_AUTH => RadrootsSimplexSmpCommandError::NoAuth,
            COMMAND_ERR_HAS_AUTH => RadrootsSimplexSmpCommandError::HasAuth,
            COMMAND_ERR_NO_ENTITY | COMMAND_ERR_NO_QUEUE => {
                RadrootsSimplexSmpCommandError::NoEntity
            }
            raw => RadrootsSimplexSmpCommandError::Other(raw.to_vec()),
        };
        return Ok(RadrootsSimplexSmpError::Command(command_error));
    }
    Ok(RadrootsSimplexSmpError::Other(bytes.to_vec()))
}

fn parse_tag(bytes: &[u8]) -> Result<(Vec<u8>, &[u8]), RadrootsSimplexSmpProtoError> {
    if bytes.is_empty() {
        return Err(RadrootsSimplexSmpProtoError::UnexpectedEof);
    }
    if let Some(space_index) = bytes.iter().position(|byte| *byte == b' ') {
        Ok((bytes[..space_index].to_vec(), &bytes[space_index + 1..]))
    } else {
        Ok((bytes.to_vec(), &[]))
    }
}

fn encode_subscription_mode(mode: RadrootsSimplexSmpSubscriptionMode) -> u8 {
    match mode {
        RadrootsSimplexSmpSubscriptionMode::Subscribe => b'S',
        RadrootsSimplexSmpSubscriptionMode::OnlyCreate => b'C',
    }
}

fn decode_subscription_mode(
    value: u8,
) -> Result<RadrootsSimplexSmpSubscriptionMode, RadrootsSimplexSmpProtoError> {
    match value {
        b'S' => Ok(RadrootsSimplexSmpSubscriptionMode::Subscribe),
        b'C' => Ok(RadrootsSimplexSmpSubscriptionMode::OnlyCreate),
        _ => Err(RadrootsSimplexSmpProtoError::InvalidTag(
            String::from_utf8_lossy(&[value]).into_owned(),
        )),
    }
}

fn encode_bool(value: bool) -> u8 {
    if value { b'T' } else { b'F' }
}

fn decode_bool(value: u8) -> Result<bool, RadrootsSimplexSmpProtoError> {
    match value {
        b'T' => Ok(true),
        b'F' => Ok(false),
        other => Err(RadrootsSimplexSmpProtoError::InvalidBoolEncoding(other)),
    }
}

fn push_short_bytes(
    buffer: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<(), RadrootsSimplexSmpProtoError> {
    let len = u8::try_from(bytes.len())
        .map_err(|_| RadrootsSimplexSmpProtoError::InvalidShortFieldLength(bytes.len()))?;
    buffer.push(len);
    buffer.extend_from_slice(bytes);
    Ok(())
}

fn push_large_bytes(
    buffer: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<(), RadrootsSimplexSmpProtoError> {
    let len = u16::try_from(bytes.len())
        .map_err(|_| RadrootsSimplexSmpProtoError::InvalidLargeFieldLength(bytes.len()))?;
    buffer.extend_from_slice(&len.to_be_bytes());
    buffer.extend_from_slice(bytes);
    Ok(())
}

fn push_maybe<T, F>(
    buffer: &mut Vec<u8>,
    value: Option<T>,
    mut encode: F,
) -> Result<(), RadrootsSimplexSmpProtoError>
where
    T: Copy,
    F: FnMut(&mut Vec<u8>, T) -> Result<(), RadrootsSimplexSmpProtoError>,
{
    match value {
        None => {
            buffer.push(b'0');
            Ok(())
        }
        Some(value) => {
            buffer.push(b'1');
            encode(buffer, value)
        }
    }
}

fn push_maybe_short_bytes(
    buffer: &mut Vec<u8>,
    value: Option<&[u8]>,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    push_maybe(buffer, value, push_short_bytes)
}

fn push_maybe_string(
    buffer: &mut Vec<u8>,
    value: Option<&str>,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    match value {
        None => {
            buffer.push(b'0');
            Ok(())
        }
        Some(value) => {
            validate_basic_auth(value)?;
            buffer.push(b'1');
            push_short_bytes(buffer, value.as_bytes())
        }
    }
}

fn validate_basic_auth(value: &str) -> Result<(), RadrootsSimplexSmpProtoError> {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_graphic() && byte != b'@' && byte != b':' && byte != b'/')
    {
        Ok(())
    } else {
        Err(RadrootsSimplexSmpProtoError::InvalidUri(value.to_string()))
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn is_empty(&self) -> bool {
        self.offset >= self.bytes.len()
    }

    fn remaining_len(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn read_byte(&mut self) -> Result<u8, RadrootsSimplexSmpProtoError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(RadrootsSimplexSmpProtoError::UnexpectedEof)?;
        self.offset += 1;
        Ok(byte)
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], RadrootsSimplexSmpProtoError> {
        let end = self.offset + len;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(RadrootsSimplexSmpProtoError::UnexpectedEof)?;
        self.offset = end;
        Ok(value)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], RadrootsSimplexSmpProtoError> {
        let mut array = [0_u8; N];
        array.copy_from_slice(self.read_exact(N)?);
        Ok(array)
    }

    fn read_short_bytes(&mut self) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        let len = usize::from(self.read_byte()?);
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_large_bytes(&mut self) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        let len = usize::from(u16::from_be_bytes(self.read_array::<2>()?));
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_maybe_string(&mut self) -> Result<Option<String>, RadrootsSimplexSmpProtoError> {
        self.read_maybe(|cursor| {
            let value = cursor.read_short_bytes()?;
            let string = String::from_utf8(value)
                .map_err(|error| RadrootsSimplexSmpProtoError::InvalidUtf8(error.to_string()))?;
            validate_basic_auth(&string)?;
            Ok(string)
        })
    }

    fn read_maybe<T, F>(&mut self, decode: F) -> Result<Option<T>, RadrootsSimplexSmpProtoError>
    where
        F: FnOnce(&mut Self) -> Result<T, RadrootsSimplexSmpProtoError>,
    {
        match self.read_byte()? {
            b'0' => Ok(None),
            b'1' => Ok(Some(decode(self)?)),
            other => Err(RadrootsSimplexSmpProtoError::InvalidMaybeTag(other)),
        }
    }

    fn read_remaining(&mut self) -> &'a [u8] {
        let remaining = &self.bytes[self.offset..];
        self.offset = self.bytes.len();
        remaining
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn correlation_id(byte: u8) -> RadrootsSimplexSmpCorrelationId {
        RadrootsSimplexSmpCorrelationId::new([byte; 24])
    }

    #[test]
    fn round_trips_current_new_command_transmission() {
        let transmission = RadrootsSimplexSmpCommandTransmission {
            authorization: vec![1, 2, 3],
            correlation_id: Some(correlation_id(7)),
            entity_id: Vec::new(),
            command: RadrootsSimplexSmpCommand::New(RadrootsSimplexSmpNewQueueRequest {
                recipient_auth_public_key: vec![0x01, 0x02, 0x03],
                recipient_dh_public_key: vec![0x04, 0x05],
                basic_auth: Some("server-pass".to_string()),
                subscription_mode: RadrootsSimplexSmpSubscriptionMode::Subscribe,
                queue_request_data: Some(RadrootsSimplexSmpQueueRequestData::Messaging(Some(
                    RadrootsSimplexSmpMessagingQueueRequest {
                        sender_id: vec![0x10, 0x11],
                        link_data: RadrootsSimplexSmpQueueLinkData {
                            fixed_data: vec![0xaa, 0xbb],
                            user_data: vec![0xcc, 0xdd, 0xee],
                        },
                    },
                ))),
                notifier_credentials: Some(RadrootsSimplexSmpNewNotifierCredentials {
                    notifier_auth_public_key: vec![0x21, 0x22],
                    recipient_notification_dh_public_key: vec![0x23, 0x24],
                }),
            }),
        };

        let encoded = transmission.encode().unwrap();
        let decoded = RadrootsSimplexSmpCommandTransmission::decode(&encoded).unwrap();
        assert_eq!(decoded, transmission);
    }

    #[test]
    fn round_trips_v9_new_command_transmission() {
        let transmission = RadrootsSimplexSmpCommandTransmission {
            authorization: vec![1, 2, 3],
            correlation_id: Some(correlation_id(7)),
            entity_id: Vec::new(),
            command: RadrootsSimplexSmpCommand::New(RadrootsSimplexSmpNewQueueRequest {
                recipient_auth_public_key: vec![0x01, 0x02, 0x03],
                recipient_dh_public_key: vec![0x04, 0x05],
                basic_auth: Some("server-pass".to_string()),
                subscription_mode: RadrootsSimplexSmpSubscriptionMode::Subscribe,
                queue_request_data: Some(RadrootsSimplexSmpQueueRequestData::Messaging(None)),
                notifier_credentials: None,
            }),
        };

        let encoded = transmission
            .encode_for_version(RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION)
            .unwrap();
        let decoded = RadrootsSimplexSmpCommandTransmission::decode_for_version(
            RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION,
            &encoded,
        )
        .unwrap();
        assert_eq!(decoded, transmission);
    }

    #[test]
    fn round_trips_send_command_transmission() {
        let transmission = RadrootsSimplexSmpCommandTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(9)),
            entity_id: vec![0xaa, 0xbb],
            command: RadrootsSimplexSmpCommand::Send(RadrootsSimplexSmpSendCommand {
                flags: RadrootsSimplexSmpMessageFlags {
                    notification: true,
                    reserved: b"0".to_vec(),
                },
                message_body: vec![0xde, 0xad, 0xbe, 0xef],
            }),
        };

        let encoded = transmission.encode().unwrap();
        let decoded = RadrootsSimplexSmpCommandTransmission::decode(&encoded).unwrap();
        assert_eq!(decoded, transmission);
    }

    #[test]
    fn round_trips_current_ids_broker_transmission() {
        let transmission = RadrootsSimplexSmpBrokerTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(3)),
            entity_id: Vec::new(),
            message: RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: vec![0x10],
                sender_id: vec![0x11],
                server_dh_public_key: vec![0x12, 0x13],
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: Some(vec![0x14, 0x15]),
                service_id: Some(vec![0x16, 0x17]),
                server_notification_credentials: Some(
                    RadrootsSimplexSmpServerNotifierCredentials {
                        notifier_id: vec![0x18, 0x19],
                        server_notification_dh_public_key: vec![0x1a, 0x1b],
                    },
                ),
            }),
        };

        let encoded = transmission.encode().unwrap();
        let decoded = RadrootsSimplexSmpBrokerTransmission::decode(&encoded).unwrap();
        assert_eq!(decoded, transmission);
    }

    #[test]
    fn round_trips_v9_ids_broker_transmission() {
        let transmission = RadrootsSimplexSmpBrokerTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(3)),
            entity_id: Vec::new(),
            message: RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: vec![0x10],
                sender_id: vec![0x11],
                server_dh_public_key: vec![0x12, 0x13],
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: None,
                service_id: None,
                server_notification_credentials: None,
            }),
        };

        let encoded = transmission
            .encode_for_version(RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION)
            .unwrap();
        let decoded = RadrootsSimplexSmpBrokerTransmission::decode_for_version(
            RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION,
            &encoded,
        )
        .unwrap();
        assert_eq!(decoded, transmission);
    }

    #[test]
    fn round_trips_error_broker_transmission() {
        let transmission = RadrootsSimplexSmpBrokerTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(5)),
            entity_id: vec![0x01],
            message: RadrootsSimplexSmpBrokerMessage::Err(RadrootsSimplexSmpError::Command(
                RadrootsSimplexSmpCommandError::Prohibited,
            )),
        };

        let encoded = transmission.encode().unwrap();
        let decoded = RadrootsSimplexSmpBrokerTransmission::decode(&encoded).unwrap();
        assert_eq!(decoded, transmission);
    }

    #[test]
    fn round_trips_message_notification() {
        let transmission = RadrootsSimplexSmpBrokerTransmission {
            authorization: Vec::new(),
            correlation_id: None,
            entity_id: vec![0x99],
            message: RadrootsSimplexSmpBrokerMessage::NMsg {
                nonce: [0x22; 24],
                encrypted_metadata: vec![0x33, 0x44, 0x55],
            },
        };

        let encoded = transmission.encode().unwrap();
        let decoded = RadrootsSimplexSmpBrokerTransmission::decode(&encoded).unwrap();
        assert_eq!(decoded, transmission);
    }
}
