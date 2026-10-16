use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::service_source_lock::{
    NoDuplicateJsonValue, valid_nix_sha256, validate_deferred_nix_lock,
    validate_deferred_nix_material,
};

pub(crate) struct BoundNixMaterial {
    pub(crate) revision: String,
    pub(crate) lock_sha256: String,
}

pub(crate) fn validate_bound_nix_lock(bytes: &[u8]) -> Result<String, ()> {
    parse_lock(bytes).map(|(revision, _)| revision)
}

pub(crate) fn validate_bound_nix_material(
    expression: &[u8],
    lock: &[u8],
) -> Result<BoundNixMaterial, ()> {
    if expression.len() > 1_048_576 {
        return Err(());
    }
    let (revision, helpers) = parse_lock(lock)?;
    if helpers {
        let text = std::str::from_utf8(expression).map_err(|_| ())?;
        let lines = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();
        let expected = format!("url = \"github:radrootslabs/lib/{revision}\";");
        let selectors = lines
            .iter()
            .filter(|line| line.contains("github:radrootslabs/lib"))
            .count();
        let blocks = lines
            .windows(4)
            .filter(|block| {
                block[0] == "lib = {"
                    && lines.iter().filter(|line| **line == "inputs = {").count() == 1
                    && block[1] == expected
                    && block[2] == "inputs.crane.follows = \"crane\";"
                    && block[3] == "};"
            })
            .count();
        if selectors != 1 || blocks != 1 {
            return Err(());
        }
    } else {
        let evidence = validate_deferred_nix_material(expression, lock).map_err(|_| ())?;
        return Ok(BoundNixMaterial {
            revision: evidence.lib_revision().to_owned(),
            lock_sha256: evidence.flake_lock_sha256().to_owned(),
        });
    }
    Ok(BoundNixMaterial {
        revision,
        lock_sha256: hex::encode(Sha256::digest(lock)),
    })
}

fn parse_lock(bytes: &[u8]) -> Result<(String, bool), ()> {
    if bytes.len() > 4_194_304 {
        return Err(());
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let mut value = NoDuplicateJsonValue::deserialize(&mut decoder)
        .map_err(|_| ())?
        .0;
    decoder.end().map_err(|_| ())?;
    let root = value.get("root").and_then(Value::as_str).ok_or(())?;
    let nodes = value.get("nodes").and_then(Value::as_object).ok_or(())?;
    let root_inputs = nodes
        .get(root)
        .and_then(|node| node.get("inputs"))
        .and_then(Value::as_object)
        .ok_or(())?;
    let lib_name = root_inputs
        .get("lib")
        .and_then(Value::as_str)
        .ok_or(())?
        .to_owned();
    let lib = nodes.get(&lib_name).and_then(Value::as_object).ok_or(())?;
    let helpers = if let Some(inputs) = lib.get("inputs") {
        let inputs = inputs.as_object().ok_or(())?;
        if inputs.keys().map(String::as_str).collect::<BTreeSet<_>>()
            != BTreeSet::from([
                "crane",
                "flake-parts",
                "nixpkgs",
                "rust-overlay",
                "treefmt-nix",
            ])
        {
            return Err(());
        }
        for (name, target) in inputs {
            let target = if name == "crane" {
                if target != &serde_json::json!(["crane"]) {
                    return Err(());
                }
                root_inputs.get("crane").and_then(Value::as_str).ok_or(())?
            } else {
                target.as_str().ok_or(())?
            };
            if target == lib_name || target == root {
                return Err(());
            }
            let locked = nodes
                .get(target)
                .and_then(|node| node.get("locked"))
                .ok_or(())?;
            let revision = locked.get("rev").and_then(Value::as_str).ok_or(())?;
            if locked.get("type").and_then(Value::as_str) != Some("github")
                || revision.len() != 40
                || !revision
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || !locked
                    .get("narHash")
                    .and_then(Value::as_str)
                    .is_some_and(valid_nix_sha256)
            {
                return Err(());
            }
        }
        true
    } else {
        false
    };
    if helpers {
        value["nodes"][&lib_name]
            .as_object_mut()
            .ok_or(())?
            .remove("inputs");
    }
    let normalized = serde_json::to_vec(&value).map_err(|_| ())?;
    let revision = validate_deferred_nix_lock(&normalized).map_err(|_| ())?;
    Ok((revision, helpers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (String, Value) {
        let revision = "a".repeat(40);
        let mut lock = json!({
            "version": 7, "root": "root",
            "nodes": {
                "root": {"inputs": {"lib": "lib", "crane": "crane"}},
                "lib": {
                    "inputs": {"crane": ["crane"], "flake-parts": "parts", "nixpkgs": "pkgs", "rust-overlay": "rust", "treefmt-nix": "fmt"},
                    "locked": {"lastModified": 1, "narHash": format!("sha256-{}=", "A".repeat(43)), "owner": "radrootslabs", "repo": "lib", "rev": revision, "type": "github"},
                    "original": {"owner": "radrootslabs", "repo": "lib", "rev": revision, "type": "github"}
                }
            }
        });
        for node in ["crane", "parts", "pkgs", "rust", "fmt"] {
            lock["nodes"][node] = json!({"locked": {"type": "github", "rev": "b".repeat(40), "narHash": format!("sha256-{}=", "A".repeat(43))}});
        }
        let expression = format!(
            "{{\n inputs = {{\n crane.url = \"github:owner/crane/{}\";\n lib = {{\n url = \"github:radrootslabs/lib/{revision}\";\n inputs.crane.follows = \"crane\";\n }};\n }};\n outputs = {{...}}: {{}};\n}}",
            "b".repeat(40)
        );
        (expression, lock)
    }

    #[test]
    fn actual_service_helper_shape_binds_exact_inputs_without_qualification() {
        let (expression, lock) = fixture();
        let bytes = serde_json::to_vec(&lock).unwrap();
        assert!(validate_deferred_nix_material(expression.as_bytes(), &bytes).is_err());
        let evidence = validate_bound_nix_material(expression.as_bytes(), &bytes).unwrap();
        assert_eq!(evidence.revision, "a".repeat(40));
        assert_eq!(evidence.lock_sha256, hex::encode(Sha256::digest(&bytes)));
        let direct = expression.replace("lib = {", "inputs.lib = {");
        assert!(validate_bound_nix_material(direct.as_bytes(), &bytes).is_err());
    }

    #[test]
    fn helper_graph_identity_and_source_drift_are_refused() {
        let (expression, lock) = fixture();
        for (pointer, value) in [
            ("/nodes/lib/original/rev", json!("c".repeat(40))),
            ("/nodes/lib/unexpected", json!(true)),
            ("/nodes/lib/inputs/extra", json!("pkgs")),
            ("/nodes/lib/inputs/crane", json!(["other"])),
            ("/nodes/lib/inputs/nixpkgs", json!("missing")),
            ("/nodes/lib/inputs/nixpkgs", json!("lib")),
            ("/nodes/pkgs/locked/rev", json!("stable")),
            ("/nodes/pkgs/locked/narHash", json!("invalid")),
        ] {
            let mut changed = lock.clone();
            if pointer.ends_with("unexpected") {
                changed["nodes"]["lib"]["unexpected"] = value;
            } else if pointer.ends_with("extra") {
                changed["nodes"]["lib"]["inputs"]["extra"] = value;
            } else {
                *changed.pointer_mut(pointer).unwrap() = value;
            }
            assert!(
                validate_bound_nix_material(
                    expression.as_bytes(),
                    &serde_json::to_vec(&changed).unwrap()
                )
                .is_err(),
                "{pointer}"
            );
        }
        for source in [
            expression.replace(&"a".repeat(40), &"c".repeat(40)),
            expression.replace("inputs.crane.follows = \"crane\";", "flake = false;"),
            format!(
                "{expression}\nurl = \"github:radrootslabs/lib/{}\";",
                "a".repeat(40)
            ),
        ] {
            assert!(
                validate_bound_nix_material(source.as_bytes(), &serde_json::to_vec(&lock).unwrap())
                    .is_err()
            );
        }
        let duplicate = serde_json::to_string(&lock).unwrap().replacen(
            "\"version\":7",
            "\"version\":7,\"version\":7",
            1,
        );
        assert!(validate_bound_nix_lock(duplicate.as_bytes()).is_err());
    }
}
