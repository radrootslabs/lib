use std::{fs, path::Path};

use serde_json::Value;

use crate::CommunityProvisioningError;

pub fn load_contract_schema(path: &Path) -> Result<Value, CommunityProvisioningError> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn load_schema_checked_value(
    path: &Path,
    schema: &Value,
) -> Result<Value, CommunityProvisioningError> {
    let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    validate_contract_schema(schema, &value).map_err(|error| {
        CommunityProvisioningError::Invalid(format!(
            "{} failed schema validation: {error}",
            path.display()
        ))
    })?;
    Ok(value)
}

pub fn validate_contract_schema(
    schema: &Value,
    instance: &Value,
) -> Result<(), CommunityProvisioningError> {
    validate_supported_schema(schema, "$").map_err(CommunityProvisioningError::Invalid)?;
    validate_schema_node(schema, instance, schema, "$").map_err(CommunityProvisioningError::Invalid)
}

fn validate_supported_schema(schema: &Value, path: &str) -> Result<(), String> {
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    for key in object.keys() {
        if ![
            "$defs",
            "$id",
            "$ref",
            "$schema",
            "additionalProperties",
            "const",
            "contains",
            "enum",
            "items",
            "minimum",
            "minItems",
            "minLength",
            "oneOf",
            "pattern",
            "properties",
            "required",
            "title",
            "type",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("{path} uses unsupported schema keyword {key}"));
        }
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (key, property_schema) in properties {
            validate_supported_schema(property_schema, &format!("{path}.properties.{key}"))?;
        }
    }
    if let Some(definitions) = object.get("$defs").and_then(Value::as_object) {
        for (key, definition_schema) in definitions {
            validate_supported_schema(definition_schema, &format!("{path}.$defs.{key}"))?;
        }
    }
    if let Some(options) = object.get("oneOf").and_then(Value::as_array) {
        for (index, option) in options.iter().enumerate() {
            validate_supported_schema(option, &format!("{path}.oneOf[{index}]"))?;
        }
    }
    if let Some(items) = object.get("items") {
        validate_supported_schema(items, &format!("{path}.items"))?;
    }
    if let Some(contains) = object.get("contains") {
        validate_supported_schema(contains, &format!("{path}.contains"))?;
    }
    Ok(())
}

fn validate_schema_node(
    schema: &Value,
    instance: &Value,
    root: &Value,
    path: &str,
) -> Result<(), String> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return validate_schema_node(resolve_ref(root, reference)?, instance, root, path);
    }
    if let Some(options) = schema.get("oneOf").and_then(Value::as_array) {
        let matches = options
            .iter()
            .filter(|option| validate_schema_node(option, instance, root, path).is_ok())
            .count();
        if matches != 1 {
            return Err(format!("{path} matched {matches} oneOf branches"));
        }
    }
    if let Some(expected) = schema.get("const")
        && instance != expected
    {
        return Err(format!("{path} did not match const {expected}"));
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array)
        && !values.iter().any(|value| value == instance)
    {
        return Err(format!("{path} did not match enum"));
    }
    if let Some(expected_type) = schema.get("type").and_then(Value::as_str) {
        validate_json_type(expected_type, instance, path)?;
    }
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        let value = instance
            .as_f64()
            .ok_or_else(|| format!("{path} must be number for minimum"))?;
        if value < minimum {
            return Err(format!("{path} is below {minimum}"));
        }
    }
    if let Some(min_length) = schema.get("minLength").and_then(Value::as_u64) {
        let value = instance
            .as_str()
            .ok_or_else(|| format!("{path} must be string for minLength"))?;
        if value.len() < min_length as usize {
            return Err(format!("{path} length is below {min_length}"));
        }
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        let value = instance
            .as_str()
            .ok_or_else(|| format!("{path} must be string for pattern"))?;
        if !matches_contract_pattern(pattern, value)? {
            return Err(format!("{path} did not match pattern {pattern}"));
        }
    }
    if let Some(min_items) = schema.get("minItems").and_then(Value::as_u64) {
        let value = instance
            .as_array()
            .ok_or_else(|| format!("{path} must be array for minItems"))?;
        if value.len() < min_items as usize {
            return Err(format!("{path} item count is below {min_items}"));
        }
    }
    if let Some(object) = instance.as_object()
        && let Some(properties) = schema.get("properties").and_then(Value::as_object)
    {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for key in required {
                let key = key
                    .as_str()
                    .ok_or_else(|| format!("{path} required key must be string"))?;
                if !object.contains_key(key) {
                    return Err(format!("{path} missing required key {key}"));
                }
            }
        }
        for (key, property_schema) in properties {
            if let Some(value) = object.get(key) {
                validate_schema_node(property_schema, value, root, &format!("{path}.{key}"))?;
            }
        }
        if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
            for key in object.keys() {
                if !properties.contains_key(key) {
                    return Err(format!("{path} contains unknown key {key}"));
                }
            }
        }
    }
    if let Some(array) = instance.as_array() {
        if let Some(item_schema) = schema.get("items") {
            for (index, item) in array.iter().enumerate() {
                validate_schema_node(item_schema, item, root, &format!("{path}[{index}]"))?;
            }
        }
        if let Some(contains_schema) = schema.get("contains")
            && !array
                .iter()
                .any(|item| validate_schema_node(contains_schema, item, root, path).is_ok())
        {
            return Err(format!("{path} does not contain required item"));
        }
    }
    Ok(())
}

fn validate_json_type(expected_type: &str, instance: &Value, path: &str) -> Result<(), String> {
    let matches = match expected_type {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        "integer" => instance.as_i64().is_some(),
        "number" => instance.as_f64().is_some(),
        _ => return Err(format!("{path} uses unsupported type {expected_type}")),
    };
    if matches {
        Ok(())
    } else {
        Err(format!("{path} is not {expected_type}"))
    }
}

fn resolve_ref<'a>(root: &'a Value, reference: &str) -> Result<&'a Value, String> {
    let pointer = reference
        .strip_prefix('#')
        .ok_or_else(|| format!("unsupported ref {reference}"))?;
    root.pointer(pointer)
        .ok_or_else(|| format!("unresolved ref {reference}"))
}

fn matches_contract_pattern(pattern: &str, value: &str) -> Result<bool, String> {
    match pattern {
        "^[a-z0-9_][a-z0-9_-]*$" => Ok(is_contract_id(value)),
        "^[a-f0-9]{64}$" => Ok(is_lower_hex_64(value)),
        "^wss?://" => Ok(value.starts_with("ws://") || value.starts_with("wss://")),
        "^[a-z0-9][a-z0-9.-]+$" => Ok(value.len() >= 2
            && (value.as_bytes()[0].is_ascii_lowercase() || value.as_bytes()[0].is_ascii_digit())
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-'
            })),
        "^tenants/[a-z0-9_][a-z0-9_-]*[.]json$" => Ok(value
            .strip_prefix("tenants/")
            .and_then(|value| value.strip_suffix(".json"))
            .is_some_and(is_contract_id)),
        "^runtime/tenants/" => Ok(value.starts_with("runtime/tenants/")),
        _ => Err(format!("unsupported schema pattern {pattern}")),
    }
}

fn is_contract_id(value: &str) -> bool {
    let Some(first) = value.as_bytes().first() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit() || *first == b'_')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(is_lower_hex)
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate_contract_schema;

    #[test]
    fn schema_validation_rejects_unknown_fields() {
        let schema = json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {"type": "string", "minLength": 1}
            },
            "additionalProperties": false
        });
        let instance = json!({"name": "Market", "unexpected": true});

        let error = validate_contract_schema(&schema, &instance).expect_err("schema error");

        assert!(error.to_string().contains("unknown key unexpected"));
    }

    #[test]
    fn schema_validation_rejects_minimum_min_items_and_contains_failures() {
        let schema = json!({
            "type": "object",
            "required": ["count", "tags"],
            "properties": {
                "count": {"type": "integer", "minimum": 1},
                "tags": {
                    "type": "array",
                    "minItems": 1,
                    "contains": {"const": "h"}
                }
            },
            "additionalProperties": false
        });

        assert!(validate_contract_schema(&schema, &json!({"count": 0, "tags": ["h"]})).is_err());
        assert!(validate_contract_schema(&schema, &json!({"count": 1, "tags": []})).is_err());
        assert!(validate_contract_schema(&schema, &json!({"count": 1, "tags": ["name"]})).is_err());
        assert!(validate_contract_schema(&schema, &json!({"count": 1, "tags": ["h"]})).is_ok());
    }
}
