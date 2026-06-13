use std::fs;
use std::path::{Path, PathBuf};

pub fn run(args: &[String], root: &Path) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("invariants") => validate_invariants(root),
        _ => Err("unknown phase1-1 subcommand".to_string()),
    }
}

pub fn validate_invariants(root: &Path) -> Result<(), String> {
    let mut failures = Vec::new();
    reject_substrings(
        root,
        &[PathBuf::from("crates/relay_transport/src")],
        &["RadrootsEventIngest::verified"],
        "relay fetch must not bypass event-store verification",
        &mut failures,
    );
    reject_substrings(
        root,
        &[PathBuf::from("crates/event_store/src")],
        &["last_created_at", "last_event_id"],
        "event-store projection cursors must use last_event_seq",
        &mut failures,
    );
    reject_raw_protocol_strings(root, &mut failures);
    reject_substrings(
        root,
        &[
            PathBuf::from("crates/events/src"),
            PathBuf::from("crates/events_codec/src"),
            PathBuf::from("crates/trade/src"),
            PathBuf::from("crates/sdk/src"),
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
            "radroots_sdk::trade::",
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
        "legacy trade identifiers must not reappear",
        &mut failures,
    );

    if failures.is_empty() {
        println!("phase1-1 invariants passed");
        Ok(())
    } else {
        Err(format!(
            "phase1-1 invariant violations:\n{}",
            failures.join("\n")
        ))
    }
}

fn reject_substrings(
    root: &Path,
    rel_roots: &[PathBuf],
    patterns: &[&str],
    label: &str,
    failures: &mut Vec<String>,
) {
    for file in files_under(root, rel_roots) {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            for pattern in patterns {
                if line.contains(pattern) {
                    failures.push(format!(
                        "{label}: {}:{}: {}",
                        display_path(root, &file),
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
    .iter()
    .any(|field| line == *field)
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
            Some("rs" | "sql" | "sh")
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
        std::env::temp_dir().join(format!("radroots_xtask_phase1_1_{prefix}_{ns}"))
    }

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, content).expect("write");
    }

    #[test]
    fn invariants_accept_clean_synthetic_tree() {
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
        validate_invariants(&root).expect("clean tree");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invariants_reject_phase1_regressions() {
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
        let err = validate_invariants(&root).expect_err("dirty tree");
        assert!(err.contains("relay fetch must not bypass event-store verification"));
        assert!(err.contains("event-store projection cursors must use last_event_seq"));
        assert!(err.contains("raw commercial protocol identifier String fields are forbidden"));
        let _ = fs::remove_dir_all(root);
    }
}
