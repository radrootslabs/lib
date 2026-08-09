use std::{collections::BTreeMap, fs, path::Path};

use serde::Deserialize;

const CONTRACT_RELATIVE: &str = "contracts/architecture/retired_compatibility.v1.toml";
const CONTRACT_ID: &str = "radroots.retired_compatibility.v1";
const SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetiredCompatibility {
    schema_version: u16,
    contract_id: String,
    status: String,
    retired_bridge: Vec<RetiredBridge>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetiredBridge {
    id: String,
    final_owners: Vec<String>,
    removal_step: u16,
}

pub(super) fn validate(workspace_root: &Path) -> Result<(), String> {
    let path = workspace_root.join(CONTRACT_RELATIVE);
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    validate_raw(&raw).map_err(|error| format!("{CONTRACT_RELATIVE}: {error}"))
}

fn validate_raw(raw: &str) -> Result<(), String> {
    let contract = toml::from_str::<RetiredCompatibility>(raw)
        .map_err(|error| format!("invalid TOML: {error}"))?;
    if contract.schema_version != SCHEMA_VERSION
        || contract.contract_id != CONTRACT_ID
        || contract.status != "enforced"
    {
        return Err("identity or lifecycle drifted".to_owned());
    }

    let mut actual = BTreeMap::new();
    for bridge in contract.retired_bridge {
        if bridge.id.trim().is_empty() || bridge.final_owners.is_empty() {
            return Err("bridge ids and final owners must not be empty".to_owned());
        }
        if bridge
            .final_owners
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(format!(
                "bridge {} final owners must be sorted and unique",
                bridge.id
            ));
        }
        if actual
            .insert(
                bridge.id.clone(),
                (bridge.final_owners, bridge.removal_step),
            )
            .is_some()
        {
            return Err(format!("bridge {} is duplicated", bridge.id));
        }
    }

    let expected = BTreeMap::from([
        (
            "nostrdb_runtime_adapter".to_owned(),
            (vec!["radroots_storage_sqlite".to_owned()], 301),
        ),
        (
            "radroots_authority".to_owned(),
            (vec!["radroots_signing".to_owned()], 313),
        ),
        (
            "radroots_geocoder".to_owned(),
            (vec!["radroots_geonames".to_owned()], 313),
        ),
        (
            "radroots_net".to_owned(),
            (vec!["radroots_transport".to_owned()], 301),
        ),
        (
            "radroots_nostr_connect_hidden_prelude".to_owned(),
            (vec!["radroots_nostr_connect".to_owned()], 313),
        ),
        (
            "radroots_nostr_connect_prefixed_client_bridge".to_owned(),
            (vec!["radroots_nostr_connect".to_owned()], 313),
        ),
        (
            "radroots_nostr_runtime".to_owned(),
            (vec!["radroots_transport_nostr".to_owned()], 301),
        ),
        (
            "radroots_nostr_signer".to_owned(),
            (
                vec![
                    "radroots_nostr_connect".to_owned(),
                    "radroots_signing".to_owned(),
                ],
                313,
            ),
        ),
    ]);
    if actual != expected {
        return Err("retired bridge inventory, final owners, or removal steps drifted".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_raw;

    const CONTRACT: &str =
        include_str!("../../../../contracts/architecture/retired_compatibility.v1.toml");

    #[test]
    fn current_retirement_contract_is_complete() {
        validate_raw(CONTRACT).expect("complete retirement contract");
    }

    #[test]
    fn malformed_or_incomplete_retirement_contract_fails_closed() {
        let malformed = validate_raw("not toml = [\n").expect_err("malformed contract must fail");
        assert!(malformed.contains("invalid TOML"));

        let geocoder = r#"[[retired_bridge]]
id = "radroots_geocoder"
final_owners = ["radroots_geonames"]
removal_step = 313

"#;
        let incomplete = validate_raw(&CONTRACT.replace(geocoder, ""))
            .expect_err("missing geocoder retirement must fail");
        assert!(incomplete.contains("inventory"));

        let no_prefixed_bridge = CONTRACT.replace(
            "radroots_nostr_connect_prefixed_client_bridge",
            "radroots_nostr_connect_unknown_bridge",
        );
        let prefixed = validate_raw(&no_prefixed_bridge)
            .expect_err("missing prefixed-client retirement must fail");
        assert!(prefixed.contains("inventory"));
    }
}
