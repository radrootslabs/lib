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
    use alloc::format;
    #[cfg(not(feature = "std"))]
    use alloc::string::ToString;
    use serde::Deserialize;
    #[cfg(feature = "std")]
    use std::string::ToString;

    #[derive(Debug, Deserialize)]
    struct EpochSecondsFixture {
        #[serde(deserialize_with = "epoch_seconds::de")]
        ts: u32,
    }

    #[test]
    fn epoch_seconds_accepts_u32_max() {
        let fixture: EpochSecondsFixture =
            serde_json::from_str(&format!(r#"{{"ts":{}}}"#, u32::MAX)).unwrap();
        assert_eq!(fixture.ts, u32::MAX);
    }

    #[test]
    fn epoch_seconds_rejects_overflow() {
        let err = serde_json::from_str::<EpochSecondsFixture>(&format!(
            r#"{{"ts":{}}}"#,
            u32::MAX as u64 + 1
        ))
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("epoch **seconds**"));
    }

    #[test]
    fn epoch_seconds_rejects_invalid_input_type() {
        let err = serde_json::from_str::<EpochSecondsFixture>(r#"{"ts":"1700000000"}"#).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid type"));
    }
}
