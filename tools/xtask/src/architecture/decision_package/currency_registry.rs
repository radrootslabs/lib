use std::{collections::BTreeSet, path::Path};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const CONTRACT: &str = "contracts/architecture/decisions/currency_registry.v1.json";
const DATA: &str = "crates/domain/data/currency_registry_v1.csv";
const LICENSE: &str = "crates/domain/data/currency_registry_v1.license";
const NOTICE: &str = "crates/domain/data/currency_registry_v1.notice";
const DATA_SHA256: &str = "60d8282b2d7ddc27929b097d4de9e76a900a64f1de4a5aa7a4cb3308b55b82d0";
const LICENSE_SHA256: &str = "665618a8c4b9ef6ceafb35482a3e010834fae9e9cbb22041b74310bcc52ac500";
const NOTICE_SHA256: &str = "c43ef0d0f9d6f64656ca6a619c3c43cd9cc2b378217ab19a083ca92255a91d7d";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: u16,
    contract_id: String,
    state: String,
    registry_id: String,
    implementation: String,
    upstream: Upstream,
    data: Data,
    notices: Notices,
    independent_crosscheck: Crosscheck,
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Upstream {
    repository: String,
    revision: String,
    path: String,
    sha256: String,
    license: String,
    license_sha256: String,
    copyright: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    path: String,
    sha256: String,
    entries: usize,
    minimum_exponent: u8,
    maximum_exponent: u8,
    current_currency_and_funds_only: bool,
    historic_entries_excluded: bool,
    no_minor_unit_entries_excluded: bool,
    guessed_default_exponents_allowed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Notices {
    license_path: String,
    license_sha256: String,
    notice_path: String,
    notice_sha256: String,
    source_distribution_required: bool,
    derived_artifact_inclusion_required_before_adoption: bool,
    crate_owned_rust_license_unchanged: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Crosscheck {
    authority: String,
    url: String,
    publication_date: String,
    source_sha256: String,
    all_selected_pairs_equal: bool,
    official_xml_redistributed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    runtime_io_allowed: bool,
    automatic_updates_allowed: bool,
    exchange_or_payment_semantics_allowed: bool,
    qualified_economics_or_agreement_authoring_claimed: bool,
}

pub(super) fn validate(root: &Path) -> Result<(), String> {
    let read = |path: &str, max: u64| {
        crate::safe_artifact_io::read_regular(root, Path::new(path), max)
            .map_err(|_| "currency registry: bounded regular source admission failed".to_owned())
    };
    validate_materials(
        &read(CONTRACT, 16_384)?,
        &read(DATA, 4_096)?,
        &read(LICENSE, 16_384)?,
        &read(NOTICE, 4_096)?,
    )
}

fn require(condition: bool, message: &'static str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(format!("currency registry: {message}"))
    }
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn validate_materials(
    raw: &[u8],
    data: &[u8],
    license: &[u8],
    notice: &[u8],
) -> Result<(), String> {
    require(raw.len() <= 16_384, "policy exceeds bound")?;
    let policy: Policy = serde_json::from_slice(raw)
        .map_err(|_| "currency registry: invalid closed policy".to_owned())?;
    require(
        policy.schema_version == 1
            && policy.contract_id == "radroots.currency_registry.v1"
            && policy.registry_id == policy.contract_id
            && policy.state == "pinned_data_source"
            && policy.implementation == "independent_rust_api_over_minimal_pinned_data",
        "identity or implementation scope changed",
    )?;
    validate_upstream(&policy.upstream, &policy.independent_crosscheck)?;
    let p = &policy.data;
    require(
        p.path == DATA
            && p.sha256 == DATA_SHA256
            && p.entries == 165
            && p.minimum_exponent == 0
            && p.maximum_exponent == 4
            && p.current_currency_and_funds_only
            && p.historic_entries_excluded
            && p.no_minor_unit_entries_excluded
            && !p.guessed_default_exponents_allowed,
        "fixed supported data contract changed",
    )?;
    validate_notices(&policy.notices, license, notice)?;
    require(
        !policy.limits.runtime_io_allowed
            && !policy.limits.automatic_updates_allowed
            && !policy.limits.exchange_or_payment_semantics_allowed
            && !policy
                .limits
                .qualified_economics_or_agreement_authoring_claimed,
        "metadata cannot imply effects or qualification",
    )?;
    require(
        data.len() <= 4_096 && digest(data) == DATA_SHA256,
        "pinned data digest mismatch",
    )?;
    let text =
        std::str::from_utf8(data).map_err(|_| "currency registry: invalid data UTF8".to_owned())?;
    let mut codes = BTreeSet::new();
    let mut previous = None;
    require(text.ends_with('\n'), "unterminated data")?;
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let bytes = line.as_bytes();
        require(
            bytes.len() == 5
                && bytes[..3].iter().all(u8::is_ascii_uppercase)
                && bytes[3] == b','
                && (b'0'..=b'4').contains(&bytes[4]),
            "invalid data row",
        )?;
        let code = &line[..3];
        require(
            previous.is_none_or(|p| p < code) && codes.insert(code),
            "duplicate or unsorted code",
        )?;
        previous = Some(code);
    }
    require(codes.len() == 165, "incomplete fixed registry")
}

fn validate_upstream(upstream: &Upstream, crosscheck: &Crosscheck) -> Result<(), String> {
    require(
        upstream.repository == "https://github.com/rust-iso/rust_iso4217"
            && upstream.revision == "ce6b74546b3645ce11866acc93117128d2f7b1e0"
            && upstream.path == "src/lib.rs"
            && upstream.sha256
                == "7401727ea4da05fcbf0d1d679705c0245989e83dbc697e831066f992875e183d"
            && upstream.license == "Apache-2.0"
            && upstream.license_sha256 == LICENSE_SHA256
            && upstream.copyright == "Copyright 2023-2026 rust_iso4217 contributors",
        "source or license provenance changed",
    )?;
    require(
        crosscheck.authority == "SIX ISO 4217 Maintenance Agency"
            && crosscheck.url
                == "https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/lists/list-one.xml"
            && crosscheck.publication_date == "2026-09-17"
            && crosscheck.source_sha256
                == "33139b438657d1cee116ba737807ea71d19d6de4b90f799a09c56f0cc6a1b0ff"
            && crosscheck.all_selected_pairs_equal
            && !crosscheck.official_xml_redistributed,
        "independent factual crosscheck scope changed",
    )
}

fn validate_notices(policy: &Notices, license: &[u8], notice: &[u8]) -> Result<(), String> {
    require(
        policy.license_path == LICENSE
            && policy.license_sha256 == LICENSE_SHA256
            && policy.notice_path == NOTICE
            && policy.notice_sha256 == NOTICE_SHA256
            && policy.source_distribution_required
            && policy.derived_artifact_inclusion_required_before_adoption
            && policy.crate_owned_rust_license_unchanged,
        "notice distribution requirement changed",
    )?;
    require(
        license.len() <= 16_384
            && digest(license) == LICENSE_SHA256
            && notice.len() <= 4_096
            && digest(notice) == NOTICE_SHA256,
        "license or notice digest mismatch",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const POLICY: &[u8] =
        include_bytes!("../../../../../contracts/architecture/decisions/currency_registry.v1.json");
    const CSV: &[u8] = include_bytes!("../../../../../crates/domain/data/currency_registry_v1.csv");
    const LICENSE_BYTES: &[u8] =
        include_bytes!("../../../../../crates/domain/data/currency_registry_v1.license");
    const NOTICE_BYTES: &[u8] =
        include_bytes!("../../../../../crates/domain/data/currency_registry_v1.notice");

    #[test]
    fn fixed_source_data_license_and_notice_bindings_are_valid() {
        validate_materials(POLICY, CSV, LICENSE_BYTES, NOTICE_BYTES).unwrap();
    }

    #[test]
    fn forged_self_consistent_hashes_or_stripped_attribution_cannot_replace_registry_v1() {
        let changed = String::from_utf8(CSV.to_vec())
            .unwrap()
            .replace("CAD,2", "CAD,4");
        let mut policy: serde_json::Value = serde_json::from_slice(POLICY).unwrap();
        policy["data"]["sha256"] = json!(digest(changed.as_bytes()));
        let raw = serde_json::to_vec(&policy).unwrap();
        assert!(validate_materials(&raw, changed.as_bytes(), LICENSE_BYTES, NOTICE_BYTES).is_err());
        assert!(
            validate_materials(POLICY, changed.as_bytes(), LICENSE_BYTES, NOTICE_BYTES).is_err()
        );
        assert!(validate_materials(POLICY, CSV, b"Apache-2.0", NOTICE_BYTES).is_err());
        assert!(validate_materials(POLICY, CSV, LICENSE_BYTES, b"notice").is_err());
    }

    #[test]
    fn closed_policy_rejects_unpinned_defaults_wrong_sources_and_effect_claims() {
        for (pointer, replacement) in [
            ("/upstream/license", json!("MIT")),
            ("/upstream/revision", json!("master")),
            ("/data/path", json!("../private.csv")),
            ("/data/entries", json!(164)),
            ("/data/guessed_default_exponents_allowed", json!(true)),
            (
                "/independent_crosscheck/official_xml_redistributed",
                json!(true),
            ),
            (
                "/notices/derived_artifact_inclusion_required_before_adoption",
                json!(false),
            ),
            ("/limits/automatic_updates_allowed", json!(true)),
            (
                "/limits/qualified_economics_or_agreement_authoring_claimed",
                json!(true),
            ),
        ] {
            let mut value: serde_json::Value = serde_json::from_slice(POLICY).unwrap();
            *value.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                validate_materials(
                    &serde_json::to_vec(&value).unwrap(),
                    CSV,
                    LICENSE_BYTES,
                    NOTICE_BYTES
                )
                .is_err(),
                "{pointer}"
            );
        }
        let mut value: serde_json::Value = serde_json::from_slice(POLICY).unwrap();
        value["unknown"] = json!(true);
        assert!(
            validate_materials(
                &serde_json::to_vec(&value).unwrap(),
                CSV,
                LICENSE_BYTES,
                NOTICE_BYTES
            )
            .is_err()
        );
        assert!(validate_materials(b"{}", CSV, LICENSE_BYTES, NOTICE_BYTES).is_err());
        assert!(validate_materials(&vec![b' '; 16_385], CSV, LICENSE_BYTES, NOTICE_BYTES).is_err());
    }
}
