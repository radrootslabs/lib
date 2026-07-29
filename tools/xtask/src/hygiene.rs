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
        _ => Err("unknown hygiene subcommand".to_string()),
    }
}

pub fn validate_forbidden_identifiers(root: &Path) -> Result<(), String> {
    let mut failures = Vec::new();
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
        &["tools/xtask/src/hygiene.rs"],
        &mut failures,
    );
    reject_retired_listing_aliases(root, &mut failures);
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
