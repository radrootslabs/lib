use std::fs;
use std::path::Path;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, Token};

const NOSTR_MANIFEST_RELATIVE: &str = "crates/nostr/Cargo.toml";
const NOSTR_LIB_RELATIVE: &str = "crates/nostr/src/lib.rs";

pub(super) fn validate_feature_support(workspace_root: &Path) -> Result<(), String> {
    validate_nostr_manifest(workspace_root)?;
    let path = workspace_root.join(NOSTR_LIB_RELATIVE);
    let source =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    validate_nostr_std_only_source(&source)
}

fn validate_nostr_manifest(workspace_root: &Path) -> Result<(), String> {
    let path = workspace_root.join(NOSTR_MANIFEST_RELATIVE);
    let source =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let manifest = toml::from_str::<toml::Value>(&source)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;
    let features = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{NOSTR_MANIFEST_RELATIVE} must define features"))?;
    let default = string_array(features.get("default"), "radroots_nostr default feature")?;
    let std = string_array(features.get("std"), "radroots_nostr std feature")?;
    if default != ["std"] || !std.is_empty() {
        return Err(
            "radroots_nostr must remain std-only with default = [\"std\"] and an empty std marker"
                .to_owned(),
        );
    }
    Ok(())
}

fn string_array<'a>(value: Option<&'a toml::Value>, label: &str) -> Result<Vec<&'a str>, String> {
    value
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{label} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{label} entries must be strings"))
        })
        .collect()
}

fn validate_nostr_std_only_source(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse {NOSTR_LIB_RELATIVE} as Rust: {error}"))?;
    for attribute in &file.attrs {
        if attribute.path().is_ident("no_std") || cfg_attr_contains_no_std(attribute)? {
            return Err(format!(
                "{NOSTR_LIB_RELATIVE} must not declare a no_std crate mode"
            ));
        }
    }
    Ok(())
}

fn cfg_attr_contains_no_std(attribute: &Attribute) -> Result<bool, String> {
    if !attribute.path().is_ident("cfg_attr") {
        return Ok(false);
    }
    let Meta::List(list) = &attribute.meta else {
        return Err("cfg_attr must use list syntax".to_owned());
    };
    let entries = Punctuated::<Meta, Token![,]>::parse_terminated
        .parse2(list.tokens.clone())
        .map_err(|error| format!("parse cfg_attr in {NOSTR_LIB_RELATIVE}: {error}"))?;
    Ok(entries.iter().skip(1).any(meta_contains_no_std))
}

fn meta_contains_no_std(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("no_std"),
        Meta::List(list) => Punctuated::<Meta, Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .map(|entries| entries.iter().any(meta_contains_no_std))
            .unwrap_or(false),
        Meta::NameValue(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_only_source_rejects_direct_and_conditional_no_std_modes() {
        validate_nostr_std_only_source("#![forbid(unsafe_code)]\nextern crate alloc;")
            .expect("std-only source");
        for invalid in [
            "#![no_std]\nextern crate alloc;",
            "#![cfg_attr(not(feature = \"std\"), no_std)]\nextern crate alloc;",
        ] {
            assert!(validate_nostr_std_only_source(invalid).is_err());
        }
    }
}
