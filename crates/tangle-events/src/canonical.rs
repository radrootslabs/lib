#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::RadrootsTangleEventsError;

pub fn canonical_json_string<T: Serialize>(value: &T) -> Result<String, RadrootsTangleEventsError> {
    let value = serde_json::to_value(value).map_err(|_| {
        RadrootsTangleEventsError::InvalidData("canonical json serialization failed".to_string())
    })?;
    Ok(canonicalize_value(value).to_string())
}

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(map) => canonicalize_object(map),
        Value::Array(values) => {
            let values = values
                .into_iter()
                .map(canonicalize_value)
                .collect::<Vec<_>>();
            Value::Array(values)
        }
        other => other,
    }
}

fn canonicalize_object(map: Map<String, Value>) -> Value {
    let mut entries = map.into_iter().collect::<Vec<_>>();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut ordered = Map::new();
    for (key, value) in entries {
        ordered.insert(key, canonicalize_value(value));
    }
    Value::Object(ordered)
}

#[cfg(test)]
mod tests {
    use super::canonical_json_string;
    use serde::Serialize;

    #[derive(Serialize)]
    struct CanonicalFixture {
        z: u32,
        a: NestedFixture,
    }

    #[derive(Serialize)]
    struct NestedFixture {
        b: u32,
        a: u32,
    }

    #[test]
    fn canonical_json_string_sorts_object_keys_recursively() {
        let value = CanonicalFixture {
            z: 2,
            a: NestedFixture { b: 3, a: 1 },
        };
        let json = canonical_json_string(&value).expect("json");
        assert_eq!(json, r#"{"a":{"a":1,"b":3},"z":2}"#);
    }

    #[test]
    fn canonical_json_string_handles_arrays() {
        let json = canonical_json_string(&serde_json::json!([{"b": 2, "a": 1}])).expect("json");
        assert_eq!(json, r#"[{"a":1,"b":2}]"#);
    }

    struct AlwaysErr;

    impl Serialize for AlwaysErr {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("always fail"))
        }
    }

    #[test]
    fn canonical_json_string_propagates_serialization_errors() {
        let err = canonical_json_string(&AlwaysErr).expect_err("serialize fail");
        assert!(
            err.to_string()
                .contains("canonical json serialization failed")
        );
    }
}
