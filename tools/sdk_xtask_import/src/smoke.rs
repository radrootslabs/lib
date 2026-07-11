use std::{fs, path::Path, process::Command};

use crate::{
    fs::workspace_root,
    wasm::{ResolvedRustToolchain, resolve_rust_toolchain},
};

pub fn run(args: &[String]) -> Result<(), String> {
    match args {
        [target] if target == "knowledge-rust-local" => knowledge_rust_local(),
        _ => Err("usage: cargo xtask smoke knowledge-rust-local".to_owned()),
    }
}

fn knowledge_rust_local() -> Result<(), String> {
    let root = workspace_root()?;
    let sdk_path = root.join("crates/sdk");
    let toolchain = resolve_rust_toolchain(&root)?;
    let nostr_version = exact_workspace_dependency_pin(&root.join("Cargo.toml"), "nostr")?;
    let tempdir =
        tempfile::tempdir().map_err(|error| format!("failed to create smoke tempdir: {error}"))?;
    write_consumer(
        &tempdir.path().join("Cargo.toml"),
        &sdk_path,
        &nostr_version,
    )?;
    let src_dir = tempdir.path().join("src");
    fs::create_dir_all(&src_dir)
        .map_err(|error| format!("failed to create {}: {error}", src_dir.display()))?;
    fs::write(src_dir.join("main.rs"), CONSUMER_MAIN)
        .map_err(|error| format!("failed to write smoke consumer main.rs: {error}"))?;
    let mut command = smoke_cargo_check_command(tempdir.path(), &toolchain);
    let status = command
        .status()
        .map_err(|error| format!("failed to run smoke cargo check: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "knowledge Rust local smoke failed with status {status}"
        ))
    }
}

fn smoke_cargo_check_command(path: &Path, toolchain: &ResolvedRustToolchain) -> Command {
    let mut command = Command::new(&toolchain.cargo);
    command.arg("check").arg("--quiet").current_dir(path);
    toolchain.apply_to_command(&mut command);
    command
}

fn write_consumer(path: &Path, sdk_path: &Path, nostr_version: &str) -> Result<(), String> {
    let cargo_toml = render_consumer_manifest(sdk_path, nostr_version)?;
    fs::write(path, cargo_toml)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn render_consumer_manifest(sdk_path: &Path, nostr_version: &str) -> Result<String, String> {
    let sdk_path = serde_json::to_string(&sdk_path.to_string_lossy())
        .map_err(|error| format!("failed to render SDK path: {error}"))?;
    Ok(format!(
        r#"[package]
name = "radroots-sdk-knowledge-smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
radroots_sdk = {{ path = {sdk_path}, features = ["std", "serde", "serde_json", "nostr", "knowledge"] }}
nostr = "{nostr_version}"
"#
    ))
}

fn exact_workspace_dependency_pin(manifest_path: &Path, name: &str) -> Result<String, String> {
    let contents = fs::read_to_string(manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let version = workspace_dependency_version_from(&contents, name)?;
    exact_dependency_pin(name, &version)
}

fn workspace_dependency_version_from(contents: &str, name: &str) -> Result<String, String> {
    let value = contents
        .parse::<toml::Value>()
        .map_err(|error| format!("failed to parse workspace manifest: {error}"))?;
    let dependency = value
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(|dependencies| dependencies.get(name))
        .ok_or_else(|| format!("workspace dependency {name} is not defined"))?;
    if let Some(version) = dependency.as_str() {
        return Ok(version.to_owned());
    }
    dependency
        .get("version")
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("workspace dependency {name} must define version"))
}

fn exact_dependency_pin(name: &str, version: &str) -> Result<String, String> {
    let trimmed = version.trim().strip_prefix('=').unwrap_or(version.trim());
    if !is_exact_crates_version(trimmed) {
        return Err(format!(
            "workspace dependency {name} must use a full exact version for smoke pinning: {version}"
        ));
    }
    Ok(format!("={trimmed}"))
}

fn is_exact_crates_version(version: &str) -> bool {
    let base = version
        .split_once(['-', '+'])
        .map_or(version, |(base, _)| base);
    let parts = base.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
}

const CONSUMER_MAIN: &str = r#"use nostr::{EventBuilder, Keys, Kind, Tag, Timestamp};
use radroots_sdk::knowledge::prelude::*;

const SECRET_KEY_HEX: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const CREATED_AT: u32 = 1_800_000_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let claim = claim_builder().build()?;
    let parts = claim_builder().build_event()?;
    let draft = claim_builder().build_draft(public_key_hex(), CREATED_AT)?;
    let decoded = verify_and_decode_radroots_event(sign_parts(parts)?)?;
    let manifest = contract_manifest();

    assert_eq!(draft.contract_id, KNOWLEDGE_CLAIM_CONTRACT_ID);
    assert_eq!(manifest.contract_count, 11);
    match decoded {
        RadrootsDecodedEvent::KnowledgeClaim(parsed) => assert_eq!(parsed.data.data.text, claim.text),
        _ => return Err("expected knowledge claim".into()),
    }
    Ok(())
}

fn sign_parts(parts: WireEventParts) -> Result<RadrootsEventEnvelope, Box<dyn std::error::Error>> {
    let tags = parts
        .tags
        .into_iter()
        .map(Tag::parse)
        .collect::<Result<Vec<_>, _>>()?;
    let keys = Keys::parse(SECRET_KEY_HEX)?;
    let event = EventBuilder::new(Kind::Custom(parts.kind as u16), parts.content)
        .tags(tags)
        .custom_created_at(Timestamp::from_secs(u64::from(CREATED_AT)))
        .sign_with_keys(&keys)?;
    Ok(RadrootsEventEnvelope {
        id: event.id.to_hex(),
        author: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs() as u32,
        kind: u32::from(event.kind.as_u16()),
        tags: event
            .tags
            .as_slice()
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect(),
        content: event.content,
        sig: event.sig.to_string(),
    })
}

fn public_key_hex() -> String {
    Keys::parse(SECRET_KEY_HEX)
        .expect("keys")
        .public_key()
        .to_hex()
}

fn hex_64(character: char) -> String {
    character.to_string().repeat(64)
}

fn event_ref(character: char, kind: u32) -> RadrootsEventRef {
    RadrootsEventRef {
        id: hex_64(character),
        author: hex_64('a'),
        kind,
        d_tag: None,
        relays: Some(vec!["wss://relay.radroots.example".to_owned()]),
    }
}

fn claim_builder() -> RadrootsKnowledgeClaimBuilder {
    RadrootsKnowledgeClaimBuilder::new()
        .claim_type("practice_effect")
        .text("Cover crops improve soil structure.")
        .citation_span(RadrootsKnowledgeCitationSpan {
            source_ref: event_ref('4', KIND_KNOWLEDGE_SOURCE),
            artifact_ref: None,
            page_start: Some(12),
            page_end: Some(13),
            section_path: vec!["chapter-1".to_owned()],
            quote_hash: Some(hex_64('5')),
            chunk_id: Some("chunk-1".to_owned()),
        })
        .topic("cover-crops")
        .applies_to("local-food")
        .author_asserted_confidence("medium")
}
"#;

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::wasm::ResolvedRustToolchain;

    use super::{
        exact_dependency_pin, render_consumer_manifest, smoke_cargo_check_command,
        workspace_dependency_version_from,
    };

    #[test]
    fn smoke_consumer_manifest_uses_exact_direct_dependency_pin() {
        let manifest =
            render_consumer_manifest(Path::new("/tmp/radroots-sdk"), "=0.44.2").expect("manifest");

        assert!(manifest.contains("nostr = \"=0.44.2\""));
        assert!(!manifest.contains("nostr = \"0.44.2\""));
    }

    #[test]
    fn smoke_workspace_dependency_version_reads_table_version() {
        let version = workspace_dependency_version_from(
            r#"[workspace.dependencies]
nostr = { version = "0.44.2" }
"#,
            "nostr",
        )
        .expect("version");

        assert_eq!(version, "0.44.2");
    }

    #[test]
    fn smoke_dependency_pin_rejects_floating_major_version() {
        let error = exact_dependency_pin("nostr", "0.44").expect_err("floating version");

        assert!(error.contains("full exact version"));
    }

    #[test]
    fn smoke_command_uses_resolved_cargo_path() {
        let cargo = PathBuf::from("/tmp/rust-toolchain/bin/cargo");
        let toolchain = ResolvedRustToolchain {
            channel: "1.92.0".to_owned(),
            rustc: PathBuf::from("/tmp/rust-toolchain/bin/rustc"),
            cargo: cargo.clone(),
            bin_dir: PathBuf::from("/tmp/rust-toolchain/bin"),
        };

        let command = smoke_cargo_check_command(Path::new("/tmp/smoke"), &toolchain);

        assert_eq!(command.get_program(), cargo.as_os_str());
    }
}
