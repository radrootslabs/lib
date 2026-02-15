#[cfg(feature = "serde")]
pub mod epoch_seconds {
    use serde::{Deserialize, Deserializer, de::Error as DeError};

    pub fn de<'de, D>(de: D) -> Result<u32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = u64::deserialize(de)?;
        if v > u32::MAX as u64 {
            return Err(D::Error::custom(
                "timestamp must be epoch **seconds**, not ms",
            ));
        }
        Ok(v as u32)
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::epoch_seconds;
    #[cfg(not(feature = "std"))]
    use alloc::string::ToString;
    use serde::de::value::{Error as DeError, U64Deserializer};
    #[cfg(feature = "std")]
    use std::string::ToString;

    #[test]
    fn epoch_seconds_accepts_u32_max() {
        let de = U64Deserializer::<DeError>::new(u32::MAX as u64);
        let val = epoch_seconds::de(de).unwrap();
        assert_eq!(val, u32::MAX);
    }

    #[test]
    fn epoch_seconds_rejects_overflow() {
        let de = U64Deserializer::<DeError>::new(u32::MAX as u64 + 1);
        let err = epoch_seconds::de(de).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("epoch **seconds**"));
    }
}
