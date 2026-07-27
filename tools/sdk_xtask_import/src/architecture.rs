use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

use serde::Deserialize;

const DEVIATIONS_RELATIVE: &str = "docs/implementation/deviations.toml";
const ARCHITECTURE_RELATIVE: &str = "docs/specs/radroots_crates_release_v1.toml";
const ARCHITECTURE_ID: &str = "radroots.crates.release.v1";
const PUBLIC_HOMEPAGE: &str = "https://radroots.org";
const PUBLIC_README: &str = "README.md";
const PUBLIC_AUTHORS: &[&str] = &["Tyson Lupul <tyson@radroots.org>"];

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
    initial_version: String,
    edition: String,
    resolver: String,
    rust_version: String,
    license: String,
    canonical_repositories: Vec<String>,
    repositories: BTreeMap<String, ArchitectureRepository>,
    package: Vec<ArchitecturePackage>,
}

#[derive(Debug, Deserialize)]
struct ArchitectureRepository {
    url: String,
    packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ArchitecturePackage {
    name: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceManifest {
    workspace: WorkspaceMembers,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMembershipManifest {
    workspace: WorkspaceMembership,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMembership {
    members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMembers {
    members: Vec<String>,
    resolver: String,
    package: WorkspacePackage,
    metadata: WorkspaceMetadata,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMetadata {
    radroots: RadrootsWorkspaceMetadata,
}

#[derive(Debug, Deserialize)]
struct RadrootsWorkspaceMetadata {
    #[serde(rename = "public-package")]
    public_package: PublicPackageMetadata,
}

#[derive(Debug, Deserialize)]
struct PublicPackageMetadata {
    version: String,
    authors: Vec<String>,
    readme: String,
}

#[derive(Debug, Deserialize)]
struct WorkspacePackage {
    version: String,
    edition: String,
    #[serde(rename = "rust-version")]
    rust_version: String,
    license: String,
    repository: String,
    homepage: String,
    readme: String,
    authors: Vec<String>,
}

pub fn validate(workspace_root: &Path) -> Result<(), String> {
    validate_workspace_members(workspace_root)?;
    let architecture_path = workspace_root.join(ARCHITECTURE_RELATIVE);
    let architecture_raw = fs::read_to_string(&architecture_path)
        .map_err(|error| format!("read {}: {error}", architecture_path.display()))?;
    let architecture = toml::from_str::<ArchitectureIdentity>(&architecture_raw)
        .map_err(|error| format!("parse {}: {error}", architecture_path.display()))?;

    validate_workspace_toolchain(workspace_root, &architecture)?;
    validate_public_package_metadata(workspace_root, &architecture)?;

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
    let workspace_package = &manifest.workspace.package;
    let public_metadata = &manifest.workspace.metadata.radroots.public_package;
    if public_metadata.version != architecture.initial_version
        || architecture.initial_version != "0.1.0"
    {
        return Err(format!(
            "public package version source {} must match architecture initial_version {}",
            public_metadata.version, architecture.initial_version
        ));
    }
    if public_metadata.authors != PUBLIC_AUTHORS {
        return Err("public package authors source must match the workspace convention".to_owned());
    }
    if public_metadata.readme != PUBLIC_README {
        return Err(format!(
            "public package readme source must be {PUBLIC_README}"
        ));
    }
    if workspace_package.edition != architecture.edition || architecture.edition != "2024" {
        return Err(format!(
            "workspace edition {} must match architecture edition {}",
            workspace_package.edition, architecture.edition
        ));
    }
    if workspace_package.license != architecture.license {
        return Err(format!(
            "workspace license {} must match architecture license {}",
            workspace_package.license, architecture.license
        ));
    }
    if !architecture
        .canonical_repositories
        .contains(&workspace_package.repository)
    {
        return Err(format!(
            "workspace repository {} is not canonical",
            workspace_package.repository
        ));
    }
    if workspace_package.homepage != PUBLIC_HOMEPAGE {
        return Err(format!(
            "workspace homepage {} must be {PUBLIC_HOMEPAGE}",
            workspace_package.homepage
        ));
    }
    if workspace_package.authors != PUBLIC_AUTHORS {
        return Err("workspace authors must match the public package convention".to_owned());
    }
    if !workspace_root.join(&workspace_package.readme).is_file() {
        return Err(format!(
            "workspace readme {} does not exist",
            workspace_package.readme
        ));
    }
    for license in ["LICENSE-MIT", "LICENSE-APACHE"] {
        if !workspace_root.join(license).is_file() {
            return Err(format!("workspace is missing {license}"));
        }
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

fn validate_public_package_metadata(
    workspace_root: &Path,
    architecture: &ArchitectureIdentity,
) -> Result<(), String> {
    let workspace_raw = fs::read_to_string(workspace_root.join("Cargo.toml"))
        .map_err(|error| format!("read workspace Cargo.toml: {error}"))?;
    let workspace = toml::from_str::<WorkspaceManifest>(&workspace_raw)
        .map_err(|error| format!("parse workspace Cargo.toml: {error}"))?;
    let repository = architecture
        .repositories
        .values()
        .find(|repository| repository.url == workspace.workspace.package.repository)
        .ok_or_else(|| "workspace repository has no architecture allocation".to_owned())?;
    let local_packages = repository.packages.iter().collect::<BTreeSet<_>>();
    let all_packages = architecture
        .package
        .iter()
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();

    for member in &workspace.workspace.members {
        let manifest_path = workspace_root.join(member).join("Cargo.toml");
        let raw = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
        let manifest = raw
            .parse::<toml::Value>()
            .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
        let package = manifest
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("{} is missing [package]", manifest_path.display()))?;
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{} is missing package.name", manifest_path.display()))?;
        if !all_packages.contains(name) {
            continue;
        }
        if !local_packages.contains(&name.to_owned()) {
            return Err(format!(
                "public package {name} belongs to a different canonical repository"
            ));
        }
        validate_public_manifest_field(
            package,
            "version",
            &workspace.workspace.package.version,
            &architecture.initial_version,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "edition",
            &workspace.workspace.package.edition,
            &architecture.edition,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "rust-version",
            &workspace.workspace.package.rust_version,
            &architecture.rust_version,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "license",
            &workspace.workspace.package.license,
            &architecture.license,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "repository",
            &workspace.workspace.package.repository,
            &repository.url,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "homepage",
            &workspace.workspace.package.homepage,
            PUBLIC_HOMEPAGE,
            name,
        )?;
        let authors = resolve_public_authors(package, &workspace.workspace.package.authors)
            .ok_or_else(|| format!("public package {name} must declare authors"))?;
        if authors != PUBLIC_AUTHORS {
            return Err(format!(
                "public package {name} authors must match the workspace convention"
            ));
        }
        let readme = package.get("readme").and_then(toml::Value::as_str);
        if readme != Some(PUBLIC_README)
            || !workspace_root.join(member).join(PUBLIC_README).is_file()
        {
            return Err(format!(
                "public package {name} must use an existing crate-local {PUBLIC_README}"
            ));
        }
        if package.get("publish").and_then(toml::Value::as_bool) != Some(false) {
            return Err(format!(
                "public package {name} must remain publish = false during migration"
            ));
        }
    }
    Ok(())
}

fn validate_public_manifest_field(
    package: &toml::value::Table,
    key: &str,
    workspace_value: &str,
    expected: &str,
    package_name: &str,
) -> Result<(), String> {
    let value = resolve_public_string(package, key, workspace_value)
        .ok_or_else(|| format!("public package {package_name} must declare {key}"))?;
    if value != expected {
        return Err(format!(
            "public package {package_name} {key} {value} must be {expected}"
        ));
    }
    Ok(())
}

fn resolve_public_string<'a>(
    package: &'a toml::value::Table,
    key: &str,
    workspace_value: &'a str,
) -> Option<&'a str> {
    match package.get(key)? {
        toml::Value::String(value) => Some(value),
        toml::Value::Table(value)
            if value.get("workspace").and_then(toml::Value::as_bool) == Some(true) =>
        {
            Some(workspace_value)
        }
        _ => None,
    }
}

fn resolve_public_authors<'a>(
    package: &'a toml::value::Table,
    workspace_authors: &'a [String],
) -> Option<Vec<&'a str>> {
    match package.get("authors")? {
        toml::Value::Array(values) => values.iter().map(toml::Value::as_str).collect(),
        toml::Value::Table(value)
            if value.get("workspace").and_then(toml::Value::as_bool) == Some(true) =>
        {
            Some(workspace_authors.iter().map(String::as_str).collect())
        }
        _ => None,
    }
}

fn validate_workspace_members(workspace_root: &Path) -> Result<(), String> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let manifest = toml::from_str::<WorkspaceMembershipManifest>(&manifest_raw)
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
        collections::BTreeMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        ArchitectureIdentity, ArchitecturePackage, ArchitectureRepository, validate_ledger,
        validate_public_package_metadata, validate_workspace_members, validate_workspace_toolchain,
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

    fn architecture() -> ArchitectureIdentity {
        ArchitectureIdentity {
            spec_id: "radroots.crates.release.v1".to_string(),
            initial_version: "0.1.0".to_string(),
            edition: "2024".to_string(),
            resolver: "3".to_string(),
            rust_version: "1.97.1".to_string(),
            license: "MIT OR Apache-2.0".to_string(),
            canonical_repositories: vec!["https://github.com/radrootslabs/sdk".to_string()],
            repositories: BTreeMap::from([(
                "sdk".to_string(),
                ArchitectureRepository {
                    url: "https://github.com/radrootslabs/sdk".to_string(),
                    packages: vec!["radroots".to_string()],
                },
            )]),
            package: vec![ArchitecturePackage {
                name: "radroots".to_string(),
            }],
        }
    }

    fn complete_workspace_manifest(members: &str) -> String {
        format!(
            "[workspace]\nmembers = [{members}]\nresolver = \"3\"\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.97.1\"\nlicense = \"MIT OR Apache-2.0\"\nrepository = \"https://github.com/radrootslabs/sdk\"\nhomepage = \"https://radroots.org\"\nreadme = \"README\"\nauthors = [\"Tyson Lupul <tyson@radroots.org>\"]\n\n[workspace.metadata.radroots.public-package]\nversion = \"0.1.0\"\nauthors = [\"Tyson Lupul <tyson@radroots.org>\"]\nreadme = \"README.md\"\n"
        )
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
        fs::write(root.join("Cargo.toml"), complete_workspace_manifest(""))
            .expect("write workspace manifest");
        for path in ["README", "LICENSE-MIT", "LICENSE-APACHE"] {
            fs::write(root.join(path), "fixture\n").expect("write workspace metadata file");
        }
        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.97.1\"\n",
        )
        .expect("write toolchain");
        let architecture = architecture();
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

    #[test]
    fn public_package_metadata_requires_canonical_inheritance() {
        let root = test_root("public_package_metadata");
        fs::create_dir_all(root.join("crates/radroots")).expect("create public package");
        fs::write(
            root.join("Cargo.toml"),
            complete_workspace_manifest("\"crates/radroots\""),
        )
        .expect("write workspace manifest");
        let manifest = "[package]\nname = \"radroots\"\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\nlicense.workspace = true\nrepository.workspace = true\nhomepage.workspace = true\nauthors.workspace = true\nreadme = \"README.md\"\npublish = false\n";
        fs::write(root.join("crates/radroots/Cargo.toml"), manifest)
            .expect("write public manifest");
        fs::write(root.join("crates/radroots/README.md"), "fixture\n")
            .expect("write public readme");
        validate_public_package_metadata(&root, &architecture()).expect("canonical metadata");

        fs::write(
            root.join("crates/radroots/Cargo.toml"),
            manifest.replace("authors.workspace = true\n", ""),
        )
        .expect("write incomplete public manifest");
        let error = validate_public_package_metadata(&root, &architecture())
            .expect_err("missing authors must fail");
        assert!(error.contains("must declare authors"));
        let _ = fs::remove_dir_all(root);
    }
}
