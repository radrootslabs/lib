#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::RadrootsTangleEventsError;

pub fn canonical_json_string<T: Serialize>(
    value: &T,
) -> Result<String, RadrootsTangleEventsError> {
    let value = serde_json::to_value(value).map_err(|_| {
        RadrootsTangleEventsError::InvalidData("canonical json serialization failed".to_string())
    })?;
    let canonical = canonicalize_value(value);
    serde_json::to_string(&canonical).map_err(|_| {
        RadrootsTangleEventsError::InvalidData("canonical json encoding failed".to_string())
    })
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
