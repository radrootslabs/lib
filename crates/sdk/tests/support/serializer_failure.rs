use serde::Serialize;
use serde::ser::{self, SerializeStruct};

#[derive(Clone, Copy)]
enum FailingSerializeFailure {
    Start,
    Field(usize),
    End,
}

struct FailingStructSerializer {
    failure: FailingSerializeFailure,
}

struct FailingSerializeStruct {
    field_index: usize,
    failure: FailingSerializeFailure,
}

#[derive(Debug)]
struct FailingSerializeError;

impl core::fmt::Display for FailingSerializeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("intentional serializer failure")
    }
}

impl std::error::Error for FailingSerializeError {}

impl ser::Error for FailingSerializeError {
    fn custom<T>(_message: T) -> Self
    where
        T: core::fmt::Display,
    {
        Self
    }
}

impl FailingStructSerializer {
    fn start() -> Self {
        Self {
            failure: FailingSerializeFailure::Start,
        }
    }

    fn field(field_index: usize) -> Self {
        Self {
            failure: FailingSerializeFailure::Field(field_index),
        }
    }

    fn end() -> Self {
        Self {
            failure: FailingSerializeFailure::End,
        }
    }
}

impl ser::Serializer for FailingStructSerializer {
    type Ok = ();
    type Error = FailingSerializeError;
    type SerializeSeq = ser::Impossible<(), FailingSerializeError>;
    type SerializeTuple = ser::Impossible<(), FailingSerializeError>;
    type SerializeTupleStruct = ser::Impossible<(), FailingSerializeError>;
    type SerializeTupleVariant = ser::Impossible<(), FailingSerializeError>;
    type SerializeMap = ser::Impossible<(), FailingSerializeError>;
    type SerializeStruct = FailingSerializeStruct;
    type SerializeStructVariant = ser::Impossible<(), FailingSerializeError>;

    fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_str(self, _value: &str) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(FailingSerializeError)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(FailingSerializeError)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(FailingSerializeError)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        match self.failure {
            FailingSerializeFailure::Start => Err(FailingSerializeError),
            failure => Ok(FailingSerializeStruct {
                field_index: 0,
                failure,
            }),
        }
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(FailingSerializeError)
    }
}

impl SerializeStruct for FailingSerializeStruct {
    type Ok = ();
    type Error = FailingSerializeError;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.field_index += 1;
        match self.failure {
            FailingSerializeFailure::Field(field) if self.field_index == field => {
                Err(FailingSerializeError)
            }
            _ => Ok(()),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self.failure {
            FailingSerializeFailure::End => Err(FailingSerializeError),
            _ => Ok(()),
        }
    }
}

pub fn assert_struct_serialize_error_paths<T>(value: &T, field_count: usize)
where
    T: Serialize,
{
    value
        .serialize(FailingStructSerializer::start())
        .expect_err("struct start failure");
    for field_index in 1..=field_count {
        value
            .serialize(FailingStructSerializer::field(field_index))
            .expect_err("struct field failure");
    }
    value
        .serialize(FailingStructSerializer::end())
        .expect_err("struct end failure");
}
