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
    resolver: String,
    rust_version: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceManifest {
    workspace: WorkspaceMembers,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMembers {
    members: Vec<String>,
    resolver: String,
    package: WorkspacePackage,
}

#[derive(Debug, Deserialize)]
struct WorkspacePackage {
    #[serde(rename = "rust-version")]
    rust_version: String,
}

pub fn validate(workspace_root: &Path) -> Result<(), String> {
    validate_workspace_members(workspace_root)?;
    let architecture_path = workspace_root.join(ARCHITECTURE_RELATIVE);
    let architecture_raw = fs::read_to_string(&architecture_path)
        .map_err(|error| format!("read {}: {error}", architecture_path.display()))?;
    let architecture = toml::from_str::<ArchitectureIdentity>(&architecture_raw)
        .map_err(|error| format!("parse {}: {error}", architecture_path.display()))?;

    validate_workspace_toolchain(workspace_root, &architecture)?;

    let ledger_path = workspace_root.join(DEVIATIONS_RELATIVE);
    let ledger_raw = fs::read_to_string(&ledger_path)
        .map_err(|error| format!("read {}: {error}", ledger_path.display()))?;
    validate_ledger(workspace_root, &architecture.spec_id, &ledger_raw)
}

fn validate_workspace_toolchain(
    workspace_root: &Path,
    architecture: &ArchitectureIdentity,
) -> Result<(), String> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let manifest = toml::from_str::<WorkspaceManifest>(&manifest_raw)
        .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
    if manifest.workspace.resolver != architecture.resolver || architecture.resolver != "3" {
        return Err(format!(
            "workspace resolver {} must match architecture resolver {}",
            manifest.workspace.resolver, architecture.resolver
        ));
    }
    if manifest.workspace.package.rust_version != architecture.rust_version
        || architecture.rust_version != "1.97.1"
    {
        return Err(format!(
            "workspace rust-version {} must match architecture rust_version {}",
            manifest.workspace.package.rust_version, architecture.rust_version
        ));
    }
    let toolchain_path = workspace_root.join("rust-toolchain.toml");
    let toolchain_raw = fs::read_to_string(&toolchain_path)
        .map_err(|error| format!("read {}: {error}", toolchain_path.display()))?;
    let toolchain = toolchain_raw
        .parse::<toml::Value>()
        .map_err(|error| format!("parse {}: {error}", toolchain_path.display()))?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|value| value.get("channel"))
        .and_then(toml::Value::as_str);
    if channel != Some(architecture.rust_version.as_str()) {
        return Err(format!(
            "rust-toolchain.toml channel must match architecture rust_version {}",
            architecture.rust_version
        ));
    }
    Ok(())
}

fn validate_workspace_members(workspace_root: &Path) -> Result<(), String> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let manifest = toml::from_str::<WorkspaceManifest>(&manifest_raw)
        .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
    let declared = manifest
        .workspace
        .members
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut discovered = BTreeSet::new();
    for root in ["crates", "tools"] {
        let directory = workspace_root.join(root);
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?
        {
            let entry =
                entry.map_err(|error| format!("read {} entry: {error}", directory.display()))?;
            if entry
                .file_type()
                .map_err(|error| format!("read {} type: {error}", entry.path().display()))?
                .is_dir()
                && entry.path().join("Cargo.toml").is_file()
            {
                discovered.insert(format!("{root}/{}", entry.file_name().to_string_lossy()));
            }
        }
    }
    if declared != discovered {
        let missing = discovered
            .difference(&declared)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let unknown = declared
            .difference(&discovered)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "workspace membership is missing local package roots: {missing}; workspace membership has unknown package roots: {unknown}"
        ));
    }
    Ok(())
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

    use super::{
        ArchitectureIdentity, validate_ledger, validate_workspace_members,
        validate_workspace_toolchain,
    };

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

    #[test]
    fn workspace_membership_requires_every_local_package_root() {
        let root = test_root("workspace_members");
        for path in ["crates/a", "tools/xtask"] {
            fs::create_dir_all(root.join(path)).expect("create package root");
            fs::write(
                root.join(path).join("Cargo.toml"),
                "[package]\nname = \"fixture\"\n",
            )
            .expect("write package manifest");
        }
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a\", \"tools/xtask\"]\nresolver = \"3\"\n\n[workspace.package]\nrust-version = \"1.97.1\"\n",
        )
        .expect("write complete workspace");
        validate_workspace_members(&root).expect("complete membership");

        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"tools/xtask\"]\nresolver = \"3\"\n\n[workspace.package]\nrust-version = \"1.97.1\"\n",
        )
        .expect("write incomplete workspace");
        let error = validate_workspace_members(&root).expect_err("missing member must fail");
        assert!(error.contains("missing local package roots: crates/a"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_toolchain_requires_exact_resolver_and_rust_version() {
        let root = test_root("workspace_toolchain");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = []\nresolver = \"3\"\n\n[workspace.package]\nrust-version = \"1.97.1\"\n",
        )
        .expect("write workspace manifest");
        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.97.1\"\n",
        )
        .expect("write toolchain");
        let architecture = ArchitectureIdentity {
            spec_id: "radroots.crates.release.v1".to_string(),
            resolver: "3".to_string(),
            rust_version: "1.97.1".to_string(),
        };
        validate_workspace_toolchain(&root, &architecture).expect("exact toolchain policy");

        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.97.0\"\n",
        )
        .expect("write mismatched toolchain");
        let error = validate_workspace_toolchain(&root, &architecture)
            .expect_err("mismatched toolchain must fail");
        assert!(error.contains("channel must match"));
        let _ = fs::remove_dir_all(root);
    }
}
