//! Bounded borrowed JSON arrays shared by public and protected codecs.
use alloc::vec::Vec;
use core::{fmt, marker::PhantomData};
use serde::{
    Deserialize,
    de::{IgnoredAny, SeqAccess, Visitor},
};
use serde_json::value::RawValue;

pub(crate) struct RawArray<'a, const N: usize>(pub(crate) Vec<&'a RawValue>);
impl<'de: 'a, 'a, const N: usize> Deserialize<'de> for RawArray<'a, N> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ArrayVisitor<'a, const N: usize>(PhantomData<&'a ()>);
        impl<'de: 'a, 'a, const N: usize> Visitor<'de> for ArrayVisitor<'a, N> {
            type Value = RawArray<'a, N>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                loop {
                    if values.len() == N {
                        if seq.next_element::<IgnoredAny>()?.is_some() {
                            return Err(serde::de::Error::custom("array limit exceeded"));
                        }
                        break;
                    }
                    let Some(value) = seq.next_element::<&'de RawValue>()? else {
                        break;
                    };
                    values.push(value);
                }
                Ok(RawArray(values))
            }
        }
        d.deserialize_seq(ArrayVisitor::<N>(PhantomData))
    }
}
