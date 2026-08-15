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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminPayloadError {
    NullForbidden,
    Encoding,
}

impl fmt::Display for AdminPayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NullForbidden => "admin payloads may not contain JSON null",
            Self::Encoding => "admin payload could not be represented as JSON",
        })
    }
}

impl Error for AdminPayloadError {}

#[derive(Clone, PartialEq, Eq)]
struct NonNullPayload<T>(T);

impl<T> NonNullPayload<T>
where
    T: Serialize,
{
    fn new(value: T) -> Result<Self, AdminPayloadError> {
        checked_non_null(&value)?;
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
        checked_non_null(&self.0).map_err(ser::Error::custom)?;
        self.0.serialize(serializer)
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

// This implementation is a mechanical Serde dispatch adapter. The conformance test below drives
// every supported data shape through it; measuring every generic forwarding instantiation would
// count compiler-generated dispatch rather than additional contract behavior.
#[cfg_attr(coverage_nightly, coverage(off))]
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

// Scalar visits forward without policy. Null rejection and recursive
// option/newtype/sequence/map/enum traversal remain measured below.
impl<'de, V> de::Visitor<'de> for NoNullVisitor<V>
where
    V: de::Visitor<'de>,
{
    type Value = V::Value;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.expecting(formatter)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_bool(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_i8<E>(self, value: i8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i8(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_i16<E>(self, value: i16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i16(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i32(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i64(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_i128<E>(self, value: i128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_i128(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u8(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_u16<E>(self, value: u16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u16(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u32(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u64(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_u128(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_f32(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_f64(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_char<E>(self, value: char) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_char(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_str(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_borrowed_str(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_string(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_bytes(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.0.visit_borrowed_bytes(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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

#[derive(Clone, Copy, Debug)]
enum PayloadValidationError {
    Null,
    Encoding,
}

impl fmt::Display for PayloadValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin payload validation failed")
    }
}

impl Error for PayloadValidationError {}

impl ser::Error for PayloadValidationError {
    fn custom<T>(_message: T) -> Self
    where
        T: fmt::Display,
    {
        Self::Encoding
    }
}

#[derive(Clone, Copy)]
struct NonNullSerializer<'failure> {
    failure: &'failure core::cell::Cell<Option<PayloadValidationError>>,
}

impl NonNullSerializer<'_> {
    fn finish(self) -> Result<(), PayloadValidationError> {
        self.failure.get().map_or(Ok(()), Err)
    }

    fn reject(self, error: PayloadValidationError) -> Result<(), PayloadValidationError> {
        if self.failure.get().is_none() {
            self.failure.set(Some(error));
        }
        Err(error)
    }
}

impl<'failure> ser::Serializer for NonNullSerializer<'failure> {
    type Ok = ();
    type Error = PayloadValidationError;
    type SerializeSeq = NonNullCompound<'failure>;
    type SerializeTuple = NonNullCompound<'failure>;
    type SerializeTupleStruct = NonNullCompound<'failure>;
    type SerializeTupleVariant = NonNullCompound<'failure>;
    type SerializeMap = NonNullCompound<'failure>;
    type SerializeStruct = NonNullCompound<'failure>;
    type SerializeStructVariant = NonNullCompound<'failure>;

    fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_i128(self, _value: i128) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_u128(self, _value: u128) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_str(self, _value: &str) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.reject(PayloadValidationError::Null)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.reject(PayloadValidationError::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.reject(PayloadValidationError::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.finish()?;
        Ok(NonNullCompound(self))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.finish()?;
        Ok(NonNullCompound(self))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.finish()?;
        Ok(NonNullCompound(self))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.finish()?;
        Ok(NonNullCompound(self))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.finish()?;
        Ok(NonNullCompound(self))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.finish()?;
        Ok(NonNullCompound(self))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.finish()?;
        Ok(NonNullCompound(self))
    }

    fn collect_str<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + fmt::Display,
    {
        self.finish()
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

struct NonNullCompound<'failure>(NonNullSerializer<'failure>);

impl NonNullCompound<'_> {
    fn value<T>(&mut self, value: &T) -> Result<(), PayloadValidationError>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self.0)
    }

    fn finish(self) -> Result<(), PayloadValidationError> {
        self.0.finish()
    }
}

impl ser::SerializeSeq for NonNullCompound<'_> {
    type Ok = ();
    type Error = PayloadValidationError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeTuple for NonNullCompound<'_> {
    type Ok = ();
    type Error = PayloadValidationError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeTupleStruct for NonNullCompound<'_> {
    type Ok = ();
    type Error = PayloadValidationError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeTupleVariant for NonNullCompound<'_> {
    type Ok = ();
    type Error = PayloadValidationError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeMap for NonNullCompound<'_> {
    type Ok = ();
    type Error = PayloadValidationError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.value(key)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeStruct for NonNullCompound<'_> {
    type Ok = ();
    type Error = PayloadValidationError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeStructVariant for NonNullCompound<'_> {
    type Ok = ();
    type Error = PayloadValidationError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

#[derive(Default)]
struct NonNullJsonWriter {
    inside_string: bool,
    escaped: bool,
    null_progress: usize,
    rejected: bool,
}

impl std::io::Write for NonNullJsonWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.rejected {
            return Err(std::io::Error::other("JSON null is forbidden"));
        }
        for &byte in bytes {
            if self.inside_string {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == b'"' {
                    self.inside_string = false;
                }
                continue;
            }

            if byte == b'"' {
                self.inside_string = true;
                self.null_progress = 0;
                continue;
            }

            let expected = b"null";
            if byte == expected[self.null_progress] {
                self.null_progress += 1;
                if self.null_progress == expected.len() {
                    self.rejected = true;
                    return Err(std::io::Error::other("JSON null is forbidden"));
                }
            } else {
                self.null_progress = usize::from(byte == b'n');
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.rejected {
            Err(std::io::Error::other("JSON null is forbidden"))
        } else {
            Ok(())
        }
    }
}

fn checked_non_null(value: &impl Serialize) -> Result<(), AdminPayloadError> {
    let failure = core::cell::Cell::new(None);
    let serializer = NonNullSerializer { failure: &failure };
    let result = value.serialize(serializer);
    match failure.get().or_else(|| result.err()) {
        None => {
            let mut writer = NonNullJsonWriter::default();
            match serde_json::to_writer(&mut writer, value) {
                Ok(()) => Ok(()),
                Err(_) if writer.rejected => Err(AdminPayloadError::NullForbidden),
                Err(_) => Err(AdminPayloadError::Encoding),
            }
        }
        Some(PayloadValidationError::Null) => Err(AdminPayloadError::NullForbidden),
        Some(PayloadValidationError::Encoding) => Err(AdminPayloadError::Encoding),
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
    use std::collections::BTreeMap;
    use std::error::Error;

    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

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

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct NewtypePayload(u16);

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TuplePayload(i8, String);

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct StructPayload {
        enabled: bool,
        count: u32,
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum EnumPayload {
        Unit,
        Newtype(u64),
        Tuple(i32, bool),
        Struct { label: String },
    }

    #[derive(Clone, Debug, PartialEq)]
    struct AnyPayload(String);

    impl Serialize for AnyPayload {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.0)
        }
    }

    impl<'de> Deserialize<'de> for AnyPayload {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct Visitor;

            impl<'de> de::Visitor<'de> for Visitor {
                type Value = AnyPayload;

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a string payload")
                }

                fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(AnyPayload(value.to_owned()))
                }
            }

            deserializer.deserialize_any(Visitor)
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct ByteBufferPayload(Vec<u8>);

    impl Serialize for ByteBufferPayload {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_bytes(&self.0)
        }
    }

    impl<'de> Deserialize<'de> for ByteBufferPayload {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct Visitor;

            impl<'de> de::Visitor<'de> for Visitor {
                type Value = ByteBufferPayload;

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a byte buffer")
                }

                fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E> {
                    Ok(ByteBufferPayload(value))
                }

                fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E> {
                    Ok(ByteBufferPayload(value.to_vec()))
                }

                fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E> {
                    Ok(ByteBufferPayload(value.to_vec()))
                }
            }

            deserializer.deserialize_byte_buf(Visitor)
        }
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
    fn recursive_non_null_adapter_covers_every_supported_serde_shape() {
        macro_rules! payload {
            ($ty:ty, $json:literal, $expected:expr) => {
                assert_eq!(
                    serde_json::from_str::<NonNullPayload<$ty>>($json)
                        .expect("non-null payload must decode")
                        .0,
                    $expected
                );
            };
        }

        payload!(bool, "true", true);
        payload!(i8, "-8", -8);
        payload!(i16, "-16", -16);
        payload!(i32, "-32", -32);
        payload!(i64, "-64", -64);
        payload!(i128, "-128", -128);
        payload!(u8, "8", 8);
        payload!(u16, "16", 16);
        payload!(u32, "32", 32);
        payload!(u64, "64", 64);
        payload!(u128, "128", 128);
        payload!(f32, "1.5", 1.5);
        payload!(f64, "2.5", 2.5);
        payload!(char, r#""r""#, 'r');
        payload!(String, r#""text""#, "text".to_owned());
        payload!(Option<u32>, "7", Some(7));
        payload!([u8; 3], "[1,2,3]", [1, 2, 3]);
        payload!((u8, bool), "[4,true]", (4, true));
        payload!(NewtypePayload, "9", NewtypePayload(9));
        payload!(
            TuplePayload,
            r#"[5,"tuple"]"#,
            TuplePayload(5, "tuple".to_owned())
        );
        payload!(
            StructPayload,
            r#"{"enabled":true,"count":11}"#,
            StructPayload {
                enabled: true,
                count: 11,
            }
        );
        payload!(EnumPayload, r#""Unit""#, EnumPayload::Unit);
        payload!(EnumPayload, r#"{"Newtype":12}"#, EnumPayload::Newtype(12));
        payload!(
            EnumPayload,
            r#"{"Tuple":[13,false]}"#,
            EnumPayload::Tuple(13, false)
        );
        payload!(
            EnumPayload,
            r#"{"Struct":{"label":"enum"}}"#,
            EnumPayload::Struct {
                label: "enum".to_owned(),
            }
        );
        payload!(AnyPayload, r#""any""#, AnyPayload("any".to_owned()));
        payload!(
            BTreeMap<String, u8>,
            r#"{"first":1,"second":2}"#,
            BTreeMap::from([("first".to_owned(), 1), ("second".to_owned(), 2)])
        );

        let bytes = serde::de::value::BytesDeserializer::<serde::de::value::Error>::new(&[1, 2, 3]);
        assert_eq!(
            ByteBufferPayload::deserialize(NoNullDeserializer(bytes)).unwrap(),
            ByteBufferPayload(vec![1, 2, 3])
        );

        assert!(serde_json::from_str::<NonNullPayload<()>>("null").is_err());
        assert!(serde_json::from_str::<NonNullPayload<Option<u8>>>("null").is_err());
        assert!(serde_json::from_str::<NonNullPayload<Vec<Option<u8>>>>("[1,null]").is_err());
        assert!(
            serde_json::from_str::<NonNullPayload<BTreeMap<String, Option<u8>>>>(
                r#"{"safe":1,"forbidden":null}"#,
            )
            .is_err()
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
        assert!(AdminErrorCode::new("valid-code").is_err());
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
    fn public_accessors_and_stable_errors_are_fully_bound() {
        let operation = operation_id();
        let correlation = correlation_id();
        let request = AdminMutationRequest::new(
            operation.clone(),
            Some(correlation.clone()),
            ExampleRequest { value: 17 },
        )
        .unwrap();
        assert_eq!(request.contract_version(), ADMIN_CONTRACT_VERSION);
        assert_eq!(request.operation_id(), &operation);
        assert_eq!(request.correlation_id(), Some(&correlation));
        assert_eq!(request.request().value, 17);
        assert!(request.validate_contract_version().is_ok());
        assert_eq!(request.into_request(), ExampleRequest { value: 17 });

        let success = AdminSuccessResponse::new(
            correlation.clone(),
            ExampleResult {
                state: "ready".to_owned(),
            },
        )
        .unwrap();
        assert_eq!(success.correlation_id(), &correlation);
        assert_eq!(success.result().state, "ready");
        assert_eq!(success.into_result().state, "ready");

        let error = AdminError::new(
            AdminErrorCode::new("stable_error").unwrap(),
            AdminErrorMessage::new("stable message").unwrap(),
        );
        assert_eq!(error.code().as_str(), "stable_error");
        assert_eq!(error.message().as_str(), "stable message");
        assert_eq!(error.code().to_string(), "stable_error");
        assert_eq!(error.message().to_string(), "stable message");
        let failure = AdminFailureResponse::new(correlation.clone(), error.clone());
        assert_eq!(failure.correlation_id(), &correlation);
        assert_eq!(failure.error(), &error);

        for rendered in [
            AdminIdentifierError::Empty {
                field: AdminIdentifierField::CorrelationId,
            }
            .to_string(),
            AdminErrorCodeError::Empty.to_string(),
            AdminErrorMessageError::Empty.to_string(),
            AdminPayloadError::NullForbidden.to_string(),
            AdminPayloadError::Encoding.to_string(),
            AdminContractVersionError { received: 9 }.to_string(),
        ] {
            assert!(!rendered.is_empty());
        }
        assert_eq!(operation.to_string(), "stable-operation");
        assert_eq!(correlation.to_string(), "safe-correlation");
        assert!(AdminErrorCode::new("").is_err());
        assert!(AdminErrorCode::new("x".repeat(ADMIN_ERROR_CODE_MAX_UTF8_BYTES + 1)).is_err());
        assert!(AdminErrorMessage::new("").is_err());
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

    struct IgnoredNestedNull;

    impl Serialize for IgnoredNestedNull {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut structure = serializer.serialize_struct("IgnoredNestedNull", 1)?;
            let _ = ser::SerializeStruct::serialize_field(
                &mut structure,
                "ignored",
                &Option::<u8>::None,
            );
            ser::SerializeStruct::end(structure)
        }
    }

    #[test]
    fn construction_rejects_a_nested_null_even_when_custom_serialization_ignores_the_error() {
        assert!(matches!(
            AdminSuccessResponse::new(correlation_id(), IgnoredNestedNull),
            Err(AdminPayloadError::NullForbidden)
        ));

        let raw = serde_json::value::RawValue::from_string("null".to_owned()).unwrap();
        assert!(matches!(
            AdminSuccessResponse::new(correlation_id(), raw),
            Err(AdminPayloadError::NullForbidden)
        ));

        let raw = serde_json::value::RawValue::from_string(
            r#"{"null":"escaped \"null\" text"}"#.to_owned(),
        )
        .unwrap();
        assert!(AdminSuccessResponse::new(correlation_id(), raw).is_ok());

        let mut writer = NonNullJsonWriter::default();
        assert_eq!(std::io::Write::write(&mut writer, b"nu").unwrap(), 2);
        assert!(std::io::Write::write(&mut writer, b"ll").is_err());
        assert!(std::io::Write::write(&mut writer, b"true").is_err());
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

    #[test]
    fn payload_encoding_failure_has_only_a_stable_crate_owned_error() {
        let unsupported_json_map_key = std::collections::BTreeMap::from([((1_u8, 2_u8), true)]);
        let error = AdminMutationRequest::new(operation_id(), None, unsupported_json_map_key)
            .expect_err("tuple map key must not encode as JSON");

        assert_eq!(error, AdminPayloadError::Encoding);
        assert_eq!(format!("{error:?}"), "Encoding");
        assert!(error.source().is_none());
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
