use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path},
    process::Command,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const BASELINE_RELATIVE: &str = "contracts/consolidation/baseline.v1.toml";
const STEP_MAP_RELATIVE: &str = "contracts/consolidation/handoff_steps.v1.toml";
const HISTORY_RELATIVE: &str = "contracts/consolidation/history.v1.toml";
const BASELINE_ID: &str = "radroots.rust.consolidation.baseline.v1";
const STEP_MAP_ID: &str = "radroots.rust.consolidation.handoff-steps.v1";
const ARCHITECTURE_TARGET: &str = "radroots.crates.release.v2";
const HISTORY_ID: &str = "radroots.rust.consolidation.history.v1";
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryContract {
    schema_version: u16,
    history_id: String,
    retention_locator: String,
    hash_algorithm: String,
    bundle_hash_algorithm: String,
    required_commit_map_fields: Vec<String>,
    required_verifications: Vec<String>,
    dual_source: DualSource,
    archive: Vec<Archive>,
    path_map: Vec<PathMap>,
    #[serde(default)]
    import: Vec<ImportRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DualSource {
    state: String,
    canonical_owner_before_import: String,
    canonical_owner_after_import: String,
    emergency_fix_flow: String,
    divergence_policy: String,
    exit_condition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Archive {
    source_id: String,
    kind: String,
    frozen_commit: String,
    artifact: String,
    sha256: String,
    bytes: u64,
    source_commit_count: u64,
    source_path_commit_count: u64,
    rewritten_commit_count: u64,
    topology_support_commit_count: u64,
    omitted_empty_commit_count: u64,
    source_object_count: u64,
    refname: String,
    bot_identity_scan: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PathMap {
    source_id: String,
    source: String,
    target: String,
    package: String,
    license: String,
    disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportRecord {
    source_id: String,
    frozen_commit: String,
    filtered_head: String,
    artifact: String,
    sha256: String,
    bytes: u64,
    final_tree_sha256: String,
    path_map_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommitMapEntry {
    source: String,
    target: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportCommitMap {
    schema_version: u16,
    schema: String,
    source_id: String,
    frozen_commit: String,
    filtered_head: String,
    source_commit_count: u64,
    source_path_commit_count: u64,
    rewritten_commit_count: u64,
    topology_support_commit_count: u64,
    omitted_empty_commit_count: u64,
    final_tree_sha256: String,
    path_map_sha256: String,
    verifications: Vec<String>,
    commits: Vec<ImportCommit>,
}

fn verify_history_import(
    workspace_root: &Path,
    source_id: &str,
    source_root: &Path,
    filtered_root: &Path,
    mode: &str,
) -> Result<(), String> {
    if !matches!(mode, "check" | "write") {
        return Err("import verification mode must be check or write".to_owned());
    }
    require_real_git_root(source_root, "source root")?;
    require_real_git_root(filtered_root, "filtered root")?;
    let history = read_toml::<HistoryContract>(workspace_root, HISTORY_RELATIVE)?;
    validate_history(&history)?;
    let archive = history
        .archive
        .iter()
        .find(|archive| archive.source_id == source_id)
        .ok_or_else(|| format!("unknown history source {source_id}"))?;
    if !matches!(archive.kind.as_str(), "git_bundle" | "path_fast_export") {
        return Err(format!(
            "history source {source_id} has an unsupported archive kind"
        ));
    }
    let path_maps = history
        .path_map
        .iter()
        .filter(|path_map| path_map.source_id == source_id)
        .collect::<Vec<_>>();
    if path_maps.is_empty() {
        return Err(format!("history source {source_id} has no path map"));
    }
    let source_head = git_stdout(source_root, &["rev-parse", "master"])?
        .trim()
        .to_owned();
    validate_oid(&source_head, "source head")?;
    if archive.kind == "git_bundle" && source_head != archive.frozen_commit {
        return Err(format!(
            "history source {source_id} is not at its frozen commit"
        ));
    }
    git(source_root, &["fsck", "--full", "--strict"])?;
    git(filtered_root, &["fsck", "--full", "--strict"])?;

    let commit_map = parse_commit_map(&filtered_root.join(".git/filter-repo/commit-map"))?;
    let source_commits = git_stdout(source_root, &["rev-list", "master"])?
        .lines()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if source_commits.len() as u64 != archive.source_commit_count
        || commit_map.len() != source_commits.len()
        || commit_map
            .iter()
            .any(|entry| !source_commits.contains(&entry.source))
    {
        return Err("source history and filter-repo commit map differ".to_owned());
    }

    let source_path_args = path_maps
        .iter()
        .map(|path_map| path_map.source.as_str())
        .collect::<Vec<_>>();
    let source_path_commits = rev_list_paths(source_root, &source_path_args)?;
    let by_source = commit_map
        .iter()
        .map(|entry| (entry.source.as_str(), entry.target.as_deref()))
        .collect::<BTreeMap<_, _>>();
    let filtered_head = git_stdout(filtered_root, &["rev-parse", "master"])?
        .trim()
        .to_owned();
    validate_oid(&filtered_head, "filtered head")?;
    let expected_filtered_head = by_source
        .get(source_head.as_str())
        .and_then(|target| *target)
        .ok_or_else(|| "frozen source head was omitted by filtering".to_owned())?;
    if filtered_head != expected_filtered_head {
        return Err("filtered master does not map from the frozen source head".to_owned());
    }

    verify_import_path_coverage(
        source_root,
        filtered_root,
        &source_head,
        &path_maps,
        &filtered_head,
    )?;
    verify_import_final_tree(
        source_root,
        filtered_root,
        &source_head,
        &filtered_head,
        &path_maps,
    )?;
    verify_import_context(filtered_root, &filtered_head, &path_maps)?;
    verify_commit_identities(filtered_root, &filtered_head)?;
    verify_import_follow_history(source_root, filtered_root, &path_maps, &by_source)?;

    let mut commits = Vec::with_capacity(commit_map.len());
    let mut rewritten = 0_u64;
    let mut topology = 0_u64;
    let mut omitted = 0_u64;
    for entry in &commit_map {
        let source_parents = commit_parents(source_root, &entry.source)?;
        let source_paths = changed_import_paths(source_root, &entry.source, &source_path_args)?;
        let in_path_history = source_path_commits.contains(&entry.source);
        let source_metadata = import_commit_metadata(source_root, &entry.source)?;
        let source_patch = normalized_import_patch(source_root, &entry.source, &path_maps, true)?;
        let (target_parents, target_metadata, disposition) = match entry.target.as_deref() {
            Some(target_commit) => {
                rewritten += 1;
                let mut expected_parents = Vec::new();
                for parent in &source_parents {
                    collect_effective_parents(
                        source_root,
                        parent,
                        &by_source,
                        &mut expected_parents,
                    )?;
                }
                deduplicate(&mut expected_parents);
                prune_ancestor_parents(filtered_root, &mut expected_parents)?;
                let target_parents = commit_parents(filtered_root, target_commit)?;
                if target_parents != expected_parents {
                    return Err(format!(
                        "mapped parent closure drifted for {}",
                        entry.source
                    ));
                }
                let target_metadata = import_commit_metadata(filtered_root, target_commit)?;
                verify_import_metadata(&entry.source, &source_metadata, &target_metadata)?;
                let target_patch =
                    normalized_import_patch(filtered_root, target_commit, &path_maps, false)?;
                let source_tree =
                    normalized_import_tree(source_root, &entry.source, &path_maps, true)?;
                let target_tree =
                    normalized_import_tree(filtered_root, target_commit, &path_maps, false)?;
                if source_tree != target_tree {
                    return Err(format!("normalized tree drifted for {}", entry.source));
                }
                let mut direct_target_parents = source_parents
                    .iter()
                    .filter_map(|parent| by_source.get(parent.as_str()).copied().flatten())
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let every_parent_retained = direct_target_parents.len() == source_parents.len();
                deduplicate(&mut direct_target_parents);
                prune_ancestor_parents(filtered_root, &mut direct_target_parents)?;
                if every_parent_retained
                    && direct_target_parents == target_parents
                    && source_patch != target_patch
                {
                    return Err(format!("normalized patch drifted for {}", entry.source));
                }
                let disposition = if in_path_history {
                    "retained"
                } else {
                    topology += 1;
                    "topology_support"
                };
                (target_parents, Some(target_metadata), disposition)
            }
            None => {
                let disposition = if in_path_history {
                    omitted += 1;
                    "omitted_empty"
                } else {
                    "out_of_scope"
                };
                (Vec::new(), None, disposition)
            }
        };
        commits.push(ImportCommit {
            source_commit: if archive.kind == "path_fast_export" {
                archive.frozen_commit.clone()
            } else {
                entry.source.clone()
            },
            target_commit: entry.target.clone(),
            source_parents,
            target_parents,
            source_subject: source_metadata.subject,
            target_subject: target_metadata
                .as_ref()
                .map(|metadata| metadata.subject.clone()),
            source_author: source_metadata.author,
            target_author: target_metadata
                .as_ref()
                .map(|metadata| metadata.author.clone()),
            source_author_time: source_metadata.author_time,
            target_author_time: target_metadata
                .as_ref()
                .map(|metadata| metadata.author_time.clone()),
            source_committer_time: source_metadata.committer_time,
            target_committer_time: target_metadata
                .as_ref()
                .map(|metadata| metadata.committer_time.clone()),
            source_paths,
            normalized_patch_sha256: sha256_bytes(source_patch.as_bytes()),
            empty_commit_disposition: disposition.to_owned(),
        });
    }
    if source_path_commits.len() as u64 != archive.source_path_commit_count
        || rewritten != archive.rewritten_commit_count
        || topology != archive.topology_support_commit_count
        || omitted != archive.omitted_empty_commit_count
    {
        return Err(format!(
            "history counts drifted: path={}, rewritten={rewritten}, topology={topology}, omitted={omitted}",
            source_path_commits.len()
        ));
    }

    let path_map_bytes = path_maps
        .iter()
        .map(|path_map| {
            format!(
                "{}\0{}\0{}\n",
                path_map.source, path_map.target, path_map.license
            )
        })
        .collect::<String>();
    let final_tree = normalized_import_tree(filtered_root, &filtered_head, &path_maps, false)?;
    let artifact = ImportCommitMap {
        schema_version: 1,
        schema: "radroots.history-import.commit-map.v1".to_owned(),
        source_id: source_id.to_owned(),
        frozen_commit: archive.frozen_commit.clone(),
        filtered_head,
        source_commit_count: archive.source_commit_count,
        source_path_commit_count: archive.source_path_commit_count,
        rewritten_commit_count: archive.rewritten_commit_count,
        topology_support_commit_count: archive.topology_support_commit_count,
        omitted_empty_commit_count: archive.omitted_empty_commit_count,
        final_tree_sha256: sha256_bytes(final_tree.as_bytes()),
        path_map_sha256: sha256_bytes(path_map_bytes.as_bytes()),
        verifications: history.required_verifications.clone(),
        commits,
    };
    let mut bytes = serde_json::to_vec_pretty(&artifact)
        .map_err(|error| format!("serialize history import artifact: {error}"))?;
    bytes.push(b'\n');
    let output = workspace_root.join(format!(
        "contracts/consolidation/imports/{source_id}.commit-map.v1.json"
    ));
    match mode {
        "write" => crate::build_control::atomic_write(&output, &bytes),
        "check" => {
            let current =
                fs::read(&output).map_err(|error| format!("read {}: {error}", output.display()))?;
            if current == bytes {
                Ok(())
            } else {
                Err(format!(
                    "history import artifact {} is stale",
                    output.display()
                ))
            }
        }
        _ => unreachable!("mode validated"),
    }
}

fn prune_ancestor_parents(root: &Path, parents: &mut Vec<String>) -> Result<(), String> {
    let original = parents.clone();
    let mut keep = Vec::new();
    for parent in &original {
        let mut redundant = false;
        for candidate in &original {
            if parent == candidate {
                continue;
            }
            let status = Command::new("git")
                .args(["merge-base", "--is-ancestor", parent, candidate])
                .current_dir(root)
                .status()
                .map_err(|error| format!("run git merge-base --is-ancestor: {error}"))?;
            match status.code() {
                Some(0) => {
                    redundant = true;
                    break;
                }
                Some(1) => {}
                _ => return Err("git merge-base --is-ancestor failed".to_owned()),
            }
        }
        if !redundant {
            keep.push(parent.clone());
        }
    }
    *parents = keep;
    Ok(())
}

#[derive(Debug)]
struct ImportMetadata {
    author: String,
    author_time: String,
    committer_time: String,
    subject: String,
}

fn require_real_git_root(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must be a real directory"));
    }
    let git_dir = git_stdout(path, &["rev-parse", "--absolute-git-dir"])?;
    let git_dir = Path::new(git_dir.trim());
    if !git_dir.is_absolute() {
        return Err(format!("{label} Git directory must be absolute"));
    }
    let metadata = fs::symlink_metadata(git_dir)
        .map_err(|error| format!("inspect {label} Git directory: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must contain a real Git directory"));
    }
    Ok(())
}

fn rev_list_paths(root: &Path, paths: &[&str]) -> Result<BTreeSet<String>, String> {
    let mut args = vec!["rev-list", "--full-history", "master", "--"];
    args.extend_from_slice(paths);
    Ok(git_stdout(root, &args)?
        .lines()
        .map(str::to_owned)
        .collect())
}

fn changed_import_paths(root: &Path, commit: &str, paths: &[&str]) -> Result<Vec<String>, String> {
    let mut args = vec![
        "diff-tree",
        "--root",
        "-m",
        "-r",
        "--no-commit-id",
        "--name-only",
        commit,
        "--",
    ];
    args.extend_from_slice(paths);
    let mut changed = git_stdout(root, &args)?
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    changed.sort();
    changed.dedup();
    Ok(changed)
}

fn import_commit_metadata(root: &Path, commit: &str) -> Result<ImportMetadata, String> {
    let raw = git_stdout(
        root,
        &[
            "show",
            "-s",
            "--format=%an%x00%ae%x00%aI%x00%cI%x00%s",
            commit,
        ],
    )?;
    let fields = raw.trim_end().split('\0').collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err(format!("commit metadata cardinality drifted for {commit}"));
    }
    Ok(ImportMetadata {
        author: format!("{} <{}>", fields[0], fields[1]),
        author_time: fields[2].to_owned(),
        committer_time: fields[3].to_owned(),
        subject: fields[4].to_owned(),
    })
}

fn verify_import_metadata(
    source_commit: &str,
    source: &ImportMetadata,
    target: &ImportMetadata,
) -> Result<(), String> {
    if source.author != target.author
        || source.author_time != target.author_time
        || source.committer_time != target.committer_time
        || !message_is_preserved(&source.subject, &target.subject)
    {
        return Err(format!(
            "attribution or message drifted for {source_commit}"
        ));
    }
    Ok(())
}

fn normalized_import_patch(
    root: &Path,
    commit: &str,
    path_maps: &[&PathMap],
    source_side: bool,
) -> Result<String, String> {
    let mut normalized = String::new();
    for path_map in path_maps {
        let path = if source_side {
            path_map.source.as_str()
        } else {
            path_map.target.as_str()
        };
        let patch = git_stdout(
            root,
            &[
                "-c",
                "core.quotePath=false",
                "diff-tree",
                "--root",
                "-m",
                "-r",
                "--binary",
                "--full-index",
                "--no-commit-id",
                commit,
                "--",
                path,
            ],
        )?;
        normalized.push_str(&normalize_import_patch_paths(patch, path_maps, source_side));
    }
    Ok(normalized)
}

fn normalize_import_patch_paths(
    value: String,
    path_maps: &[&PathMap],
    source_side: bool,
) -> String {
    value
        .split_inclusive('\n')
        .map(|line| {
            if line.starts_with("diff --git ")
                || line.starts_with("--- ")
                || line.starts_with("+++ ")
                || line.starts_with("rename from ")
                || line.starts_with("rename to ")
                || line.starts_with("copy from ")
                || line.starts_with("copy to ")
                || line.starts_with("Binary files ")
            {
                normalize_import_paths(line.to_owned(), path_maps, source_side)
            } else {
                line.to_owned()
            }
        })
        .collect()
}

fn normalize_import_paths(mut value: String, path_maps: &[&PathMap], source_side: bool) -> String {
    let mut replacements = path_maps
        .iter()
        .enumerate()
        .map(|(index, path_map)| {
            let path = if source_side {
                path_map.source.as_str()
            } else {
                path_map.target.as_str()
            };
            (path, format!("__IMPORT__/{index:02}"))
        })
        .collect::<Vec<_>>();
    replacements.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
    for (path, replacement) in replacements {
        value = value.replace(path, &replacement);
    }
    value
}

fn normalized_import_tree(
    root: &Path,
    commit: &str,
    path_maps: &[&PathMap],
    source_side: bool,
) -> Result<String, String> {
    let mut args = vec!["ls-tree", "-r", "--full-tree", commit, "--"];
    args.extend(path_maps.iter().map(|path_map| {
        if source_side {
            path_map.source.as_str()
        } else {
            path_map.target.as_str()
        }
    }));
    let normalized = normalize_import_paths(git_stdout(root, &args)?, path_maps, source_side);
    let mut lines = normalized.lines().collect::<Vec<_>>();
    lines.sort_unstable();
    Ok(format!("{}\n", lines.join("\n")))
}

fn verify_import_final_tree(
    source_root: &Path,
    filtered_root: &Path,
    source_commit: &str,
    target_commit: &str,
    path_maps: &[&PathMap],
) -> Result<(), String> {
    let source = normalized_import_tree(source_root, source_commit, path_maps, true)?;
    let target = normalized_import_tree(filtered_root, target_commit, path_maps, false)?;
    if source == target {
        Ok(())
    } else {
        Err("filtered import final tree drifted".to_owned())
    }
}

fn verify_import_path_coverage(
    source_root: &Path,
    filtered_root: &Path,
    source_commit: &str,
    path_maps: &[&PathMap],
    filtered_head: &str,
) -> Result<(), String> {
    for path_map in path_maps {
        git(
            source_root,
            &[
                "cat-file",
                "-e",
                &format!("{source_commit}:{}", path_map.source),
            ],
        )?;
        git(
            filtered_root,
            &[
                "cat-file",
                "-e",
                &format!("{filtered_head}:{}", path_map.target),
            ],
        )?;
    }
    Ok(())
}

fn verify_import_context(
    filtered_root: &Path,
    filtered_head: &str,
    path_maps: &[&PathMap],
) -> Result<(), String> {
    let paths = git_stdout(
        filtered_root,
        &["ls-tree", "-r", "--name-only", filtered_head],
    )?;
    for path in paths.lines() {
        let lower = path.to_ascii_lowercase();
        if !path_maps.iter().any(|path_map| {
            path == path_map.target || path.starts_with(&format!("{}/", path_map.target))
        }) || path.split('/').any(|segment| segment == ".github")
            || matches!(path.rsplit('/').next(), Some("AGENTS.md" | "CLAUDE.md"))
            || lower.ends_with(".pem")
            || lower.ends_with(".key")
            || lower.ends_with("/.env")
        {
            return Err(format!("context firewall rejected {path}"));
        }
        let contents = git_bytes(filtered_root, &["show", &format!("{filtered_head}:{path}")])?;
        let text = String::from_utf8_lossy(&contents);
        let lower_contents = text.to_ascii_lowercase();
        let license_metadata_stripped = lower_contents
            .replace("license = \"gpl-3.0-only\"", "")
            .replace("license = \"gpl-3.0-or-later\"", "")
            .replace("license = \"mit or apache-2.0\"", "")
            .replace("license = \"mpl-2.0\"", "");
        if text.contains("PRIVATE KEY-----")
            || text.contains("ghp_")
            || lower_contents.contains("github-actions[bot]")
            || license_metadata_stripped.contains("gpl-3.0")
        {
            return Err(format!(
                "secret, bot, or license content rejected in {path}"
            ));
        }
    }
    Ok(())
}

fn verify_import_follow_history(
    source_root: &Path,
    filtered_root: &Path,
    path_maps: &[&PathMap],
    by_source: &BTreeMap<&str, Option<&str>>,
) -> Result<(), String> {
    let mapped_targets = by_source
        .values()
        .filter_map(|target| *target)
        .collect::<BTreeSet<_>>();
    for path_map in path_maps {
        let target_paths = git_stdout(
            filtered_root,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                "master",
                "--",
                &path_map.target,
            ],
        )?;
        for target_path in target_paths.lines() {
            let suffix = target_path
                .strip_prefix(&path_map.target)
                .ok_or_else(|| "target path escaped its import root".to_owned())?;
            let source_path = format!("{}{}", path_map.source, suffix);
            let source_log_has_mapped_commit = git_stdout(
                source_root,
                &["log", "--follow", "--format=%H", "--", &source_path],
            )?
            .lines()
            .any(|commit| by_source.get(commit).copied().flatten().is_some());
            let target_log = git_stdout(
                filtered_root,
                &["log", "--follow", "--format=%H", "--", target_path],
            )?
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
            if !source_log_has_mapped_commit
                || target_log.is_empty()
                || target_log
                    .iter()
                    .any(|commit| !mapped_targets.contains(commit.as_str()))
            {
                return Err(format!("git log --follow drifted for {source_path}"));
            }
        }
    }
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportCommit {
    source_commit: String,
    target_commit: Option<String>,
    source_parents: Vec<String>,
    target_parents: Vec<String>,
    source_subject: String,
    target_subject: Option<String>,
    source_author: String,
    target_author: Option<String>,
    source_author_time: String,
    target_author_time: Option<String>,
    source_committer_time: String,
    target_committer_time: Option<String>,
    source_paths: Vec<String>,
    normalized_patch_sha256: String,
    empty_commit_disposition: String,
}

pub fn run(args: &[String], workspace_root: &Path) -> Result<(), String> {
    match args {
        [command] if command == "baseline" => validate_baseline_contracts(workspace_root),
        [command] if command == "history" => validate_history_contract(workspace_root, None),
        [command] if command == "history-rehearsal" => run_history_rehearsal(),
        [command, source_flag, source_id, source_root_flag, source_root, filtered_root_flag, filtered_root, mode_flag, mode]
            if command == "import-verify"
                && source_flag == "--source"
                && source_root_flag == "--source-root"
                && filtered_root_flag == "--filtered-root"
                && mode_flag == "--mode" =>
        {
            verify_history_import(
                workspace_root,
                source_id,
                Path::new(source_root),
                Path::new(filtered_root),
                mode,
            )
        }
        [command, flag, archive_root] if command == "history" && flag == "--archive-root" => {
            validate_history_contract(workspace_root, Some(Path::new(archive_root)))
        }
        _ => Err(
            "consolidation accepts baseline, history [--archive-root <absolute-directory>], history-rehearsal, or import-verify --source <id> --source-root <absolute-directory> --filtered-root <absolute-directory> --mode <check|write>"
                .to_owned(),
        ),
    }
}

pub fn validate_baseline_contracts(workspace_root: &Path) -> Result<(), String> {
    let baseline = read_toml::<Baseline>(workspace_root, BASELINE_RELATIVE)?;
    let step_map = read_toml::<StepMap>(workspace_root, STEP_MAP_RELATIVE)?;
    validate_baseline(&baseline)?;
    validate_step_map(&step_map)
}

fn validate_history_contract(
    workspace_root: &Path,
    archive_root: Option<&Path>,
) -> Result<(), String> {
    let history = read_toml::<HistoryContract>(workspace_root, HISTORY_RELATIVE)?;
    validate_history(&history)?;
    validate_retired_import_targets(workspace_root, &history.path_map)?;
    validate_import_records(workspace_root, &history)?;
    if let Some(archive_root) = archive_root {
        validate_archives(&history, archive_root)?;
    }
    Ok(())
}

fn validate_retired_import_targets(
    workspace_root: &Path,
    path_maps: &[PathMap],
) -> Result<(), String> {
    for path_map in path_maps
        .iter()
        .filter(|path_map| path_map.disposition == "import_unique_behavior_then_retire")
    {
        let target = workspace_root.join(&path_map.target);
        match fs::symlink_metadata(&target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "inspect retired import target {}: {error}",
                    path_map.target
                ));
            }
            Ok(_) => {
                return Err(format!(
                    "retired import target remains in the active tree: {}",
                    path_map.target
                ));
            }
        }
    }
    Ok(())
}

fn validate_import_records(workspace_root: &Path, history: &HistoryContract) -> Result<(), String> {
    let mut sources = BTreeSet::new();
    for record in &history.import {
        if !sources.insert(record.source_id.as_str()) {
            return Err(format!(
                "duplicate history import source {}",
                record.source_id
            ));
        }
        let archive = history
            .archive
            .iter()
            .find(|archive| archive.source_id == record.source_id)
            .ok_or_else(|| format!("unknown history import source {}", record.source_id))?;
        if record.frozen_commit != archive.frozen_commit {
            return Err(format!(
                "history import {} frozen commit drifted",
                record.source_id
            ));
        }
        validate_oid(&record.filtered_head, "filtered import head")?;
        validate_artifact_name(&record.artifact)?;
        validate_sha256(&record.sha256, "commit-map artifact digest")?;
        validate_sha256(&record.final_tree_sha256, "import final-tree digest")?;
        validate_sha256(&record.path_map_sha256, "import path-map digest")?;
        let path = workspace_root
            .join("contracts/consolidation/imports")
            .join(&record.artifact);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() != record.bytes
        {
            return Err(format!(
                "history import artifact {} metadata drifted",
                path.display()
            ));
        }
        let bytes = fs::read(&path)
            .map_err(|error| format!("read history import artifact {}: {error}", path.display()))?;
        if sha256_bytes(&bytes) != record.sha256 {
            return Err(format!(
                "history import artifact {} digest drifted",
                path.display()
            ));
        }
        let artifact = serde_json::from_slice::<ImportCommitMap>(&bytes).map_err(|error| {
            format!("parse history import artifact {}: {error}", path.display())
        })?;
        if artifact.schema_version != 1
            || artifact.schema != "radroots.history-import.commit-map.v1"
            || artifact.source_id != record.source_id
            || artifact.frozen_commit != record.frozen_commit
            || artifact.filtered_head != record.filtered_head
            || artifact.final_tree_sha256 != record.final_tree_sha256
            || artifact.path_map_sha256 != record.path_map_sha256
            || artifact.source_commit_count != archive.source_commit_count
            || artifact.source_path_commit_count != archive.source_path_commit_count
            || artifact.rewritten_commit_count != archive.rewritten_commit_count
            || artifact.topology_support_commit_count != archive.topology_support_commit_count
            || artifact.omitted_empty_commit_count != archive.omitted_empty_commit_count
            || artifact.commits.len() as u64 != archive.source_commit_count
            || to_unique_set(&artifact.verifications, "import verification")?
                != to_unique_set(&history.required_verifications, "required verification")?
        {
            return Err(format!(
                "history import artifact {} contract drifted",
                record.source_id
            ));
        }
    }
    Ok(())
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

fn validate_history(history: &HistoryContract) -> Result<(), String> {
    if history.schema_version != 1
        || history.history_id != HISTORY_ID
        || history.retention_locator != "external://radroots-rust-consolidation-v1-20260805"
        || history.hash_algorithm != "sha256"
        || history.bundle_hash_algorithm != "sha1"
    {
        return Err("history preservation identity drifted".to_owned());
    }

    let expected_commit_map_fields = BTreeSet::from([
        "empty_commit_disposition",
        "normalized_patch_sha256",
        "source_author",
        "source_author_time",
        "source_commit",
        "source_committer_time",
        "source_parents",
        "source_paths",
        "source_subject",
        "target_author",
        "target_author_time",
        "target_commit",
        "target_committer_time",
        "target_parents",
        "target_subject",
    ]);
    if to_unique_set(&history.required_commit_map_fields, "commit map field")?
        != expected_commit_map_fields
    {
        return Err("commit map requirements drifted".to_owned());
    }
    let expected_verifications = BTreeSet::from([
        "archive_restore",
        "authorship",
        "bot_identity_scan",
        "commit_count",
        "context_firewall",
        "final_tree_digest",
        "git_fsck",
        "git_log_follow",
        "github_workflow_exclusion",
        "license_scan",
        "mapped_parent_closure",
        "message_scope_only",
        "normalized_patch_equivalence",
        "path_coverage",
        "secret_scan",
        "timestamps",
    ]);
    if to_unique_set(&history.required_verifications, "verification")? != expected_verifications {
        return Err("history verification requirements drifted".to_owned());
    }
    let dual_source = &history.dual_source;
    if dual_source.state != "pre_import"
        || dual_source.canonical_owner_before_import != "frozen_donor_commit"
        || dual_source.canonical_owner_after_import != "verified_lib_import_merge"
        || dual_source.divergence_policy != "forbidden"
        || dual_source.emergency_fix_flow.trim().is_empty()
        || dual_source.exit_condition.trim().is_empty()
    {
        return Err("dual-source control drifted".to_owned());
    }

    let expected_sources =
        BTreeSet::from(["app_rt", "sdk", "studio_app", "studio_mpl_legacy_core"]);
    let mut archives = BTreeMap::new();
    let mut artifact_names = BTreeSet::new();
    for archive in &history.archive {
        validate_identifier(&archive.source_id, "archive source id")?;
        if archives
            .insert(archive.source_id.as_str(), archive)
            .is_some()
        {
            return Err(format!("duplicate archive source {}", archive.source_id));
        }
        validate_oid(&archive.frozen_commit, "archive frozen commit")?;
        validate_sha256(&archive.sha256, "archive digest")?;
        validate_artifact_name(&archive.artifact)?;
        if !artifact_names.insert(archive.artifact.as_str()) {
            return Err(format!("duplicate archive artifact {}", archive.artifact));
        }
        if !matches!(archive.kind.as_str(), "git_bundle" | "path_fast_export")
            || archive.bytes == 0
            || archive.source_commit_count == 0
            || archive.source_path_commit_count == 0
            || archive.source_path_commit_count > archive.source_commit_count
            || archive.rewritten_commit_count == 0
            || archive.rewritten_commit_count + archive.omitted_empty_commit_count
                != archive.source_path_commit_count + archive.topology_support_commit_count
            || archive.source_object_count == 0
            || archive.refname != "refs/heads/master"
            || !archive.bot_identity_scan
        {
            return Err(format!("archive {} is incomplete", archive.source_id));
        }
    }
    if archives.keys().copied().collect::<BTreeSet<_>>() != expected_sources {
        return Err("history archive inventory is incomplete".to_owned());
    }
    if archives["studio_mpl_legacy_core"].kind != "path_fast_export"
        || archives
            .iter()
            .filter(|(id, archive)| {
                **id != "studio_mpl_legacy_core" && archive.kind == "git_bundle"
            })
            .count()
            != 3
    {
        return Err("history archive kinds drifted".to_owned());
    }

    let allowed_licenses = BTreeSet::from([
        "GPL-3.0-only",
        "GPL-3.0-or-later",
        "MIT OR Apache-2.0",
        "MPL-2.0",
    ]);
    let allowed_dispositions = BTreeSet::from([
        "import_unique_behavior_then_retire",
        "merge_then_retire",
        "retain",
        "retain_private",
    ]);
    let mut source_paths = BTreeSet::new();
    let mut target_paths = BTreeSet::new();
    let mut package_sources = BTreeSet::new();
    for path_map in &history.path_map {
        if !archives.contains_key(path_map.source_id.as_str()) {
            return Err(format!("unknown path-map source {}", path_map.source_id));
        }
        validate_relative_path(&path_map.source)?;
        validate_relative_path(&path_map.target)?;
        validate_identifier(&path_map.package, "path-map package")?;
        if !allowed_licenses.contains(path_map.license.as_str())
            || !allowed_dispositions.contains(path_map.disposition.as_str())
        {
            return Err(format!(
                "path map {}/{} has invalid license or disposition",
                path_map.source_id, path_map.source
            ));
        }
        if !source_paths.insert((path_map.source_id.as_str(), path_map.source.as_str())) {
            return Err(format!(
                "duplicate source path {}/{}",
                path_map.source_id, path_map.source
            ));
        }
        if !target_paths.insert(path_map.target.as_str()) {
            return Err(format!("duplicate target path {}", path_map.target));
        }
        if !package_sources.insert((path_map.source_id.as_str(), path_map.package.as_str())) {
            return Err(format!(
                "duplicate package {} in source {}",
                path_map.package, path_map.source_id
            ));
        }
    }
    for source in expected_sources {
        if !source_paths
            .iter()
            .any(|(candidate, _)| *candidate == source)
        {
            return Err(format!("source {source} has no path map"));
        }
    }
    Ok(())
}

fn validate_archives(history: &HistoryContract, archive_root: &Path) -> Result<(), String> {
    if !archive_root.is_absolute() {
        return Err("archive root must be absolute".to_owned());
    }
    let root_metadata = fs::symlink_metadata(archive_root)
        .map_err(|error| format!("inspect archive root {}: {error}", archive_root.display()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err("archive root must be a real directory, not a symlink".to_owned());
    }

    for archive in &history.archive {
        let artifact = archive_root.join(&archive.artifact);
        let metadata = fs::symlink_metadata(&artifact)
            .map_err(|error| format!("inspect archive {}: {error}", artifact.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "archive {} must be a regular non-symlink file",
                artifact.display()
            ));
        }
        if metadata.len() != archive.bytes {
            return Err(format!(
                "archive {} size drifted: expected {}, got {}",
                artifact.display(),
                archive.bytes,
                metadata.len()
            ));
        }
        let digest = sha256_file(&artifact)?;
        if digest != archive.sha256 {
            return Err(format!("archive {} digest drifted", artifact.display()));
        }
        if archive.kind == "git_bundle" {
            let output = Command::new("git")
                .args(["bundle", "verify"])
                .arg(&artifact)
                .output()
                .map_err(|error| format!("run git bundle verify: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "git bundle verify failed for {}: {}",
                    artifact.display(),
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        } else {
            validate_fast_export(&artifact, archive)?;
        }
    }
    Ok(())
}

fn validate_fast_export(path: &Path, archive: &Archive) -> Result<(), String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("read fast-export archive {}: {error}", path.display()))?;
    if !raw.contains(&format!("original-oid {}", archive.frozen_commit))
        || !raw.contains("reset refs/heads/master")
        || !raw.contains("author triesap <tyson@radroots.org>")
        || !raw.contains("committer triesap <tyson@radroots.org>")
        || raw.to_ascii_lowercase().contains("github-actions")
        || raw.to_ascii_lowercase().contains("[bot]")
    {
        return Err("legacy Studio fast-export identity or ref drifted".to_owned());
    }
    for line in raw.lines() {
        let Some(path) = line
            .strip_prefix("M ")
            .and_then(|line| line.splitn(3, ' ').nth(2))
        else {
            continue;
        };
        if !path.starts_with("studio_app/studio_app_core/") {
            return Err(format!("fast-export contains out-of-scope path {path}"));
        }
    }
    Ok(())
}

fn run_history_rehearsal() -> Result<(), String> {
    let fixture = tempfile::TempDir::new()
        .map_err(|error| format!("create history rehearsal root: {error}"))?;
    let source = fixture.path().join("source");
    let restored = fixture.path().join("restored");
    let filtered = fixture.path().join("filtered");
    let import_target = fixture.path().join("import-target");
    let bundle = fixture.path().join("source.bundle");

    create_history_fixture(&source)?;
    git(&source, &["bundle", "create", path_arg(&bundle)?, "master"])?;
    git(
        fixture.path(),
        &["clone", path_arg(&bundle)?, path_arg(&restored)?],
    )?;
    git(&restored, &["fsck", "--full", "--strict"])?;

    git(
        fixture.path(),
        &["clone", path_arg(&bundle)?, path_arg(&filtered)?],
    )?;
    git(
        &filtered,
        &[
            "filter-repo",
            "--force",
            "--path",
            "src/",
            "--path-rename",
            "src/:crates/imported/",
        ],
    )?;
    let commit_map_path = filtered.join(".git/filter-repo/commit-map");
    let commit_map = parse_commit_map(&commit_map_path)?;
    verify_filtered_history(&source, &filtered, &commit_map, "src", "crates/imported")?;

    create_import_target(&import_target)?;
    git(
        &import_target,
        &[
            "fetch",
            path_arg(&filtered)?,
            "master:refs/remotes/rehearsal/imported",
        ],
    )?;
    let merge_tree = git_stdout(
        &import_target,
        &[
            "merge-tree",
            "--write-tree",
            "--allow-unrelated-histories",
            "HEAD",
            "refs/remotes/rehearsal/imported",
        ],
    )?;
    let merge_tree = merge_tree.trim();
    validate_oid(merge_tree, "rehearsal merge tree")?;
    if git_stdout(&import_target, &["cat-file", "-t", merge_tree])?.trim() != "tree" {
        return Err("no-op import rehearsal did not produce a tree".to_owned());
    }
    if git_stdout(&import_target, &["status", "--porcelain"])
        .map(|output| !output.trim().is_empty())?
    {
        return Err("no-op import rehearsal changed the target worktree".to_owned());
    }

    exercise_history_negative_cases(&source, &filtered, &commit_map)?;
    Ok(())
}

fn create_history_fixture(root: &Path) -> Result<(), String> {
    fs::create_dir(root).map_err(|error| format!("create {}: {error}", root.display()))?;
    git(root, &["init", "--initial-branch=master"])?;
    git(root, &["config", "user.name", "Radroots History Fixture"])?;
    git(
        root,
        &["config", "user.email", "history-fixture@radroots.org"],
    )?;
    write_fixture(root, "src/LICENSE", "MIT OR Apache-2.0\n")?;
    write_fixture(root, "src/item.txt", "base\n")?;
    write_fixture(root, "AGENTS.md", "context only\n")?;
    fixture_commit(root, "seed reusable source", "2001-01-01T00:00:00+00:00")?;

    git(root, &["branch", "feature"])?;
    append_fixture(root, "src/item.txt", "main\n")?;
    fixture_commit(root, "extend main source", "2001-01-02T00:00:00+00:00")?;
    append_fixture(root, "AGENTS.md", "must not import\n")?;
    fixture_commit(root, "change donor context", "2001-01-03T00:00:00+00:00")?;

    git(root, &["checkout", "feature"])?;
    write_fixture(root, "src/feature.txt", "feature\n")?;
    fixture_commit(root, "add feature source", "2001-01-04T00:00:00+00:00")?;
    git(root, &["checkout", "master"])?;
    git_with_identity(
        root,
        &["merge", "--no-ff", "feature", "-m", "merge reusable source"],
        "2001-01-05T00:00:00+00:00",
    )?;
    write_fixture(root, ".github/workflows/forbidden.yml", "forbidden: true\n")?;
    fixture_commit(
        root,
        "add forbidden donor automation",
        "2001-01-06T00:00:00+00:00",
    )?;
    append_fixture(root, "src/item.txt", "final\n")?;
    fixture_commit(root, "finish reusable source", "2001-01-07T00:00:00+00:00")?;
    Ok(())
}

fn create_import_target(root: &Path) -> Result<(), String> {
    fs::create_dir(root).map_err(|error| format!("create {}: {error}", root.display()))?;
    git(root, &["init", "--initial-branch=master"])?;
    git(root, &["config", "user.name", "Radroots History Fixture"])?;
    git(
        root,
        &["config", "user.email", "history-fixture@radroots.org"],
    )?;
    write_fixture(root, "README", "import target\n")?;
    fixture_commit(root, "seed import target", "2001-01-08T00:00:00+00:00")
}

fn write_fixture(root: &Path, relative: &str, contents: &str) -> Result<(), String> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(&path, contents).map_err(|error| format!("write {}: {error}", path.display()))
}

fn append_fixture(root: &Path, relative: &str, contents: &str) -> Result<(), String> {
    use std::io::Write;

    let path = root.join(relative);
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| format!("append {}: {error}", path.display()))
}

fn fixture_commit(root: &Path, subject: &str, timestamp: &str) -> Result<(), String> {
    git(root, &["add", "--all"])?;
    git_with_identity(root, &["commit", "-m", subject], timestamp)
}

fn git_with_identity(root: &Path, args: &[&str], timestamp: &str) -> Result<(), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_DATE", timestamp)
        .env("GIT_COMMITTER_DATE", timestamp)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    command_success(output, root, args).map(|_| ())
}

fn parse_commit_map(path: &Path) -> Result<Vec<CommitMapEntry>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("read commit map {}: {error}", path.display()))?;
    let mut entries = Vec::new();
    for (line_index, line) in raw.lines().enumerate() {
        if line_index == 0 && line == "old                                      new" {
            continue;
        }
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.len() != 2 {
            return Err(format!("commit map line {} is malformed", line_index + 1));
        }
        validate_oid(fields[0], "source commit-map object")?;
        validate_oid(fields[1], "target commit-map object")?;
        entries.push(CommitMapEntry {
            source: fields[0].to_owned(),
            target: (fields[1] != "0000000000000000000000000000000000000000")
                .then(|| fields[1].to_owned()),
        });
    }
    if entries.is_empty() {
        return Err("commit map contains no entries".to_owned());
    }
    let unique = entries
        .iter()
        .map(|entry| entry.source.as_str())
        .collect::<BTreeSet<_>>();
    if unique.len() != entries.len() {
        return Err("commit map contains duplicate source commits".to_owned());
    }
    Ok(entries)
}

fn verify_filtered_history(
    source: &Path,
    target: &Path,
    entries: &[CommitMapEntry],
    source_prefix: &str,
    target_prefix: &str,
) -> Result<(), String> {
    validate_relative_path(source_prefix)?;
    validate_relative_path(target_prefix)?;
    git(source, &["fsck", "--full", "--strict"])?;
    git(target, &["fsck", "--full", "--strict"])?;

    let by_source = entries
        .iter()
        .map(|entry| (entry.source.as_str(), entry.target.as_deref()))
        .collect::<BTreeMap<_, _>>();
    let mut saw_merge = false;
    let mut saw_omitted = false;
    for entry in entries {
        git(
            source,
            &["cat-file", "-e", &format!("{}^{{commit}}", entry.source)],
        )?;
        let Some(target_commit) = entry.target.as_deref() else {
            saw_omitted = true;
            continue;
        };
        git(
            target,
            &["cat-file", "-e", &format!("{target_commit}^{{commit}}")],
        )?;
        verify_commit_metadata(source, target, &entry.source, target_commit)?;
        let source_parents = commit_parents(source, &entry.source)?;
        saw_merge |= source_parents.len() > 1;
        let mut expected_target_parents = Vec::new();
        for parent in source_parents {
            collect_effective_parents(source, &parent, &by_source, &mut expected_target_parents)?;
        }
        deduplicate(&mut expected_target_parents);
        if commit_parents(target, target_commit)? != expected_target_parents {
            return Err(format!(
                "mapped parent closure drifted for {}",
                entry.source
            ));
        }
        let source_patch = normalized_patch(source, &entry.source, source_prefix, "__IMPORT__")?;
        let target_patch = normalized_patch(target, target_commit, target_prefix, "__IMPORT__")?;
        if source_patch != target_patch {
            return Err(format!("normalized patch drifted for {}", entry.source));
        }
    }
    if !saw_merge || !saw_omitted {
        return Err("history rehearsal must include a merge and an omitted commit".to_owned());
    }

    let source_head = git_stdout(source, &["rev-parse", "master"])?;
    let source_head = source_head.trim();
    let expected_target_head = by_source
        .get(source_head)
        .and_then(|target| *target)
        .ok_or_else(|| "source master is not retained in commit map".to_owned())?;
    if git_stdout(target, &["rev-parse", "master"])?.trim() != expected_target_head {
        return Err("filtered master has unexpected commits".to_owned());
    }
    verify_final_tree(
        source,
        target,
        source_head,
        expected_target_head,
        source_prefix,
        target_prefix,
    )?;
    verify_context_firewall(target, expected_target_head, target_prefix)?;
    verify_commit_identities(target, expected_target_head)?;
    verify_follow_history(source, target, source_prefix, target_prefix)?;
    Ok(())
}

fn verify_commit_metadata(
    source: &Path,
    target: &Path,
    source_commit: &str,
    target_commit: &str,
) -> Result<(), String> {
    let format = "%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI%x00%s";
    let source_meta = git_stdout(
        source,
        &["show", "-s", &format!("--format={format}"), source_commit],
    )?;
    let target_meta = git_stdout(
        target,
        &["show", "-s", &format!("--format={format}"), target_commit],
    )?;
    let mut source_fields = source_meta.trim_end().split('\0').collect::<Vec<_>>();
    let mut target_fields = target_meta.trim_end().split('\0').collect::<Vec<_>>();
    if source_fields.len() != 7 || target_fields.len() != 7 {
        return Err("commit metadata has unexpected cardinality".to_owned());
    }
    let source_subject = source_fields.pop().expect("cardinality checked");
    let target_subject = target_fields.pop().expect("cardinality checked");
    if source_fields != target_fields || !message_is_preserved(source_subject, target_subject) {
        return Err(format!(
            "attribution or message drifted for {source_commit}"
        ));
    }
    Ok(())
}

fn message_is_preserved(source: &str, target: &str) -> bool {
    if source == target {
        return true;
    }
    let Some(scope) = target
        .strip_prefix(source)
        .and_then(|suffix| suffix.strip_prefix(" ("))
        .and_then(|suffix| suffix.strip_suffix(')'))
    else {
        return false;
    };
    !scope.is_empty()
        && scope.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'/')
        })
}

fn commit_parents(repo: &Path, commit: &str) -> Result<Vec<String>, String> {
    let output = git_stdout(repo, &["show", "-s", "--format=%P", commit])?;
    Ok(output.split_ascii_whitespace().map(str::to_owned).collect())
}

fn collect_effective_parents(
    source: &Path,
    commit: &str,
    by_source: &BTreeMap<&str, Option<&str>>,
    output: &mut Vec<String>,
) -> Result<(), String> {
    match by_source.get(commit).copied().flatten() {
        Some(target) => output.push(target.to_owned()),
        None => {
            for parent in commit_parents(source, commit)? {
                collect_effective_parents(source, &parent, by_source, output)?;
            }
        }
    }
    Ok(())
}

fn deduplicate(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn normalized_patch(
    repo: &Path,
    commit: &str,
    prefix: &str,
    normalized_prefix: &str,
) -> Result<String, String> {
    let patch = git_stdout(
        repo,
        &[
            "-c",
            "core.quotePath=false",
            "diff-tree",
            "--root",
            "-m",
            "-r",
            "--binary",
            "--full-index",
            "--no-commit-id",
            commit,
            "--",
            prefix,
        ],
    )?;
    Ok(patch.replace(prefix, normalized_prefix))
}

fn verify_final_tree(
    source: &Path,
    target: &Path,
    source_commit: &str,
    target_commit: &str,
    source_prefix: &str,
    target_prefix: &str,
) -> Result<(), String> {
    let source_tree = tree_manifest(source, source_commit, source_prefix, source_prefix)?;
    let target_tree = tree_manifest(target, target_commit, target_prefix, source_prefix)?;
    if source_tree != target_tree {
        return Err("filtered final tree drifted".to_owned());
    }
    Ok(())
}

fn tree_manifest(
    repo: &Path,
    commit: &str,
    prefix: &str,
    normalized_prefix: &str,
) -> Result<String, String> {
    let output = git_stdout(
        repo,
        &["ls-tree", "-r", "--full-tree", commit, "--", prefix],
    )?;
    Ok(output.replace(prefix, normalized_prefix))
}

fn verify_context_firewall(repo: &Path, commit: &str, target_prefix: &str) -> Result<(), String> {
    let paths = git_stdout(repo, &["ls-tree", "-r", "--name-only", commit])?;
    let required_prefix = format!("{target_prefix}/");
    for path in paths.lines() {
        let lower = path.to_ascii_lowercase();
        if !path.starts_with(&required_prefix)
            || path.split('/').any(|segment| segment == ".github")
            || matches!(path.rsplit('/').next(), Some("AGENTS.md" | "CLAUDE.md"))
            || lower.ends_with(".pem")
            || lower.ends_with(".key")
            || lower.ends_with("/.env")
        {
            return Err(format!("context firewall rejected {path}"));
        }
        let contents = git_stdout(repo, &["show", &format!("{commit}:{path}")])?;
        let lower_contents = contents.to_ascii_lowercase();
        if contents.contains("PRIVATE KEY-----")
            || contents.contains("ghp_")
            || lower_contents.contains("github-actions[bot]")
        {
            return Err(format!("secret or bot content rejected in {path}"));
        }
    }
    Ok(())
}

fn verify_commit_identities(repo: &Path, commit: &str) -> Result<(), String> {
    let identities = git_stdout(repo, &["log", "--format=%an%x00%ae%x00%cn%x00%ce", commit])?;
    let lower = identities.to_ascii_lowercase();
    if lower.contains("github-actions") || lower.contains("[bot]") {
        return Err("bot identity found in filtered history".to_owned());
    }
    Ok(())
}

fn verify_follow_history(
    source: &Path,
    target: &Path,
    source_prefix: &str,
    target_prefix: &str,
) -> Result<(), String> {
    let source_file = format!("{source_prefix}/item.txt");
    let target_file = format!("{target_prefix}/item.txt");
    let source_log = git_stdout(
        source,
        &["log", "--follow", "--format=%s", "--", &source_file],
    )?;
    let target_log = git_stdout(
        target,
        &["log", "--follow", "--format=%s", "--", &target_file],
    )?;
    if source_log != target_log {
        return Err("git log --follow attribution drifted".to_owned());
    }
    Ok(())
}

fn exercise_history_negative_cases(
    source: &Path,
    filtered: &Path,
    entries: &[CommitMapEntry],
) -> Result<(), String> {
    if message_is_preserved("preserve this", "rewritten message")
        || message_is_preserved("preserve this", "preserve this (Bad Scope)")
    {
        return Err("message negative fixture was accepted".to_owned());
    }
    let malformed_map = filtered.join("malformed-commit-map");
    fs::write(&malformed_map, "old new\nnot-an-oid still-not-an-oid\n")
        .map_err(|error| format!("write negative commit map: {error}"))?;
    if parse_commit_map(&malformed_map).is_ok() {
        return Err("malformed commit map was accepted".to_owned());
    }
    let head = entries
        .iter()
        .rev()
        .find_map(|entry| entry.target.as_deref())
        .ok_or_else(|| "negative fixture has no target head".to_owned())?;
    if verify_context_firewall(filtered, head, "wrong/prefix").is_ok() {
        return Err("context-firewall negative fixture was accepted".to_owned());
    }
    if normalized_patch(source, &entries[0].source, "src", "__IMPORT__")?
        == normalized_patch(filtered, head, "crates/imported", "__WRONG__")?
    {
        return Err("normalized-patch negative fixture was accepted".to_owned());
    }

    let source_head = git_stdout(source, &["rev-parse", "master"])?;
    let source_head = source_head.trim();
    let negative_root = filtered
        .parent()
        .ok_or_else(|| "filtered fixture has no parent".to_owned())?;

    let context = clone_negative(filtered, negative_root, "negative-context")?;
    write_fixture(
        &context,
        "AGENTS.md",
        "must not cross the source boundary\n",
    )?;
    fixture_commit(&context, "inject context", "2001-02-01T00:00:00+00:00")?;
    let context_head = git_stdout(&context, &["rev-parse", "HEAD"])?;
    if verify_context_firewall(&context, context_head.trim(), "crates/imported").is_ok() {
        return Err("context negative fixture was accepted".to_owned());
    }

    let workflow = clone_negative(filtered, negative_root, "negative-workflow")?;
    write_fixture(
        &workflow,
        ".github/workflows/forbidden.yml",
        "forbidden: true\n",
    )?;
    fixture_commit(&workflow, "inject workflow", "2001-02-02T00:00:00+00:00")?;
    let workflow_head = git_stdout(&workflow, &["rev-parse", "HEAD"])?;
    if verify_context_firewall(&workflow, workflow_head.trim(), "crates/imported").is_ok() {
        return Err("GitHub-workflow negative fixture was accepted".to_owned());
    }

    let secret = clone_negative(filtered, negative_root, "negative-secret")?;
    write_fixture(
        &secret,
        "crates/imported/secret.pem",
        "-----BEGIN PRIVATE KEY-----\nfixture\n",
    )?;
    fixture_commit(&secret, "inject secret", "2001-02-03T00:00:00+00:00")?;
    let secret_head = git_stdout(&secret, &["rev-parse", "HEAD"])?;
    if verify_context_firewall(&secret, secret_head.trim(), "crates/imported").is_ok() {
        return Err("secret negative fixture was accepted".to_owned());
    }

    let bot = clone_negative(filtered, negative_root, "negative-bot")?;
    git_commit_with_actor(
        &bot,
        "inject bot identity",
        "github-actions[bot]",
        "41898282+github-actions[bot]@users.noreply.github.com",
        "2001-02-04T00:00:00+00:00",
        true,
    )?;
    let bot_head = git_stdout(&bot, &["rev-parse", "HEAD"])?;
    if verify_commit_identities(&bot, bot_head.trim()).is_ok() {
        return Err("bot-identity negative fixture was accepted".to_owned());
    }

    let license = clone_negative(filtered, negative_root, "negative-license")?;
    write_fixture(&license, "crates/imported/LICENSE", "GPL-3.0-only\n")?;
    fixture_commit(&license, "change license", "2001-02-05T00:00:00+00:00")?;
    let license_head = git_stdout(&license, &["rev-parse", "HEAD"])?;
    if verify_final_tree(
        source,
        &license,
        source_head,
        license_head.trim(),
        "src",
        "crates/imported",
    )
    .is_ok()
    {
        return Err("license/tree negative fixture was accepted".to_owned());
    }

    let attribution = clone_negative(filtered, negative_root, "negative-attribution")?;
    git_commit_with_actor(
        &attribution,
        "finish reusable source",
        "Wrong Author",
        "wrong-author@radroots.org",
        "2001-01-07T00:00:00+00:00",
        false,
    )?;
    let attribution_head = git_stdout(&attribution, &["rev-parse", "HEAD"])?;
    if verify_commit_metadata(source, &attribution, source_head, attribution_head.trim()).is_ok() {
        return Err("attribution negative fixture was accepted".to_owned());
    }

    let timestamp = clone_negative(filtered, negative_root, "negative-timestamp")?;
    git_commit_with_actor(
        &timestamp,
        "finish reusable source",
        "Radroots History Fixture",
        "history-fixture@radroots.org",
        "2002-01-07T00:00:00+00:00",
        false,
    )?;
    let timestamp_head = git_stdout(&timestamp, &["rev-parse", "HEAD"])?;
    if verify_commit_metadata(source, &timestamp, source_head, timestamp_head.trim()).is_ok() {
        return Err("timestamp negative fixture was accepted".to_owned());
    }

    let message = clone_negative(filtered, negative_root, "negative-message")?;
    git_commit_with_actor(
        &message,
        "replace the original message",
        "Radroots History Fixture",
        "history-fixture@radroots.org",
        "2001-01-07T00:00:00+00:00",
        false,
    )?;
    let message_head = git_stdout(&message, &["rev-parse", "HEAD"])?;
    if verify_commit_metadata(source, &message, source_head, message_head.trim()).is_ok() {
        return Err("message negative fixture was accepted".to_owned());
    }

    let follow = clone_negative(filtered, negative_root, "negative-follow")?;
    append_fixture(&follow, "crates/imported/item.txt", "unmapped\n")?;
    fixture_commit(
        &follow,
        "inject unmapped history",
        "2001-02-06T00:00:00+00:00",
    )?;
    if verify_follow_history(source, &follow, "src", "crates/imported").is_ok() {
        return Err("git-log-follow negative fixture was accepted".to_owned());
    }

    let mut broken_map = entries.to_vec();
    let replacement = broken_map
        .iter()
        .find_map(|entry| entry.target.clone())
        .ok_or_else(|| "negative fixture has no replacement target".to_owned())?;
    broken_map
        .last_mut()
        .ok_or_else(|| "negative fixture has no final map entry".to_owned())?
        .target = Some(replacement);
    if verify_filtered_history(source, filtered, &broken_map, "src", "crates/imported").is_ok() {
        return Err("commit-map topology negative fixture was accepted".to_owned());
    }

    let corrupt = clone_negative(filtered, negative_root, "negative-fsck")?;
    let payload = corrupt.join("fsck-negative-payload");
    fs::write(&payload, "unique fsck negative fixture payload\n")
        .map_err(|error| format!("write fsck negative payload: {error}"))?;
    let object = git_stdout(&corrupt, &["hash-object", "-w", path_arg(&payload)?])?;
    let object = object.trim();
    validate_oid(object, "fsck negative object")?;
    let object_path = corrupt
        .join(".git/objects")
        .join(&object[..2])
        .join(&object[2..]);
    fs::remove_file(&object_path).map_err(|error| {
        format!(
            "unlink exact temporary negative object {}: {error}",
            object_path.display()
        )
    })?;
    fs::write(&object_path, "corrupt")
        .map_err(|error| format!("corrupt negative object {}: {error}", object_path.display()))?;
    if git(&corrupt, &["fsck", "--full", "--strict"]).is_ok() {
        return Err("fsck negative fixture was accepted".to_owned());
    }
    Ok(())
}

fn clone_negative(source: &Path, root: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let target = root.join(name);
    git(root, &["clone", path_arg(source)?, path_arg(&target)?])?;
    git(
        &target,
        &["config", "user.name", "Radroots History Fixture"],
    )?;
    git(
        &target,
        &["config", "user.email", "history-fixture@radroots.org"],
    )?;
    Ok(target)
}

fn git_commit_with_actor(
    root: &Path,
    subject: &str,
    actor: &str,
    email: &str,
    timestamp: &str,
    allow_empty: bool,
) -> Result<(), String> {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .args(["commit", "--amend", "-m", subject])
        .env("GIT_AUTHOR_NAME", actor)
        .env("GIT_AUTHOR_EMAIL", email)
        .env("GIT_COMMITTER_NAME", actor)
        .env("GIT_COMMITTER_EMAIL", email)
        .env("GIT_AUTHOR_DATE", timestamp)
        .env("GIT_COMMITTER_DATE", timestamp);
    if allow_empty {
        command.arg("--allow-empty");
    }
    let output = command
        .output()
        .map_err(|error| format!("run negative commit fixture: {error}"))?;
    command_success(output, root, &["commit", "--amend"]).map(|_| ())
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    command_success(output, root, args).map(|_| ())
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(git_bytes(root, args)?)
        .map_err(|error| format!("git {} emitted non-UTF-8 output: {error}", args.join(" ")))
}

fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    command_success(output, root, args)
}

fn command_success(
    output: std::process::Output,
    root: &Path,
    args: &[&str],
) -> Result<Vec<u8>, String> {
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(format!(
        "git {} failed in {}: {}",
        args.join(" "),
        root.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn path_arg(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("path {} is not valid UTF-8", path.display()))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("open archive {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("read archive {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn to_unique_set<'a>(values: &'a [String], context: &str) -> Result<BTreeSet<&'a str>, String> {
    let set = values.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if set.len() != values.len() || set.iter().any(|value| value.trim().is_empty()) {
        return Err(format!("{context} entries must be nonempty and unique"));
    }
    Ok(set)
}

fn validate_sha256(value: &str, context: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{context} must be lowercase 64-hex SHA-256"));
    }
    Ok(())
}

fn validate_artifact_name(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.contains('/')
        || value.contains('\\')
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err("archive artifact must be a safe portable file name".to_owned());
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
        validate_baseline_contracts(&crate::workspace_root())
            .expect("checked-in consolidation baseline");
        validate_history_contract(&crate::workspace_root(), None)
            .expect("checked-in history contract");
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
    fn retired_import_targets_must_be_absent() {
        let root = tempfile::TempDir::new().expect("temporary workspace");
        let target = "imports/retired_core";
        let path_maps = vec![PathMap {
            source_id: "retired_core".to_owned(),
            source: "legacy/core".to_owned(),
            target: target.to_owned(),
            package: "retired_core".to_owned(),
            license: "MPL-2.0".to_owned(),
            disposition: "import_unique_behavior_then_retire".to_owned(),
        }];

        validate_retired_import_targets(root.path(), &path_maps).expect("absent retired import");
        fs::create_dir_all(root.path().join(target)).expect("create retired import fixture");
        assert!(validate_retired_import_targets(root.path(), &path_maps).is_err());
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

    #[test]
    fn archive_names_and_digests_are_strict() {
        assert!(validate_artifact_name("sdk-0123.bundle").is_ok());
        assert!(validate_artifact_name("../sdk.bundle").is_err());
        assert!(validate_artifact_name("sdk/bundle").is_err());
        assert!(validate_sha256(&"a".repeat(64), "digest").is_ok());
        assert!(validate_sha256(&"A".repeat(64), "digest").is_err());
    }

    #[test]
    fn merge_bearing_history_rehearsal_is_green() {
        run_history_rehearsal().expect("history rewrite rehearsal");
    }
}
