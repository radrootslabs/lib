use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, validate_canonical_json_artifact,
    validate_sha256_artifact, with_artifact_bundle_transaction,
};
use radroots_event_codec::{
    RADROOTS_EVENT_CONTRACT_REGISTRY_V7_EVENT_COUNT,
    RADROOTS_EVENT_CONTRACT_REGISTRY_V7_INVENTORY_SCHEMA_VERSION,
    RADROOTS_EVENT_CONTRACT_REGISTRY_V7_KIND_COUNT, RADROOTS_EVENT_CONTRACT_REGISTRY_V7_VERSION,
    RadrootsEventContractRegistryV7Inventory, event_contract_registry_v7_inventory,
    event_contract_registry_v7_inventory_json, event_contract_registry_v7_inventory_sha256,
    parse_event_contract_registry_v7_inventory_json,
};
#[cfg(test)]
use std::fs;
use std::path::Path;

const INVENTORY_RELATIVE: &str = "contracts/event_store/event_contract_registry_v7.inventory.json";
const INVENTORY_SHA256_RELATIVE: &str =
    "contracts/event_store/event_contract_registry_v7.inventory.sha256";
const WRITE_COMMAND: &str = "cargo xtask contract event-contract-registry-v7 --write";

pub(crate) fn write_event_contract_registry_v7_inventory(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        let inventory_json = expected_inventory_json()?;
        let inventory_sha256 = expected_inventory_sha256()?;

        transaction.write(vec![
            GeneratedArtifact {
                relative: INVENTORY_RELATIVE,
                contents: inventory_json.into_bytes(),
            },
            GeneratedArtifact {
                relative: INVENTORY_SHA256_RELATIVE,
                contents: format!("{inventory_sha256}\n").into_bytes(),
            },
        ])?;

        validate_event_contract_registry_v7_inventory_under_lock(workspace_root)
    })
}

pub(crate) fn validate_event_contract_registry_v7_inventory(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_event_contract_registry_v7_inventory_under_lock(workspace_root)
    })
}

pub(super) fn validate_event_contract_registry_v7_inventory_under_lock(
    workspace_root: &Path,
) -> Result<(), String> {
    let expected_json = expected_inventory_json()?;
    let expected_sha256 = expected_inventory_sha256()?;
    let actual_json = read_regular_file(workspace_root, INVENTORY_RELATIVE)?;
    let actual_sha256 = read_regular_file(workspace_root, INVENTORY_SHA256_RELATIVE)?;
    let actual_json_text = std::str::from_utf8(&actual_json)
        .map_err(|error| format!("{INVENTORY_RELATIVE} must be UTF-8 JSON: {error}"))?;
    let parsed = parse_event_contract_registry_v7_inventory_json(actual_json_text)
        .map_err(|error| format!("parse {INVENTORY_RELATIVE}: {error}"))?;

    validate_inventory_shape(&parsed)?;
    validate_canonical_json_artifact(INVENTORY_RELATIVE, &actual_json)?;
    validate_sha256_artifact(INVENTORY_SHA256_RELATIVE, &actual_sha256)?;

    if actual_json != expected_json.as_bytes() {
        return Err(stale_error(INVENTORY_RELATIVE));
    }
    if actual_sha256 != format!("{expected_sha256}\n").as_bytes() {
        return Err(stale_error(INVENTORY_SHA256_RELATIVE));
    }
    if parsed != event_contract_registry_v7_inventory() {
        return Err(stale_error(INVENTORY_RELATIVE));
    }

    Ok(())
}

fn expected_inventory_json() -> Result<String, String> {
    event_contract_registry_v7_inventory_json()
        .map_err(|error| format!("serialize event-contract registry-v7 inventory: {error}"))
}

fn expected_inventory_sha256() -> Result<String, String> {
    event_contract_registry_v7_inventory_sha256()
        .map_err(|error| format!("hash event-contract registry-v7 inventory: {error}"))
}

fn validate_inventory_shape(
    inventory: &RadrootsEventContractRegistryV7Inventory,
) -> Result<(), String> {
    if inventory.schema_version != RADROOTS_EVENT_CONTRACT_REGISTRY_V7_INVENTORY_SCHEMA_VERSION {
        return Err(format!(
            "{INVENTORY_RELATIVE} schema_version must be {}",
            RADROOTS_EVENT_CONTRACT_REGISTRY_V7_INVENTORY_SCHEMA_VERSION
        ));
    }
    if inventory.event_contract_registry_version != RADROOTS_EVENT_CONTRACT_REGISTRY_V7_VERSION {
        return Err(format!(
            "{INVENTORY_RELATIVE} event_contract_registry_version must be {}",
            RADROOTS_EVENT_CONTRACT_REGISTRY_V7_VERSION
        ));
    }
    if inventory.kind_contracts.len() != RADROOTS_EVENT_CONTRACT_REGISTRY_V7_KIND_COUNT {
        return Err(format!(
            "{INVENTORY_RELATIVE} must contain {} kind contracts",
            RADROOTS_EVENT_CONTRACT_REGISTRY_V7_KIND_COUNT
        ));
    }
    if inventory.event_contracts.len() != RADROOTS_EVENT_CONTRACT_REGISTRY_V7_EVENT_COUNT {
        return Err(format!(
            "{INVENTORY_RELATIVE} must contain {} event contracts",
            RADROOTS_EVENT_CONTRACT_REGISTRY_V7_EVENT_COUNT
        ));
    }
    if inventory
        .kind_contracts
        .iter()
        .enumerate()
        .any(|(ordinal, contract)| contract.ordinal != ordinal)
    {
        return Err(format!(
            "{INVENTORY_RELATIVE} kind-contract ordinals must be contiguous from zero"
        ));
    }
    if inventory
        .event_contracts
        .iter()
        .enumerate()
        .any(|(ordinal, contract)| contract.ordinal != ordinal)
    {
        return Err(format!(
            "{INVENTORY_RELATIVE} event-contract ordinals must be contiguous from zero"
        ));
    }
    Ok(())
}

fn stale_error(relative: &str) -> String {
    format!("{relative} is stale; run `{WRITE_COMMAND}`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_validate_are_exactly_fresh() {
        let workspace = tempfile::TempDir::new().expect("workspace");

        write_event_contract_registry_v7_inventory(workspace.path()).expect("write inventory");
        validate_event_contract_registry_v7_inventory(workspace.path()).expect("fresh inventory");

        let json = fs::read(workspace.path().join(INVENTORY_RELATIVE)).expect("inventory JSON");
        let digest =
            fs::read(workspace.path().join(INVENTORY_SHA256_RELATIVE)).expect("inventory digest");
        assert_eq!(
            json,
            expected_inventory_json().expect("expected JSON").as_bytes()
        );
        assert_eq!(
            digest,
            format!(
                "{}\n",
                expected_inventory_sha256().expect("expected digest")
            )
            .as_bytes()
        );
    }

    #[test]
    fn validation_rejects_stale_and_noncanonical_artifacts() {
        let workspace = tempfile::TempDir::new().expect("workspace");
        write_event_contract_registry_v7_inventory(workspace.path()).expect("write inventory");

        fs::write(
            workspace.path().join(INVENTORY_SHA256_RELATIVE),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n",
        )
        .expect("write uppercase digest");
        let uppercase = validate_event_contract_registry_v7_inventory(workspace.path())
            .expect_err("uppercase digest must fail");
        assert!(uppercase.contains("lowercase SHA-256"));

        write_event_contract_registry_v7_inventory(workspace.path()).expect("restore inventory");
        let inventory_path = workspace.path().join(INVENTORY_RELATIVE);
        let mut json = fs::read_to_string(&inventory_path).expect("read inventory");
        json.push('\n');
        fs::write(&inventory_path, json).expect("write extra LF");
        let extra_lf = validate_event_contract_registry_v7_inventory(workspace.path())
            .expect_err("extra LF must fail");
        assert!(extra_lf.contains("exactly one LF"));
    }

    #[test]
    fn validation_rejects_unknown_typed_fields_before_freshness() {
        let workspace = tempfile::TempDir::new().expect("workspace");
        write_event_contract_registry_v7_inventory(workspace.path()).expect("write inventory");
        let inventory_path = workspace.path().join(INVENTORY_RELATIVE);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&inventory_path).expect("read inventory"))
                .expect("parse inventory value");
        value
            .as_object_mut()
            .expect("inventory object")
            .insert("unknown".to_string(), serde_json::Value::Bool(true));
        let mut json = serde_json::to_string_pretty(&value).expect("serialize mutated inventory");
        json.push('\n');
        fs::write(inventory_path, json).expect("write mutated inventory");

        let error = validate_event_contract_registry_v7_inventory(workspace.path())
            .expect_err("unknown field must fail");
        assert!(error.contains("unknown field"));
    }

    #[cfg(unix)]
    #[test]
    fn validation_and_write_reject_symlink_artifacts() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::TempDir::new().expect("workspace");
        let parent = workspace.path().join("contracts/event_store");
        fs::create_dir_all(&parent).expect("create artifact parent");
        let target = workspace.path().join("target.json");
        fs::write(&target, "{}\n").expect("write symlink target");
        symlink(&target, workspace.path().join(INVENTORY_RELATIVE))
            .expect("create artifact symlink");

        let validation = validate_event_contract_registry_v7_inventory(workspace.path())
            .expect_err("validation rejects symlink");
        assert!(validation.contains("symlink component"));
        let write = write_event_contract_registry_v7_inventory(workspace.path())
            .expect_err("write rejects symlink");
        assert!(write.contains("symlink component"));
    }
}
