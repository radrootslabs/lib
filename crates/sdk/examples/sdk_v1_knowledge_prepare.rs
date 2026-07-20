use nostr::{EventBuilder, Keys, Kind, Tag, Timestamp};
use radroots_event::RadrootsEventEnvelopeParts;
use radroots_sdk::knowledge::prelude::*;

const CREATED_AT: u32 = 1_800_000_000;
const RELAY: &str = "wss://relay.radroots.example";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keys = Keys::generate();
    let claim = claim_builder().build()?;
    let parts = claim_builder().build_event()?;
    let draft = claim_builder().build_draft(keys.public_key().to_hex(), CREATED_AT)?;
    let signed = sign_parts(parts, &keys)?;
    let decoded = KnowledgeCodec::new().verify_and_decode_radroots_event(signed)?;
    let manifest = contract_manifest();
    let manifest_hash = contract_manifest_sha256()?;

    assert_eq!(draft.contract_id(), KNOWLEDGE_CLAIM_CONTRACT_ID);
    assert_eq!(manifest.contract_count, 11);
    assert_eq!(manifest_hash.len(), 64);

    match decoded {
        RadrootsDecodedEvent::KnowledgeClaim(parsed) => {
            assert_eq!(parsed.data.data.text, claim.text);
        }
        _ => return Err("expected knowledge claim".into()),
    }

    println!(
        "prepared knowledge claim draft: {}",
        draft.expected_event_id_str()
    );
    println!("knowledge manifest sha256: {manifest_hash}");
    Ok(())
}

fn sign_parts(
    parts: RadrootsNip01EventWireParts,
    keys: &Keys,
) -> Result<RadrootsEventEnvelope, Box<dyn std::error::Error>> {
    let tags = parts
        .tags
        .into_iter()
        .map(Tag::parse)
        .collect::<Result<Vec<_>, _>>()?;
    let event = EventBuilder::new(Kind::Custom(parts.kind as u16), parts.content)
        .tags(tags)
        .custom_created_at(Timestamp::from_secs(u64::from(CREATED_AT)))
        .sign_with_keys(keys)?;
    Ok(RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: event.id.to_hex(),
        author: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs(),
        kind: u32::from(event.kind.as_u16()),
        tags: event
            .tags
            .as_slice()
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect(),
        content: event.content,
        sig: event.sig.to_string(),
    })?)
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
        relays: Some(vec![RELAY.to_owned()]),
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
