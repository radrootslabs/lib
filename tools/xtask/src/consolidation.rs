use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

use serde::Deserialize;

const BASELINE_RELATIVE: &str = "contracts/consolidation/baseline.v1.toml";
const STEP_MAP_RELATIVE: &str = "contracts/consolidation/handoff_steps.v1.toml";
const BASELINE_ID: &str = "radroots.rust.consolidation.baseline.v1";
const STEP_MAP_ID: &str = "radroots.rust.consolidation.handoff-steps.v1";
const ARCHITECTURE_TARGET: &str = "radroots.crates.release.v2";
const PACKAGE_VERSION: &str = "0.1.0-alpha";
const EXPECTED_PUBLIC_PACKAGES: u16 = 19;
const EXPECTED_HANDOFF_STEPS: u16 = 275;

const RCLD_OWNERS: &[&str] = &[
    "rcld-rlc-010",
    "rcld-rlc-020",
    "rcld-rlc-030",
    "rcld-rlc-040",
    "rcld-rlc-050",
    "rcld-rlc-060",
    "rcld-rlc-070",
    "rcld-rlc-080",
    "rcld-rlc-090",
    "rcld-rlc-100",
    "rcld-rlc-110",
    "rcld-rlc-120",
    "rcld-rlc-130",
    "rcld-rlc-140",
    "rcld-rlc-150",
    "rcld-rlc-160",
    "rcld-rlc-170",
    "rcld-rlc-180",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Baseline {
    schema_version: u16,
    baseline_id: String,
    architecture_target: String,
    captured_date: String,
    public_package_version: String,
    expected_public_packages: u16,
    expected_handoff_steps: u16,
    repository: Vec<Repository>,
    surface: Vec<Surface>,
    consumer: Vec<Consumer>,
    additional_source: Vec<AdditionalSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Repository {
    id: String,
    canonical_url: String,
    commit: String,
    tree: String,
    branch: String,
    clean: bool,
    origin_synchronized: bool,
    worktree_count: u16,
    rust_version: String,
    resolver: String,
    package_version: String,
    commands: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Surface {
    repository: String,
    path: String,
    tree: String,
    authority: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Consumer {
    id: String,
    repository: String,
    current_source: String,
    current_revision: String,
    target_source: String,
    target_acquisition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdditionalSource {
    id: String,
    commit: String,
    tree: String,
    license: String,
    disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepMap {
    schema_version: u16,
    map_id: String,
    source_step_count: u16,
    source_sequence: String,
    range: Vec<StepRange>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepRange {
    start: u16,
    end: u16,
    owners: Vec<String>,
    disposition: String,
    reason: String,
}

pub fn validate(workspace_root: &Path) -> Result<(), String> {
    let baseline = read_toml::<Baseline>(workspace_root, BASELINE_RELATIVE)?;
    let step_map = read_toml::<StepMap>(workspace_root, STEP_MAP_RELATIVE)?;
    validate_baseline(&baseline)?;
    validate_step_map(&step_map)
}

fn read_toml<T: for<'de> Deserialize<'de>>(
    workspace_root: &Path,
    relative: &str,
) -> Result<T, String> {
    let path = workspace_root.join(relative);
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn validate_baseline(baseline: &Baseline) -> Result<(), String> {
    if baseline.schema_version != 1
        || baseline.baseline_id != BASELINE_ID
        || baseline.architecture_target != ARCHITECTURE_TARGET
        || baseline.public_package_version != PACKAGE_VERSION
        || baseline.expected_public_packages != EXPECTED_PUBLIC_PACKAGES
        || baseline.expected_handoff_steps != EXPECTED_HANDOFF_STEPS
    {
        return Err("consolidation baseline identity or cardinality drifted".to_owned());
    }
    validate_date(&baseline.captured_date)?;

    let expected_repositories = BTreeSet::from(["app_rt", "lib", "sdk", "studio_app"]);
    let mut repositories = BTreeMap::new();
    for repository in &baseline.repository {
        if repositories
            .insert(repository.id.as_str(), repository)
            .is_some()
        {
            return Err(format!("duplicate repository id {}", repository.id));
        }
        validate_identifier(&repository.id, "repository id")?;
        if repository.canonical_url != format!("https://github.com/radrootslabs/{}", repository.id)
        {
            return Err(format!(
                "repository {} has a noncanonical URL",
                repository.id
            ));
        }
        validate_oid(&repository.commit, "repository commit")?;
        validate_oid(&repository.tree, "repository tree")?;
        if repository.branch != "master"
            || !repository.clean
            || !repository.origin_synchronized
            || repository.worktree_count != 1
            || repository.rust_version != "1.97.1"
            || !matches!(repository.resolver.as_str(), "2" | "3")
            || repository.commands.is_empty()
        {
            return Err(format!(
                "repository {} baseline is not frozen",
                repository.id
            ));
        }
        if repository
            .commands
            .iter()
            .any(|command| command.trim().is_empty())
        {
            return Err(format!("repository {} has an empty command", repository.id));
        }
    }
    if repositories.keys().copied().collect::<BTreeSet<_>>() != expected_repositories {
        return Err(
            "consolidation baseline must contain exactly lib, sdk, app_rt, and studio_app"
                .to_owned(),
        );
    }
    if repositories["lib"].resolver != "3"
        || repositories["sdk"].resolver != "3"
        || repositories["studio_app"].resolver != "3"
        || repositories["app_rt"].resolver != "2"
    {
        return Err("reviewed resolver baseline drifted".to_owned());
    }
    if repositories["lib"].package_version != PACKAGE_VERSION
        || repositories["sdk"].package_version != PACKAGE_VERSION
        || repositories["studio_app"].package_version != PACKAGE_VERSION
        || repositories["app_rt"].package_version != "0.1.0-alpha.1"
    {
        return Err("reviewed package version baseline drifted".to_owned());
    }

    let mut surfaces = BTreeSet::new();
    let mut authorities = BTreeSet::new();
    for surface in &baseline.surface {
        if !repositories.contains_key(surface.repository.as_str()) {
            return Err(format!("unknown surface repository {}", surface.repository));
        }
        validate_relative_path(&surface.path)?;
        validate_oid(&surface.tree, "surface tree")?;
        validate_identifier(&surface.authority, "surface authority")?;
        if !surfaces.insert((surface.repository.as_str(), surface.path.as_str())) {
            return Err(format!(
                "duplicate surface {}/{}",
                surface.repository, surface.path
            ));
        }
        if !authorities.insert(surface.authority.as_str()) {
            return Err(format!("duplicate surface authority {}", surface.authority));
        }
    }
    for repository in expected_repositories {
        if !surfaces
            .iter()
            .any(|(candidate, _)| *candidate == repository)
        {
            return Err(format!(
                "repository {repository} has no compatibility surface"
            ));
        }
    }

    let expected_consumers = BTreeSet::from([
        "cli",
        "integration_parent",
        "ios_app",
        "mobile_runtime",
        "myc",
        "radrootsd",
        "rhi",
        "sdk_product",
        "studio_product",
        "event_indexer",
    ]);
    let mut consumers = BTreeSet::new();
    for consumer in &baseline.consumer {
        validate_identifier(&consumer.id, "consumer id")?;
        if !consumers.insert(consumer.id.as_str()) {
            return Err(format!("duplicate consumer id {}", consumer.id));
        }
        if consumer.repository.trim().is_empty()
            || consumer.current_source.trim().is_empty()
            || consumer.target_source != "lib"
        {
            return Err(format!("consumer {} has incomplete ownership", consumer.id));
        }
        validate_oid(&consumer.current_revision, "consumer revision")?;
        if !matches!(
            consumer.target_acquisition.as_str(),
            "exact_git_rev" | "exact_registered_gitlink"
        ) {
            return Err(format!(
                "consumer {} has invalid target acquisition",
                consumer.id
            ));
        }
    }
    if consumers != expected_consumers {
        return Err("consumer census is incomplete".to_owned());
    }

    if baseline.additional_source.len() != 1 {
        return Err("exactly one additional Studio source is required".to_owned());
    }
    let additional = &baseline.additional_source[0];
    validate_identifier(&additional.id, "additional source id")?;
    validate_oid(&additional.commit, "additional source commit")?;
    validate_oid(&additional.tree, "additional source tree")?;
    if additional.id != "studio_mpl_legacy_core"
        || additional.license != "MPL-2.0"
        || additional.disposition != "import_unique_behavior_then_zero_logic_capsule"
    {
        return Err("additional Studio source disposition drifted".to_owned());
    }
    Ok(())
}

fn validate_step_map(step_map: &StepMap) -> Result<(), String> {
    if step_map.schema_version != 1
        || step_map.map_id != STEP_MAP_ID
        || step_map.source_step_count != EXPECTED_HANDOFF_STEPS
        || !step_map
            .source_sequence
            .ends_with("implementation/COMMIT_SEQUENCE.md")
    {
        return Err("handoff step map identity drifted".to_owned());
    }
    let allowed_owners = RCLD_OWNERS.iter().copied().collect::<BTreeSet<_>>();
    let allowed_dispositions = BTreeSet::from(["already_satisfied", "execute", "reassigned"]);
    let mut next = 1_u16;
    for range in &step_map.range {
        if range.start != next || range.end < range.start || range.end > EXPECTED_HANDOFF_STEPS {
            return Err(format!(
                "handoff step range {}-{} is overlapping, gapped, or invalid; expected {}",
                range.start, range.end, next
            ));
        }
        if range.owners.is_empty()
            || range
                .owners
                .iter()
                .any(|owner| !allowed_owners.contains(owner.as_str()))
        {
            return Err(format!(
                "handoff step range {}-{} has an invalid owner",
                range.start, range.end
            ));
        }
        if !allowed_dispositions.contains(range.disposition.as_str())
            || range.reason.trim().is_empty()
        {
            return Err(format!(
                "handoff step range {}-{} has an invalid disposition",
                range.start, range.end
            ));
        }
        next = range
            .end
            .checked_add(1)
            .ok_or_else(|| "handoff step range overflow".to_owned())?;
    }
    if next != EXPECTED_HANDOFF_STEPS + 1 {
        return Err(format!(
            "handoff step map ends at {}, expected {}",
            next.saturating_sub(1),
            EXPECTED_HANDOFF_STEPS
        ));
    }
    Ok(())
}

fn validate_oid(value: &str, context: &str) -> Result<(), String> {
    if value.len() != 40
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{context} must be a full lowercase 40-hex Git object id"
        ));
    }
    Ok(())
}

fn validate_identifier(value: &str, context: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(format!("{context} must use lowercase snake case"));
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || value.contains('\\')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "surface path {value:?} must be a safe relative path"
        ));
    }
    Ok(())
}

fn validate_date(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err("captured_date must use YYYY-MM-DD".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_baseline_and_step_map_validate() {
        validate(&crate::workspace_root()).expect("checked-in consolidation baseline");
    }

    #[test]
    fn object_ids_require_full_lowercase_hex() {
        assert!(validate_oid("0123456789abcdef0123456789abcdef01234567", "commit").is_ok());
        assert!(validate_oid("0123456", "commit").is_err());
        assert!(validate_oid("0123456789ABCDEF0123456789abcdef01234567", "commit").is_err());
        assert!(validate_oid("g123456789abcdef0123456789abcdef01234567", "commit").is_err());
    }

    #[test]
    fn paths_reject_escape_and_non_normal_components() {
        assert!(validate_relative_path("crates/sdk").is_ok());
        assert!(validate_relative_path("../sdk").is_err());
        assert!(validate_relative_path("crates/./sdk").is_err());
        assert!(validate_relative_path("/crates/sdk").is_err());
    }

    #[test]
    fn step_ranges_must_cover_every_step_once() {
        let valid = StepMap {
            schema_version: 1,
            map_id: STEP_MAP_ID.to_owned(),
            source_step_count: EXPECTED_HANDOFF_STEPS,
            source_sequence: "implementation/COMMIT_SEQUENCE.md".to_owned(),
            range: vec![StepRange {
                start: 1,
                end: EXPECTED_HANDOFF_STEPS,
                owners: vec!["rcld-rlc-010".to_owned()],
                disposition: "execute".to_owned(),
                reason: "fixture".to_owned(),
            }],
        };
        validate_step_map(&valid).expect("complete map");

        let mut gapped = valid;
        gapped.range[0].start = 2;
        assert!(validate_step_map(&gapped).is_err());
    }
}
