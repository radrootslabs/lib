//! Prospective wire assignments, without authoring or qualification authority.

use radroots_protocol::profile;
use serde::Deserialize;
use std::path::Path;

const CONTRACT: &str = "contracts/architecture/decisions/shared_sdk_protocol.v1.json";
const MAX_BYTES: u64 = 8192;
const INVALID: &str = "shared protocol assignment contract is invalid";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    schema_version: u32,
    contract_id: String,
    state: String,
    profile_version: u32,
    commerce: Assignment,
    capabilities: Assignment,
    structured_authoring_qualified: bool,
    collision_review: CollisionReview,
    rules: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Assignment {
    kind: u16,
    class: String,
    profile: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollisionReview {
    reviewed_at: String,
    nips_repository: String,
    nips_revision: String,
    nips_readme_sha256: String,
    registry_repository: String,
    registry_revision: String,
    registry_schema_sha256: String,
    registry_numeric_entries: usize,
    nips_source_files_scanned: usize,
    known_collisions: Vec<u16>,
    coordination: String,
    proposal_search: String,
    limits: String,
}

pub(crate) fn validate_contract(root: &Path) -> Result<(), String> {
    let bytes = crate::safe_artifact_io::read_regular(root, Path::new(CONTRACT), MAX_BYTES)
        .map_err(|_| INVALID.to_string())?;
    validate_bytes(&bytes).map_err(str::to_string)
}

fn validate_bytes(bytes: &[u8]) -> Result<(), &'static str> {
    let shape: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
    if !shape.is_object()
        || ["commerce", "capabilities", "collision_review"]
            .iter()
            .any(|key| !shape[key].is_object())
    {
        return Err(INVALID);
    }
    // The independent typed pass retains duplicate, required-field and closure checks.
    let value: Decision = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
    let review = &value.collision_review;
    let exact = value.schema_version == 1
        && value.contract_id == "radroots.shared_sdk.protocol.v1"
        && value.state == "assigned_implementation_and_qualification_pending"
        && value.profile_version == profile::PROFILE_VERSION
        && value.commerce.kind == profile::COMMERCE_KIND
        && (1000..10000).contains(&value.commerce.kind)
        && value.commerce.class == "regular"
        && value.commerce.profile == profile::COMMERCE_PROFILE
        && value.capabilities.kind == profile::CAPABILITIES_KIND
        && (10000..20000).contains(&value.capabilities.kind)
        && value.capabilities.class == "replaceable"
        && value.capabilities.profile == profile::CAPABILITIES_PROFILE
        && !value.structured_authoring_qualified
        && review.reviewed_at == "2026-10-22"
        && review.nips_repository == "https://github.com/nostr-protocol/nips"
        && review.nips_revision == "d47a2ec8aacb9d01ac00e6294ae77582f483ff68"
        && review.nips_readme_sha256
            == "6db76d7a87f9f0997fe10f323d68fdde38fe5a2505b0679172703a9d6ce6c692"
        && review.registry_repository == "https://github.com/nostr-protocol/registry-of-kinds"
        && review.registry_revision == "2ae5c82dc82e0d1e62dc23f9895e5ef1d4a922de"
        && review.registry_schema_sha256
            == "560821b6dff52a4faf7b736aeca958011a0f7a3070aae7da717e47ba79e21445"
        && review.registry_numeric_entries == 281
        && review.nips_source_files_scanned == 101
        && review.known_collisions.is_empty()
        && !review.coordination.is_empty()
        && !review.proposal_search.is_empty()
        && !review.limits.is_empty()
        && value.rules.len() == 9
        && value.rules.iter().all(|rule| !rule.is_empty());
    if exact { Ok(()) } else { Err(INVALID) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const SOURCE: &[u8] =
        include_bytes!("../../../contracts/architecture/decisions/shared_sdk_protocol.v1.json");

    #[test]
    fn assignments_preserve_classes_profiles_and_unqualified_authoring() {
        validate_bytes(SOURCE).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        validate_contract(root).unwrap();
        for (pointer, replacement) in [
            ("/commerce/kind", json!(14)),
            ("/commerce/kind", json!(65536)),
            ("/commerce/kind", json!(18890)),
            ("/capabilities/kind", json!(38990)),
            ("/capabilities/class", json!("addressable")),
            ("/commerce/profile", json!("radroots.capabilities")),
            ("/profile_version", json!(2)),
            ("/structured_authoring_qualified", json!(true)),
            ("/collision_review/known_collisions", json!([8890])),
            ("/collision_review/registry_schema_sha256", json!("changed")),
            ("/rules", json!([])),
        ] {
            let mut value: Value = serde_json::from_slice(SOURCE).unwrap();
            *value.pointer_mut(pointer).unwrap() = replacement;
            assert_eq!(
                validate_bytes(&serde_json::to_vec(&value).unwrap()),
                Err(INVALID)
            );
        }
    }

    #[test]
    fn contract_rejects_duplicate_extra_missing_and_oversized_inputs() {
        for (pointer, keys) in [
            (
                "",
                &[
                    "schema_version",
                    "contract_id",
                    "state",
                    "profile_version",
                    "commerce",
                    "capabilities",
                    "structured_authoring_qualified",
                    "collision_review",
                    "rules",
                ][..],
            ),
            ("/commerce", &["kind", "class", "profile"][..]),
            ("/capabilities", &["kind", "class", "profile"][..]),
            (
                "/collision_review",
                &[
                    "reviewed_at",
                    "nips_repository",
                    "nips_revision",
                    "nips_readme_sha256",
                    "registry_repository",
                    "registry_revision",
                    "registry_schema_sha256",
                    "registry_numeric_entries",
                    "nips_source_files_scanned",
                    "known_collisions",
                    "coordination",
                    "proposal_search",
                    "limits",
                ][..],
            ),
        ] {
            let mut value: Value = serde_json::from_slice(SOURCE).unwrap();
            let array = Value::Array(
                keys.iter()
                    .map(|key| value.pointer(pointer).unwrap()[key].clone())
                    .collect(),
            );
            *value.pointer_mut(pointer).unwrap() = array;
            assert_eq!(
                validate_bytes(&serde_json::to_vec(&value).unwrap()),
                Err(INVALID)
            );
        }
        for bytes in [
            [b"{\"schema_version\":1,".as_slice(), &SOURCE[1..]].concat(),
            [b"{\"unexpected\":true,".as_slice(), &SOURCE[1..]].concat(),
            b"{}".to_vec(),
            b"[]".to_vec(),
        ] {
            assert_eq!(validate_bytes(&bytes), Err(INVALID));
        }
        let root = tempfile::tempdir().unwrap();
        assert!(validate_contract(root.path()).is_err());
        let path = root.path().join(CONTRACT);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut bytes = SOURCE.to_vec();
        bytes.resize(MAX_BYTES as usize + 1, b' ');
        std::fs::write(path, bytes).unwrap();
        assert!(validate_contract(root.path()).is_err());
    }
}
