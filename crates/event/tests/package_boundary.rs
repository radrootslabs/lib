use std::collections::BTreeSet;

use radroots_event::id::{
    CandidateId, DTag, EventId, EventSignature, MutationId, Nip01Coordinate, TradeId,
};
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
const IDENTIFIERS: &str = include_str!("../src/id.rs");
const CONTRACT_REGISTRY: &str = include_str!("../src/contract/registry_v7.rs");
const RELAY_HINT: &str = include_str!("../src/relay_hint.rs");
const TRADE: &str = include_str!("../src/trade.rs");
const ADMISSION: &str = include_str!("../src/admission.rs");
const VERIFICATION: &str = include_str!("../src/verification.rs");

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
    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from(["default", "knowledge", "serde", "std"]),
        "radroots_event must expose only its approved user-visible feature vocabulary"
    );
    for implementation_feature in ["dto-bindgen", "fixture", "knowledge-nip54", "signature"] {
        assert!(
            !table_keys(MANIFEST, "[features]").contains(implementation_feature),
            "implementation feature `{implementation_feature}` must remain private"
        );
    }
    assert!(!table_keys(MANIFEST, "[dependencies]").contains("dto_bindgen"));
    assert!(!table_keys(MANIFEST, "[dependencies]").contains("secp256k1"));
    for forbidden_dependency in [
        "libsqlite3-sys",
        "nostr",
        "nostr-sdk",
        "reqwest",
        "sqlx",
        "tokio",
    ] {
        assert!(
            !table_keys(MANIFEST, "[dependencies]").contains(forbidden_dependency),
            "forbidden implementation dependency `{forbidden_dependency}`"
        );
    }
}

#[test]
fn public_traits_are_exact_and_classified_as_host_spis() {
    let declarations = [ADMISSION, VERIFICATION]
        .into_iter()
        .flat_map(|source| source.lines())
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("pub trait "))
        .filter_map(|declaration| declaration.split([':', ' ', '{']).next())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        declarations,
        BTreeSet::from(["AdmissionPolicy", "SignatureVerifier", "VisibilityPolicy"])
    );
    for declaration in [
        "pub trait AdmissionPolicy: Send + Sync",
        "pub trait VisibilityPolicy: Send + Sync",
        "pub trait SignatureVerifier: Send + Sync",
    ] {
        assert!(
            ADMISSION.contains(declaration) || VERIFICATION.contains(declaration),
            "missing native Host SPI contract `{declaration}`"
        );
    }
    assert_eq!(
        ADMISSION.matches("/// **Host SPI:**").count()
            + VERIFICATION.matches("/// **Host SPI:**").count(),
        3,
        "every public event trait must carry one explicit Host SPI classification"
    );
}

#[test]
fn crate_root_declares_every_approved_module() {
    let declared = root_declarations("pub mod ");
    let approved = [
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
    ];
    assert_eq!(
        declared,
        approved.into_iter().collect(),
        "the crate root must expose exactly the approved singular module vocabulary"
    );
}

#[test]
fn crate_root_exposes_the_curated_native_event_vocabulary() {
    type CuratedRootTypes = (
        radroots_event::Event,
        radroots_event::EventDraft,
        radroots_event::SignedEvent,
        radroots_event::VerifiedEvent,
        radroots_event::EventId,
        radroots_event::EventKind,
        radroots_event::EventTag,
        radroots_event::Error,
    );
    let _: Option<CuratedRootTypes> = None;
    for root_export in [
        "Event",
        "EventDraft",
        "SignedEvent",
        "VerifiedEvent",
        "EventId",
        "EventKind",
        "EventTag",
        "Error",
    ] {
        assert!(
            ROOT.contains(root_export),
            "missing root export {root_export}"
        );
    }
    assert!(!ROOT.contains("pub use *"));
    assert!(!ROOT.contains("pub use crate::*"));
}

#[test]
fn verification_typestates_are_native_private_and_policy_gated() {
    let admission = include_str!("../src/admission.rs");
    let verification = include_str!("../src/verification.rs");

    for state in [
        "RawEvent",
        "IdVerifiedEvent",
        "SignatureVerifiedEvent",
        "ContractValidatedEvent",
        "AdmittedEvent",
        "VisibleEvent",
    ] {
        assert!(
            admission.contains(state) || verification.contains(state),
            "missing native typestate {state}"
        );
    }
    assert!(ROOT.contains("mod verification;"));
    assert!(!ROOT.contains("pub mod verification;"));
    assert!(verification.contains("pub struct IdVerifiedEvent(RadrootsEventEnvelope);"));
    assert!(verification.contains("pub struct SignatureVerifiedEvent(RadrootsEventEnvelope);"));
    assert!(admission.contains("policy.admit(&self)?;"));
    assert!(admission.contains("policy.make_visible(&self)?;"));
    assert!(verification.contains("pub trait SignatureVerifier: Send + Sync"));
    assert!(verification.contains("verifier.verify_signature(&self.0)?;"));
    assert!(!verification.contains("secp256k1"));
    assert!(!verification.contains("SignatureVerificationUnavailable"));
    assert!(!verification.contains("impl From<RawEvent> for IdVerifiedEvent"));
    assert!(!verification.contains("impl From<IdVerifiedEvent> for SignatureVerifiedEvent"));
    assert!(!admission.contains("impl From<ContractValidatedEvent> for AdmittedEvent"));
    assert!(!admission.contains("impl From<AdmittedEvent> for VisibleEvent"));
}

#[test]
fn contract_author_roles_are_event_owned_requirements_not_signer_provenance() {
    let seller =
        radroots_event::contract::event_contract("radroots.operational_listing.published.v1")
            .expect("operational listing contract");

    assert_eq!(
        seller.required_author_role(),
        radroots_event::contract::AuthorRole::Seller
    );
    assert!(!CONTRACT_REGISTRY.contains("RadrootsActorRole"));
    assert!(!CONTRACT_REGISTRY.contains("pub author_role:"));
    assert!(!CONTRACT_REGISTRY.contains("radroots_signing"));
    assert!(!CONTRACT_REGISTRY.contains("RadrootsActorContext"));
    assert!(!CONTRACT_REGISTRY.contains("AccountId"));
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
    let reference = radroots_event::tag::RadrootsEventRef {
        id: "a".repeat(64),
        author,
        kind: 1,
        d_tag: None,
        relays: None,
    };

    assert_eq!(reference.author, author);
    assert!(!IDENTIFIERS.contains("pub(crate) use radroots_identity::PublicKey"));
}

#[test]
fn trade_protocol_identifiers_have_one_definition_and_deliberate_facades() {
    let trade_id = TradeId::parse("11".repeat(16)).expect("trade id");
    let candidate_id = CandidateId::parse("22".repeat(32)).expect("candidate id");
    let mutation_id = MutationId::parse("33".repeat(32)).expect("mutation id");

    let trade_surface_id: radroots_event::trade::TradeId = trade_id;
    let trade_surface_candidate: radroots_event::trade::CandidateId = candidate_id;
    let trade_surface_mutation: radroots_event::trade::MutationId = mutation_id;

    assert_eq!(trade_surface_id, trade_id);
    assert_eq!(trade_surface_candidate, candidate_id);
    assert_eq!(trade_surface_mutation, mutation_id);
    assert_eq!(core::mem::size_of::<TradeId>(), 16);
    assert_eq!(core::mem::size_of::<CandidateId>(), 32);
    assert_eq!(core::mem::size_of::<MutationId>(), 32);
    assert!(TradeId::parse("order-1").is_err());
    assert!(radroots_event::id::RadrootsOrderId::parse("order-1").is_ok());

    for definition in [
        "validated_hex_id!(RadrootsTradeId, 16);",
        "validated_hex_id!(RadrootsTradeCandidateId, 32);",
        "validated_hex_id!(RadrootsTradeMutationId, 32);",
    ] {
        assert_eq!(IDENTIFIERS.matches(definition).count(), 1, "{definition}");
    }
    for duplicate in [
        "pub struct TradeId",
        "pub struct CandidateId",
        "pub struct MutationId",
        "pub struct RadrootsTradeId",
        "pub struct RadrootsTradeCandidateId",
        "pub struct RadrootsTradeMutationId",
    ] {
        assert!(
            !TRADE.contains(duplicate),
            "duplicate definition: {duplicate}"
        );
    }
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
