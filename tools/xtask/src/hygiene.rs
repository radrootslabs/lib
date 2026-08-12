use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PROTOTYPE_CONTRACT_CONFIG_PATH: &str = "contracts/hygiene/prototype-contracts.v1.toml";
const PROTOTYPE_CONTRACT_SCHEMA: &str = "radroots.prototype-contract-source-guard.v1";
const PROTOTYPE_MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const PROTOTYPE_MAX_CONFIG_STRING_BYTES: usize = 1024;
const PROTOTYPE_MAX_CONFIG_PATHS: usize = 256;
const PROTOTYPE_MAX_CONFIG_EXTENSIONS: usize = 128;
const PROTOTYPE_MAX_CONFIG_PATTERNS: usize = 1024;
const PROTOTYPE_MAX_CONFIG_ALLOWLIST: usize = 4096;
const PROTOTYPE_MAX_REASON_BYTES: usize = 512;
const PROTOTYPE_MAX_CONFIGURED_SCAN_ENTRIES: usize = 100_000;
const PROTOTYPE_MAX_CONFIGURED_INVENTORY_BYTES: usize = 64 * 1024 * 1024;
const PROTOTYPE_MAX_CONFIGURED_FILE_BYTES: u64 = 64 * 1024 * 1024;
const PROTOTYPE_MAX_CONFIGURED_MATCHES: usize = 100_000;
const PROTOTYPE_MAX_CONFIGURED_REPORT_LINES: usize = 10_000;
const PROTOTYPE_MAX_GIT_STDERR_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum PrototypeGuardMode {
    ReportOnly,
    Strict,
}

impl PrototypeGuardMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ReportOnly => "report_only",
            Self::Strict => "strict",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum PrototypeMatchKind {
    Substring,
    WordPrefix,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrototypeGuardConfig {
    schema: String,
    mode: PrototypeGuardMode,
    scan: PrototypeScanConfig,
    limits: PrototypeGuardLimits,
    pattern: Vec<PrototypePattern>,
    #[serde(default)]
    allow: Vec<PrototypeAllow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrototypeScanConfig {
    roots: Vec<String>,
    path_roots: Vec<String>,
    path_excludes: Vec<String>,
    extensions: Vec<String>,
    extensionless_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrototypeGuardLimits {
    max_scan_entries: usize,
    max_inventory_bytes: usize,
    max_file_bytes: u64,
    max_matches: usize,
    max_reported_findings: usize,
    max_reported_allowlisted: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrototypePattern {
    id: String,
    needle: String,
    match_kind: PrototypeMatchKind,
    description: String,
    #[serde(default)]
    match_path: bool,
    #[serde(default)]
    path_prefixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrototypeAllow {
    pattern_id: String,
    path: String,
    line_contains: String,
    reason: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PrototypeFindingOrigin {
    Path,
    Content,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrototypeFinding {
    pattern_id: String,
    path: String,
    origin: PrototypeFindingOrigin,
    line: Option<usize>,
    excerpt: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrototypeAllowedMatch {
    finding: PrototypeFinding,
    reason: String,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct PrototypeGuardReport {
    findings: Vec<PrototypeFinding>,
    allowed: Vec<PrototypeAllowedMatch>,
}

const BINDING_DEPENDENCIES: &[&str] = &[
    "serde-wasm-bindgen",
    "ts-rs",
    "typeshare",
    "uniffi",
    "uniffi-build",
    "uniffi_build",
    "wasm-bindgen",
    "wasm-bindgen-futures",
    "wasm-bindgen-test",
];

const RETIRED_PROTOCOL_EVENT_SURFACE_PATTERNS: &[&str] = &[
    "KIND_LISTING_DRAFT",
    "KIND_CLASSIFIED_LISTING_DRAFT",
    "KIND_OPERATIONAL_LISTING_DRAFT",
    "KIND_ORDER_REVISION",
    "KIND_TRADE_QUESTION",
    "KIND_TRADE_ANSWER",
    "KIND_TRADE_DISCOUNT_REQUEST",
    "KIND_TRADE_DISCOUNT_OFFER",
    "KIND_TRADE_DISCOUNT_ACCEPT",
    "KIND_TRADE_FULFILLMENT_UPDATE",
    "KIND_TRADE_RECEIPT",
    "KIND_TRADE_LISTING_VALIDATION_REQUEST",
    "KIND_TRADE_LISTING_VALIDATION_RESULT",
    "KIND_TRADE_TRANSITION_PROOF_REQUEST",
    "KIND_TRADE_TRANSITION_PROOF_RESULT",
    "RADROOTS_SP1_TRADE_KIND_LISTING_DRAFT",
    "RADROOTS_SP1_TRADE_KIND_CLASSIFIED_LISTING_DRAFT",
    "RADROOTS_SP1_TRADE_KIND_OPERATIONAL_LISTING_DRAFT",
    "RadrootsListingDraft",
    "RadrootsClassifiedListingDraft",
    "RadrootsOperationalListingDraft",
    "RadrootsCanonicalListingDraft",
    "RadrootsListingDraftError",
    "RadrootsClassifiedListingDraftError",
    "RadrootsOperationalListingDraftError",
    "OrderRevision",
    "RadrootsOrderRevisionId",
    "TradeValidationListingRequest",
    "TradeValidationListingResult",
    "ListingValidationRequest",
    "ListingValidationResult",
    "TransitionProof",
    "TradeQuestion",
    "TradeAnswer",
    "TradeFulfillmentUpdated",
    "TradeReceipt",
    "canonicalize_listing_draft",
    "listing_draft",
    "order_revision",
    "pending_revision_event_id",
    "trade_answer",
    "trade_discount_accept",
    "trade_discount_offer",
    "trade_discount_request",
    "trade_fulfillment_update",
    "trade_listing_validation_request",
    "trade_listing_validation_result",
    "trade_order_revision_decision",
    "trade_order_revision_proposal",
    "trade_question",
    "trade_receipt",
    "trade_transition_proof_request",
    "trade_transition_proof_result",
];

const RETIRED_PROTOCOL_EVENT_SURFACE_ALLOWED_PATHS: &[&str] = &[
    "crates/event/src/dto.rs",
    "crates/protocol_contract_v1/src/lib.rs",
    "tools/xtask/src/hygiene.rs",
];

const RETIRED_LISTING_ALIAS_IDENTIFIER_TOKENS: &[&str] = &[
    "KIND_LISTING",
    "LISTING_EVENT_KINDS",
    "is_listing_kind",
    "is_listing_event_kind",
    "RADROOTS_SP1_TRADE_KIND_LISTING",
    "ListingTagOptions",
    "ListingDecodeError",
    "ListingParseError",
    "ListingProjection",
    "ListingInventoryAccounting",
    "ListingAddress",
    "ListingSnapshot",
    "RADROOTS_LISTING_PRODUCT_TAG_KEYS",
    "RadrootsCanonicalListingEdit",
    "RadrootsPublicListingAddress",
    "RadrootsPublicListingAddressError",
    "RadrootsTradeListing",
    "RadrootsTradeListingSubtotal",
    "RadrootsTradeListingTotal",
    "RadrootsTradeValidationListingError",
    "farm_listings_list_set",
    "farm_listings_list_set_from_listings",
    "listing_from_event",
    "listing_from_event_parts",
    "listing_from_nostr_event",
    "parse_listing_address",
    "parse_listing_address_str",
    "parse_listing_event",
    "parse_public_listing_address",
    "search_listing_projection",
    "to_json_wire_parts_with_kind",
    "validate_listing_event",
    "listing_tags",
    "listing_tags_with_options",
    "listing_tags_full",
    "listing_build_tags",
    "decode_listing_from_event_parts",
    "listing_markdown_content",
    "build_listing_mutation_draft",
    "canonicalize_listing_edit",
    "reduce_listing_inventory_accounting",
];

const RETIRED_LISTING_ALIAS_IDENTIFIER_PREFIXES: &[&str] = &["RadrootsListing"];
const RETIRED_LISTING_CONTRACT_ID: &str = "radroots.listing.published.v1";

pub fn run(args: &[String], root: &Path) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("forbidden-identifiers") => validate_forbidden_identifiers(root),
        Some("prototype-contracts") => run_prototype_contract_guard(&args[1..], root),
        _ => Err("unknown hygiene subcommand".to_string()),
    }
}

fn run_prototype_contract_guard(args: &[String], root: &Path) -> Result<(), String> {
    let (config_path, mode_override) = parse_prototype_guard_args(args)?;
    let config = load_prototype_guard_config(root, &config_path)?;
    let mode = mode_override.unwrap_or(config.mode);
    let report = scan_prototype_contracts(root, &config)?;
    print_prototype_guard_report(mode, &report, &config.limits);
    if mode == PrototypeGuardMode::Strict && !report.findings.is_empty() {
        return Err(format!(
            "prototype contract source guard found {} non-allowlisted match(es)",
            report.findings.len()
        ));
    }
    Ok(())
}

fn parse_prototype_guard_args(
    args: &[String],
) -> Result<(PathBuf, Option<PrototypeGuardMode>), String> {
    let mut config_path = PathBuf::from(PROTOTYPE_CONTRACT_CONFIG_PATH);
    let mut mode = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("prototype-contracts --config requires a path".to_string());
                };
                validate_repo_relative_path(value, "prototype guard config path")?;
                config_path = PathBuf::from(value);
                index += 2;
            }
            "--strict" => {
                set_prototype_mode(&mut mode, PrototypeGuardMode::Strict)?;
                index += 1;
            }
            "--report-only" => {
                set_prototype_mode(&mut mode, PrototypeGuardMode::ReportOnly)?;
                index += 1;
            }
            value => return Err(format!("unknown prototype-contracts argument: {value}")),
        }
    }
    Ok((config_path, mode))
}

fn set_prototype_mode(
    current: &mut Option<PrototypeGuardMode>,
    requested: PrototypeGuardMode,
) -> Result<(), String> {
    if current.replace(requested).is_some() {
        return Err("prototype-contracts accepts only one mode override".to_string());
    }
    Ok(())
}

fn load_prototype_guard_config(
    root: &Path,
    relative_path: &Path,
) -> Result<PrototypeGuardConfig, String> {
    validate_repo_relative_path(
        &relative_path.to_string_lossy(),
        "prototype guard config path",
    )?;
    let path = root.join(relative_path);
    reject_symlinked_path_components(root, relative_path, "prototype guard config path")?;
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("inspect prototype guard config {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "prototype guard config must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    let display = path.display().to_string();
    let source = read_bounded_prototype_input(&path, &display, PROTOTYPE_MAX_CONFIG_BYTES)?;
    let config: PrototypeGuardConfig = toml::from_str(&source)
        .map_err(|error| format!("parse prototype guard config {}: {error}", path.display()))?;
    validate_prototype_guard_config(&config)?;
    Ok(config)
}

fn validate_prototype_guard_config(config: &PrototypeGuardConfig) -> Result<(), String> {
    if config.schema != PROTOTYPE_CONTRACT_SCHEMA {
        return Err(format!(
            "prototype guard schema must be {PROTOTYPE_CONTRACT_SCHEMA}"
        ));
    }
    if config.scan.roots.is_empty() {
        return Err("prototype guard scan roots must not be empty".to_string());
    }
    if config.scan.path_roots.is_empty() {
        return Err("prototype guard path scan roots must not be empty".to_string());
    }
    if config.scan.extensions.is_empty() {
        return Err("prototype guard extensions must not be empty".to_string());
    }
    if config.limits.max_scan_entries == 0
        || config.limits.max_inventory_bytes == 0
        || config.limits.max_file_bytes == 0
        || config.limits.max_matches == 0
        || config.limits.max_reported_findings == 0
        || config.limits.max_reported_allowlisted == 0
        || config.limits.max_reported_findings > config.limits.max_matches
        || config.limits.max_reported_allowlisted > config.limits.max_matches
        || config.limits.max_scan_entries > PROTOTYPE_MAX_CONFIGURED_SCAN_ENTRIES
        || config.limits.max_inventory_bytes > PROTOTYPE_MAX_CONFIGURED_INVENTORY_BYTES
        || config.limits.max_file_bytes > PROTOTYPE_MAX_CONFIGURED_FILE_BYTES
        || config.limits.max_matches > PROTOTYPE_MAX_CONFIGURED_MATCHES
        || config.limits.max_reported_findings > PROTOTYPE_MAX_CONFIGURED_REPORT_LINES
        || config.limits.max_reported_allowlisted > PROTOTYPE_MAX_CONFIGURED_REPORT_LINES
    {
        return Err(
            "prototype guard limits must be positive, report limits must not exceed max_matches, and every value must remain within the compiled resource ceiling"
                .to_string(),
        );
    }
    if config.pattern.is_empty() {
        return Err("prototype guard patterns must not be empty".to_string());
    }
    if config.scan.roots.len() > PROTOTYPE_MAX_CONFIG_PATHS
        || config.scan.path_roots.len() > PROTOTYPE_MAX_CONFIG_PATHS
        || config.scan.path_excludes.len() > PROTOTYPE_MAX_CONFIG_PATHS
        || config.scan.extensions.len() > PROTOTYPE_MAX_CONFIG_EXTENSIONS
        || config.scan.extensionless_names.len() > PROTOTYPE_MAX_CONFIG_EXTENSIONS
        || config.pattern.len() > PROTOTYPE_MAX_CONFIG_PATTERNS
        || config.allow.len() > PROTOTYPE_MAX_CONFIG_ALLOWLIST
    {
        return Err("prototype guard configuration collection exceeds compiled limit".to_string());
    }

    let mut roots = HashSet::new();
    for root in &config.scan.roots {
        validate_repo_relative_path(root, "prototype guard scan root")?;
        if !roots.insert(root.as_str()) {
            return Err(format!("duplicate prototype guard scan root: {root}"));
        }
    }

    let mut path_roots = HashSet::new();
    for root in &config.scan.path_roots {
        validate_repo_relative_or_root_path(root, "prototype guard path scan root")?;
        if !path_roots.insert(root.as_str()) {
            return Err(format!("duplicate prototype guard path scan root: {root}"));
        }
    }

    let mut path_excludes = HashSet::new();
    for excluded in &config.scan.path_excludes {
        validate_repo_relative_path(excluded, "prototype guard path exclusion")?;
        if !config
            .scan
            .path_roots
            .iter()
            .any(|root| root == "." || repository_path_is_within(excluded, root))
        {
            return Err(format!(
                "prototype guard path exclusion is outside path scan roots: {excluded}"
            ));
        }
        if !path_excludes.insert(excluded.as_str()) {
            return Err(format!(
                "duplicate prototype guard path exclusion: {excluded}"
            ));
        }
    }

    let mut extensions = HashSet::new();
    for extension in &config.scan.extensions {
        if extension.is_empty()
            || extension.len() > 32
            || extension.starts_with('.')
            || !extension
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            return Err(format!(
                "invalid prototype guard extension (omit the dot): {extension:?}"
            ));
        }
        if !extensions.insert(extension.as_str()) {
            return Err(format!("duplicate prototype guard extension: {extension}"));
        }
    }

    let mut extensionless_names = HashSet::new();
    for name in &config.scan.extensionless_names {
        if name.is_empty()
            || name.len() > 128
            || !name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
        {
            return Err(format!(
                "invalid prototype guard extensionless file name: {name:?}"
            ));
        }
        if !extensionless_names.insert(name.as_str()) {
            return Err(format!(
                "duplicate prototype guard extensionless file name: {name}"
            ));
        }
    }

    let mut pattern_ids = HashSet::new();
    for pattern in &config.pattern {
        if pattern.id.is_empty()
            || pattern.id.len() > 64
            || !pattern.id.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
        {
            return Err(format!(
                "invalid prototype guard pattern id: {:?}",
                pattern.id
            ));
        }
        if !pattern_ids.insert(pattern.id.as_str()) {
            return Err(format!(
                "duplicate prototype guard pattern id: {}",
                pattern.id
            ));
        }
        if pattern.needle.is_empty()
            || pattern.needle.len() > PROTOTYPE_MAX_CONFIG_STRING_BYTES
            || pattern.needle.chars().any(char::is_control)
        {
            return Err(format!(
                "prototype guard pattern {} has an invalid needle",
                pattern.id
            ));
        }
        if pattern.match_kind == PrototypeMatchKind::WordPrefix && !pattern.needle.is_ascii() {
            return Err(format!(
                "prototype guard word-prefix pattern {} must use an ASCII needle",
                pattern.id
            ));
        }
        if pattern.description.trim().is_empty()
            || pattern.description.len() > PROTOTYPE_MAX_CONFIG_STRING_BYTES
            || pattern.description.chars().any(char::is_control)
        {
            return Err(format!(
                "prototype guard pattern {} requires a description",
                pattern.id
            ));
        }
        let mut prefixes = HashSet::new();
        for prefix in &pattern.path_prefixes {
            validate_repo_relative_path(prefix, "prototype guard pattern path prefix")?;
            if !config
                .scan
                .roots
                .iter()
                .any(|root| repository_path_is_within(prefix, root))
            {
                return Err(format!(
                    "prototype guard pattern {} path prefix is outside scan roots: {prefix}",
                    pattern.id
                ));
            }
            if !prefixes.insert(prefix.as_str()) {
                return Err(format!(
                    "duplicate path prefix for prototype guard pattern {}: {prefix}",
                    pattern.id
                ));
            }
        }
    }

    let mut allow_keys = HashSet::new();
    for allowed in &config.allow {
        if !pattern_ids.contains(allowed.pattern_id.as_str()) {
            return Err(format!(
                "prototype guard allowlist references unknown pattern: {:?}",
                allowed.pattern_id
            ));
        }
        validate_repo_relative_path(&allowed.path, "prototype guard allowlist path")?;
        if !config
            .scan
            .roots
            .iter()
            .any(|root| repository_path_is_within(&allowed.path, root))
        {
            return Err(format!(
                "prototype guard allowlist path is outside scan roots: {}",
                allowed.path
            ));
        }
        let pattern = config
            .pattern
            .iter()
            .find(|pattern| pattern.id == allowed.pattern_id)
            .expect("validated pattern id must resolve");
        if !pattern.path_prefixes.is_empty()
            && !pattern
                .path_prefixes
                .iter()
                .any(|prefix| repository_path_is_within(&allowed.path, prefix))
        {
            return Err(format!(
                "prototype guard allowlist path {} is outside pattern {} path prefixes",
                allowed.path, allowed.pattern_id
            ));
        }
        if allowed.line_contains.is_empty()
            || allowed.line_contains.len() > PROTOTYPE_MAX_CONFIG_STRING_BYTES
            || allowed.line_contains.chars().any(char::is_control)
        {
            return Err(format!(
                "prototype guard allowlist for {} requires one line fragment",
                allowed.pattern_id
            ));
        }
        if allowed.reason.trim().is_empty()
            || allowed.reason.len() > PROTOTYPE_MAX_REASON_BYTES
            || allowed.reason.chars().any(char::is_control)
        {
            return Err(format!(
                "prototype guard allowlist for {} requires a reason",
                allowed.pattern_id
            ));
        }
        let key = (
            allowed.pattern_id.as_str(),
            allowed.path.as_str(),
            allowed.line_contains.as_str(),
        );
        if !allow_keys.insert(key) {
            return Err(format!(
                "duplicate prototype guard allowlist entry: {} {}",
                allowed.pattern_id, allowed.path
            ));
        }
    }
    Ok(())
}

fn validate_repo_relative_path(value: &str, label: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.len() > PROTOTYPE_MAX_CONFIG_STRING_BYTES
        || value.chars().any(char::is_control)
        || value.contains('\\')
        || value.contains(':')
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "{label} must be a normalized repository-relative path: {value:?}"
        ));
    }
    Ok(())
}

fn validate_repo_relative_or_root_path(value: &str, label: &str) -> Result<(), String> {
    if value == "." {
        return Ok(());
    }
    validate_repo_relative_path(value, label)
}

fn reject_symlinked_path_components(
    root: &Path,
    relative_path: &Path,
    label: &str,
) -> Result<(), String> {
    let mut candidate = root.to_path_buf();
    for component in relative_path.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(format!(
                "{label} must contain only normalized path components: {}",
                relative_path.display()
            ));
        };
        candidate.push(component);
        let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
            format!("inspect {label} component {}: {error}", candidate.display())
        })?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{label} must not contain a symlinked component: {}",
                candidate.display()
            ));
        }
    }
    Ok(())
}

fn repository_path_is_within(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn scan_prototype_contracts(
    root: &Path,
    config: &PrototypeGuardConfig,
) -> Result<PrototypeGuardReport, String> {
    let extensions: HashSet<&str> = config.scan.extensions.iter().map(String::as_str).collect();
    let extensionless_names: HashSet<&str> = config
        .scan
        .extensionless_names
        .iter()
        .map(String::as_str)
        .collect();
    let mut inputs = PrototypeInputs::default();
    for relative_root in &config.scan.roots {
        reject_symlinked_path_components(
            root,
            Path::new(relative_root),
            "prototype guard scan root",
        )?;
    }
    for relative_root in &config.scan.path_roots {
        let relative_path = Path::new(relative_root);
        if relative_root != "." {
            reject_symlinked_path_components(
                root,
                relative_path,
                "prototype guard path scan root",
            )?;
        }
    }
    if let Some(governed_paths) = git_governed_paths(
        root,
        config.limits.max_scan_entries,
        config.limits.max_inventory_bytes,
    )? {
        for candidate in governed_paths {
            let relative = prototype_display_path(root, &candidate)?;
            if path_is_excluded(&relative, &config.scan.path_excludes) {
                continue;
            }
            if path_is_within_any_root(&relative, &config.scan.path_roots) {
                inputs.paths.push(candidate.clone());
            }
            if path_is_within_any_root(&relative, &config.scan.roots)
                && is_prototype_text_input(&candidate, &extensions, &extensionless_names)
            {
                inputs.files.push(candidate);
            }
        }
    } else {
        for relative_root in &config.scan.roots {
            collect_prototype_inputs(
                root,
                &root.join(relative_root),
                &extensions,
                &extensionless_names,
                &config.scan.path_excludes,
                config.limits.max_scan_entries,
                &mut inputs,
            )?;
        }
        for relative_root in &config.scan.path_roots {
            collect_prototype_paths(
                root,
                &root.join(relative_root),
                &config.scan.path_excludes,
                config.limits.max_scan_entries,
                &mut inputs.paths,
            )?;
        }
    }
    inputs.files.sort();
    inputs.files.dedup();
    inputs.paths.sort();
    inputs.paths.dedup();
    if inputs.paths.len() > config.limits.max_scan_entries {
        return Err(format!(
            "prototype guard scan contains {} entries, limit is {}",
            inputs.paths.len(),
            config.limits.max_scan_entries
        ));
    }

    let mut report = PrototypeGuardReport::default();
    let mut allow_match_counts = vec![0_usize; config.allow.len()];
    for candidate in inputs.paths {
        let path = prototype_display_path(root, &candidate)?;
        for pattern in config.pattern.iter().filter(|pattern| pattern.match_path) {
            if !prototype_pattern_matches(&path, &path, pattern) {
                continue;
            }
            record_prototype_match(
                config,
                &mut report,
                &mut allow_match_counts,
                PrototypeFinding {
                    pattern_id: pattern.id.clone(),
                    path: path.clone(),
                    origin: PrototypeFindingOrigin::Path,
                    line: None,
                    excerpt: bounded_excerpt(&path),
                },
                &path,
            )?;
        }
    }
    for file in inputs.files {
        let path = prototype_display_path(root, &file)?;
        let source = read_bounded_prototype_input(&file, &path, config.limits.max_file_bytes)?;
        for (line_index, line) in source.lines().enumerate() {
            for pattern in &config.pattern {
                if !prototype_pattern_matches(&path, line, pattern) {
                    continue;
                }
                let finding = PrototypeFinding {
                    pattern_id: pattern.id.clone(),
                    path: path.clone(),
                    origin: PrototypeFindingOrigin::Content,
                    line: Some(line_index + 1),
                    excerpt: bounded_excerpt(line),
                };
                record_prototype_match(
                    config,
                    &mut report,
                    &mut allow_match_counts,
                    finding,
                    line,
                )?;
            }
        }
    }
    for (allowed, match_count) in config.allow.iter().zip(allow_match_counts) {
        if match_count != 1 {
            return Err(format!(
                "prototype guard allowlist entry must match exactly one line (matched {match_count}): {} {} contains {:?}",
                allowed.pattern_id, allowed.path, allowed.line_contains
            ));
        }
    }
    report.findings.sort_by(|left, right| {
        (&left.path, left.origin, left.line, &left.pattern_id).cmp(&(
            &right.path,
            right.origin,
            right.line,
            &right.pattern_id,
        ))
    });
    report.allowed.sort_by(|left, right| {
        (
            &left.finding.path,
            left.finding.origin,
            left.finding.line,
            &left.finding.pattern_id,
        )
            .cmp(&(
                &right.finding.path,
                right.finding.origin,
                right.finding.line,
                &right.finding.pattern_id,
            ))
    });
    Ok(report)
}

fn git_governed_paths(
    root: &Path,
    max_scan_entries: usize,
    max_inventory_bytes: usize,
) -> Result<Option<Vec<PathBuf>>, String> {
    if !root.join(".git").exists() {
        return Ok(None);
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("run git source inventory for prototype guard: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Git source inventory stdout was not captured".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Git source inventory stderr was not captured".to_string())?;
    let stderr_reader = std::thread::spawn(move || read_bounded_and_drain(stderr));
    let inventory = parse_git_inventory(root, stdout, max_scan_entries, max_inventory_bytes);
    if inventory.is_err() {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .map_err(|error| format!("wait for Git source inventory: {error}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "Git source inventory stderr reader panicked".to_string())?
        .map_err(|error| format!("read Git source inventory stderr: {error}"))?;
    let paths = inventory?;
    if !status.success() {
        return Err(format!(
            "Git source inventory for prototype guard failed: {}",
            escape_report_text(String::from_utf8_lossy(&stderr).trim())
        ));
    }
    Ok(Some(paths))
}

fn read_bounded_and_drain(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut captured = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(captured);
        }
        let remaining = PROTOTYPE_MAX_GIT_STDERR_BYTES.saturating_sub(captured.len());
        captured.extend_from_slice(&buffer[..read.min(remaining)]);
    }
}

fn parse_git_inventory(
    root: &Path,
    mut reader: impl Read,
    max_scan_entries: usize,
    max_inventory_bytes: usize,
) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    let mut raw_path = Vec::new();
    let mut total_bytes = 0_usize;
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("read Git source inventory: {error}"))?;
        if read == 0 {
            break;
        }
        total_bytes = total_bytes.checked_add(read).ok_or_else(|| {
            "prototype guard Git source inventory byte count overflowed".to_string()
        })?;
        if total_bytes > max_inventory_bytes {
            return Err(format!(
                "prototype guard Git source inventory exceeds configured byte limit {max_inventory_bytes}"
            ));
        }
        for byte in &buffer[..read] {
            if *byte != 0 {
                if raw_path.len() >= PROTOTYPE_MAX_CONFIG_STRING_BYTES {
                    return Err(format!(
                        "prototype guard Git source path exceeds compiled byte limit {PROTOTYPE_MAX_CONFIG_STRING_BYTES}"
                    ));
                }
                raw_path.push(*byte);
                continue;
            }
            if raw_path.is_empty() {
                continue;
            }
            if paths.len() >= max_scan_entries {
                return Err(format!(
                    "prototype guard Git source inventory exceeds configured entry limit {max_scan_entries}"
                ));
            }
            let relative = std::str::from_utf8(&raw_path)
                .map_err(|error| format!("prototype guard Git path is not UTF-8: {error}"))?;
            validate_repo_relative_path(relative, "prototype guard Git source path")?;
            let candidate = root.join(relative);
            match fs::symlink_metadata(&candidate) {
                Ok(_) => {
                    reject_symlinked_path_components(
                        root,
                        Path::new(relative),
                        "prototype guard Git source path",
                    )?;
                    paths.push(candidate);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "inspect prototype guard Git source {}: {error}",
                        candidate.display()
                    ));
                }
            }
            raw_path.clear();
        }
    }
    if !raw_path.is_empty() {
        return Err(
            "prototype guard Git source inventory ended without a NUL delimiter".to_string(),
        );
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn path_is_within_any_root(path: &str, roots: &[String]) -> bool {
    roots
        .iter()
        .any(|root| root == "." || repository_path_is_within(path, root))
}

fn path_is_excluded(path: &str, excluded_prefixes: &[String]) -> bool {
    excluded_prefixes
        .iter()
        .any(|prefix| repository_path_is_within(path, prefix))
}

fn prototype_display_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        format!(
            "prototype guard path is outside repository root: {}",
            path.display()
        )
    })?;
    if relative.as_os_str().is_empty() {
        return Ok(".".to_string());
    }
    let mut components = Vec::new();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err("prototype guard path must contain only normal components".to_string());
        };
        components.push(
            component
                .to_str()
                .ok_or_else(|| "prototype guard path is not UTF-8".to_string())?,
        );
    }
    let relative = components.join("/");
    validate_repo_relative_path(&relative, "prototype guard source path")?;
    Ok(relative)
}

fn is_prototype_text_input(
    path: &Path,
    extensions: &HashSet<&str>,
    extensionless_names: &HashSet<&str>,
) -> bool {
    let extension_matches = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.contains(extension));
    let extensionless_matches = path.extension().is_none()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| extensionless_names.contains(name));
    extension_matches || extensionless_matches
}

fn record_prototype_match(
    config: &PrototypeGuardConfig,
    report: &mut PrototypeGuardReport,
    allow_match_counts: &mut [usize],
    finding: PrototypeFinding,
    matched_text: &str,
) -> Result<(), String> {
    let match_count = report.findings.len() + report.allowed.len();
    if match_count >= config.limits.max_matches {
        return Err(format!(
            "prototype guard match count exceeds configured limit {}",
            config.limits.max_matches
        ));
    }
    if let Some((allow_index, allowed)) = config.allow.iter().enumerate().find(|(_, allowed)| {
        allowed.pattern_id == finding.pattern_id
            && allowed.path == finding.path
            && matched_text.contains(&allowed.line_contains)
    }) {
        allow_match_counts[allow_index] += 1;
        report.allowed.push(PrototypeAllowedMatch {
            finding,
            reason: allowed.reason.clone(),
        });
    } else {
        report.findings.push(finding);
    }
    Ok(())
}

fn read_bounded_prototype_input(
    path: &Path,
    display_path: &str,
    max_file_bytes: u64,
) -> Result<String, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("open prototype guard input {display_path}: {error}"))?;
    let mut bytes = Vec::new();
    file.take(max_file_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read prototype guard input {display_path}: {error}"))?;
    if bytes.len() as u64 > max_file_bytes {
        return Err(format!(
            "prototype guard input exceeds {max_file_bytes} bytes: {display_path}"
        ));
    }
    String::from_utf8(bytes)
        .map_err(|error| format!("prototype guard input is not UTF-8 {display_path}: {error}"))
}

#[derive(Default)]
struct PrototypeInputs {
    files: Vec<PathBuf>,
    paths: Vec<PathBuf>,
}

fn collect_prototype_inputs(
    root: &Path,
    path: &Path,
    extensions: &HashSet<&str>,
    extensionless_names: &HashSet<&str>,
    excluded_prefixes: &[String],
    max_scan_entries: usize,
    inputs: &mut PrototypeInputs,
) -> Result<(), String> {
    let relative = prototype_display_path(root, path)?;
    if path_is_excluded(&relative, excluded_prefixes) {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "inspect required prototype guard path {}: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "prototype guard refuses symlinked scan input: {}",
            path.display()
        ));
    }
    if inputs.paths.len() >= max_scan_entries {
        return Err(format!(
            "prototype guard scan exceeds configured entry limit {max_scan_entries}"
        ));
    }
    inputs.paths.push(path.to_path_buf());
    if metadata.is_file() {
        if is_prototype_text_input(path, extensions, extensionless_names) {
            inputs.files.push(path.to_path_buf());
        }
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(path)
        .map_err(|error| format!("read prototype guard directory {}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "read prototype guard entry under {}: {error}",
                path.display()
            )
        })?;
        collect_prototype_inputs(
            root,
            &entry.path(),
            extensions,
            extensionless_names,
            excluded_prefixes,
            max_scan_entries,
            inputs,
        )?;
    }
    Ok(())
}

fn collect_prototype_paths(
    root: &Path,
    path: &Path,
    excluded_prefixes: &[String],
    max_scan_entries: usize,
    paths: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let relative = prototype_display_path(root, path)?;
    if excluded_prefixes
        .iter()
        .any(|prefix| repository_path_is_within(&relative, prefix))
    {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "inspect required prototype guard path {}: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "prototype guard refuses symlinked scan input: {}",
            path.display()
        ));
    }
    if paths.len() >= max_scan_entries {
        return Err(format!(
            "prototype guard scan exceeds configured entry limit {max_scan_entries}"
        ));
    }
    paths.push(path.to_path_buf());
    if !metadata.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(path)
        .map_err(|error| format!("read prototype guard directory {}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "read prototype guard entry under {}: {error}",
                path.display()
            )
        })?;
        collect_prototype_paths(
            root,
            &entry.path(),
            excluded_prefixes,
            max_scan_entries,
            paths,
        )?;
    }
    Ok(())
}

fn prototype_pattern_matches(path: &str, line: &str, pattern: &PrototypePattern) -> bool {
    let path_matches = pattern.path_prefixes.is_empty()
        || pattern
            .path_prefixes
            .iter()
            .any(|prefix| repository_path_is_within(path, prefix));
    path_matches
        && match pattern.match_kind {
            PrototypeMatchKind::Substring => line.contains(&pattern.needle),
            PrototypeMatchKind::WordPrefix => {
                let folded_line = line.to_ascii_lowercase();
                let folded_needle = pattern.needle.to_ascii_lowercase();
                folded_line.match_indices(&folded_needle).any(|(index, _)| {
                    folded_line[..index]
                        .chars()
                        .next_back()
                        .is_none_or(|character| !is_prototype_identifier_continue(character))
                })
            }
        }
}

fn is_prototype_identifier_continue(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn bounded_excerpt(line: &str) -> String {
    const LIMIT: usize = 240;
    let escaped = escape_report_text(line.trim());
    if escaped.chars().count() <= LIMIT {
        return escaped;
    }
    let mut excerpt: String = escaped.chars().take(LIMIT - 3).collect();
    excerpt.push_str("...");
    excerpt
}

fn escape_report_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}

fn print_prototype_guard_report(
    mode: PrototypeGuardMode,
    report: &PrototypeGuardReport,
    limits: &PrototypeGuardLimits,
) {
    println!(
        "prototype contract source guard: mode={} findings={} allowlisted={}",
        mode.as_str(),
        report.findings.len(),
        report.allowed.len()
    );
    for finding in report.findings.iter().take(limits.max_reported_findings) {
        print_prototype_finding("finding", finding, None);
    }
    if report.findings.len() > limits.max_reported_findings {
        println!(
            "... {} additional finding(s) omitted by report limit",
            report.findings.len() - limits.max_reported_findings
        );
    }
    for allowed in report.allowed.iter().take(limits.max_reported_allowlisted) {
        print_prototype_finding("allowlisted", &allowed.finding, Some(&allowed.reason));
    }
    if report.allowed.len() > limits.max_reported_allowlisted {
        println!(
            "... {} additional allowlisted match(es) omitted by report limit",
            report.allowed.len() - limits.max_reported_allowlisted
        );
    }
}

fn print_prototype_finding(label: &str, finding: &PrototypeFinding, reason: Option<&str>) {
    let path = escape_report_text(&finding.path);
    let location = finding
        .line
        .map_or_else(|| format!("{path} [path]"), |line| format!("{path}:{line}"));
    if let Some(reason) = reason {
        let reason = escape_report_text(reason);
        println!(
            "{label} {} {location}: {} ({reason})",
            finding.pattern_id,
            escape_report_text(&finding.excerpt)
        );
    } else {
        println!(
            "{label} {} {location}: {}",
            finding.pattern_id,
            escape_report_text(&finding.excerpt)
        );
    }
}

pub fn validate_forbidden_identifiers(root: &Path) -> Result<(), String> {
    let mut failures = Vec::new();
    let consolidation_active = consolidation_is_active(root);
    reject_substrings(
        root,
        &[PathBuf::from("crates/transport_nostr/src")],
        &["RadrootsEventIngest::verified"],
        "relay fetch must not bypass event-store verification",
        &[],
        &mut failures,
    );
    reject_substrings(
        root,
        &[PathBuf::from("crates/event_store/src")],
        &["last_created_at", "last_event_id"],
        "event-store projection cursors must use last_event_seq",
        &[],
        &mut failures,
    );
    reject_raw_protocol_strings(root, &mut failures);
    reject_substrings(
        root,
        &[
            PathBuf::from("crates/event/src"),
            PathBuf::from("crates/event_codec/src"),
            PathBuf::from("crates/trade/src"),
        ],
        &[
            "RadrootsTradeMessageType",
            "RadrootsTradeEnvelope",
            "RadrootsTradeMessagePayload",
            "RadrootsTradeQuestion",
            "RadrootsTradeAnswer",
            "RadrootsTradeDiscount",
            "RadrootsTradeOrder",
            "RadrootsActiveOrder",
            "RadrootsActiveTrade",
            "RadrootsTradeListingParseError",
            "RadrootsClassifiedListingTradeProjectionParseError",
            "RadrootsOperationalListingTradeProjectionParseError",
            "RadrootsTradeDomain",
            "radroots_sdk::trade::",
            "TradeListingParseError",
            "TradeListingEnvelope",
            "TradeListingMessage",
            "KIND_TRADE_ORDER",
            "TRADE_LISTING_KINDS",
            "build_envelope_draft",
            "parse_envelope",
            "public_trade",
            "events::trade::",
            "event_codec::trade::",
            "trade_order_economics_digest",
            "trade_revision",
            "trade_lifecycle",
            "reduce_active_order",
            "canonicalize_active_order",
            "active_trade_",
            "ActiveOrder",
            "active_order",
            "active order",
            "active trade",
            "RADROOTS_TRADE_LISTING_DOMAIN",
            "RADROOTS_TRADE_ENVELOPE_VERSION",
        ],
        "removed trade identifiers must not reappear",
        &[],
        &mut failures,
    );
    reject_substrings(
        root,
        &[PathBuf::from("crates"), PathBuf::from("contracts")],
        &[
            "KIND_TRADE_LISTING_ORDER",
            "KIND_TRADE_LISTING_QUESTION",
            "KIND_TRADE_LISTING_ANSWER",
            "KIND_TRADE_LISTING_DISCOUNT",
            "KIND_TRADE_LISTING_CANCEL",
            "KIND_TRADE_LISTING_FULFILLMENT",
            "KIND_TRADE_LISTING_RECEIPT",
            "KIND_TRADE_LISTING_VALIDATE_REQ",
            "KIND_TRADE_LISTING_VALIDATE_RES",
            "KIND_WORKER_TRADE_TRANSITION_PROOF_REQ",
            "KIND_WORKER_TRADE_TRANSITION_PROOF_RES",
        ],
        "removed trade and DVM kind constants must not reappear",
        &[],
        &mut failures,
    );
    reject_substrings(
        root,
        &[
            PathBuf::from("crates"),
            PathBuf::from("contracts"),
            PathBuf::from("tools"),
        ],
        RETIRED_PROTOCOL_EVENT_SURFACE_PATTERNS,
        "retired V1 public event surfaces must not reappear outside negative contract guards",
        RETIRED_PROTOCOL_EVENT_SURFACE_ALLOWED_PATHS,
        &mut failures,
    );
    reject_substrings(
        root,
        &[
            PathBuf::from("crates"),
            PathBuf::from("contracts"),
            PathBuf::from("tools"),
            PathBuf::from("build"),
        ],
        &["tangle"],
        "removed identifier 'tangle' must not reappear",
        &[
            "contracts/consolidation/baseline.v1.toml",
            "contracts/consolidation/imports/studio_app.commit-map.v1.json",
            "tools/xtask/src/sdk_generation/package_matrix.rs",
            "tools/xtask/src/hygiene.rs",
        ],
        &mut failures,
    );
    reject_retired_listing_aliases(root, &mut failures);
    if !consolidation_active {
        reject_binding_dependencies(root, &mut failures);
        reject_forbidden_crate_paths(root, &mut failures);
    }
    reject_existing_paths(
        root,
        &[
            "spec",
            "policy",
            "nix",
            "scripts",
            "bindings",
            "dist",
            "ffi",
            "generated",
            "packages",
            "pkg",
            "contracts/exports",
            "contracts/language-exports",
            "contracts/language-exports.toml",
            "contracts/language_exports",
            "contracts/language_exports.toml",
            "contracts/package-matrix",
            "contracts/package-matrix.toml",
            "contracts/package_matrix",
            "contracts/package_matrix.toml",
            "contracts/sdk-exports",
            "contracts/sdk_exports",
            "spec/exports",
            "spec/sdk-exports",
        ],
        "SDK, binding, generated-package, and retired layout paths must stay outside rr-rs",
        &mut failures,
    );

    if failures.is_empty() {
        println!("forbidden identifier hygiene passed");
        Ok(())
    } else {
        Err(format!(
            "forbidden identifier hygiene violations:\n{}",
            failures.join("\n")
        ))
    }
}

fn consolidation_is_active(root: &Path) -> bool {
    let Ok(workspace) = fs::read_to_string(root.join("Cargo.toml")) else {
        return false;
    };
    let Ok(workspace) = workspace.parse::<toml::Value>() else {
        return false;
    };
    let Some(repository) = workspace
        .get("workspace")
        .and_then(|value| value.get("package"))
        .and_then(|value| value.get("repository"))
        .and_then(toml::Value::as_str)
    else {
        return false;
    };
    let Ok(consolidation) =
        fs::read_to_string(root.join("contracts/consolidation/architecture.v1.toml"))
    else {
        return false;
    };
    let Ok(consolidation) = consolidation.parse::<toml::Value>() else {
        return false;
    };
    consolidation
        .get("canonical_rust_repository")
        .and_then(toml::Value::as_str)
        == Some(repository)
}

fn reject_retired_listing_aliases(root: &Path, failures: &mut Vec<String>) {
    let rel_roots = [
        PathBuf::from("crates"),
        PathBuf::from("contracts"),
        PathBuf::from("tools"),
        PathBuf::from("build"),
        PathBuf::from("dto_bindgen.toml"),
    ];

    for file in files_under(root, &rel_roots) {
        let rel = display_path(root, &file);
        if rel == "tools/xtask/src/hygiene.rs" {
            continue;
        }
        if is_retired_listing_module_path(&rel) {
            failures.push(format!(
                "retired listing public aliases must not reappear: {rel}: legacy listing module path"
            ));
        }

        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            if is_retired_listing_negative_guard(&rel, line) {
                continue;
            }
            let trimmed = line.trim_start();
            if trimmed.starts_with("let ") || trimmed.starts_with("assert") {
                continue;
            }

            for token in RETIRED_LISTING_ALIAS_IDENTIFIER_TOKENS {
                if contains_identifier_token(line, token) {
                    failures.push(format!(
                        "retired listing public aliases must not reappear: {}:{}: {}",
                        rel,
                        line_index + 1,
                        line.trim()
                    ));
                }
            }
            for prefix in RETIRED_LISTING_ALIAS_IDENTIFIER_PREFIXES {
                if contains_identifier_prefix(line, prefix) {
                    failures.push(format!(
                        "retired listing public aliases must not reappear: {}:{}: {}",
                        rel,
                        line_index + 1,
                        line.trim()
                    ));
                }
            }
            if line.contains(RETIRED_LISTING_CONTRACT_ID) {
                failures.push(format!(
                    "retired listing public aliases must not reappear: {}:{}: {}",
                    rel,
                    line_index + 1,
                    line.trim()
                ));
            }
            if is_listing_module_scope(&rel)
                && !is_canonical_event_listing_module_reference(&rel, line)
                && contains_retired_listing_module_reference(line)
            {
                failures.push(format!(
                    "retired listing public aliases must not reappear: {}:{}: {}",
                    rel,
                    line_index + 1,
                    line.trim()
                ));
            }
        }
    }
}

fn contains_identifier_token(line: &str, token: &str) -> bool {
    line.match_indices(token).any(|(index, _)| {
        let before = line[..index].chars().next_back();
        let after = line[index + token.len()..].chars().next();
        before.is_none_or(|ch| !is_identifier_continue(ch))
            && after.is_none_or(|ch| !is_identifier_continue(ch))
    })
}

fn contains_identifier_prefix(line: &str, prefix: &str) -> bool {
    line.match_indices(prefix).any(|(index, _)| {
        line[..index]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_identifier_continue(ch))
    })
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_retired_listing_module_path(rel: &str) -> bool {
    is_listing_module_scope(rel)
        && !is_canonical_event_listing_module_path(rel)
        && Path::new(rel).components().any(|component| {
            component.as_os_str() == "listing"
                || Path::new(component.as_os_str())
                    .file_stem()
                    .is_some_and(|stem| stem == "listing")
        })
}

fn is_canonical_event_listing_module_path(rel: &str) -> bool {
    rel == "crates/event/src/listing.rs" || rel.starts_with("crates/event/src/listing/")
}

fn is_listing_module_scope(rel: &str) -> bool {
    rel.starts_with("crates/event/src/")
        || rel.starts_with("crates/event/tests/")
        || rel.starts_with("crates/event_codec/src/")
        || rel.starts_with("crates/event_codec/tests/")
        || rel.starts_with("crates/trade/src/")
        || rel.starts_with("crates/trade/tests/")
        || rel.starts_with("contracts/conformance/vectors/")
        || rel == "dto_bindgen.toml"
}

fn contains_retired_listing_module_reference(line: &str) -> bool {
    line.match_indices("listing").any(|(index, _)| {
        let before = &line[..index];
        let after = line[index + "listing".len()..].chars().next();
        let starts_module_segment =
            before.ends_with("::") || before.trim_end().ends_with("mod") || before.ends_with('/');
        starts_module_segment && after.is_none_or(|ch| !is_identifier_continue(ch))
    })
}

fn is_canonical_event_listing_module_reference(rel: &str, line: &str) -> bool {
    (rel == "crates/event/src/lib.rs" && line.trim() == "pub mod listing;")
        || (rel.starts_with("crates/event/src/") && line.contains("crate::listing::"))
        || line.contains("radroots_event::listing::")
}

fn is_retired_listing_negative_guard(rel: &str, line: &str) -> bool {
    (rel == "crates/event/src/dto.rs"
        && (line.contains("\"ListingCancel\"") || line.contains("\"RadrootsListingCancel\"")))
        || (rel == "crates/event/src/contract/registry_v7/tests.rs"
            && line.contains("event_contract(\"radroots.listing.published.v1\").is_none()"))
}

fn reject_binding_dependencies(root: &Path, failures: &mut Vec<String>) {
    for file in manifest_files(root) {
        let rel = display_path(root, &file);
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let Ok(manifest) = content.parse::<toml::Value>() else {
            failures.push(format!("Cargo manifest must parse as TOML: {rel}"));
            continue;
        };
        reject_binding_dependencies_in_value(&manifest, &mut Vec::new(), &rel, failures);
    }
}

fn reject_binding_dependencies_in_value(
    value: &toml::Value,
    path: &mut Vec<String>,
    manifest_rel: &str,
    failures: &mut Vec<String>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    if path
        .last()
        .is_some_and(|segment| is_dependency_table_name(segment))
    {
        for dependency in BINDING_DEPENDENCIES {
            if table.contains_key(*dependency) {
                failures.push(format!(
                    "SDK, FFI, binding, and generated-package dependencies are forbidden in rr-rs: {manifest_rel}: {dependency} in [{}]",
                    path.join(".")
                ));
            }
        }
    }
    for (key, child) in table {
        path.push(key.clone());
        reject_binding_dependencies_in_value(child, path, manifest_rel, failures);
        path.pop();
    }
}

fn is_dependency_table_name(segment: &str) -> bool {
    matches!(
        segment,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    )
}

fn manifest_files(root: &Path) -> Vec<PathBuf> {
    let mut files = vec![root.join("Cargo.toml")];
    files.extend(files_under(
        root,
        &[PathBuf::from("crates"), PathBuf::from("tools")],
    ));
    files.retain(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml"));
    files.sort();
    files.dedup();
    files
}

fn reject_forbidden_crate_paths(root: &Path, failures: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root.join("crates")) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_forbidden_crate_dir_name(&name) {
            failures.push(format!(
                "SDK, FFI, binding, and generated-package crate paths are forbidden in rr-rs: crates/{name}"
            ));
        }
    }
}

fn is_forbidden_crate_dir_name(name: &str) -> bool {
    let lowercase = name.to_ascii_lowercase();
    lowercase.contains("ffi") || lowercase.contains("binding") || lowercase.contains("_wasm")
}

fn reject_existing_paths(root: &Path, rel_paths: &[&str], label: &str, failures: &mut Vec<String>) {
    for rel_path in rel_paths {
        if root.join(rel_path).exists() {
            failures.push(format!("{label}: {rel_path}"));
        }
    }
}

fn reject_substrings(
    root: &Path,
    rel_roots: &[PathBuf],
    patterns: &[&str],
    label: &str,
    ignored_rel_paths: &[&str],
    failures: &mut Vec<String>,
) {
    for file in files_under(root, rel_roots) {
        let rel = display_path(root, &file);
        if ignored_rel_paths.contains(&rel.as_str()) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            for pattern in patterns {
                if line.contains(pattern) {
                    failures.push(format!(
                        "{label}: {}:{}: {}",
                        rel,
                        line_index + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
}

fn reject_raw_protocol_strings(root: &Path, failures: &mut Vec<String>) {
    let rel_roots = [
        PathBuf::from("crates/event/src"),
        PathBuf::from("crates/event_codec/src"),
        PathBuf::from("crates/trade/src"),
    ];
    for file in files_under(root, &rel_roots) {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let mut struct_name = String::new();
        for (line_index, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("pub struct ") {
                struct_name = rest
                    .split(['<', '{', ' ', '('])
                    .next()
                    .unwrap_or_default()
                    .to_owned();
            }
            if trimmed == "}" {
                struct_name.clear();
            }
            if is_raw_protocol_field(trimmed) && !is_allowed_raw_boundary(&struct_name) {
                failures.push(format!(
                    "raw commercial protocol identifier String fields are forbidden: {}:{}: {}",
                    display_path(root, &file),
                    line_index + 1,
                    trimmed
                ));
            }
        }
    }
}

fn is_raw_protocol_field(line: &str) -> bool {
    [
        "pub order_id: String,",
        "pub listing_addr: String,",
        "pub revision_id: String,",
        "pub quote_id: String,",
        "pub primary_bin_id: String,",
        "pub bin_id: String,",
        "pub economics_digest: String,",
    ]
    .contains(&line)
}

fn is_allowed_raw_boundary(struct_name: &str) -> bool {
    struct_name == "OrderEnvelope"
        || struct_name == "RadrootsValidationReceiptTags"
        || struct_name == "RadrootsOperationalListingTradeProjection"
        || struct_name.ends_with("Projection")
        || struct_name.ends_with("Accounting")
        || struct_name.ends_with("Availability")
        || struct_name.ends_with("Reservation")
        || struct_name.ends_with("Issue")
        || struct_name.ends_with("NormalizedInventoryCount")
}

fn files_under(root: &Path, rel_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for rel_root in rel_roots {
        collect_files(root.join(rel_root), &mut files);
    }
    files.sort();
    files
}

fn collect_files(path: PathBuf, files: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::metadata(&path) else {
        return;
    };
    if metadata.is_file() {
        if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("json" | "md" | "nix" | "rs" | "sh" | "sql" | "toml")
        ) {
            files.push(path);
        }
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_files(entry.path(), files);
    }
}

fn display_path(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("radroots_xtask_hygiene_{prefix}_{ns}"))
    }

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, content).expect("write");
    }

    #[test]
    fn forbidden_identifiers_accept_clean_synthetic_tree() {
        let root = unique_temp_dir("clean");
        write_file(
            &root,
            "crates/transport_nostr/src/fetch.rs",
            "fn fetch() { let _ = RadrootsEventIngest::new; }\n",
        );
        write_file(
            &root,
            "crates/event_store/src/store.rs",
            "pub struct RadrootsProjectionCursor { pub last_event_seq: i64 }\n",
        );
        write_file(
            &root,
            "crates/trade/src/order.rs",
            "pub struct RadrootsOrderProjection { pub order_id: OrderId, }\npub enum RadrootsTradeFulfillmentStateV1 { NotStarted }\n",
        );
        write_file(
            &root,
            "crates/protocol_contract_v1/src/lib.rs",
            "const RETIRED: &str = \"listing_draft\";\n",
        );
        write_file(
            &root,
            "crates/event/src/dto.rs",
            "const OBSOLETE: &str = \"OrderRevision\";\n",
        );
        write_file(
            &root,
            "tools/xtask/src/hygiene.rs",
            "const GUARD: &str = \"trade_order_revision_proposal\";\n",
        );
        validate_forbidden_identifiers(&root).expect("clean tree");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn consolidated_repository_accepts_governed_binding_surfaces() {
        let root = unique_temp_dir("consolidated_bindings");
        write_file(
            &root,
            "Cargo.toml",
            "[workspace]\nmembers = []\n\n[workspace.package]\nrepository = \"https://github.com/radrootslabs/lib\"\n\n[workspace.dependencies]\nuniffi = \"0.29\"\nwasm-bindgen = \"0.2\"\n",
        );
        write_file(
            &root,
            "contracts/consolidation/architecture.v1.toml",
            "canonical_rust_repository = \"https://github.com/radrootslabs/lib\"\n",
        );
        fs::create_dir_all(root.join("crates/sdk_ffi")).expect("create FFI crate dir");
        fs::create_dir_all(root.join("crates/event_codec_wasm")).expect("create WASM crate dir");

        validate_forbidden_identifiers(&root).expect("consolidated binding surfaces are governed");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn forbidden_identifiers_reject_regressions() {
        let root = unique_temp_dir("dirty");
        write_file(
            &root,
            "crates/transport_nostr/src/fetch.rs",
            "fn fetch() { let _ = RadrootsEventIngest::verified; }\n",
        );
        write_file(
            &root,
            "crates/event_store/src/store.rs",
            "pub struct Cursor { pub last_event_id: String }\n",
        );
        write_file(
            &root,
            "crates/trade/src/order.rs",
            "pub struct BadOrder {\n    pub order_id: String,\n}\n",
        );
        write_file(&root, "contracts/events/social-events.md", "tangle\n");
        write_file(
            &root,
            "crates/event/src/kinds.rs",
            "pub const KIND_TRADE_LISTING_ORDER: u64 = 1;\npub const KIND_TRADE_LISTING_VALIDATE_REQ: u64 = 5321;\npub const KIND_LISTING_DRAFT: u32 = 30403;\npub const KIND_CLASSIFIED_LISTING_DRAFT: u32 = 30403;\npub const KIND_LISTING: u32 = 30402;\n",
        );
        write_file(
            &root,
            "contracts/conformance/retired.json",
            "{\"name\":\"trade_order_revision_proposal\",\"type\":\"TradeFulfillmentUpdated\"}\n",
        );
        write_file(
            &root,
            "Cargo.toml",
            "[workspace]\n[workspace.dependencies]\nwasm-bindgen = \"0.2\"\nuniffi = \"0.29\"\n",
        );
        fs::create_dir_all(root.join("crates/sql_wasm_bridge")).expect("create wasm crate dir");
        fs::create_dir_all(root.join("scripts")).expect("create scripts dir");
        fs::create_dir_all(root.join("contracts/sdk-exports")).expect("create sdk exports dir");
        let err = validate_forbidden_identifiers(&root).expect_err("dirty tree");
        assert!(err.contains("relay fetch must not bypass event-store verification"));
        assert!(err.contains("event-store projection cursors must use last_event_seq"));
        assert!(err.contains("raw commercial protocol identifier String fields are forbidden"));
        assert!(err.contains("removed identifier 'tangle' must not reappear"));
        assert!(err.contains("removed trade and DVM kind constants must not reappear"));
        assert!(err.contains("retired V1 public event surfaces must not reappear"));
        assert!(err.contains("retired listing public aliases must not reappear"));
        assert!(err.contains("wasm-bindgen"));
        assert!(err.contains("uniffi"));
        assert!(err.contains("crates/sql_wasm_bridge"));
        assert!(err.contains("scripts"));
        assert!(err.contains("contracts/sdk-exports"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn listing_alias_guard_is_token_and_path_aware() {
        let clean_root = unique_temp_dir("listing_alias_clean");
        write_file(&clean_root, "crates/event/src/lib.rs", "pub mod listing;\n");
        write_file(
            &clean_root,
            "crates/event/src/listing.rs",
            "pub struct CanonicalListing;\n",
        );
        write_file(
            &clean_root,
            "crates/event/src/kinds.rs",
            "pub const CLASSIFIED_LISTING_EVENT_KINDS: [u32; 1] = [KIND_CLASSIFIED_LISTING];\n",
        );
        write_file(
            &clean_root,
            "crates/event/src/operational_listing.rs",
            "pub struct OperationalListing;\n",
        );
        write_file(
            &clean_root,
            "crates/event/src/contract/registry_v7/tests.rs",
            "assert!(event_contract(\"radroots.listing.published.v1\").is_none());\n",
        );
        validate_forbidden_identifiers(&clean_root).expect("new listing taxonomy is accepted");
        let _ = fs::remove_dir_all(clean_root);

        let dirty_root = unique_temp_dir("listing_alias_dirty");
        write_file(
            &dirty_root,
            "crates/event/src/listing.rs",
            "pub const LISTING_EVENT_KINDS: [u32; 1] = [KIND_LISTING];\n",
        );
        write_file(
            &dirty_root,
            "contracts/conformance/listing.json",
            "{\"event_contract\":\"radroots.listing.published.v1\"}\n",
        );
        write_file(
            &dirty_root,
            "crates/event_codec/tests/listing.rs",
            "#[test]\nfn legacy_module_path() {}\n",
        );
        write_file(
            &dirty_root,
            "contracts/conformance/vectors/listing/build_tags.v1.json",
            "{}\n",
        );
        write_file(
            &dirty_root,
            "crates/event_codec/tests/retired_aliases.rs",
            "use radroots_trade::{RadrootsPublicListingAddress, listing_from_event, listing_tags_with_options};\nconst _: &str = RADROOTS_LISTING_PRODUCT_TAG_KEYS;\n",
        );
        let err =
            validate_forbidden_identifiers(&dirty_root).expect_err("old aliases are rejected");
        assert!(err.contains("retired listing public aliases must not reappear"));
        assert!(err.contains("legacy listing module path"));
        assert!(err.contains(RETIRED_LISTING_CONTRACT_ID));
        assert!(err.contains("crates/event_codec/tests/listing.rs"));
        assert!(err.contains("RadrootsPublicListingAddress"));
        assert!(err.contains("listing_from_event"));
        assert!(err.contains("listing_tags_with_options"));
        assert!(err.contains("contracts/conformance/vectors/listing/build_tags.v1.json"));
        assert!(err.contains("RADROOTS_LISTING_PRODUCT_TAG_KEYS"));
        let _ = fs::remove_dir_all(dirty_root);
    }

    fn prototype_config(
        mode: &str,
        pattern_id: &str,
        needle: &str,
        match_kind: &str,
        allow: Option<(&str, &str, &str)>,
    ) -> String {
        let allow = allow.map_or_else(String::new, |(path, line_contains, reason)| {
            format!(
                r#"
[[allow]]
pattern_id = "{pattern_id}"
path = "{path}"
line_contains = "{line_contains}"
reason = "{reason}"
"#
            )
        });
        format!(
            r#"schema = "{PROTOTYPE_CONTRACT_SCHEMA}"
mode = "{mode}"

[scan]
roots = ["src", "docs"]
path_roots = ["."]
path_excludes = [".git", "target"]
extensions = ["capnp", "md", "rs", "toml", "ts"]
extensionless_names = [".gitignore", "README"]

[limits]
max_scan_entries = 100
max_inventory_bytes = 4096
max_file_bytes = 1024
max_matches = 16
max_reported_findings = 4
max_reported_allowlisted = 4

[[pattern]]
id = "{pattern_id}"
needle = "{needle}"
match_kind = "{match_kind}"
description = "test prototype pattern"
{allow}"#
        )
    }

    #[test]
    fn prototype_guard_reports_matches_and_narrow_allowlists() {
        let root = unique_temp_dir("prototype_report");
        let needle = ["config", ".env"].concat();
        write_file(
            &root,
            "contracts/test-prototype-guard.toml",
            &prototype_config(
                "report_only",
                "config-environment",
                &needle,
                "substring",
                Some((
                    "docs/history.md",
                    "historical fixture",
                    "Historical fixture text is not an active configuration path.",
                )),
            ),
        );
        write_file(
            &root,
            "src/config.rs",
            &format!("const PROTOTYPE: &str = \"{needle}\";\n"),
        );
        write_file(
            &root,
            "docs/history.md",
            &format!("historical fixture: {needle}\nactive example: {needle}\n"),
        );

        let config =
            load_prototype_guard_config(&root, Path::new("contracts/test-prototype-guard.toml"))
                .expect("load prototype guard config");
        let report = scan_prototype_contracts(&root, &config).expect("scan prototype contracts");
        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.allowed.len(), 1);
        assert_eq!(report.findings[0].path, "docs/history.md");
        assert_eq!(report.findings[0].origin, PrototypeFindingOrigin::Content);
        assert_eq!(report.findings[1].path, "src/config.rs");
        assert_eq!(report.allowed[0].finding.path, "docs/history.md");

        run_prototype_contract_guard(
            &[
                "--config".to_string(),
                "contracts/test-prototype-guard.toml".to_string(),
            ],
            &root,
        )
        .expect("report-only prototype guard");
        let strict_error = run_prototype_contract_guard(
            &[
                "--config".to_string(),
                "contracts/test-prototype-guard.toml".to_string(),
                "--strict".to_string(),
            ],
            &root,
        )
        .expect_err("strict prototype guard rejects findings");
        assert!(strict_error.contains("2 non-allowlisted match(es)"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prototype_guard_word_prefix_avoids_embedded_false_positives() {
        let root = unique_temp_dir("prototype_word_prefix");
        let prefix = ["com", "pat"].concat();
        write_file(
            &root,
            "contracts/test-prototype-guard.toml",
            &prototype_config(
                "strict",
                "compatibility-concept",
                &prefix,
                "word_prefix",
                Some((
                    "docs/interoperability.md",
                    "compatible peer",
                    "External interoperability is not a compatibility implementation path.",
                )),
            ),
        );
        write_file(
            &root,
            "docs/interoperability.md",
            "incompatible input\ncompatible peer\nCompatReader\nÉcompatReader\n",
        );
        write_file(&root, "src/clean.rs", "fn current_contract() {}\n");

        let config =
            load_prototype_guard_config(&root, Path::new("contracts/test-prototype-guard.toml"))
                .expect("load prototype guard config");
        let report = scan_prototype_contracts(&root, &config).expect("scan prototype contracts");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].line, Some(3));
        assert_eq!(report.findings[0].excerpt, "CompatReader");
        assert_eq!(report.allowed.len(), 1);
        assert_eq!(report.allowed[0].finding.line, Some(2));

        let unsafe_path =
            parse_prototype_guard_args(&["--config".to_string(), "../outside.toml".to_string()])
                .expect_err("parent traversal must fail");
        assert!(unsafe_path.contains("normalized repository-relative path"));
        let duplicate_mode =
            parse_prototype_guard_args(&["--strict".to_string(), "--report-only".to_string()])
                .expect_err("duplicate mode must fail");
        assert!(duplicate_mode.contains("only one mode override"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prototype_guard_rejects_broad_or_stale_allowlists_and_symlinks() {
        let root = unique_temp_dir("prototype_allowlist_integrity");
        let needle = ["import", "_json"].concat();
        write_file(
            &root,
            "contracts/test-prototype-guard.toml",
            &prototype_config(
                "report_only",
                "state-import-identifier",
                &needle,
                "substring",
                Some((
                    "src/import.rs",
                    "import",
                    "A test allowance that is intentionally too broad.",
                )),
            ),
        );
        write_file(
            &root,
            "src/import.rs",
            &format!("fn {needle}() {{}}\nfn second_{needle}() {{}}\n"),
        );
        write_file(&root, "docs/README", "Current contract.\n");
        let config =
            load_prototype_guard_config(&root, Path::new("contracts/test-prototype-guard.toml"))
                .expect("load prototype guard config");
        let broad_error = scan_prototype_contracts(&root, &config)
            .expect_err("one allowance must not authorize multiple matching lines");
        assert!(broad_error.contains("must match exactly one line (matched 2)"));

        write_file(&root, "src/import.rs", "fn current_name() {}\n");
        let stale_error =
            scan_prototype_contracts(&root, &config).expect_err("stale allowance must fail closed");
        assert!(stale_error.contains("must match exactly one line (matched 0)"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let outside = unique_temp_dir("prototype_symlink_target");
            write_file(&outside, "forbidden.rs", &format!("fn {needle}() {{}}\n"));
            symlink(outside.join("forbidden.rs"), root.join("src/linked.rs"))
                .expect("create scan symlink");
            let symlink_error = scan_prototype_contracts(&root, &config)
                .expect_err("symlinked source must fail closed");
            assert!(symlink_error.contains("refuses symlinked scan input"));
            fs::remove_file(root.join("src/linked.rs")).expect("remove direct scan symlink");

            symlink(&outside, root.join("linked-root")).expect("create intermediate scan symlink");
            let linked_root_source = prototype_config(
                "report_only",
                "state-import-identifier",
                &needle,
                "substring",
                None,
            )
            .replace("roots = [\"src\", \"docs\"]", "roots = [\"linked-root\"]")
            .replace("path_roots = [\".\"]", "path_roots = [\"docs\"]")
            .replace(
                "path_excludes = [\".git\", \"target\"]",
                "path_excludes = []",
            );
            write_file(
                &root,
                "contracts/linked-root-guard.toml",
                &linked_root_source,
            );
            let linked_root_config =
                load_prototype_guard_config(&root, Path::new("contracts/linked-root-guard.toml"))
                    .expect("load intermediate symlink scan config");
            let linked_root_error = scan_prototype_contracts(&root, &linked_root_config)
                .expect_err("intermediate scan-root symlink must fail closed");
            assert!(linked_root_error.contains("must not contain a symlinked component"));

            fs::create_dir_all(root.join("configs")).expect("create config parent");
            symlink(&outside, root.join("configs/linked"))
                .expect("create intermediate config symlink");
            write_file(&outside, "guard.toml", &linked_root_source);
            let linked_config_error =
                load_prototype_guard_config(&root, Path::new("configs/linked/guard.toml"))
                    .expect_err("intermediate config symlink must fail closed");
            assert!(linked_config_error.contains("must not contain a symlinked component"));
            let _ = fs::remove_dir_all(outside);
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prototype_guard_scans_paths_non_rust_inputs_and_required_roots() {
        let root = unique_temp_dir("prototype_path_and_fixture_scan");
        let path_needle = ["identity", ".example.json"].concat();
        let content_needle = ["allow", "_generate_identity"].concat();
        let env_path_needle = [".env", ".example"].concat();
        let worker_path_needle = ["workers", "/rhi"].concat();
        let config_source = prototype_config(
            "report_only",
            "identity-example-path",
            &path_needle,
            "substring",
            None,
        )
        .replace(
            "roots = [\"src\", \"docs\"]",
            "roots = [\"src\", \"docs\", \".gitignore\"]",
        )
        .replace(
            "description = \"test prototype pattern\"",
            "description = \"test prototype pattern\"\nmatch_path = true",
        ) + &format!(
            r#"
[[pattern]]
id = "identity-generation-content"
needle = "{content_needle}"
match_kind = "substring"
description = "test non-Rust fixture pattern"

[[pattern]]
id = "environment-example-path"
needle = "{env_path_needle}"
match_kind = "substring"
description = "test environment example path"
match_path = true

[[pattern]]
id = "worker-directory-path"
needle = "{worker_path_needle}"
match_kind = "substring"
description = "test worker directory path"
match_path = true
"#,
        );
        write_file(&root, "contracts/test-prototype-guard.toml", &config_source);
        let identity_path = format!("src/{path_needle}");
        let env_path = env_path_needle.clone();
        let worker_path = format!("src/{worker_path_needle}");
        write_file(&root, &identity_path, "{}\n");
        write_file(&root, &env_path, "CURRENT_SETTING=true\n");
        write_file(&root, ".gitignore", &format!("# {content_needle}\n"));
        fs::create_dir_all(root.join(&worker_path)).expect("create forbidden worker path");
        write_file(
            &root,
            "src/generated.ts",
            &format!("export const flag = \"{content_needle}\";\n"),
        );
        write_file(&root, "src/service.capnp", &format!("# {content_needle}\n"));
        write_file(&root, "docs/README", &format!("{content_needle}\n"));

        let config =
            load_prototype_guard_config(&root, Path::new("contracts/test-prototype-guard.toml"))
                .expect("load prototype guard config");
        let report = scan_prototype_contracts(&root, &config).expect("scan active textual inputs");
        assert_eq!(report.findings.len(), 7);
        for path in [&env_path, &identity_path, &worker_path] {
            assert!(report.findings.iter().any(|finding| {
                finding.path == *path
                    && finding.origin == PrototypeFindingOrigin::Path
                    && finding.line.is_none()
            }));
        }
        for path in [
            ".gitignore",
            "docs/README",
            "src/generated.ts",
            "src/service.capnp",
        ] {
            assert!(report.findings.iter().any(|finding| {
                finding.path == path && finding.origin == PrototypeFindingOrigin::Content
            }));
        }

        fs::remove_dir_all(root.join("docs")).expect("remove required scan root");
        let missing_error = scan_prototype_contracts(&root, &config)
            .expect_err("a missing required scan root must fail closed");
        assert!(missing_error.contains("prototype guard scan root component"));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn prototype_guard_git_inventory_ignores_workstation_symlinks() {
        use std::os::unix::fs::symlink;

        let root = unique_temp_dir("prototype_git_inventory");
        let needle = [".env", ".example"].concat();
        let config_source = prototype_config(
            "report_only",
            "environment-example-path",
            &needle,
            "substring",
            None,
        )
        .replace(
            "description = \"test prototype pattern\"",
            "description = \"test prototype pattern\"\nmatch_path = true",
        );
        write_file(&root, "contracts/test-prototype-guard.toml", &config_source);
        write_file(&root, "src/current.rs", "fn current_contract() {}\n");
        write_file(&root, "docs/README", "Current contract.\n");
        let ignore = format!(".direnv/\nresult\n.env.*\n!{needle}\n");
        write_file(&root, ".gitignore", &ignore);
        write_file(&root, &needle, "CURRENT_SETTING=true\n");
        let init = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .expect("run git init");
        assert!(init.success());

        let config =
            load_prototype_guard_config(&root, Path::new("contracts/test-prototype-guard.toml"))
                .expect("load prototype guard config");
        let before = scan_prototype_contracts(&root, &config).expect("scan governed Git source");
        assert_eq!(before.findings.len(), 1);
        assert_eq!(before.findings[0].path, needle);
        assert_eq!(before.findings[0].origin, PrototypeFindingOrigin::Path);

        let control_path = "src/control\n\u{1b}.rs";
        write_file(&root, control_path, "fn current_contract() {}\n");
        let control_error = scan_prototype_contracts(&root, &config)
            .expect_err("control characters in Git paths must fail closed");
        assert_eq!(control_error.lines().count(), 1);
        assert!(!control_error.contains('\u{1b}'));
        assert!(control_error.contains("control\\n\\u{1b}.rs"));
        fs::remove_file(root.join(control_path)).expect("remove control-character path");

        let outside = unique_temp_dir("prototype_ignored_symlink_target");
        write_file(&outside, "ignored.rs", "Current ignored cache.\n");
        fs::create_dir_all(root.join(".direnv")).expect("create ignored environment cache");
        symlink(outside.join("ignored.rs"), root.join(".direnv/linked.rs"))
            .expect("create ignored environment symlink");
        symlink(&outside, root.join("result")).expect("create ignored Nix result symlink");

        let after = scan_prototype_contracts(&root, &config)
            .expect("ignored workstation symlinks must not enter the source inventory");
        assert_eq!(after, before);
        let _ = fs::remove_dir_all(outside);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prototype_guard_git_inventory_parser_enforces_byte_and_entry_bounds() {
        let root = unique_temp_dir("prototype_git_inventory_bounds");
        write_file(&root, "src/one.rs", "fn one() {}\n");
        write_file(&root, "src/two.rs", "fn two() {}\n");
        let inventory = b"src/one.rs\0src/two.rs\0";

        let parsed = parse_git_inventory(&root, &inventory[..], 2, inventory.len())
            .expect("bounded inventory must parse");
        assert_eq!(parsed.len(), 2);

        let byte_error = parse_git_inventory(&root, &inventory[..], 2, inventory.len() - 1)
            .expect_err("inventory bytes above the configured ceiling must fail");
        assert!(byte_error.contains("configured byte limit"));

        let entry_error = parse_git_inventory(&root, &inventory[..], 1, inventory.len())
            .expect_err("inventory entries above the configured ceiling must fail");
        assert!(entry_error.contains("configured entry limit 1"));

        let delimiter_error =
            parse_git_inventory(&root, &inventory[..inventory.len() - 1], 2, 4096)
                .expect_err("unterminated Git inventory must fail");
        assert!(delimiter_error.contains("without a NUL delimiter"));

        let escaped = bounded_excerpt("safe\tvalue\u{1b}[31m");
        assert_eq!(escaped, "safe\\tvalue\\u{1b}[31m");
        assert!(!escaped.chars().any(char::is_control));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prototype_guard_bounds_configuration_bytes_counts_and_reasons() {
        let root = unique_temp_dir("prototype_config_bounds");
        let config_path = "contracts/test-prototype-guard.toml";
        write_file(
            &root,
            config_path,
            &"x".repeat(PROTOTYPE_MAX_CONFIG_BYTES as usize + 1),
        );
        let bytes_error = load_prototype_guard_config(&root, Path::new(config_path))
            .expect_err("oversized configuration must fail before parsing");
        assert!(bytes_error.contains("exceeds 1048576 bytes"));

        let needle = ["config", ".env"].concat();
        let oversized_reason = "r".repeat(PROTOTYPE_MAX_REASON_BYTES + 1);
        let reason_config = prototype_config(
            "report_only",
            "config-environment",
            &needle,
            "substring",
            Some(("src/config.rs", "prototype", &oversized_reason)),
        );
        write_file(&root, config_path, &reason_config);
        let reason_error = load_prototype_guard_config(&root, Path::new(config_path))
            .expect_err("oversized printed reason must fail validation");
        assert!(reason_error.contains("requires a reason"));

        let mut pattern_config =
            prototype_config("report_only", "pattern-0", &needle, "substring", None);
        for index in 1..=PROTOTYPE_MAX_CONFIG_PATTERNS {
            pattern_config.push_str(&format!(
                r#"
[[pattern]]
id = "pattern-{index}"
needle = "current-{index}"
match_kind = "substring"
description = "bounded pattern"
"#,
            ));
        }
        write_file(&root, config_path, &pattern_config);
        let count_error = load_prototype_guard_config(&root, Path::new(config_path))
            .expect_err("excessive pattern count must fail validation");
        assert!(count_error.contains("configuration collection exceeds compiled limit"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prototype_guard_enforces_file_and_match_resource_bounds() {
        let root = unique_temp_dir("prototype_resource_bounds");
        let needle = ["config", ".env"].concat();
        write_file(
            &root,
            "contracts/test-prototype-guard.toml",
            &prototype_config(
                "report_only",
                "config-environment",
                &needle,
                "substring",
                None,
            ),
        );
        write_file(&root, "docs/README", "Current contract.\n");
        write_file(&root, "src/large.rs", &"x".repeat(1025));
        let config =
            load_prototype_guard_config(&root, Path::new("contracts/test-prototype-guard.toml"))
                .expect("load prototype guard config");
        let size_error = scan_prototype_contracts(&root, &config)
            .expect_err("oversized source input must fail closed");
        assert!(size_error.contains("exceeds 1024 bytes"));

        write_file(&root, "src/large.rs", &format!("{needle}\n").repeat(17));
        let match_error = scan_prototype_contracts(&root, &config)
            .expect_err("excessive matches must fail closed");
        assert!(match_error.contains("match count exceeds configured limit 16"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn run_dispatches_forbidden_identifiers() {
        let root = unique_temp_dir("run");
        write_file(
            &root,
            "crates/transport_nostr/src/fetch.rs",
            "fn fetch() { let _ = RadrootsEventIngest::new; }\n",
        );
        run(&["forbidden-identifiers".to_string()], &root).expect("hygiene run");
        let unknown = run(&["unknown".to_string()], &root).expect_err("unknown hygiene command");
        assert!(unknown.contains("unknown hygiene subcommand"));
        let _ = fs::remove_dir_all(root);
    }
}
