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
