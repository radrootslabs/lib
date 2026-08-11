use core::fmt;
use std::error::Error;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser};

use crate::HostError;

pub const ADMIN_CONTRACT_VERSION: u32 = 1;
pub const ADMIN_OPERATION_ID_MAX_UTF8_BYTES: usize = 128;
pub const ADMIN_CORRELATION_ID_MAX_UTF8_BYTES: usize = 128;
pub const ADMIN_ERROR_CODE_MAX_UTF8_BYTES: usize = 64;
pub const ADMIN_ERROR_MESSAGE_MAX_UTF8_BYTES: usize = 256;

const UNSUPPORTED_CONTRACT_VERSION_CODE: &str = "unsupported_contract_version";
const UNSUPPORTED_CONTRACT_VERSION_MESSAGE: &str = "admin contract version is unsupported";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminIdentifierField {
    OperationId,
    CorrelationId,
}

impl AdminIdentifierField {
    const fn maximum_utf8_bytes(self) -> usize {
        match self {
            Self::OperationId => ADMIN_OPERATION_ID_MAX_UTF8_BYTES,
            Self::CorrelationId => ADMIN_CORRELATION_ID_MAX_UTF8_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminIdentifierError {
    Empty { field: AdminIdentifierField },
    TooLong { field: AdminIdentifierField },
}

impl fmt::Display for AdminIdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin identifier is outside its required UTF-8 byte bounds")
    }
}

impl Error for AdminIdentifierError {}

macro_rules! admin_identifier {
    ($name:ident, $field:expr) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, AdminIdentifierError> {
                let value = value.into();
                let field = $field;
                if value.is_empty() {
                    return Err(AdminIdentifierError::Empty { field });
                }
                if value.len() > field.maximum_utf8_bytes() {
                    return Err(AdminIdentifierError::TooLong { field });
                }
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(de::Error::custom)
            }
        }
    };
}

admin_identifier!(AdminOperationId, AdminIdentifierField::OperationId);
admin_identifier!(AdminCorrelationId, AdminIdentifierField::CorrelationId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminErrorCodeError {
    Empty,
    TooLong,
    InvalidCharacter,
}

impl fmt::Display for AdminErrorCodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin error code must be a bounded lowercase snake identifier")
    }
}

impl Error for AdminErrorCodeError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdminErrorCode(String);

impl AdminErrorCode {
    pub fn new(value: impl Into<String>) -> Result<Self, AdminErrorCodeError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AdminErrorCodeError::Empty);
        }
        if value.len() > ADMIN_ERROR_CODE_MAX_UTF8_BYTES {
            return Err(AdminErrorCodeError::TooLong);
        }
        let mut bytes = value.bytes();
        if !matches!(bytes.next(), Some(b'a'..=b'z'))
            || !bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_'))
        {
            return Err(AdminErrorCodeError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    fn known(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AdminErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for AdminErrorCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AdminErrorCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminErrorMessageError {
    Empty,
    TooLong,
    ControlCharacter,
}

impl fmt::Display for AdminErrorMessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin error message is not a bounded safe message")
    }
}

impl Error for AdminErrorMessageError {}

pub enum AdminPayloadError {
    NullForbidden,
    Encoding(serde_json::Error),
}

impl fmt::Debug for AdminPayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NullForbidden => "AdminPayloadError::NullForbidden",
            Self::Encoding(_) => "AdminPayloadError::Encoding(<redacted>)",
        })
    }
}

impl fmt::Display for AdminPayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NullForbidden => "admin payloads may not contain JSON null",
            Self::Encoding(_) => "admin payload could not be represented as JSON",
        })
    }
}

impl Error for AdminPayloadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NullForbidden => None,
            Self::Encoding(error) => Some(error),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct NonNullPayload<T>(T);

impl<T> NonNullPayload<T>
where
    T: Serialize,
{
    fn new(value: T) -> Result<Self, AdminPayloadError> {
        checked_json_value(&value)?;
        Ok(Self(value))
    }
}

impl<T> Serialize for NonNullPayload<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        checked_json_value(&self.0)
            .map_err(ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for NonNullPayload<T>
where
    T: Deserialize<'de> + Serialize,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = T::deserialize(NoNullDeserializer(deserializer))?;
        Self::new(value).map_err(de::Error::custom)
    }
}

struct NoNullDeserializer<D>(D);

impl<'de, D> Deserializer<'de> for NoNullDeserializer<D>
where
    D: Deserializer<'de>,
{
    type Error = D::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_any(NoNullVisitor(visitor))
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_bool(NoNullVisitor(visitor))
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_i8(NoNullVisitor(visitor))
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_i16(NoNullVisitor(visitor))
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_i32(NoNullVisitor(visitor))
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_i64(NoNullVisitor(visitor))
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_i128(NoNullVisitor(visitor))
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_u8(NoNullVisitor(visitor))
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_u16(NoNullVisitor(visitor))
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_u32(NoNullVisitor(visitor))
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_u64(NoNullVisitor(visitor))
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_u128(NoNullVisitor(visitor))
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_f32(NoNullVisitor(visitor))
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_f64(NoNullVisitor(visitor))
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_char(NoNullVisitor(visitor))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_str(NoNullVisitor(visitor))
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_string(NoNullVisitor(visitor))
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_bytes(NoNullVisitor(visitor))
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_byte_buf(NoNullVisitor(visitor))
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_option(NoNullVisitor(visitor))
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_unit(NoNullVisitor(visitor))
    }

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_unit_struct(name, NoNullVisitor(visitor))
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_newtype_struct(name, NoNullVisitor(visitor))
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_seq(NoNullVisitor(visitor))
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_tuple(len, NoNullVisitor(visitor))
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_tuple_struct(name, len, NoNullVisitor(visitor))
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_map(NoNullVisitor(visitor))
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_struct(name, fields, NoNullVisitor(visitor))
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0
            .deserialize_enum(name, variants, NoNullVisitor(visitor))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_identifier(NoNullVisitor(visitor))
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.deserialize_ignored_any(NoNullVisitor(visitor))
    }

    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

struct NoNullVisitor<V>(V);

impl<'de, V> de::Visitor<'de> for NoNullVisitor<V>
where
    V: de::Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.expecting(formatter)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_bool(value)
    }

    fn visit_i8<E>(self, value: i8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i8(value)
    }

    fn visit_i16<E>(self, value: i16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i16(value)
    }

    fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i32(value)
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i64(value)
    }

    fn visit_i128<E>(self, value: i128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i128(value)
    }

    fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u8(value)
    }

    fn visit_u16<E>(self, value: u16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u16(value)
    }

    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u32(value)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u64(value)
    }

    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u128(value)
    }

    fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_f32(value)
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_f64(value)
    }

    fn visit_char<E>(self, value: char) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_char(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_str(value)
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_borrowed_str(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_string(value)
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_bytes(value)
    }

    fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_borrowed_bytes(value)
    }

    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_byte_buf(value)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom("admin payloads may not contain JSON null"))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0.visit_some(NoNullDeserializer(deserializer))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom("admin payloads may not contain JSON null"))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0
            .visit_newtype_struct(NoNullDeserializer(deserializer))
    }

    fn visit_seq<A>(self, sequence: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        self.0.visit_seq(NoNullSeqAccess(sequence))
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        self.0.visit_map(NoNullMapAccess(map))
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: de::EnumAccess<'de>,
    {
        self.0.visit_enum(NoNullEnumAccess(data))
    }
}

struct NoNullSeed<S>(S);

impl<'de, S> de::DeserializeSeed<'de> for NoNullSeed<S>
where
    S: de::DeserializeSeed<'de>,
{
    type Value = S::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0.deserialize(NoNullDeserializer(deserializer))
    }
}

struct NoNullSeqAccess<A>(A);

impl<'de, A> de::SeqAccess<'de> for NoNullSeqAccess<A>
where
    A: de::SeqAccess<'de>,
{
    type Error = A::Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        self.0.next_element_seed(NoNullSeed(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}

struct NoNullMapAccess<A>(A);

impl<'de, A> de::MapAccess<'de> for NoNullMapAccess<A>
where
    A: de::MapAccess<'de>,
{
    type Error = A::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        self.0.next_key_seed(seed)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        self.0.next_value_seed(NoNullSeed(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}

struct NoNullEnumAccess<A>(A);

impl<'de, A> de::EnumAccess<'de> for NoNullEnumAccess<A>
where
    A: de::EnumAccess<'de>,
{
    type Error = A::Error;
    type Variant = NoNullVariantAccess<A::Variant>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let (value, variant) = self.0.variant_seed(seed)?;
        Ok((value, NoNullVariantAccess(variant)))
    }
}

struct NoNullVariantAccess<A>(A);

impl<'de, A> de::VariantAccess<'de> for NoNullVariantAccess<A>
where
    A: de::VariantAccess<'de>,
{
    type Error = A::Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        self.0.unit_variant()
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        self.0.newtype_variant_seed(NoNullSeed(seed))
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.tuple_variant(len, NoNullVisitor(visitor))
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.0.struct_variant(fields, NoNullVisitor(visitor))
    }
}

fn checked_json_value(value: &impl Serialize) -> Result<serde_json::Value, AdminPayloadError> {
    let value = serde_json::to_value(value).map_err(AdminPayloadError::Encoding)?;
    if contains_null(&value) {
        Err(AdminPayloadError::NullForbidden)
    } else {
        Ok(value)
    }
}

fn contains_null(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::Array(values) => values.iter().any(contains_null),
        serde_json::Value::Object(values) => values.values().any(contains_null),
        serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminErrorMessage(String);

impl AdminErrorMessage {
    pub fn new(value: impl Into<String>) -> Result<Self, AdminErrorMessageError> {
        let value = value.into();
        if value.is_empty() {
            return Err(AdminErrorMessageError::Empty);
        }
        if value.len() > ADMIN_ERROR_MESSAGE_MAX_UTF8_BYTES {
            return Err(AdminErrorMessageError::TooLong);
        }
        if value.chars().any(char::is_control) {
            return Err(AdminErrorMessageError::ControlCharacter);
        }
        Ok(Self(value))
    }

    fn known(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AdminErrorMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for AdminErrorMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AdminErrorMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminError {
    code: AdminErrorCode,
    message: AdminErrorMessage,
}

impl AdminError {
    #[must_use]
    pub const fn new(code: AdminErrorCode, message: AdminErrorMessage) -> Self {
        Self { code, message }
    }

    #[must_use]
    pub fn from_host_error(error: &HostError) -> Self {
        let safe = error.safe_error();
        Self {
            code: AdminErrorCode::known(safe.code_str()),
            message: AdminErrorMessage::known(safe.message()),
        }
    }

    #[must_use]
    pub fn code(&self) -> &AdminErrorCode {
        &self.code
    }

    #[must_use]
    pub fn message(&self) -> &AdminErrorMessage {
        &self.message
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SuccessLiteral(#[serde(with = "success_literal")] ());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ContractVersionLiteral(#[serde(with = "contract_version_literal")] ());

mod contract_version_literal {
    use serde::{Deserialize, Deserializer, Serializer, de};

    use super::ADMIN_CONTRACT_VERSION;

    pub fn serialize<S>(_: &(), serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(ADMIN_CONTRACT_VERSION)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        let received = u32::deserialize(deserializer)?;
        if received == ADMIN_CONTRACT_VERSION {
            Ok(())
        } else {
            Err(de::Error::custom(
                "admin response contract version must be 1",
            ))
        }
    }
}

mod success_literal {
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S>(_: &(), serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bool(true)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        if bool::deserialize(deserializer)? {
            Ok(())
        } else {
            Err(de::Error::custom("admin success envelope requires ok=true"))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FailureLiteral(#[serde(with = "failure_literal")] ());

mod failure_literal {
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S>(_: &(), serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bool(false)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        if bool::deserialize(deserializer)? {
            Err(de::Error::custom(
                "admin failure envelope requires ok=false",
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum OptionalCorrelationId {
    #[default]
    Absent,
    Present(AdminCorrelationId),
}

impl OptionalCorrelationId {
    fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    fn as_option(&self) -> Option<&AdminCorrelationId> {
        match self {
            Self::Absent => None,
            Self::Present(value) => Some(value),
        }
    }
}

impl Serialize for OptionalCorrelationId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Absent => serializer.serialize_none(),
            Self::Present(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for OptionalCorrelationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        AdminCorrelationId::deserialize(deserializer).map(Self::Present)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "T: Serialize",
        deserialize = "T: Deserialize<'de> + Serialize"
    )
)]
pub struct AdminMutationRequest<T> {
    contract_version: u32,
    operation_id: AdminOperationId,
    #[serde(default, skip_serializing_if = "OptionalCorrelationId::is_absent")]
    correlation_id: OptionalCorrelationId,
    request: NonNullPayload<T>,
}

impl<T> AdminMutationRequest<T>
where
    T: Serialize,
{
    pub fn new(
        operation_id: AdminOperationId,
        correlation_id: Option<AdminCorrelationId>,
        request: T,
    ) -> Result<Self, AdminPayloadError> {
        Ok(Self {
            contract_version: ADMIN_CONTRACT_VERSION,
            operation_id,
            correlation_id: correlation_id
                .map(OptionalCorrelationId::Present)
                .unwrap_or_default(),
            request: NonNullPayload::new(request)?,
        })
    }

    #[must_use]
    pub const fn contract_version(&self) -> u32 {
        self.contract_version
    }

    pub fn validate_contract_version(&self) -> Result<(), AdminContractVersionError> {
        if self.contract_version == ADMIN_CONTRACT_VERSION {
            Ok(())
        } else {
            Err(AdminContractVersionError {
                received: self.contract_version,
            })
        }
    }

    #[must_use]
    pub const fn operation_id(&self) -> &AdminOperationId {
        &self.operation_id
    }

    #[must_use]
    pub fn correlation_id(&self) -> Option<&AdminCorrelationId> {
        self.correlation_id.as_option()
    }

    #[must_use]
    pub const fn request(&self) -> &T {
        &self.request.0
    }

    #[must_use]
    pub fn into_request(self) -> T {
        self.request.0
    }
}

impl<T> fmt::Debug for AdminMutationRequest<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminMutationRequest")
            .field("contract_version", &self.contract_version)
            .field("operation_id", &self.operation_id)
            .field("correlation_id", &self.correlation_id.as_option())
            .field("request", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "T: Serialize",
        deserialize = "T: Deserialize<'de> + Serialize"
    )
)]
pub struct AdminSuccessResponse<T> {
    contract_version: ContractVersionLiteral,
    ok: SuccessLiteral,
    correlation_id: AdminCorrelationId,
    result: NonNullPayload<T>,
}

impl<T> AdminSuccessResponse<T>
where
    T: Serialize,
{
    pub fn new(correlation_id: AdminCorrelationId, result: T) -> Result<Self, AdminPayloadError> {
        Ok(Self {
            contract_version: ContractVersionLiteral(()),
            ok: SuccessLiteral(()),
            correlation_id,
            result: NonNullPayload::new(result)?,
        })
    }

    #[must_use]
    pub const fn correlation_id(&self) -> &AdminCorrelationId {
        &self.correlation_id
    }

    #[must_use]
    pub const fn result(&self) -> &T {
        &self.result.0
    }

    #[must_use]
    pub fn into_result(self) -> T {
        self.result.0
    }
}

impl<T> fmt::Debug for AdminSuccessResponse<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminSuccessResponse")
            .field("contract_version", &ADMIN_CONTRACT_VERSION)
            .field("ok", &true)
            .field("correlation_id", &self.correlation_id)
            .field("result", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminFailureResponse {
    contract_version: ContractVersionLiteral,
    ok: FailureLiteral,
    correlation_id: AdminCorrelationId,
    error: AdminError,
}

impl AdminFailureResponse {
    #[must_use]
    pub const fn new(correlation_id: AdminCorrelationId, error: AdminError) -> Self {
        Self {
            contract_version: ContractVersionLiteral(()),
            ok: FailureLiteral(()),
            correlation_id,
            error,
        }
    }

    #[must_use]
    pub fn unsupported_contract_version(correlation_id: AdminCorrelationId) -> Self {
        Self::new(
            correlation_id,
            AdminError::new(
                AdminErrorCode::known(UNSUPPORTED_CONTRACT_VERSION_CODE),
                AdminErrorMessage::known(UNSUPPORTED_CONTRACT_VERSION_MESSAGE),
            ),
        )
    }

    #[must_use]
    pub const fn correlation_id(&self) -> &AdminCorrelationId {
        &self.correlation_id
    }

    #[must_use]
    pub const fn error(&self) -> &AdminError {
        &self.error
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdminContractVersionError {
    received: u32,
}

impl AdminContractVersionError {
    #[must_use]
    pub const fn received(self) -> u32 {
        self.received
    }

    #[must_use]
    pub fn response(self, correlation_id: AdminCorrelationId) -> AdminFailureResponse {
        AdminFailureResponse::unsupported_contract_version(correlation_id)
    }
}

impl fmt::Display for AdminContractVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin contract version is unsupported")
    }
}

impl Error for AdminContractVersionError {}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use serde::{Deserialize, Serialize};

    use crate::HostErrorKind;

    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ExampleRequest {
        value: u32,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ExampleResult {
        state: String,
    }

    fn operation_id() -> AdminOperationId {
        AdminOperationId::new("stable-operation").unwrap()
    }

    fn correlation_id() -> AdminCorrelationId {
        AdminCorrelationId::new("safe-correlation").unwrap()
    }

    #[test]
    fn request_and_response_snapshots_are_exact() {
        let request = AdminMutationRequest::new(
            operation_id(),
            Some(correlation_id()),
            ExampleRequest { value: 7 },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            r#"{"contract_version":1,"operation_id":"stable-operation","correlation_id":"safe-correlation","request":{"value":7}}"#
        );

        let without_correlation =
            AdminMutationRequest::new(operation_id(), None, ExampleRequest { value: 7 }).unwrap();
        assert_eq!(
            serde_json::to_string(&without_correlation).unwrap(),
            r#"{"contract_version":1,"operation_id":"stable-operation","request":{"value":7}}"#
        );

        let success = AdminSuccessResponse::new(
            correlation_id(),
            ExampleResult {
                state: "committed".to_owned(),
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&success).unwrap(),
            r#"{"contract_version":1,"ok":true,"correlation_id":"safe-correlation","result":{"state":"committed"}}"#
        );

        let failure = AdminFailureResponse::new(
            correlation_id(),
            AdminError::new(
                AdminErrorCode::new("operation_id_conflict").unwrap(),
                AdminErrorMessage::new("operation identity conflicts with prior input").unwrap(),
            ),
        );
        assert_eq!(
            serde_json::to_string(&failure).unwrap(),
            r#"{"contract_version":1,"ok":false,"correlation_id":"safe-correlation","error":{"code":"operation_id_conflict","message":"operation identity conflicts with prior input"}}"#
        );
    }

    #[test]
    fn unknown_version_maps_to_the_stable_failure_response() {
        let request: AdminMutationRequest<ExampleRequest> = serde_json::from_str(
            r#"{"contract_version":2,"operation_id":"stable-operation","request":{"value":7}}"#,
        )
        .unwrap();
        let mismatch = request.validate_contract_version().unwrap_err();
        assert_eq!(mismatch.received(), 2);
        let response = mismatch.response(correlation_id());
        assert_eq!(
            serde_json::to_string(&response).unwrap(),
            r#"{"contract_version":1,"ok":false,"correlation_id":"safe-correlation","error":{"code":"unsupported_contract_version","message":"admin contract version is unsupported"}}"#
        );
    }

    #[test]
    fn invalid_ids_codes_messages_and_literals_fail_closed() {
        assert_eq!(
            AdminOperationId::new("").unwrap_err(),
            AdminIdentifierError::Empty {
                field: AdminIdentifierField::OperationId
            }
        );
        assert!(AdminOperationId::new("x".repeat(ADMIN_OPERATION_ID_MAX_UTF8_BYTES)).is_ok());
        assert!(AdminOperationId::new("x".repeat(ADMIN_OPERATION_ID_MAX_UTF8_BYTES + 1)).is_err());
        assert!(AdminCorrelationId::new("é".repeat(64)).is_ok());
        assert!(AdminCorrelationId::new(format!("{}x", "é".repeat(64))).is_err());
        assert!(AdminErrorCode::new("valid_code_2").is_ok());
        assert!(AdminErrorCode::new("Invalid-Code").is_err());
        assert!(AdminErrorMessage::new("x".repeat(ADMIN_ERROR_MESSAGE_MAX_UTF8_BYTES)).is_ok());
        assert!(
            AdminErrorMessage::new("x".repeat(ADMIN_ERROR_MESSAGE_MAX_UTF8_BYTES + 1)).is_err()
        );
        assert!(AdminErrorMessage::new("unsafe\nmessage").is_err());

        let wrong_success: Result<AdminSuccessResponse<ExampleResult>, _> = serde_json::from_str(
            r#"{"contract_version":1,"ok":false,"correlation_id":"safe-correlation","result":{"state":"committed"}}"#,
        );
        assert!(wrong_success.is_err());
        let wrong_failure: Result<AdminFailureResponse, _> = serde_json::from_str(
            r#"{"contract_version":1,"ok":true,"correlation_id":"safe-correlation","error":{"code":"valid_code","message":"safe"}}"#,
        );
        assert!(wrong_failure.is_err());
        let wrong_version: Result<AdminFailureResponse, _> = serde_json::from_str(
            r#"{"contract_version":2,"ok":false,"correlation_id":"safe-correlation","error":{"code":"valid_code","message":"safe"}}"#,
        );
        assert!(wrong_version.is_err());
    }

    #[test]
    fn duplicate_unknown_and_null_fields_are_rejected() {
        for document in [
            r#"{"contract_version":1,"contract_version":1,"operation_id":"stable-operation","request":{"value":7}}"#,
            r#"{"contract_version":1,"operation_id":"stable-operation","unknown":true,"request":{"value":7}}"#,
            r#"{"contract_version":1,"operation_id":"stable-operation","correlation_id":null,"request":{"value":7}}"#,
            r#"{"contract_version":1,"operation_id":"stable-operation","request":{"value":7,"unknown":true}}"#,
        ] {
            assert!(
                serde_json::from_str::<AdminMutationRequest<ExampleRequest>>(document).is_err()
            );
        }

        assert!(
            serde_json::from_str::<AdminMutationRequest<Option<ExampleRequest>>>(
                r#"{"contract_version":1,"operation_id":"stable-operation","request":null}"#,
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<AdminSuccessResponse<Option<ExampleResult>>>(
                r#"{"contract_version":1,"ok":true,"correlation_id":"safe-correlation","result":null}"#,
            )
            .is_err()
        );
        assert!(matches!(
            AdminMutationRequest::new(operation_id(), None, Option::<ExampleRequest>::None),
            Err(AdminPayloadError::NullForbidden)
        ));
        assert!(matches!(
            AdminSuccessResponse::new(correlation_id(), Option::<ExampleResult>::None),
            Err(AdminPayloadError::NullForbidden)
        ));
    }

    #[derive(Serialize)]
    struct SensitivePayload {
        credential: String,
    }

    struct NullNormalizingPayload;

    impl Serialize for NullNormalizingPayload {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str("normalized")
        }
    }

    impl<'de> Deserialize<'de> for NullNormalizingPayload {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            Option::<de::IgnoredAny>::deserialize(deserializer)?;
            Ok(Self)
        }
    }

    #[test]
    fn raw_null_is_rejected_before_a_payload_can_normalize_it() {
        assert!(
            serde_json::from_str::<AdminMutationRequest<NullNormalizingPayload>>(
                r#"{"contract_version":1,"operation_id":"stable-operation","request":null}"#,
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<AdminSuccessResponse<NullNormalizingPayload>>(
                r#"{"contract_version":1,"ok":true,"correlation_id":"safe-correlation","result":null}"#,
            )
            .is_err()
        );
    }

    #[test]
    fn ordinary_debug_redacts_request_and_result_payloads() {
        let request = AdminMutationRequest::new(
            operation_id(),
            Some(correlation_id()),
            SensitivePayload {
                credential: "secret request credential".to_owned(),
            },
        )
        .unwrap();
        let response = AdminSuccessResponse::new(
            correlation_id(),
            SensitivePayload {
                credential: "secret response credential".to_owned(),
            },
        )
        .unwrap();

        let request_debug = format!("{request:?}");
        let response_debug = format!("{response:?}");
        assert!(request_debug.contains("<redacted>"));
        assert!(response_debug.contains("<redacted>"));
        assert!(!request_debug.contains("secret request"));
        assert!(!response_debug.contains("secret response"));
    }

    #[derive(Debug)]
    struct SensitiveCause;

    impl fmt::Display for SensitiveCause {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("database password and raw SQL")
        }
    }

    impl Error for SensitiveCause {}

    #[test]
    fn host_error_mapping_preserves_only_the_safe_projection() {
        let internal = HostError::with_source(HostErrorKind::TaskFailure, SensitiveCause);
        let response =
            AdminFailureResponse::new(correlation_id(), AdminError::from_host_error(&internal));
        let encoded = serde_json::to_string(&response).unwrap();

        assert_eq!(
            encoded,
            r#"{"contract_version":1,"ok":false,"correlation_id":"safe-correlation","error":{"code":"host_task_failure","message":"authoritative service task failed"}}"#
        );
        assert!(!encoded.contains("password"));
        assert!(!encoded.contains("SQL"));
    }
}
