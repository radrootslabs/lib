use crate::RadrootsTransportError;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use serde::de::{Error as _, SeqAccess, Visitor};

pub(crate) fn deserialize_string<'de, D>(
    deserializer: D,
    field: &'static str,
    max: usize,
) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_string(BoundedStringVisitor { field, max })
}

pub(crate) fn deserialize_vec<'de, D, T>(
    deserializer: D,
    field: &'static str,
    max: usize,
) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    deserializer.deserialize_seq(BoundedVecVisitor {
        field,
        max,
        marker: PhantomData,
    })
}

struct BoundedStringVisitor {
    field: &'static str,
    max: usize,
}

impl BoundedStringVisitor {
    fn validate<E>(self, value: &str) -> Result<String, E>
    where
        E: serde::de::Error,
    {
        if value.len() > self.max {
            return Err(E::custom(RadrootsTransportError::ResourceLimitExceeded {
                field: self.field,
                max: self.max,
                actual: value.len(),
            }));
        }
        Ok(value.into())
    }
}

impl Visitor<'_> for BoundedStringVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a string of at most {} UTF-8 bytes", self.max)
    }

    fn visit_borrowed_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.validate(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.validate(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value.len() > self.max {
            return Err(E::custom(RadrootsTransportError::ResourceLimitExceeded {
                field: self.field,
                max: self.max,
                actual: value.len(),
            }));
        }
        Ok(value)
    }
}

struct BoundedVecVisitor<T> {
    field: &'static str,
    max: usize,
    marker: PhantomData<T>,
}

impl<'de, T> Visitor<'de> for BoundedVecVisitor<T>
where
    T: serde::Deserialize<'de>,
{
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequence of at most {} items", self.max)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if let Some(actual) = sequence.size_hint().filter(|actual| *actual > self.max) {
            return Err(A::Error::custom(
                RadrootsTransportError::ResourceLimitExceeded {
                    field: self.field,
                    max: self.max,
                    actual,
                },
            ));
        }
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(self.max));
        while let Some(value) = sequence.next_element()? {
            if values.len() == self.max {
                return Err(A::Error::custom(
                    RadrootsTransportError::ResourceLimitExceeded {
                        field: self.field,
                        max: self.max,
                        actual: self.max.saturating_add(1),
                    },
                ));
            }
            values.push(value);
        }
        Ok(values)
    }
}
