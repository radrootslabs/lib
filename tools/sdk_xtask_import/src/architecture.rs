use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path},
};

use serde::Deserialize;

const DEVIATIONS_RELATIVE: &str = "docs/implementation/deviations.toml";
const ARCHITECTURE_RELATIVE: &str = "docs/specs/radroots_crates_release_v1.toml";
const ARCHITECTURE_ID: &str = "radroots.crates.release.v1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviationLedger {
    schema_version: u16,
    architecture_id: String,
    deviation: Vec<DeviationRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviationRecord {
    id: String,
    date: String,
    status: String,
    approval: String,
    affected_steps: Vec<String>,
    spec_anchors: Vec<String>,
    source_evidence: Vec<String>,
    replacement_action: String,
    verification: Vec<String>,
    unresolved_risk: String,
    normative_architecture_change: bool,
    adr_required: bool,
    #[serde(default)]
    closure_evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ArchitectureIdentity {
    spec_id: String,
}

pub fn validate(workspace_root: &Path) -> Result<(), String> {
    let architecture_path = workspace_root.join(ARCHITECTURE_RELATIVE);
    let architecture_raw = fs::read_to_string(&architecture_path)
        .map_err(|error| format!("read {}: {error}", architecture_path.display()))?;
    let architecture = toml::from_str::<ArchitectureIdentity>(&architecture_raw)
        .map_err(|error| format!("parse {}: {error}", architecture_path.display()))?;

    let ledger_path = workspace_root.join(DEVIATIONS_RELATIVE);
    let ledger_raw = fs::read_to_string(&ledger_path)
        .map_err(|error| format!("read {}: {error}", ledger_path.display()))?;
    validate_ledger(workspace_root, &architecture.spec_id, &ledger_raw)
}

fn validate_ledger(
    workspace_root: &Path,
    expected_architecture_id: &str,
    raw: &str,
) -> Result<(), String> {
    let ledger = toml::from_str::<DeviationLedger>(raw)
        .map_err(|error| format!("parse {DEVIATIONS_RELATIVE}: {error}"))?;
    if ledger.schema_version != 1 {
        return Err("deviation ledger schema_version must be 1".to_owned());
    }
    if ledger.architecture_id != expected_architecture_id
        || ledger.architecture_id != ARCHITECTURE_ID
    {
        return Err(format!(
            "deviation ledger architecture_id {} must match {}",
            ledger.architecture_id, expected_architecture_id
        ));
    }

    let mut ids = BTreeSet::new();
    for record in &ledger.deviation {
        validate_record(workspace_root, record)?;
        if !ids.insert(record.id.as_str()) {
            return Err(format!("duplicate deviation id {}", record.id));
        }
    }
    Ok(())
}

fn validate_record(workspace_root: &Path, record: &DeviationRecord) -> Result<(), String> {
    if !is_deviation_id(&record.id) {
        return Err(format!("deviation id {} must use RCRV1-DEV-NNN", record.id));
    }
    if !is_iso_date(&record.date) {
        return Err(format!("deviation {} date must use YYYY-MM-DD", record.id));
    }
    if !matches!(record.status.as_str(), "active" | "closed" | "superseded") {
        return Err(format!(
            "deviation {} status must be active, closed, or superseded",
            record.id
        ));
    }
    require_text(&record.id, "approval", &record.approval)?;
    require_text(&record.id, "replacement_action", &record.replacement_action)?;
    require_text(&record.id, "unresolved_risk", &record.unresolved_risk)?;
    require_nonempty_list(&record.id, "affected_steps", &record.affected_steps)?;
    require_nonempty_list(&record.id, "spec_anchors", &record.spec_anchors)?;
    require_nonempty_list(&record.id, "source_evidence", &record.source_evidence)?;
    require_nonempty_list(&record.id, "verification", &record.verification)?;

    for step in &record.affected_steps {
        let valid = step.len() == 3
            && step.bytes().all(|byte| byte.is_ascii_digit())
            && step
                .parse::<u16>()
                .is_ok_and(|value| (1..=315).contains(&value));
        if !valid {
            return Err(format!(
                "deviation {} affected step {} must be in 001..315",
                record.id, step
            ));
        }
    }
    for anchor in &record.spec_anchors {
        validate_spec_anchor(workspace_root, &record.id, anchor)?;
    }
    if record.status == "active" && !record.closure_evidence.is_empty() {
        return Err(format!(
            "active deviation {} must not carry closure_evidence",
            record.id
        ));
    }
    if record.status != "active" {
        require_nonempty_list(&record.id, "closure_evidence", &record.closure_evidence)?;
    }

    let _ = (record.normative_architecture_change, record.adr_required);
    Ok(())
}

fn validate_spec_anchor(
    workspace_root: &Path,
    deviation_id: &str,
    anchor: &str,
) -> Result<(), String> {
    let (relative, fragment) = anchor
        .split_once('#')
        .map_or((anchor, None), |(path, fragment)| (path, Some(fragment)));
    if relative.trim().is_empty() || fragment.is_some_and(|value| value.trim().is_empty()) {
        return Err(format!(
            "deviation {deviation_id} has invalid spec anchor {anchor}"
        ));
    }
    let path = Path::new(relative);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "deviation {deviation_id} spec anchor must be repository-relative: {anchor}"
        ));
    }
    if !relative.starts_with("docs/specs/") || !workspace_root.join(path).is_file() {
        return Err(format!(
            "deviation {deviation_id} spec anchor does not resolve to a local spec: {anchor}"
        ));
    }
    Ok(())
}

fn require_text(deviation_id: &str, field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!(
            "deviation {deviation_id} field {field} must not be empty"
        ));
    }
    Ok(())
}

fn require_nonempty_list(deviation_id: &str, field: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return Err(format!(
            "deviation {deviation_id} field {field} must contain non-empty values"
        ));
    }
    Ok(())
}

fn is_deviation_id(value: &str) -> bool {
    value
        .strip_prefix("RCRV1-DEV-")
        .is_some_and(|suffix| suffix.len() == 3 && suffix.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_iso_date(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::validate_ledger;

    fn test_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("radroots_architecture_{label}_{nonce}"));
        fs::create_dir_all(root.join("docs/specs")).expect("create spec root");
        fs::write(
            root.join("docs/specs/radroots_crates_release_v1.md"),
            "# Architecture\n",
        )
        .expect("write spec");
        root
    }

    fn complete_ledger() -> &'static str {
        r#"schema_version = 1
architecture_id = "radroots.crates.release.v1"

[[deviation]]
id = "RCRV1-DEV-001"
date = "2026-07-27"
status = "active"
approval = "Explicit user correction dated 2026-07-27."
affected_steps = ["015", "016"]
spec_anchors = ["docs/specs/radroots_crates_release_v1.md#repository-topology"]
source_evidence = ["The approved architecture assigns packages to the existing lib and sdk repositories."]
replacement_action = "Keep both standalone repositories and verify them independently."
verification = ["Repository-local architecture validation passes."]
unresolved_risk = "Remote publication remains separately authorized."
normative_architecture_change = false
adr_required = false
"#
    }

    #[test]
    fn accepts_complete_active_deviation() {
        let root = test_root("complete");
        validate_ledger(&root, "radroots.crates.release.v1", complete_ledger())
            .expect("complete deviation");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_incomplete_active_deviation() {
        let root = test_root("incomplete");
        let incomplete = complete_ledger().replace(
            "spec_anchors = [\"docs/specs/radroots_crates_release_v1.md#repository-topology\"]",
            "spec_anchors = []",
        );
        let error = validate_ledger(&root, "radroots.crates.release.v1", &incomplete)
            .expect_err("missing anchor must fail");
        assert!(error.contains("field spec_anchors must contain non-empty values"));
        let _ = fs::remove_dir_all(root);
    }
}
