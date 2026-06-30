use std::fs;
use std::path::{Path, PathBuf};

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

pub fn run(args: &[String], root: &Path) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("forbidden-identifiers") => validate_forbidden_identifiers(root),
        _ => Err("unknown hygiene subcommand".to_string()),
    }
}

pub fn validate_forbidden_identifiers(root: &Path) -> Result<(), String> {
    let mut failures = Vec::new();
    reject_substrings(
        root,
        &[PathBuf::from("crates/relay_transport/src")],
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
            PathBuf::from("crates/events/src"),
            PathBuf::from("crates/events_codec/src"),
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
            "events_codec::trade::",
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
            PathBuf::from("build"),
        ],
        &["tangle"],
        "removed identifier 'tangle' must not reappear",
        &["tools/xtask/src/hygiene.rs"],
        &mut failures,
    );
    reject_binding_dependencies(root, &mut failures);
    reject_forbidden_crate_paths(root, &mut failures);
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
        PathBuf::from("crates/events/src"),
        PathBuf::from("crates/events_codec/src"),
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
    struct_name == "RadrootsOrderEnvelope"
        || struct_name == "RadrootsValidationReceiptTags"
        || struct_name == "RadrootsTradeListing"
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
            "crates/relay_transport/src/fetch.rs",
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
            "pub struct RadrootsOrderProjection { pub order_id: RadrootsOrderId, }\n",
        );
        validate_forbidden_identifiers(&root).expect("clean tree");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn forbidden_identifiers_reject_regressions() {
        let root = unique_temp_dir("dirty");
        write_file(
            &root,
            "crates/relay_transport/src/fetch.rs",
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
            "crates/events/src/kinds.rs",
            "pub const KIND_TRADE_LISTING_ORDER: u64 = 1;\npub const KIND_TRADE_LISTING_VALIDATE_REQ: u64 = 5321;\n",
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
        assert!(err.contains("wasm-bindgen"));
        assert!(err.contains("uniffi"));
        assert!(err.contains("crates/sql_wasm_bridge"));
        assert!(err.contains("scripts"));
        assert!(err.contains("contracts/sdk-exports"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn run_dispatches_forbidden_identifiers() {
        let root = unique_temp_dir("run");
        write_file(
            &root,
            "crates/relay_transport/src/fetch.rs",
            "fn fetch() { let _ = RadrootsEventIngest::new; }\n",
        );
        run(&["forbidden-identifiers".to_string()], &root).expect("hygiene run");
        let unknown = run(&["unknown".to_string()], &root).expect_err("unknown hygiene command");
        assert!(unknown.contains("unknown hygiene subcommand"));
        let _ = fs::remove_dir_all(root);
    }
}
