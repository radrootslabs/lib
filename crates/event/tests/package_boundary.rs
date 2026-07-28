use std::collections::BTreeSet;

use radroots_event::id::{DTag, EventId, EventSignature, Nip01Coordinate};
#[cfg(feature = "knowledge")]
#[allow(unused_imports)]
use radroots_event::knowledge as _;
#[allow(unused_imports)]
use radroots_event::{
    admission as _, calendar as _, contract as _, draft as _, envelope as _, farm as _, food as _,
    id as _, listing as _, media as _, post as _, profile as _, social as _, tag as _, trade as _,
    wire as _,
};
use radroots_identity::PublicKey;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const IDENTIFIERS: &str = include_str!("../src/ids.rs");
const RELAY_HINT: &str = include_str!("../src/relay_hint.rs");

#[test]
fn manifest_has_final_identity_and_required_radroots_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_event\""));
    assert!(MANIFEST.contains("version = \"0.1.0-alpha\""));
    assert!(MANIFEST.contains("publish = false"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_event\""));
    assert!(MANIFEST.contains("default = [\"std\", \"serde\"]"));
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]")
            .into_iter()
            .filter(|dependency| dependency.starts_with("radroots_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "radroots_blossom",
            "radroots_core",
            "radroots_identity",
            "radroots_protocol",
        ])
    );
    assert!(
        MANIFEST.contains("radroots_protocol = { workspace = true, default-features = false }")
    );
}

#[test]
fn crate_root_declares_every_approved_module() {
    let declared = root_declarations("pub mod ");
    for module in [
        "admission",
        "calendar",
        "contract",
        "draft",
        "envelope",
        "farm",
        "food",
        "id",
        "knowledge",
        "listing",
        "media",
        "post",
        "profile",
        "social",
        "tag",
        "trade",
        "wire",
    ] {
        assert!(
            declared.contains(module),
            "missing approved module {module}"
        );
    }
}

#[test]
fn canonical_identifier_api_owns_bytes_and_requires_explicit_text_encoding() {
    let event_id = EventId::parse("A".repeat(64)).expect("event id");
    let signature = EventSignature::parse("B".repeat(128)).expect("event signature");
    let d_tag = DTag::parse("listing-1").expect("d tag");
    let coordinate = Nip01Coordinate::parse(format!("30000:{}:listing-1", event_id.to_hex()))
        .expect("coordinate");

    assert_eq!(core::mem::size_of::<EventId>(), 32);
    assert_eq!(core::mem::size_of::<EventSignature>(), 64);
    assert_eq!(event_id.to_hex(), "a".repeat(64));
    assert_eq!(EventId::from_bytes(event_id.into_bytes()), event_id);
    assert_eq!(signature.to_hex(), "b".repeat(128));
    assert_eq!(d_tag.as_str(), "listing-1");
    assert_eq!(coordinate.identifier(), "listing-1");
    assert!(!IDENTIFIERS.contains("impl Deref"));
    assert!(!IDENTIFIERS.contains("impl Borrow<str>"));
    assert!(!RELAY_HINT.contains("impl Deref"));
    assert!(!RELAY_HINT.contains("impl Borrow<str>"));
}

#[test]
fn event_references_use_the_identity_owned_public_key() {
    let author =
        PublicKey::from_hex("585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df")
            .expect("valid public key");
    let reference = radroots_event::RadrootsEventRef {
        id: "a".repeat(64),
        author,
        kind: 1,
        d_tag: None,
        relays: None,
    };

    assert_eq!(reference.author, author);
    assert!(!IDENTIFIERS.contains("pub(crate) use radroots_identity::PublicKey"));
}

fn table_keys<'a>(manifest: &'a str, heading: &str) -> BTreeSet<&'a str> {
    let Some((_, table)) = manifest.split_once(heading) else {
        return BTreeSet::new();
    };
    table
        .lines()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let line = line.trim();
            (line
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
                && !line.starts_with('#'))
            .then(|| line.split_once('=').map(|(key, _)| key.trim()))
            .flatten()
        })
        .collect()
}

fn root_declarations(prefix: &str) -> BTreeSet<&str> {
    ROOT.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|name| name.strip_suffix(';'))
        .collect()
}
