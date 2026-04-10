use crate::error::RadrootsSimplexChatProtoError;
use crate::version::RadrootsSimplexChatVersionRange;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

pub type RadrootsSimplexChatObject = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsSimplexChatBase64Url(String);

impl RadrootsSimplexChatBase64Url {
    pub fn new(value: impl Into<String>) -> Result<Self, RadrootsSimplexChatProtoError> {
        let value = value.into();
        URL_SAFE_NO_PAD.decode(value.as_bytes()).map_err(|_| {
            RadrootsSimplexChatProtoError::InvalidBase64Url {
                field: "base64url",
                value: value.clone(),
            }
        })?;
        Ok(Self(value))
    }

    pub fn parse_field(
        value: String,
        field: &'static str,
    ) -> Result<Self, RadrootsSimplexChatProtoError> {
        URL_SAFE_NO_PAD.decode(value.as_bytes()).map_err(|_| {
            RadrootsSimplexChatProtoError::InvalidBase64Url {
                field,
                value: value.clone(),
            }
        })?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for RadrootsSimplexChatBase64Url {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RadrootsSimplexChatBase64Url {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = <String as Deserialize>::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexChatPeerType {
    Human,
    Bot,
    Unknown(String),
}

impl Serialize for RadrootsSimplexChatPeerType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::Human => "human",
            Self::Bot => "bot",
            Self::Unknown(value) => value,
        })
    }
}

impl<'de> Deserialize<'de> for RadrootsSimplexChatPeerType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = <String as Deserialize>::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "human" => Self::Human,
            "bot" => Self::Bot,
            _ => Self::Unknown(value),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadrootsSimplexChatProfile {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "fullName")]
    pub full_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(rename = "shortDescr", skip_serializing_if = "Option::is_none")]
    pub short_descr: Option<String>,
    #[serde(rename = "contactLink", skip_serializing_if = "Option::is_none")]
    pub contact_link: Option<String>,
    #[serde(rename = "peerType", skip_serializing_if = "Option::is_none")]
    pub peer_type: Option<RadrootsSimplexChatPeerType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferences: Option<Value>,
    #[serde(flatten, default)]
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadrootsSimplexChatMessageRef {
    #[serde(rename = "msgId", skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<RadrootsSimplexChatBase64Url>,
    #[serde(rename = "sentAt")]
    pub sent_at: String,
    pub sent: bool,
    #[serde(rename = "memberId", skip_serializing_if = "Option::is_none")]
    pub member_id: Option<RadrootsSimplexChatBase64Url>,
    #[serde(flatten, default)]
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadrootsSimplexChatMention {
    #[serde(rename = "memberId")]
    pub member_id: RadrootsSimplexChatBase64Url,
    #[serde(flatten, default)]
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RadrootsSimplexChatLinkContent {
    Page {
        extra: RadrootsSimplexChatObject,
    },
    Image {
        extra: RadrootsSimplexChatObject,
    },
    Video {
        duration: Option<i64>,
        extra: RadrootsSimplexChatObject,
    },
    Unknown {
        content_type: String,
        raw: RadrootsSimplexChatObject,
    },
}

impl Serialize for RadrootsSimplexChatLinkContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = RadrootsSimplexChatObject::new();
        match self {
            Self::Page { extra } => {
                object.insert(String::from("type"), Value::String(String::from("page")));
                object.extend(extra.clone());
            }
            Self::Image { extra } => {
                object.insert(String::from("type"), Value::String(String::from("image")));
                object.extend(extra.clone());
            }
            Self::Video { duration, extra } => {
                object.insert(String::from("type"), Value::String(String::from("video")));
                if let Some(duration) = duration {
                    object.insert(String::from("duration"), Value::from(*duration));
                }
                object.extend(extra.clone());
            }
            Self::Unknown { raw, .. } => {
                object = raw.clone();
            }
        }
        object.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RadrootsSimplexChatLinkContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut raw = <RadrootsSimplexChatObject as Deserialize>::deserialize(deserializer)?;
        let content_type = match raw.remove("type") {
            Some(Value::String(value)) => value,
            Some(_) => return Err(D::Error::custom("invalid link content type")),
            None => return Err(D::Error::custom("missing link content type")),
        };

        Ok(match content_type.as_str() {
            "page" => Self::Page { extra: raw },
            "image" => Self::Image { extra: raw },
            "video" => {
                let duration = match raw.remove("duration") {
                    Some(Value::Number(value)) => value.as_i64(),
                    Some(_) => return Err(D::Error::custom("invalid duration")),
                    None => None,
                };
                Self::Video {
                    duration,
                    extra: raw,
                }
            }
            _ => {
                raw.insert(String::from("type"), Value::String(content_type.clone()));
                Self::Unknown { content_type, raw }
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadrootsSimplexChatLinkPreview {
    pub uri: String,
    pub title: String,
    pub description: String,
    pub image: RadrootsSimplexChatBase64Url,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<RadrootsSimplexChatLinkContent>,
    #[serde(flatten, default)]
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RadrootsSimplexChatContent {
    Text {
        text: String,
        extra: RadrootsSimplexChatObject,
    },
    Link {
        text: String,
        preview: RadrootsSimplexChatLinkPreview,
        extra: RadrootsSimplexChatObject,
    },
    Image {
        text: String,
        image: RadrootsSimplexChatBase64Url,
        extra: RadrootsSimplexChatObject,
    },
    Video {
        text: String,
        image: RadrootsSimplexChatBase64Url,
        duration: i64,
        extra: RadrootsSimplexChatObject,
    },
    Voice {
        text: String,
        duration: i64,
        extra: RadrootsSimplexChatObject,
    },
    File {
        text: String,
        extra: RadrootsSimplexChatObject,
    },
    Unknown {
        content_type: String,
        text: Option<String>,
        raw: RadrootsSimplexChatObject,
    },
}

impl Serialize for RadrootsSimplexChatContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = RadrootsSimplexChatObject::new();
        match self {
            Self::Text { text, extra } => {
                object.insert(String::from("type"), Value::String(String::from("text")));
                object.insert(String::from("text"), Value::String(text.clone()));
                object.extend(extra.clone());
            }
            Self::Link {
                text,
                preview,
                extra,
            } => {
                object.insert(String::from("type"), Value::String(String::from("link")));
                object.insert(String::from("text"), Value::String(text.clone()));
                object.insert(
                    String::from("preview"),
                    serde_json::to_value(preview).map_err(serde::ser::Error::custom)?,
                );
                object.extend(extra.clone());
            }
            Self::Image { text, image, extra } => {
                object.insert(String::from("type"), Value::String(String::from("image")));
                object.insert(String::from("text"), Value::String(text.clone()));
                object.insert(
                    String::from("image"),
                    Value::String(image.as_str().to_string()),
                );
                object.extend(extra.clone());
            }
            Self::Video {
                text,
                image,
                duration,
                extra,
            } => {
                object.insert(String::from("type"), Value::String(String::from("video")));
                object.insert(String::from("text"), Value::String(text.clone()));
                object.insert(
                    String::from("image"),
                    Value::String(image.as_str().to_string()),
                );
                object.insert(String::from("duration"), Value::from(*duration));
                object.extend(extra.clone());
            }
            Self::Voice {
                text,
                duration,
                extra,
            } => {
                object.insert(String::from("type"), Value::String(String::from("voice")));
                object.insert(String::from("text"), Value::String(text.clone()));
                object.insert(String::from("duration"), Value::from(*duration));
                object.extend(extra.clone());
            }
            Self::File { text, extra } => {
                object.insert(String::from("type"), Value::String(String::from("file")));
                object.insert(String::from("text"), Value::String(text.clone()));
                object.extend(extra.clone());
            }
            Self::Unknown { raw, .. } => {
                object = raw.clone();
            }
        }

        object.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RadrootsSimplexChatContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut raw = <RadrootsSimplexChatObject as Deserialize>::deserialize(deserializer)?;
        let content_type = match raw.remove("type") {
            Some(Value::String(value)) => value,
            Some(_) => return Err(D::Error::custom("invalid content type")),
            None => return Err(D::Error::custom("missing content type")),
        };

        Ok(match content_type.as_str() {
            "text" => Self::Text {
                text: expect_string::<D>(&mut raw, "text")?,
                extra: raw,
            },
            "link" => Self::Link {
                text: expect_string::<D>(&mut raw, "text")?,
                preview: serde_json::from_value(
                    expect_value(&mut raw, "preview").map_err(D::Error::custom)?,
                )
                .map_err(D::Error::custom)?,
                extra: raw,
            },
            "image" => Self::Image {
                text: expect_string::<D>(&mut raw, "text")?,
                image: expect_base64url::<D>(&mut raw, "image")?,
                extra: raw,
            },
            "video" => Self::Video {
                text: expect_string::<D>(&mut raw, "text")?,
                image: expect_base64url::<D>(&mut raw, "image")?,
                duration: expect_i64::<D>(&mut raw, "duration")?,
                extra: raw,
            },
            "voice" => Self::Voice {
                text: expect_string::<D>(&mut raw, "text")?,
                duration: expect_i64::<D>(&mut raw, "duration")?,
                extra: raw,
            },
            "file" => Self::File {
                text: expect_string::<D>(&mut raw, "text")?,
                extra: raw,
            },
            _ => {
                let text = match raw.get("text") {
                    Some(Value::String(value)) => Some(value.clone()),
                    _ => None,
                };
                raw.insert(String::from("type"), Value::String(content_type.clone()));
                Self::Unknown {
                    content_type,
                    text,
                    raw,
                }
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadrootsSimplexChatFileDescription {
    #[serde(rename = "fileDescrText")]
    pub file_descr_text: String,
    #[serde(rename = "fileDescrPartNo")]
    pub file_descr_part_no: i64,
    #[serde(rename = "fileDescrComplete")]
    pub file_descr_complete: bool,
    #[serde(flatten, default)]
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadrootsSimplexChatFileInvitation {
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "fileSize")]
    pub file_size: u32,
    #[serde(rename = "fileDigest", skip_serializing_if = "Option::is_none")]
    pub file_digest: Option<RadrootsSimplexChatBase64Url>,
    #[serde(rename = "fileConnReq", skip_serializing_if = "Option::is_none")]
    pub file_conn_req: Option<String>,
    #[serde(rename = "fileDescr", skip_serializing_if = "Option::is_none")]
    pub file_descr: Option<RadrootsSimplexChatFileDescription>,
    #[serde(flatten, default)]
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadrootsSimplexChatQuotedMessage {
    #[serde(rename = "msgRef")]
    pub msg_ref: RadrootsSimplexChatMessageRef,
    pub content: RadrootsSimplexChatContent,
    #[serde(flatten, default)]
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RadrootsSimplexChatForwardMarker {
    Flag,
    Object(RadrootsSimplexChatObject),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RadrootsSimplexChatScope {
    Member {
        member_id: RadrootsSimplexChatBase64Url,
        extra: RadrootsSimplexChatObject,
    },
    Unknown(Value),
}

impl Serialize for RadrootsSimplexChatScope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Member { member_id, extra } => {
                let mut data = RadrootsSimplexChatObject::new();
                data.insert(
                    String::from("memberId"),
                    Value::String(member_id.as_str().to_string()),
                );
                data.extend(extra.clone());

                let mut object = RadrootsSimplexChatObject::new();
                object.insert(String::from("type"), Value::String(String::from("member")));
                object.insert(String::from("data"), Value::Object(data));
                object.serialize(serializer)
            }
            Self::Unknown(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for RadrootsSimplexChatScope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let original = value.clone();
        let Value::Object(mut object) = value else {
            return Ok(Self::Unknown(original));
        };

        let Some(Value::String(scope_type)) = object.remove("type") else {
            return Ok(Self::Unknown(original));
        };

        if scope_type != "member" {
            return Ok(Self::Unknown(original));
        }

        let Some(Value::Object(mut data)) = object.remove("data") else {
            return Ok(Self::Unknown(original));
        };

        let member_id = expect_base64url::<D>(&mut data, "memberId")?;
        Ok(Self::Member {
            member_id,
            extra: data,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RadrootsSimplexChatContainerKind {
    Simple,
    Quote(RadrootsSimplexChatQuotedMessage),
    Comment(RadrootsSimplexChatMessageRef),
    Forward(RadrootsSimplexChatForwardMarker),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatMessageContainer {
    pub kind: RadrootsSimplexChatContainerKind,
    pub content: RadrootsSimplexChatContent,
    pub mentions: BTreeMap<String, RadrootsSimplexChatMention>,
    pub file: Option<RadrootsSimplexChatFileInvitation>,
    pub ttl: Option<i64>,
    pub live: Option<bool>,
    pub scope: Option<RadrootsSimplexChatScope>,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatMessageContentReference {
    pub msg_id: RadrootsSimplexChatBase64Url,
    pub content: RadrootsSimplexChatContent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatNoParamsEvent {
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatContactEvent {
    pub profile: RadrootsSimplexChatProfile,
    pub contact_req_id: Option<RadrootsSimplexChatBase64Url>,
    pub welcome_msg_id: Option<RadrootsSimplexChatBase64Url>,
    pub request_message: Option<RadrootsSimplexChatMessageContentReference>,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatInfoEvent {
    pub profile: RadrootsSimplexChatProfile,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatProbeEvent {
    pub probe: RadrootsSimplexChatBase64Url,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatProbeCheckEvent {
    pub probe_hash: RadrootsSimplexChatBase64Url,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatMsgNewEvent {
    pub container: RadrootsSimplexChatMessageContainer,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatFileDescriptionEvent {
    pub msg_id: RadrootsSimplexChatBase64Url,
    pub file_descr: RadrootsSimplexChatFileDescription,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatMsgUpdateEvent {
    pub msg_id: RadrootsSimplexChatBase64Url,
    pub content: RadrootsSimplexChatContent,
    pub mentions: BTreeMap<String, RadrootsSimplexChatMention>,
    pub ttl: Option<i64>,
    pub live: Option<bool>,
    pub scope: Option<RadrootsSimplexChatScope>,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatDeleteEvent {
    pub msg_id: RadrootsSimplexChatBase64Url,
    pub member_id: Option<RadrootsSimplexChatBase64Url>,
    pub scope: Option<RadrootsSimplexChatScope>,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatFileAcceptEvent {
    pub file_name: String,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatFileAcceptInvitationEvent {
    pub msg_id: RadrootsSimplexChatBase64Url,
    pub file_conn_req: Option<String>,
    pub file_name: String,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatFileCancelEvent {
    pub msg_id: RadrootsSimplexChatBase64Url,
    pub extra: RadrootsSimplexChatObject,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RadrootsSimplexChatEvent {
    Contact(RadrootsSimplexChatContactEvent),
    Info(RadrootsSimplexChatInfoEvent),
    InfoProbe(RadrootsSimplexChatProbeEvent),
    InfoProbeCheck(RadrootsSimplexChatProbeCheckEvent),
    InfoProbeOk(RadrootsSimplexChatProbeEvent),
    MsgNew(RadrootsSimplexChatMsgNewEvent),
    MsgFileDescr(RadrootsSimplexChatFileDescriptionEvent),
    MsgUpdate(RadrootsSimplexChatMsgUpdateEvent),
    MsgDel(RadrootsSimplexChatDeleteEvent),
    FileAcpt(RadrootsSimplexChatFileAcceptEvent),
    FileAcptInv(RadrootsSimplexChatFileAcceptInvitationEvent),
    FileCancel(RadrootsSimplexChatFileCancelEvent),
    DirectDel(RadrootsSimplexChatNoParamsEvent),
    Ok(RadrootsSimplexChatNoParamsEvent),
    Unknown {
        event: String,
        params: RadrootsSimplexChatObject,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadrootsSimplexChatMessage {
    pub version: Option<RadrootsSimplexChatVersionRange>,
    pub msg_id: Option<RadrootsSimplexChatBase64Url>,
    pub event: RadrootsSimplexChatEvent,
}

pub(crate) fn expect_value(
    map: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<Value, RadrootsSimplexChatProtoError> {
    map.remove(field)
        .ok_or(RadrootsSimplexChatProtoError::MissingField(field))
}

fn expect_string<'de, D>(
    map: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    match expect_value(map, field).map_err(D::Error::custom)? {
        Value::String(value) => Ok(value),
        _ => Err(D::Error::custom(
            RadrootsSimplexChatProtoError::InvalidField(field),
        )),
    }
}

fn expect_i64<'de, D>(
    map: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    match expect_value(map, field).map_err(D::Error::custom)? {
        Value::Number(value) => value
            .as_i64()
            .ok_or_else(|| D::Error::custom(RadrootsSimplexChatProtoError::InvalidField(field))),
        _ => Err(D::Error::custom(
            RadrootsSimplexChatProtoError::InvalidField(field),
        )),
    }
}

fn expect_base64url<'de, D>(
    map: &mut RadrootsSimplexChatObject,
    field: &'static str,
) -> Result<RadrootsSimplexChatBase64Url, D::Error>
where
    D: Deserializer<'de>,
{
    expect_string::<D>(map, field).and_then(|value| {
        RadrootsSimplexChatBase64Url::parse_field(value, field).map_err(D::Error::custom)
    })
}
