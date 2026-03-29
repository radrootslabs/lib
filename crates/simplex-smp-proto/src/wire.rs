use crate::error::RadrootsSimplexSmpProtoError;
use crate::uri::RadrootsSimplexSmpQueueMode;
use crate::version::{
    RADROOTS_SIMPLEX_SMP_BLOCKED_ENTITY_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION,
    RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION, RadrootsSimplexSmpVersionRange,
};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::net::Ipv6Addr;
use core::str::FromStr;

const TAG_NEW: &[u8] = b"NEW";
const TAG_SUB: &[u8] = b"SUB";
const TAG_SUBS: &[u8] = b"SUBS";
const TAG_KEY: &[u8] = b"KEY";
const TAG_RKEY: &[u8] = b"RKEY";
const TAG_LSET: &[u8] = b"LSET";
const TAG_LDEL: &[u8] = b"LDEL";
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
const TAG_LKEY: &[u8] = b"LKEY";
const TAG_LGET: &[u8] = b"LGET";
const TAG_NSUB: &[u8] = b"NSUB";
const TAG_NSUBS: &[u8] = b"NSUBS";
const TAG_PRXY: &[u8] = b"PRXY";
const TAG_PFWD: &[u8] = b"PFWD";
const TAG_RFWD: &[u8] = b"RFWD";

const TAG_IDS: &[u8] = b"IDS";
const TAG_LNK: &[u8] = b"LNK";
const TAG_SOK: &[u8] = b"SOK";
const TAG_SOKS: &[u8] = b"SOKS";
const TAG_NID: &[u8] = b"NID";
const TAG_MSG: &[u8] = b"MSG";
const TAG_NMSG: &[u8] = b"NMSG";
const TAG_PKEY: &[u8] = b"PKEY";
const TAG_RRES: &[u8] = b"RRES";
const TAG_PRES: &[u8] = b"PRES";
const TAG_END: &[u8] = b"END";
const TAG_ENDS: &[u8] = b"ENDS";
const TAG_DELD: &[u8] = b"DELD";
const TAG_INFO: &[u8] = b"INFO";
const TAG_OK: &[u8] = b"OK";
const TAG_ERR: &[u8] = b"ERR";
const TAG_PONG: &[u8] = b"PONG";

const COMMAND_ERR_UNKNOWN: &[u8] = b"UNKNOWN";
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
pub struct RadrootsSimplexSmpKeyList {
    pub first: Vec<u8>,
    pub rest: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpProtocolServer {
    pub hosts: Vec<String>,
    pub port: String,
    pub key_hash: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpCertChainPublicKey {
    pub certificate_chain: Vec<Vec<u8>>,
    pub signed_public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpBlockingReason {
    Spam,
    Content,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpBlockingInfo {
    pub reason: RadrootsSimplexSmpBlockingReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpHandshakeError {
    Parse,
    Identity,
    BadAuth,
    BadService,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpTransportError {
    Block,
    Version,
    LargeMsg,
    Session,
    NoAuth,
    Handshake(RadrootsSimplexSmpHandshakeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpNetworkError {
    Connect(String),
    Tls(String),
    UnknownCa,
    Failed,
    Timeout,
    Subscribe(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpBrokerError {
    Response(String),
    Unexpected(String),
    Network(RadrootsSimplexSmpNetworkError),
    Host,
    NoService,
    Transport(RadrootsSimplexSmpTransportError),
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpProxyError {
    Protocol(Box<RadrootsSimplexSmpError>),
    Broker(RadrootsSimplexSmpBrokerError),
    BasicAuth,
    NoSession,
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
    Subs,
    Key(Vec<u8>),
    RKey(RadrootsSimplexSmpKeyList),
    LSet {
        link_id: Vec<u8>,
        link_data: RadrootsSimplexSmpQueueLinkData,
    },
    LDel,
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
    LKey(Vec<u8>),
    LGet,
    NSub,
    NSubs,
    Prxy {
        server: RadrootsSimplexSmpProtocolServer,
        basic_auth: Option<String>,
    },
    PFwd {
        relay_version: u16,
        public_key: Vec<u8>,
        encrypted_transmission: Vec<u8>,
    },
    RFwd(Vec<u8>),
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
    Unknown,
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
    Proxy(RadrootsSimplexSmpProxyError),
    Auth,
    Blocked(RadrootsSimplexSmpBlockingInfo),
    Service,
    Crypto,
    Quota,
    Store(String),
    NoMsg,
    LargeMsg,
    Expired,
    Internal,
    Duplicate,
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
    Lnk {
        sender_id: Vec<u8>,
        link_data: RadrootsSimplexSmpQueueLinkData,
    },
    Sok(Option<Vec<u8>>),
    Soks(i64),
    Nid(RadrootsSimplexSmpNotifierIdsResponse),
    Msg(RadrootsSimplexSmpReceivedMessage),
    NMsg {
        nonce: [u8; 24],
        encrypted_metadata: Vec<u8>,
    },
    PKey {
        session_id: Vec<u8>,
        version_range: RadrootsSimplexSmpVersionRange,
        cert_chain_public_key: RadrootsSimplexSmpCertChainPublicKey,
    },
    RRes(Vec<u8>),
    PRes(Vec<u8>),
    End,
    Ends(i64),
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
            Self::Subs => buffer.extend_from_slice(TAG_SUBS),
            Self::Key(sender_auth_public_key) => {
                buffer.extend_from_slice(TAG_KEY);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, sender_auth_public_key)?;
            }
            Self::RKey(recipient_auth_public_keys) => {
                buffer.extend_from_slice(TAG_RKEY);
                buffer.push(b' ');
                push_short_key_list(&mut buffer, recipient_auth_public_keys)?;
            }
            Self::LSet { link_id, link_data } => {
                buffer.extend_from_slice(TAG_LSET);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, link_id)?;
                encode_queue_link_data(&mut buffer, link_data)?;
            }
            Self::LDel => buffer.extend_from_slice(TAG_LDEL),
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
            Self::LKey(sender_auth_public_key) => {
                buffer.extend_from_slice(TAG_LKEY);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, sender_auth_public_key)?;
            }
            Self::LGet => buffer.extend_from_slice(TAG_LGET),
            Self::NSub => buffer.extend_from_slice(TAG_NSUB),
            Self::NSubs => buffer.extend_from_slice(TAG_NSUBS),
            Self::Prxy { server, basic_auth } => {
                buffer.extend_from_slice(TAG_PRXY);
                buffer.push(b' ');
                encode_protocol_server(&mut buffer, server)?;
                push_maybe_string(&mut buffer, basic_auth.as_deref())?;
            }
            Self::PFwd {
                relay_version,
                public_key,
                encrypted_transmission,
            } => {
                buffer.extend_from_slice(TAG_PFWD);
                buffer.push(b' ');
                buffer.extend_from_slice(&relay_version.to_be_bytes());
                push_short_bytes(&mut buffer, public_key)?;
                buffer.extend_from_slice(encrypted_transmission);
            }
            Self::RFwd(encrypted_forward_transmission) => {
                buffer.extend_from_slice(TAG_RFWD);
                buffer.push(b' ');
                buffer.extend_from_slice(encrypted_forward_transmission);
            }
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
            TAG_SUBS => Self::Subs,
            TAG_KEY => Self::Key(cursor.read_short_bytes()?),
            TAG_RKEY => Self::RKey(cursor.read_short_key_list()?),
            TAG_LSET => Self::LSet {
                link_id: cursor.read_short_bytes()?,
                link_data: decode_queue_link_data(&mut cursor)?,
            },
            TAG_LDEL => Self::LDel,
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
            TAG_LKEY => Self::LKey(cursor.read_short_bytes()?),
            TAG_LGET => Self::LGet,
            TAG_NSUB => Self::NSub,
            TAG_NSUBS => Self::NSubs,
            TAG_PRXY => Self::Prxy {
                server: decode_protocol_server(&mut cursor)?,
                basic_auth: cursor.read_maybe_string()?,
            },
            TAG_PFWD => Self::PFwd {
                relay_version: u16::from_be_bytes(cursor.read_array::<2>()?),
                public_key: cursor.read_short_bytes()?,
                encrypted_transmission: cursor.read_remaining().to_vec(),
            },
            TAG_RFWD => Self::RFwd(cursor.read_remaining().to_vec()),
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
            Self::Lnk {
                sender_id,
                link_data,
            } => {
                buffer.extend_from_slice(TAG_LNK);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, sender_id)?;
                encode_queue_link_data(&mut buffer, link_data)?;
            }
            Self::Sok(service_id) => {
                if transport_version >= RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION {
                    buffer.extend_from_slice(TAG_SOK);
                    buffer.push(b' ');
                    push_maybe_short_bytes(&mut buffer, service_id.as_deref())?;
                } else {
                    buffer.extend_from_slice(TAG_OK);
                }
            }
            Self::Soks(queue_count) => {
                buffer.extend_from_slice(TAG_SOKS);
                buffer.push(b' ');
                push_i64(&mut buffer, *queue_count);
            }
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
            Self::PKey {
                session_id,
                version_range,
                cert_chain_public_key,
            } => {
                buffer.extend_from_slice(TAG_PKEY);
                buffer.push(b' ');
                push_short_bytes(&mut buffer, session_id)?;
                buffer.extend_from_slice(&version_range.min.to_be_bytes());
                buffer.extend_from_slice(&version_range.max.to_be_bytes());
                encode_cert_chain_public_key(&mut buffer, cert_chain_public_key)?;
            }
            Self::RRes(encrypted_forward_response) => {
                buffer.extend_from_slice(TAG_RRES);
                buffer.push(b' ');
                buffer.extend_from_slice(encrypted_forward_response);
            }
            Self::PRes(encrypted_response) => {
                buffer.extend_from_slice(TAG_PRES);
                buffer.push(b' ');
                buffer.extend_from_slice(encrypted_response);
            }
            Self::End => buffer.extend_from_slice(TAG_END),
            Self::Ends(queue_count) => {
                buffer.extend_from_slice(TAG_ENDS);
                buffer.push(b' ');
                push_i64(&mut buffer, *queue_count);
            }
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
                if transport_version < RADROOTS_SIMPLEX_SMP_BLOCKED_ENTITY_TRANSPORT_VERSION
                    && matches!(error, RadrootsSimplexSmpError::Blocked(_))
                {
                    buffer.extend_from_slice(b"AUTH");
                } else {
                    buffer.extend_from_slice(&encode_error(error));
                }
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
            TAG_LNK => Self::Lnk {
                sender_id: cursor.read_short_bytes()?,
                link_data: decode_queue_link_data(&mut cursor)?,
            },
            TAG_SOK => Self::Sok(cursor.read_maybe(Cursor::read_short_bytes)?),
            TAG_SOKS => Self::Soks(cursor.read_i64()?),
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
            TAG_PKEY => {
                let session_id = cursor.read_short_bytes()?;
                let min = u16::from_be_bytes(cursor.read_array::<2>()?);
                let max = u16::from_be_bytes(cursor.read_array::<2>()?);
                Self::PKey {
                    session_id,
                    version_range: RadrootsSimplexSmpVersionRange::new(min, max)?,
                    cert_chain_public_key: decode_cert_chain_public_key(&mut cursor)?,
                }
            }
            TAG_RRES => Self::RRes(cursor.read_remaining().to_vec()),
            TAG_PRES => Self::PRes(cursor.read_remaining().to_vec()),
            TAG_END => Self::End,
            TAG_ENDS => Self::Ends(cursor.read_i64()?),
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
            Self::Msg(_)
                | Self::NMsg { .. }
                | Self::RRes(_)
                | Self::PRes(_)
                | Self::Info(_)
                | Self::Err(_)
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
            transport_version,
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
        let (authorization, correlation_id, entity_id, frame) =
            decode_transmission(transport_version, bytes)?;
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
            transport_version,
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
        let (authorization, correlation_id, entity_id, frame) =
            decode_transmission(transport_version, bytes)?;
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
    if transport_version < RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION {
        return Err(RadrootsSimplexSmpProtoError::UnsupportedTransportVersion(
            transport_version,
        ));
    }

    buffer.extend_from_slice(TAG_NEW);
    buffer.push(b' ');
    push_short_bytes(buffer, &request.recipient_auth_public_key)?;
    push_short_bytes(buffer, &request.recipient_dh_public_key)?;
    if transport_version >= RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION {
        push_maybe_string(buffer, request.basic_auth.as_deref())?;
        buffer.push(encode_subscription_mode(request.subscription_mode));
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
        push_maybe_string(buffer, request.basic_auth.as_deref())?;
        buffer.push(encode_subscription_mode(request.subscription_mode));
        push_maybe(
            buffer,
            request.queue_request_data.as_ref(),
            encode_queue_request_data,
        )?;
    } else if transport_version >= RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION {
        push_maybe_string(buffer, request.basic_auth.as_deref())?;
        buffer.push(encode_subscription_mode(request.subscription_mode));
        buffer.push(encode_bool(request.sender_can_secure()));
    } else {
        push_legacy_basic_auth(buffer, request.basic_auth.as_deref())?;
        buffer.push(encode_subscription_mode(request.subscription_mode));
    }

    Ok(())
}

fn decode_new_request(
    cursor: &mut Cursor<'_>,
    transport_version: u16,
) -> Result<RadrootsSimplexSmpNewQueueRequest, RadrootsSimplexSmpProtoError> {
    if transport_version < RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION {
        return Err(RadrootsSimplexSmpProtoError::UnsupportedTransportVersion(
            transport_version,
        ));
    }

    let recipient_auth_public_key = cursor.read_short_bytes()?;
    let recipient_dh_public_key = cursor.read_short_bytes()?;
    let (basic_auth, subscription_mode, queue_request_data, notifier_credentials) =
        if transport_version >= RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION {
            (
                cursor.read_maybe_string()?,
                decode_subscription_mode(cursor.read_byte()?)?,
                cursor.read_maybe(decode_queue_request_data)?,
                cursor.read_maybe(decode_new_notifier_credentials)?,
            )
        } else if transport_version >= RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION {
            (
                cursor.read_maybe_string()?,
                decode_subscription_mode(cursor.read_byte()?)?,
                cursor.read_maybe(decode_queue_request_data)?,
                None,
            )
        } else if transport_version >= RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION {
            let basic_auth = cursor.read_maybe_string()?;
            let subscription_mode = decode_subscription_mode(cursor.read_byte()?)?;
            let sender_can_secure = decode_bool(cursor.read_byte()?)?;
            let queue_request_data = Some(if sender_can_secure {
                RadrootsSimplexSmpQueueRequestData::Messaging(None)
            } else {
                RadrootsSimplexSmpQueueRequestData::Contact(None)
            });
            (basic_auth, subscription_mode, queue_request_data, None)
        } else {
            (
                cursor.read_legacy_basic_auth()?,
                decode_subscription_mode(cursor.read_byte()?)?,
                None,
                None,
            )
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

fn encode_protocol_server(
    buffer: &mut Vec<u8>,
    server: &RadrootsSimplexSmpProtocolServer,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    validate_transport_hosts(&server.hosts)?;
    push_short_string_list(buffer, &server.hosts)?;
    push_short_string(buffer, &server.port)?;
    push_short_bytes(buffer, &server.key_hash)
}

fn decode_protocol_server(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexSmpProtocolServer, RadrootsSimplexSmpProtoError> {
    let hosts = cursor.read_short_string_list()?;
    validate_transport_hosts(&hosts)?;
    Ok(RadrootsSimplexSmpProtocolServer {
        hosts,
        port: cursor.read_short_string_lossy()?,
        key_hash: cursor.read_short_bytes()?,
    })
}

fn encode_cert_chain_public_key(
    buffer: &mut Vec<u8>,
    cert_chain_public_key: &RadrootsSimplexSmpCertChainPublicKey,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    push_large_bytes_list(buffer, &cert_chain_public_key.certificate_chain)?;
    push_large_bytes(buffer, &cert_chain_public_key.signed_public_key)
}

fn decode_cert_chain_public_key(
    cursor: &mut Cursor<'_>,
) -> Result<RadrootsSimplexSmpCertChainPublicKey, RadrootsSimplexSmpProtoError> {
    Ok(RadrootsSimplexSmpCertChainPublicKey {
        certificate_chain: cursor.read_large_bytes_list()?,
        signed_public_key: cursor.read_large_bytes()?,
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

fn validate_transport_hosts(hosts: &[String]) -> Result<(), RadrootsSimplexSmpProtoError> {
    for host in hosts {
        validate_transport_host(host)?;
    }
    Ok(())
}

fn validate_transport_host(host: &str) -> Result<(), RadrootsSimplexSmpProtoError> {
    if is_valid_ipv4_transport_host(host)
        || is_valid_ipv6_transport_host(host)
        || is_valid_onion_transport_host(host)
        || is_valid_domain_transport_host(host)
    {
        return Ok(());
    }
    Err(RadrootsSimplexSmpProtoError::InvalidHostList(
        host.to_string(),
    ))
}

fn is_valid_ipv4_transport_host(host: &str) -> bool {
    let mut segments = 0_usize;
    for segment in host.split('.') {
        if segment.is_empty() || !segment.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
        if segment.parse::<u16>().map_or(true, |value| value > 255) {
            return false;
        }
        segments += 1;
    }
    segments == 4
}

fn is_valid_ipv6_transport_host(host: &str) -> bool {
    let candidate = if let Some(stripped) = host.strip_prefix('[') {
        let Some(inner) = stripped.strip_suffix(']') else {
            return false;
        };
        inner
    } else {
        if host.ends_with(']') {
            return false;
        }
        host
    };
    if candidate.is_empty()
        || !candidate
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b':')
    {
        return false;
    }
    Ipv6Addr::from_str(candidate).is_ok()
}

fn is_valid_onion_transport_host(host: &str) -> bool {
    let Some(prefix) = host.strip_suffix(".onion") else {
        return false;
    };
    !prefix.is_empty()
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn is_valid_domain_transport_host(host: &str) -> bool {
    !host.is_empty()
        && !host.ends_with(".onion")
        && !host.starts_with('[')
        && !host.ends_with(']')
        && !host.contains(':')
        && host
            .chars()
            .all(|character| !matches!(character, '#' | ',' | ';' | '/' | ' ' | '\n' | '\r' | '\t'))
}

fn encode_transmission(
    transport_version: u16,
    authorization: &[u8],
    correlation_id: Option<RadrootsSimplexSmpCorrelationId>,
    entity_id: &[u8],
    frame: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
    let mut buffer = Vec::new();
    push_short_bytes(&mut buffer, authorization)?;
    if transport_version >= RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION
        && !authorization.is_empty()
    {
        push_maybe_short_bytes(&mut buffer, None)?;
    }
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
    transport_version: u16,
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
    if transport_version >= RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION
        && !authorization.is_empty()
    {
        let _ = cursor.read_maybe(Cursor::read_short_bytes)?;
    }
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
                RadrootsSimplexSmpCommandError::Unknown => COMMAND_ERR_UNKNOWN,
                RadrootsSimplexSmpCommandError::Syntax => COMMAND_ERR_SYNTAX,
                RadrootsSimplexSmpCommandError::Prohibited => COMMAND_ERR_PROHIBITED,
                RadrootsSimplexSmpCommandError::NoAuth => COMMAND_ERR_NO_AUTH,
                RadrootsSimplexSmpCommandError::HasAuth => COMMAND_ERR_HAS_AUTH,
                RadrootsSimplexSmpCommandError::NoEntity => COMMAND_ERR_NO_ENTITY,
                RadrootsSimplexSmpCommandError::Other(raw) => raw,
            });
            bytes
        }
        RadrootsSimplexSmpError::Proxy(proxy_error) => {
            let mut bytes = b"PROXY ".to_vec();
            bytes.extend_from_slice(&encode_proxy_error(proxy_error));
            bytes
        }
        RadrootsSimplexSmpError::Auth => b"AUTH".to_vec(),
        RadrootsSimplexSmpError::Blocked(blocking_info) => {
            let mut bytes = b"BLOCKED ".to_vec();
            bytes.extend_from_slice(&encode_blocking_info(blocking_info));
            bytes
        }
        RadrootsSimplexSmpError::Service => b"SERVICE".to_vec(),
        RadrootsSimplexSmpError::Crypto => b"CRYPTO".to_vec(),
        RadrootsSimplexSmpError::Quota => b"QUOTA".to_vec(),
        RadrootsSimplexSmpError::Store(store_error) => {
            let mut bytes = b"STORE ".to_vec();
            bytes.extend_from_slice(store_error.as_bytes());
            bytes
        }
        RadrootsSimplexSmpError::NoMsg => b"NO_MSG".to_vec(),
        RadrootsSimplexSmpError::LargeMsg => b"LARGE_MSG".to_vec(),
        RadrootsSimplexSmpError::Expired => b"EXPIRED".to_vec(),
        RadrootsSimplexSmpError::Internal => b"INTERNAL".to_vec(),
        RadrootsSimplexSmpError::Duplicate => b"DUPLICATE_".to_vec(),
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
    if bytes == b"SERVICE" {
        return Ok(RadrootsSimplexSmpError::Service);
    }
    if bytes == b"CRYPTO" {
        return Ok(RadrootsSimplexSmpError::Crypto);
    }
    if bytes == b"QUOTA" {
        return Ok(RadrootsSimplexSmpError::Quota);
    }
    if let Some(store_error) = bytes.strip_prefix(b"STORE ") {
        return Ok(RadrootsSimplexSmpError::Store(
            String::from_utf8_lossy(store_error).into_owned(),
        ));
    }
    if bytes == b"NO_MSG" {
        return Ok(RadrootsSimplexSmpError::NoMsg);
    }
    if bytes == b"LARGE_MSG" {
        return Ok(RadrootsSimplexSmpError::LargeMsg);
    }
    if bytes == b"EXPIRED" {
        return Ok(RadrootsSimplexSmpError::Expired);
    }
    if bytes == b"INTERNAL" {
        return Ok(RadrootsSimplexSmpError::Internal);
    }
    if bytes == b"DUPLICATE_" {
        return Ok(RadrootsSimplexSmpError::Duplicate);
    }
    if let Some(command) = bytes.strip_prefix(b"CMD ") {
        let command_error = match command {
            COMMAND_ERR_UNKNOWN => RadrootsSimplexSmpCommandError::Unknown,
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
    if let Some(proxy_error) = bytes.strip_prefix(b"PROXY ") {
        return Ok(RadrootsSimplexSmpError::Proxy(decode_proxy_error(
            proxy_error,
        )?));
    }
    if let Some(blocking_info) = bytes.strip_prefix(b"BLOCKED ") {
        return Ok(RadrootsSimplexSmpError::Blocked(decode_blocking_info(
            blocking_info,
        )?));
    }
    Ok(RadrootsSimplexSmpError::Other(bytes.to_vec()))
}

fn encode_proxy_error(error: &RadrootsSimplexSmpProxyError) -> Vec<u8> {
    match error {
        RadrootsSimplexSmpProxyError::Protocol(error) => {
            let mut bytes = b"PROTOCOL ".to_vec();
            bytes.extend_from_slice(&encode_error(error));
            bytes
        }
        RadrootsSimplexSmpProxyError::Broker(error) => {
            let mut bytes = b"BROKER ".to_vec();
            bytes.extend_from_slice(&encode_broker_error(error));
            bytes
        }
        RadrootsSimplexSmpProxyError::BasicAuth => b"BASIC_AUTH".to_vec(),
        RadrootsSimplexSmpProxyError::NoSession => b"NO_SESSION".to_vec(),
    }
}

fn decode_proxy_error(
    bytes: &[u8],
) -> Result<RadrootsSimplexSmpProxyError, RadrootsSimplexSmpProtoError> {
    let (tag, rest) = parse_tag(bytes)?;
    match tag.as_slice() {
        b"PROTOCOL" => Ok(RadrootsSimplexSmpProxyError::Protocol(Box::new(
            decode_error(rest)?,
        ))),
        b"BROKER" => Ok(RadrootsSimplexSmpProxyError::Broker(decode_broker_error(
            rest,
        )?)),
        b"BASIC_AUTH" if rest.is_empty() => Ok(RadrootsSimplexSmpProxyError::BasicAuth),
        b"NO_SESSION" if rest.is_empty() => Ok(RadrootsSimplexSmpProxyError::NoSession),
        _ => Err(invalid_ascii_tag(&tag)),
    }
}

fn encode_broker_error(error: &RadrootsSimplexSmpBrokerError) -> Vec<u8> {
    match error {
        RadrootsSimplexSmpBrokerError::Response(response_error) => {
            let mut bytes = b"RESPONSE ".to_vec();
            push_short_string(&mut bytes, response_error)
                .expect("response_error length is bounded by SMP short-string encoding");
            bytes
        }
        RadrootsSimplexSmpBrokerError::Unexpected(response_error) => {
            let mut bytes = b"UNEXPECTED ".to_vec();
            push_short_string(&mut bytes, response_error)
                .expect("response_error length is bounded by SMP short-string encoding");
            bytes
        }
        RadrootsSimplexSmpBrokerError::Network(_) => b"NETWORK".to_vec(),
        RadrootsSimplexSmpBrokerError::Host => b"HOST".to_vec(),
        RadrootsSimplexSmpBrokerError::NoService => b"NO_SERVICE".to_vec(),
        RadrootsSimplexSmpBrokerError::Transport(error) => {
            let mut bytes = b"TRANSPORT ".to_vec();
            bytes.extend_from_slice(&encode_transport_error(error));
            bytes
        }
        RadrootsSimplexSmpBrokerError::Timeout => b"TIMEOUT".to_vec(),
    }
}

fn decode_broker_error(
    bytes: &[u8],
) -> Result<RadrootsSimplexSmpBrokerError, RadrootsSimplexSmpProtoError> {
    let (tag, rest) = parse_tag(bytes)?;
    match tag.as_slice() {
        b"RESPONSE" => Ok(RadrootsSimplexSmpBrokerError::Response(
            decode_short_string_lossy(rest)?,
        )),
        b"UNEXPECTED" => Ok(RadrootsSimplexSmpBrokerError::Unexpected(
            decode_short_string_lossy(rest)?,
        )),
        b"TRANSPORT" => Ok(RadrootsSimplexSmpBrokerError::Transport(
            decode_transport_error(rest)?,
        )),
        b"NETWORK" if rest.is_empty() => Ok(RadrootsSimplexSmpBrokerError::Network(
            RadrootsSimplexSmpNetworkError::Failed,
        )),
        b"NETWORK" => Ok(RadrootsSimplexSmpBrokerError::Network(
            decode_network_error(rest)?,
        )),
        b"TIMEOUT" if rest.is_empty() => Ok(RadrootsSimplexSmpBrokerError::Timeout),
        b"HOST" if rest.is_empty() => Ok(RadrootsSimplexSmpBrokerError::Host),
        b"NO_SERVICE" if rest.is_empty() => Ok(RadrootsSimplexSmpBrokerError::NoService),
        _ => Err(invalid_ascii_tag(&tag)),
    }
}

fn encode_transport_error(error: &RadrootsSimplexSmpTransportError) -> Vec<u8> {
    match error {
        RadrootsSimplexSmpTransportError::Block => b"BLOCK".to_vec(),
        RadrootsSimplexSmpTransportError::Version => b"VERSION".to_vec(),
        RadrootsSimplexSmpTransportError::LargeMsg => b"LARGE_MSG".to_vec(),
        RadrootsSimplexSmpTransportError::Session => b"SESSION".to_vec(),
        RadrootsSimplexSmpTransportError::NoAuth => b"NO_AUTH".to_vec(),
        RadrootsSimplexSmpTransportError::Handshake(error) => {
            let mut bytes = b"HANDSHAKE ".to_vec();
            bytes.extend_from_slice(&encode_handshake_error(error));
            bytes
        }
    }
}

fn decode_transport_error(
    bytes: &[u8],
) -> Result<RadrootsSimplexSmpTransportError, RadrootsSimplexSmpProtoError> {
    let (tag, rest) = parse_tag(bytes)?;
    match tag.as_slice() {
        b"BLOCK" if rest.is_empty() => Ok(RadrootsSimplexSmpTransportError::Block),
        b"VERSION" if rest.is_empty() => Ok(RadrootsSimplexSmpTransportError::Version),
        b"LARGE_MSG" if rest.is_empty() => Ok(RadrootsSimplexSmpTransportError::LargeMsg),
        b"SESSION" if rest.is_empty() => Ok(RadrootsSimplexSmpTransportError::Session),
        b"NO_AUTH" if rest.is_empty() => Ok(RadrootsSimplexSmpTransportError::NoAuth),
        b"HANDSHAKE" => Ok(RadrootsSimplexSmpTransportError::Handshake(
            decode_handshake_error(rest)?,
        )),
        _ => Err(invalid_ascii_tag(&tag)),
    }
}

#[cfg(test)]
fn encode_network_error(error: &RadrootsSimplexSmpNetworkError) -> Vec<u8> {
    match error {
        RadrootsSimplexSmpNetworkError::Connect(connect_error) => {
            let mut bytes = b"CONNECT ".to_vec();
            push_short_string(&mut bytes, connect_error)
                .expect("connect_error length is bounded by SMP short-string encoding");
            bytes
        }
        RadrootsSimplexSmpNetworkError::Tls(tls_error) => {
            let mut bytes = b"TLS ".to_vec();
            push_short_string(&mut bytes, tls_error)
                .expect("tls_error length is bounded by SMP short-string encoding");
            bytes
        }
        RadrootsSimplexSmpNetworkError::UnknownCa => b"UNKNOWNCA".to_vec(),
        RadrootsSimplexSmpNetworkError::Failed => b"FAILED".to_vec(),
        RadrootsSimplexSmpNetworkError::Timeout => b"TIMEOUT".to_vec(),
        RadrootsSimplexSmpNetworkError::Subscribe(subscribe_error) => {
            let mut bytes = b"SUBSCRIBE ".to_vec();
            push_short_string(&mut bytes, subscribe_error)
                .expect("subscribe_error length is bounded by SMP short-string encoding");
            bytes
        }
    }
}

fn decode_network_error(
    bytes: &[u8],
) -> Result<RadrootsSimplexSmpNetworkError, RadrootsSimplexSmpProtoError> {
    let (tag, rest) = parse_tag(bytes)?;
    match tag.as_slice() {
        b"CONNECT" => Ok(RadrootsSimplexSmpNetworkError::Connect(
            decode_short_string_lossy(rest)?,
        )),
        b"TLS" => Ok(RadrootsSimplexSmpNetworkError::Tls(
            decode_short_string_lossy(rest)?,
        )),
        b"UNKNOWNCA" if rest.is_empty() => Ok(RadrootsSimplexSmpNetworkError::UnknownCa),
        b"FAILED" if rest.is_empty() => Ok(RadrootsSimplexSmpNetworkError::Failed),
        b"TIMEOUT" if rest.is_empty() => Ok(RadrootsSimplexSmpNetworkError::Timeout),
        b"SUBSCRIBE" => Ok(RadrootsSimplexSmpNetworkError::Subscribe(
            decode_short_string_lossy(rest)?,
        )),
        _ => Err(invalid_ascii_tag(&tag)),
    }
}

fn encode_handshake_error(error: &RadrootsSimplexSmpHandshakeError) -> Vec<u8> {
    match error {
        RadrootsSimplexSmpHandshakeError::Parse => b"PARSE".to_vec(),
        RadrootsSimplexSmpHandshakeError::Identity => b"IDENTITY".to_vec(),
        RadrootsSimplexSmpHandshakeError::BadAuth => b"BAD_AUTH".to_vec(),
        RadrootsSimplexSmpHandshakeError::BadService => b"BAD_SERVICE".to_vec(),
        RadrootsSimplexSmpHandshakeError::Other(raw) => raw.as_bytes().to_vec(),
    }
}

fn decode_handshake_error(
    bytes: &[u8],
) -> Result<RadrootsSimplexSmpHandshakeError, RadrootsSimplexSmpProtoError> {
    match bytes {
        b"PARSE" => Ok(RadrootsSimplexSmpHandshakeError::Parse),
        b"IDENTITY" => Ok(RadrootsSimplexSmpHandshakeError::Identity),
        b"BAD_AUTH" => Ok(RadrootsSimplexSmpHandshakeError::BadAuth),
        b"BAD_SERVICE" => Ok(RadrootsSimplexSmpHandshakeError::BadService),
        raw => Err(invalid_ascii_tag(raw)),
    }
}

fn encode_blocking_info(info: &RadrootsSimplexSmpBlockingInfo) -> Vec<u8> {
    let mut bytes = b"reason=".to_vec();
    bytes.extend_from_slice(match &info.reason {
        RadrootsSimplexSmpBlockingReason::Spam => b"spam",
        RadrootsSimplexSmpBlockingReason::Content => b"content",
        RadrootsSimplexSmpBlockingReason::Other(reason) => reason.as_bytes(),
    });
    bytes
}

fn decode_blocking_info(
    bytes: &[u8],
) -> Result<RadrootsSimplexSmpBlockingInfo, RadrootsSimplexSmpProtoError> {
    let Some(reason) = bytes.strip_prefix(b"reason=") else {
        return Err(invalid_ascii_tag(bytes));
    };
    let reason = match reason {
        b"spam" => RadrootsSimplexSmpBlockingReason::Spam,
        b"content" => RadrootsSimplexSmpBlockingReason::Content,
        raw => return Err(invalid_ascii_tag(raw)),
    };
    Ok(RadrootsSimplexSmpBlockingInfo { reason })
}

fn decode_short_string_lossy(bytes: &[u8]) -> Result<String, RadrootsSimplexSmpProtoError> {
    let mut cursor = Cursor::new(bytes);
    let value = cursor.read_short_string_lossy()?;
    if !cursor.is_empty() {
        return Err(RadrootsSimplexSmpProtoError::TrailingBytes);
    }
    Ok(value)
}

fn invalid_ascii_tag(bytes: &[u8]) -> RadrootsSimplexSmpProtoError {
    RadrootsSimplexSmpProtoError::InvalidTag(String::from_utf8_lossy(bytes).into_owned())
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

fn push_i64(buffer: &mut Vec<u8>, value: i64) {
    buffer.extend_from_slice(&value.to_be_bytes());
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

fn push_short_string(
    buffer: &mut Vec<u8>,
    value: &str,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    push_short_bytes(buffer, value.as_bytes())
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

fn push_short_key_list(
    buffer: &mut Vec<u8>,
    keys: &RadrootsSimplexSmpKeyList,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    let len = 1 + keys.rest.len();
    let len =
        u8::try_from(len).map_err(|_| RadrootsSimplexSmpProtoError::InvalidListLength(len))?;
    buffer.push(len);
    push_short_bytes(buffer, &keys.first)?;
    for key in &keys.rest {
        push_short_bytes(buffer, key)?;
    }
    Ok(())
}

fn push_short_string_list(
    buffer: &mut Vec<u8>,
    values: &[String],
) -> Result<(), RadrootsSimplexSmpProtoError> {
    if values.is_empty() {
        return Err(RadrootsSimplexSmpProtoError::InvalidListLength(0));
    }
    let len = u8::try_from(values.len())
        .map_err(|_| RadrootsSimplexSmpProtoError::InvalidListLength(values.len()))?;
    buffer.push(len);
    for value in values {
        push_short_string(buffer, value)?;
    }
    Ok(())
}

fn push_large_bytes_list(
    buffer: &mut Vec<u8>,
    values: &[Vec<u8>],
) -> Result<(), RadrootsSimplexSmpProtoError> {
    if values.is_empty() {
        return Err(RadrootsSimplexSmpProtoError::InvalidListLength(0));
    }
    let len = u8::try_from(values.len())
        .map_err(|_| RadrootsSimplexSmpProtoError::InvalidListLength(values.len()))?;
    buffer.push(len);
    for value in values {
        push_large_bytes(buffer, value)?;
    }
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

fn push_legacy_basic_auth(
    buffer: &mut Vec<u8>,
    value: Option<&str>,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    match value {
        None => Ok(()),
        Some(value) => {
            validate_basic_auth(value)?;
            buffer.push(b'A');
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

    fn read_short_string(&mut self) -> Result<String, RadrootsSimplexSmpProtoError> {
        String::from_utf8(self.read_short_bytes()?)
            .map_err(|error| RadrootsSimplexSmpProtoError::InvalidUtf8(error.to_string()))
    }

    fn read_short_string_lossy(&mut self) -> Result<String, RadrootsSimplexSmpProtoError> {
        Ok(String::from_utf8_lossy(&self.read_short_bytes()?).into_owned())
    }

    fn read_short_key_list(
        &mut self,
    ) -> Result<RadrootsSimplexSmpKeyList, RadrootsSimplexSmpProtoError> {
        let len = usize::from(self.read_byte()?);
        if len == 0 {
            return Err(RadrootsSimplexSmpProtoError::InvalidListLength(0));
        }
        let first = self.read_short_bytes()?;
        let mut rest = Vec::with_capacity(len.saturating_sub(1));
        for _ in 1..len {
            rest.push(self.read_short_bytes()?);
        }
        Ok(RadrootsSimplexSmpKeyList { first, rest })
    }

    fn read_short_string_list(&mut self) -> Result<Vec<String>, RadrootsSimplexSmpProtoError> {
        let len = usize::from(self.read_byte()?);
        if len == 0 {
            return Err(RadrootsSimplexSmpProtoError::InvalidListLength(0));
        }
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            values.push(self.read_short_string()?);
        }
        Ok(values)
    }

    fn read_large_bytes(&mut self) -> Result<Vec<u8>, RadrootsSimplexSmpProtoError> {
        let len = usize::from(u16::from_be_bytes(self.read_array::<2>()?));
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_large_bytes_list(&mut self) -> Result<Vec<Vec<u8>>, RadrootsSimplexSmpProtoError> {
        let len = usize::from(self.read_byte()?);
        if len == 0 {
            return Err(RadrootsSimplexSmpProtoError::InvalidListLength(0));
        }
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            values.push(self.read_large_bytes()?);
        }
        Ok(values)
    }

    fn read_i64(&mut self) -> Result<i64, RadrootsSimplexSmpProtoError> {
        Ok(i64::from_be_bytes(self.read_array::<8>()?))
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

    fn read_legacy_basic_auth(&mut self) -> Result<Option<String>, RadrootsSimplexSmpProtoError> {
        match self.bytes.get(self.offset).copied() {
            Some(b'A') => {
                self.offset += 1;
                let value = self.read_short_bytes()?;
                let string = String::from_utf8(value).map_err(|error| {
                    RadrootsSimplexSmpProtoError::InvalidUtf8(error.to_string())
                })?;
                validate_basic_auth(&string)?;
                Ok(Some(string))
            }
            Some(_) => Ok(None),
            None => Err(RadrootsSimplexSmpProtoError::UnexpectedEof),
        }
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
    fn round_trips_v6_new_command_transmission() {
        let transmission = RadrootsSimplexSmpCommandTransmission {
            authorization: vec![1, 2, 3],
            correlation_id: Some(correlation_id(7)),
            entity_id: Vec::new(),
            command: RadrootsSimplexSmpCommand::New(RadrootsSimplexSmpNewQueueRequest {
                recipient_auth_public_key: vec![0x01, 0x02, 0x03],
                recipient_dh_public_key: vec![0x04, 0x05],
                basic_auth: Some("server-pass".to_string()),
                subscription_mode: RadrootsSimplexSmpSubscriptionMode::Subscribe,
                queue_request_data: None,
                notifier_credentials: None,
            }),
        };

        let encoded = transmission
            .encode_for_version(RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION)
            .unwrap();
        let decoded = RadrootsSimplexSmpCommandTransmission::decode_for_version(
            RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION,
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
    fn round_trips_v15_new_command_transmission() {
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
                notifier_credentials: None,
            }),
        };

        let encoded = transmission
            .encode_for_version(RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION)
            .unwrap();
        let decoded = RadrootsSimplexSmpCommandTransmission::decode_for_version(
            RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION,
            &encoded,
        )
        .unwrap();
        assert_eq!(decoded, transmission);
    }

    #[test]
    fn current_authenticated_transmission_encodes_absent_service_signature_as_maybe_none() {
        let transmission = RadrootsSimplexSmpCommandTransmission {
            authorization: vec![1, 2, 3],
            correlation_id: Some(correlation_id(7)),
            entity_id: Vec::new(),
            command: RadrootsSimplexSmpCommand::Ping,
        };

        let encoded = transmission.encode().unwrap();
        assert_eq!(encoded[0], 3);
        assert_eq!(&encoded[1..4], &[1, 2, 3]);
        assert_eq!(encoded[4], b'0');

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

    #[test]
    fn v6_new_command_uses_legacy_basic_auth_layout() {
        let command = RadrootsSimplexSmpCommand::New(RadrootsSimplexSmpNewQueueRequest {
            recipient_auth_public_key: vec![0x01, 0x02, 0x03],
            recipient_dh_public_key: vec![0x04, 0x05],
            basic_auth: Some("server-pass".to_string()),
            subscription_mode: RadrootsSimplexSmpSubscriptionMode::Subscribe,
            queue_request_data: Some(RadrootsSimplexSmpQueueRequestData::Messaging(None)),
            notifier_credentials: Some(RadrootsSimplexSmpNewNotifierCredentials {
                notifier_auth_public_key: vec![0x21, 0x22],
                recipient_notification_dh_public_key: vec![0x23, 0x24],
            }),
        });

        let encoded = command
            .encode_for_version(RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION)
            .unwrap();

        assert_eq!(
            encoded,
            b"NEW \x03\x01\x02\x03\x02\x04\x05A\x0bserver-passS".to_vec()
        );
    }

    #[test]
    fn v9_new_matches_official_sender_secure_layout() {
        let command = RadrootsSimplexSmpCommand::New(RadrootsSimplexSmpNewQueueRequest {
            recipient_auth_public_key: vec![0x01, 0x02, 0x03],
            recipient_dh_public_key: vec![0x04, 0x05],
            basic_auth: Some("server-pass".to_string()),
            subscription_mode: RadrootsSimplexSmpSubscriptionMode::Subscribe,
            queue_request_data: Some(RadrootsSimplexSmpQueueRequestData::Messaging(None)),
            notifier_credentials: None,
        });

        let encoded = command
            .encode_for_version(RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION)
            .unwrap();

        assert_eq!(
            encoded,
            b"NEW \x03\x01\x02\x03\x02\x04\x051\x0bserver-passST".to_vec()
        );
    }

    #[test]
    fn v15_new_matches_official_short_link_layout() {
        let command = RadrootsSimplexSmpCommand::New(RadrootsSimplexSmpNewQueueRequest {
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
        });

        let encoded = command
            .encode_for_version(RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION)
            .unwrap();

        assert_eq!(
            encoded,
            b"NEW \x03\x01\x02\x03\x02\x04\x051\x0bserver-passS1M1\x02\x10\x11\x00\x02\xaa\xbb\x00\x03\xcc\xdd\xee"
                .to_vec()
        );
    }

    #[test]
    fn v17_ids_matches_official_notifier_layout() {
        let response = RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
            recipient_id: vec![0x10],
            sender_id: vec![0x11],
            server_dh_public_key: vec![0x12, 0x13],
            queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
            link_id: Some(vec![0x14, 0x15]),
            service_id: Some(vec![0x16, 0x17]),
            server_notification_credentials: Some(RadrootsSimplexSmpServerNotifierCredentials {
                notifier_id: vec![0x18, 0x19],
                server_notification_dh_public_key: vec![0x1a, 0x1b],
            }),
        });

        let encoded = response.encode().unwrap();

        assert_eq!(
            encoded,
            b"IDS \x01\x10\x01\x11\x02\x12\x131M1\x02\x14\x151\x02\x16\x171\x02\x18\x19\x02\x1a\x1b"
                .to_vec()
        );
    }

    #[test]
    fn v15_ids_matches_official_short_link_layout() {
        let response = RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
            recipient_id: vec![0x10],
            sender_id: vec![0x11],
            server_dh_public_key: vec![0x12, 0x13],
            queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
            link_id: Some(vec![0x14, 0x15]),
            service_id: Some(vec![0x16, 0x17]),
            server_notification_credentials: None,
        });

        let encoded = response
            .encode_for_version(RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION)
            .unwrap();

        assert_eq!(
            encoded,
            b"IDS \x01\x10\x01\x11\x02\x12\x131M1\x02\x14\x15".to_vec()
        );
    }

    #[test]
    fn v9_ids_matches_official_sender_secure_layout() {
        let response = RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
            recipient_id: vec![0x10],
            sender_id: vec![0x11],
            server_dh_public_key: vec![0x12, 0x13],
            queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
            link_id: Some(vec![0x14, 0x15]),
            service_id: Some(vec![0x16, 0x17]),
            server_notification_credentials: None,
        });

        let encoded = response
            .encode_for_version(RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION)
            .unwrap();

        assert_eq!(encoded, b"IDS \x01\x10\x01\x11\x02\x12\x13T".to_vec());
    }

    #[test]
    fn prxy_matches_official_proxy_session_layout() {
        let command = RadrootsSimplexSmpCommand::Prxy {
            server: RadrootsSimplexSmpProtocolServer {
                hosts: vec![
                    "smp4.simplex.im".to_string(),
                    "simplexabc.onion".to_string(),
                ],
                port: "5223".to_string(),
                key_hash: vec![0xaa, 0xbb, 0xcc],
            },
            basic_auth: Some("relay-pass".to_string()),
        };

        let encoded = command.encode().unwrap();

        assert_eq!(
            encoded,
            b"PRXY \x02\x0fsmp4.simplex.im\x10simplexabc.onion\x045223\x03\xaa\xbb\xcc1\x0arelay-pass"
                .to_vec()
        );
    }

    #[test]
    fn pkey_matches_official_proxy_session_key_layout() {
        let message = RadrootsSimplexSmpBrokerMessage::PKey {
            session_id: vec![0x31, 0x32],
            version_range: RadrootsSimplexSmpVersionRange::new(8, 16).unwrap(),
            cert_chain_public_key: RadrootsSimplexSmpCertChainPublicKey {
                certificate_chain: vec![vec![0x41, 0x42], vec![0x43, 0x44, 0x45]],
                signed_public_key: vec![0x51, 0x52, 0x53],
            },
        };

        let encoded = message.encode().unwrap();

        assert_eq!(
            encoded,
            b"PKEY \x0212\x00\x08\x00\x10\x02\x00\x02AB\x00\x03CDE\x00\x03QRS".to_vec()
        );
    }

    #[test]
    fn proxy_transport_error_matches_official_nested_error_layout() {
        let encoded = RadrootsSimplexSmpBrokerMessage::Err(RadrootsSimplexSmpError::Proxy(
            RadrootsSimplexSmpProxyError::Broker(RadrootsSimplexSmpBrokerError::Transport(
                RadrootsSimplexSmpTransportError::Handshake(
                    RadrootsSimplexSmpHandshakeError::Identity,
                ),
            )),
        ))
        .encode()
        .unwrap();

        assert_eq!(
            encoded,
            b"ERR PROXY BROKER TRANSPORT HANDSHAKE IDENTITY".to_vec()
        );
    }

    #[test]
    fn round_trips_proxy_and_short_link_commands() {
        let prxy = RadrootsSimplexSmpCommandTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(1)),
            entity_id: Vec::new(),
            command: RadrootsSimplexSmpCommand::Prxy {
                server: RadrootsSimplexSmpProtocolServer {
                    hosts: vec![
                        "smp4.simplex.im".to_string(),
                        "simplexabc.onion".to_string(),
                    ],
                    port: "5223".to_string(),
                    key_hash: vec![0xaa, 0xbb, 0xcc],
                },
                basic_auth: Some("relay-pass".to_string()),
            },
        };

        let rkey = RadrootsSimplexSmpCommandTransmission {
            authorization: vec![0x42],
            correlation_id: Some(correlation_id(2)),
            entity_id: vec![0x11],
            command: RadrootsSimplexSmpCommand::RKey(RadrootsSimplexSmpKeyList {
                first: vec![0x01, 0x02],
                rest: vec![vec![0x03, 0x04], vec![0x05, 0x06]],
            }),
        };

        let pkey_encoded = prxy.encode().unwrap();
        let pkey_decoded = RadrootsSimplexSmpCommandTransmission::decode(&pkey_encoded).unwrap();
        assert_eq!(pkey_decoded, prxy);

        let rkey_encoded = rkey.encode().unwrap();
        let rkey_decoded = RadrootsSimplexSmpCommandTransmission::decode(&rkey_encoded).unwrap();
        assert_eq!(rkey_decoded, rkey);
    }

    #[test]
    fn protocol_server_accepts_official_transport_host_forms() {
        let server = RadrootsSimplexSmpProtocolServer {
            hosts: vec![
                "smp4.simplex.im".to_string(),
                "192.0.2.24".to_string(),
                "2001:db8::24".to_string(),
                "[2001:db8::42]".to_string(),
                "simplexabc.onion".to_string(),
            ],
            port: "5223".to_string(),
            key_hash: vec![0xaa, 0xbb, 0xcc],
        };

        let mut encoded = Vec::new();
        encode_protocol_server(&mut encoded, &server).unwrap();
        let decoded = decode_protocol_server(&mut Cursor::new(&encoded)).unwrap();

        assert_eq!(decoded, server);
    }

    #[test]
    fn protocol_server_rejects_invalid_transport_host_forms() {
        let invalid_server = RadrootsSimplexSmpProtocolServer {
            hosts: vec!["bad host".to_string()],
            port: "5223".to_string(),
            key_hash: vec![0xaa, 0xbb, 0xcc],
        };

        let mut encoded = Vec::new();
        assert_eq!(
            encode_protocol_server(&mut encoded, &invalid_server),
            Err(RadrootsSimplexSmpProtoError::InvalidHostList(
                "bad host".to_string(),
            ))
        );

        let mut invalid_bytes = Vec::new();
        push_short_string_list(&mut invalid_bytes, &["[invalid]".to_string()]).unwrap();
        push_short_string(&mut invalid_bytes, "5223").unwrap();
        push_short_bytes(&mut invalid_bytes, &[0xaa, 0xbb, 0xcc]).unwrap();

        assert_eq!(
            decode_protocol_server(&mut Cursor::new(&invalid_bytes)),
            Err(RadrootsSimplexSmpProtoError::InvalidHostList(
                "[invalid]".to_string(),
            ))
        );
    }

    #[test]
    fn top_level_unknown_error_tags_stay_opaque() {
        assert_eq!(
            decode_error(b"FUTURE"),
            Ok(RadrootsSimplexSmpError::Other(b"FUTURE".to_vec()))
        );
    }

    #[test]
    fn malformed_nested_proxy_error_fails_decode() {
        assert_eq!(
            decode_error(b"PROXY BROKER TRANSPORT HANDSHAKE UNKNOWN"),
            Err(RadrootsSimplexSmpProtoError::InvalidTag(
                "UNKNOWN".to_string(),
            ))
        );
    }

    #[test]
    fn malformed_blocked_reason_fails_decode() {
        assert_eq!(
            decode_error(b"BLOCKED reason=custom"),
            Err(RadrootsSimplexSmpProtoError::InvalidTag(
                "custom".to_string(),
            ))
        );
    }

    #[test]
    fn malformed_network_detail_fails_decode() {
        assert_eq!(
            decode_broker_error(b"NETWORK CONNECT"),
            Err(RadrootsSimplexSmpProtoError::UnexpectedEof)
        );
    }

    #[test]
    fn round_trips_proxy_forward_commands() {
        let pfwd = RadrootsSimplexSmpCommandTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(3)),
            entity_id: vec![0x90, 0x91],
            command: RadrootsSimplexSmpCommand::PFwd {
                relay_version: 16,
                public_key: vec![0x10, 0x11, 0x12],
                encrypted_transmission: vec![0xde, 0xad, 0xbe, 0xef],
            },
        };
        let rfwd = RadrootsSimplexSmpCommandTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(4)),
            entity_id: Vec::new(),
            command: RadrootsSimplexSmpCommand::RFwd(vec![0xca, 0xfe, 0xba, 0xbe]),
        };

        let pfwd_encoded = pfwd.encode().unwrap();
        let pfwd_decoded = RadrootsSimplexSmpCommandTransmission::decode(&pfwd_encoded).unwrap();
        assert_eq!(pfwd_decoded, pfwd);

        let rfwd_encoded = rfwd.encode().unwrap();
        let rfwd_decoded = RadrootsSimplexSmpCommandTransmission::decode(&rfwd_encoded).unwrap();
        assert_eq!(rfwd_decoded, rfwd);
    }

    #[test]
    fn round_trips_service_and_proxy_broker_messages() {
        let service = RadrootsSimplexSmpBrokerTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(5)),
            entity_id: vec![0x44],
            message: RadrootsSimplexSmpBrokerMessage::Sok(Some(vec![0x20, 0x21])),
        };
        let proxy = RadrootsSimplexSmpBrokerTransmission {
            authorization: Vec::new(),
            correlation_id: None,
            entity_id: Vec::new(),
            message: RadrootsSimplexSmpBrokerMessage::PKey {
                session_id: vec![0x31, 0x32],
                version_range: RadrootsSimplexSmpVersionRange::new(8, 16).unwrap(),
                cert_chain_public_key: RadrootsSimplexSmpCertChainPublicKey {
                    certificate_chain: vec![vec![0x41, 0x42], vec![0x43, 0x44, 0x45]],
                    signed_public_key: vec![0x51, 0x52, 0x53],
                },
            },
        };

        let service_encoded = service.encode().unwrap();
        let service_decoded =
            RadrootsSimplexSmpBrokerTransmission::decode(&service_encoded).unwrap();
        assert_eq!(service_decoded, service);

        let proxy_encoded = proxy.encode().unwrap();
        let proxy_decoded = RadrootsSimplexSmpBrokerTransmission::decode(&proxy_encoded).unwrap();
        assert_eq!(proxy_decoded, proxy);
    }

    #[test]
    fn round_trips_proxy_and_blocked_errors() {
        let proxy_error = RadrootsSimplexSmpBrokerTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(6)),
            entity_id: vec![0x77],
            message: RadrootsSimplexSmpBrokerMessage::Err(RadrootsSimplexSmpError::Proxy(
                RadrootsSimplexSmpProxyError::Broker(RadrootsSimplexSmpBrokerError::Transport(
                    RadrootsSimplexSmpTransportError::Handshake(
                        RadrootsSimplexSmpHandshakeError::Identity,
                    ),
                )),
            )),
        };
        let blocked_error = RadrootsSimplexSmpBrokerTransmission {
            authorization: Vec::new(),
            correlation_id: Some(correlation_id(7)),
            entity_id: vec![0x88],
            message: RadrootsSimplexSmpBrokerMessage::Err(RadrootsSimplexSmpError::Blocked(
                RadrootsSimplexSmpBlockingInfo {
                    reason: RadrootsSimplexSmpBlockingReason::Spam,
                },
            )),
        };

        let proxy_encoded = proxy_error.encode().unwrap();
        let proxy_decoded = RadrootsSimplexSmpBrokerTransmission::decode(&proxy_encoded).unwrap();
        assert_eq!(proxy_decoded, proxy_error);

        let blocked_encoded = blocked_error.encode().unwrap();
        let blocked_decoded =
            RadrootsSimplexSmpBrokerTransmission::decode(&blocked_encoded).unwrap();
        assert_eq!(blocked_decoded, blocked_error);
    }

    #[test]
    fn service_ok_downgrades_to_ok_before_service_certs() {
        let encoded = RadrootsSimplexSmpBrokerMessage::Sok(Some(vec![0x10]))
            .encode_for_version(RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION)
            .unwrap();

        assert_eq!(encoded, b"OK".to_vec());
    }

    #[test]
    fn blocked_error_downgrades_to_auth_before_blocked_entity_version() {
        let encoded = RadrootsSimplexSmpBrokerMessage::Err(RadrootsSimplexSmpError::Blocked(
            RadrootsSimplexSmpBlockingInfo {
                reason: RadrootsSimplexSmpBlockingReason::Content,
            },
        ))
        .encode_for_version(RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION)
        .unwrap();

        assert_eq!(encoded, b"ERR AUTH".to_vec());
    }

    #[test]
    fn decodes_optional_network_detail_and_preserves_encode_behavior() {
        let detailed = decode_broker_error(b"NETWORK CONNECT \x03dns").unwrap();
        assert_eq!(
            detailed,
            RadrootsSimplexSmpBrokerError::Network(RadrootsSimplexSmpNetworkError::Connect(
                "dns".to_string(),
            ))
        );

        let encoded =
            encode_network_error(&RadrootsSimplexSmpNetworkError::Connect("dns".to_string()));
        assert_eq!(encoded, b"CONNECT \x03dns".to_vec());
        assert_eq!(
            encode_broker_error(&RadrootsSimplexSmpBrokerError::Network(
                RadrootsSimplexSmpNetworkError::Connect("dns".to_string()),
            )),
            b"NETWORK".to_vec()
        );
    }
}
