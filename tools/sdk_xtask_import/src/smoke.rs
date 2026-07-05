use std::{fs, path::Path, process::Command};

use crate::fs::workspace_root;

pub fn run(args: &[String]) -> Result<(), String> {
    match args {
        [target] if target == "knowledge-rust-local" => knowledge_rust_local(),
        _ => Err("usage: cargo xtask smoke knowledge-rust-local".to_owned()),
    }
}

fn knowledge_rust_local() -> Result<(), String> {
    let root = workspace_root()?;
    let sdk_path = root.join("crates/sdk");
    let tempdir =
        tempfile::tempdir().map_err(|error| format!("failed to create smoke tempdir: {error}"))?;
    write_consumer(&tempdir.path().join("Cargo.toml"), &sdk_path)?;
    let src_dir = tempdir.path().join("src");
    fs::create_dir_all(&src_dir)
        .map_err(|error| format!("failed to create {}: {error}", src_dir.display()))?;
    fs::write(src_dir.join("main.rs"), CONSUMER_MAIN)
        .map_err(|error| format!("failed to write smoke consumer main.rs: {error}"))?;
    let status = Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .current_dir(tempdir.path())
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

fn write_consumer(path: &Path, sdk_path: &Path) -> Result<(), String> {
    let sdk_path = serde_json::to_string(&sdk_path.to_string_lossy())
        .map_err(|error| format!("failed to render SDK path: {error}"))?;
    let cargo_toml = format!(
        r#"[package]
name = "radroots-sdk-knowledge-smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
radroots_sdk = {{ path = {sdk_path}, features = ["std", "serde", "serde_json", "nostr", "knowledge"] }}
nostr = "0.44.2"
"#
    );
    fs::write(path, cargo_toml)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

const CONSUMER_MAIN: &str = r#"use nostr::{EventBuilder, Keys, Kind, Tag, Timestamp};
use radroots_sdk::knowledge::prelude::*;

const SECRET_KEY_HEX: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const CREATED_AT: u32 = 1_800_000_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let claim = RadrootsKnowledgeClaim {
        schema: RADROOTS_KNOWLEDGE_CLAIM_SCHEMA.to_owned(),
        schema_version: RADROOTS_KNOWLEDGE_SCHEMA_VERSION,
        claim_type: "practice_effect".to_owned(),
        text: "Cover crops improve soil structure.".to_owned(),
        citation_spans: vec![RadrootsKnowledgeCitationSpan {
            source_ref: event_ref('4', KIND_KNOWLEDGE_SOURCE),
            artifact_ref: None,
            page_start: Some(12),
            page_end: Some(13),
            section_path: vec!["chapter-1".to_owned()],
            quote_hash: Some(hex_64('5')),
            chunk_id: Some("chunk-1".to_owned()),
        }],
        topics: vec!["cover-crops".to_owned()],
        applies_to: vec!["local-food".to_owned()],
        author_asserted_confidence: Some("medium".to_owned()),
        supersedes: Vec::new(),
    };
    let parts = KnowledgeEventBuilder::new().knowledge_claim(&claim)?;
    let draft = KnowledgeDraftBuilder::new(public_key_hex(), CREATED_AT).knowledge_claim(&claim)?;
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

fn sign_parts(parts: WireEventParts) -> Result<RadrootsNostrEvent, Box<dyn std::error::Error>> {
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
    Ok(RadrootsNostrEvent {
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

fn event_ref(character: char, kind: u32) -> RadrootsNostrEventRef {
    RadrootsNostrEventRef {
        id: hex_64(character),
        author: hex_64('a'),
        kind,
        d_tag: None,
        relays: Some(vec!["wss://relay.radroots.example".to_owned()]),
    }
}
"#;
