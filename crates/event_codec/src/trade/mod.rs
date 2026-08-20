#[cfg(all(not(feature = "std"), feature = "json"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

#[cfg(feature = "json")]
use crate::verification::RadrootsSignatureVerifiedEvent;
#[cfg(feature = "json")]
use radroots_event::{
    envelope::EventEnvelope,
    envelope::kind::is_trade_mutation_event_kind,
    id::{MutationId, TradeId},
    trade::{
        TradeMutationEnvelopeV1, TradeMutationKindV1, canonical_trade_mutation_content,
        trade_mutation_from_canonical_content,
    },
    wire::Nip01EventWireParts,
};
#[cfg(feature = "json")]
use radroots_identity::PublicKey;

#[cfg(feature = "json")]
const MAX_TRADE_MUTATION_TAGS: usize = 10;

#[cfg(feature = "json")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeMutationError {
    CallerStructuralTagForbidden,
    DuplicateTradeTag,
    LegacyParentEventTag,
    MissingParentTag,
    MissingMutationTag,
    MissingRootTag,
    NoncanonicalParentOrder,
    PartyTagOrderMismatch,
    UnexpectedParentTag,
    UnexpectedRootTag,
    InvalidKind,
    AuthorMismatch,
    AuthoredAtMismatch,
    CanonicalContentMismatch,
    InvalidIdentifier,
    InvalidTagShape,
    UnexpectedTag,
    ContractTagMismatch,
    TradeTagMismatch,
    MutationTagMismatch,
    RootTagMismatch,
    ParentTagMismatch,
}

#[cfg(feature = "json")]
impl RadrootsTradeMutationError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::CallerStructuralTagForbidden => "caller_structural_tag_forbidden",
            Self::DuplicateTradeTag => "duplicate_trade_tag",
            Self::LegacyParentEventTag => "legacy_parent_event_tag",
            Self::MissingParentTag => "missing_parent_tag",
            Self::MissingMutationTag => "missing_mutation_tag",
            Self::MissingRootTag => "missing_root_tag",
            Self::NoncanonicalParentOrder => "noncanonical_parent_order",
            Self::PartyTagOrderMismatch => "party_tag_order_mismatch",
            Self::UnexpectedParentTag => "unexpected_parent_tag",
            Self::UnexpectedRootTag => "unexpected_root_tag",
            Self::InvalidKind => "invalid_kind",
            Self::AuthorMismatch => "author_mismatch",
            Self::AuthoredAtMismatch => "authored_at_mismatch",
            Self::CanonicalContentMismatch => "canonical_content_mismatch",
            Self::InvalidIdentifier => "invalid_identifier",
            Self::InvalidTagShape => "invalid_tag_shape",
            Self::UnexpectedTag => "unexpected_tag",
            Self::ContractTagMismatch => "contract_tag_mismatch",
            Self::TradeTagMismatch => "trade_tag_mismatch",
            Self::MutationTagMismatch => "mutation_tag_mismatch",
            Self::RootTagMismatch => "root_tag_mismatch",
            Self::ParentTagMismatch => "parent_tag_mismatch",
        }
    }
}

#[cfg(feature = "json")]
impl core::fmt::Display for RadrootsTradeMutationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::CallerStructuralTagForbidden => "caller supplied a governed trade tag",
            Self::DuplicateTradeTag => "trade tag is duplicated",
            Self::LegacyParentEventTag => "legacy trade parent tag is forbidden",
            Self::MissingParentTag => "trade parent tag is missing",
            Self::MissingMutationTag => "trade mutation tag is missing",
            Self::MissingRootTag => "trade root tag is missing",
            Self::NoncanonicalParentOrder => "trade parent tags are not canonical",
            Self::PartyTagOrderMismatch => "trade party tags are not canonical",
            Self::UnexpectedParentTag => "trade parent tag is unexpected",
            Self::UnexpectedRootTag => "trade root tag is unexpected",
            Self::InvalidKind => "trade mutation kind is invalid",
            Self::AuthorMismatch => "trade mutation author does not match content",
            Self::AuthoredAtMismatch => "trade mutation timestamp does not match content",
            Self::CanonicalContentMismatch => "trade mutation content is not canonical",
            Self::InvalidIdentifier => "trade mutation identifier is invalid",
            Self::InvalidTagShape => "trade mutation tag shape is invalid",
            Self::UnexpectedTag => "trade mutation tag is not permitted",
            Self::ContractTagMismatch => "trade contract tag does not match content",
            Self::TradeTagMismatch => "trade identifier tag does not match content",
            Self::MutationTagMismatch => "trade mutation tag does not match content",
            Self::RootTagMismatch => "trade root tag does not match content",
            Self::ParentTagMismatch => "trade parent tag does not match content",
        })
    }
}

#[cfg(all(feature = "std", feature = "json"))]
impl std::error::Error for RadrootsTradeMutationError {}

#[cfg(feature = "json")]
pub fn trade_mutation_event_build(
    envelope: TradeMutationEnvelopeV1,
) -> Result<Nip01EventWireParts, RadrootsTradeMutationError> {
    trade_mutation_event_build_with_extra_tags(envelope, &[])
}

#[cfg(feature = "json")]
pub fn trade_mutation_event_build_with_extra_tags(
    envelope: TradeMutationEnvelopeV1,
    extra_tags: &[Vec<String>],
) -> Result<Nip01EventWireParts, RadrootsTradeMutationError> {
    if extra_tags.iter().any(|tag| {
        matches!(
            tag.first().map(String::as_str),
            Some("contract" | "d" | "x" | "p" | "e")
        )
    }) {
        return Err(RadrootsTradeMutationError::CallerStructuralTagForbidden);
    }
    if let Some(tag) = extra_tags.first() {
        if tag.is_empty() {
            return Err(RadrootsTradeMutationError::InvalidTagShape);
        }
        return Err(RadrootsTradeMutationError::UnexpectedTag);
    }
    let canonical = canonical_trade_mutation_content(envelope)
        .map_err(|_| RadrootsTradeMutationError::CanonicalContentMismatch)?;
    let tags = canonical_trade_mutation_tags(&canonical.envelope)?;
    Ok(Nip01EventWireParts {
        kind: canonical.envelope.mutation_kind().nostr_kind(),
        content: canonical.content,
        tags,
    })
}

#[cfg(feature = "json")]
/// Structurally parses and validates a trade-mutation event.
///
/// This boundary binds the event kind, declared author, timestamp, canonical
/// content, and ordered tags. It does not verify the event signature; callers
/// that require cryptographic verification must use
/// [`trade_mutation_from_verified_event`].
pub fn trade_mutation_from_event(
    event: &EventEnvelope,
) -> Result<TradeMutationEnvelopeV1, RadrootsTradeMutationError> {
    validate_trade_mutation_parts(
        event.kind_u32(),
        event.created_at_u64(),
        &event.author().to_hex(),
        &event.tags_as_vec(),
        event.content(),
    )
}

#[cfg(feature = "json")]
pub(crate) fn validate_trade_mutation_parts(
    kind: u32,
    authored_at: u64,
    author: &str,
    tags: &[Vec<String>],
    content: &str,
) -> Result<TradeMutationEnvelopeV1, RadrootsTradeMutationError> {
    if !is_trade_mutation_event_kind(kind) {
        return Err(RadrootsTradeMutationError::InvalidKind);
    }
    let envelope = trade_mutation_from_canonical_content(content)
        .map_err(|_| RadrootsTradeMutationError::CanonicalContentMismatch)?;
    if envelope.mutation_id.is_none() {
        return Err(RadrootsTradeMutationError::CanonicalContentMismatch);
    }
    if envelope.mutation_kind().nostr_kind() != kind {
        return Err(RadrootsTradeMutationError::InvalidKind);
    }
    if canonical_public_key(author)? != envelope.author_pubkey.to_hex() {
        return Err(RadrootsTradeMutationError::AuthorMismatch);
    }
    if envelope.authored_at_unix_s != authored_at {
        return Err(RadrootsTradeMutationError::AuthoredAtMismatch);
    }
    validate_trade_mutation_tags(&envelope, tags)?;
    Ok(envelope)
}

#[cfg(feature = "json")]
/// Validates a trade mutation whose NIP-01 signature has already been verified.
pub fn trade_mutation_from_verified_event(
    event: &RadrootsSignatureVerifiedEvent,
) -> Result<TradeMutationEnvelopeV1, RadrootsTradeMutationError> {
    trade_mutation_from_event(event.event())
}

#[cfg(feature = "json")]
pub fn trade_mutation_tags(
    envelope: &TradeMutationEnvelopeV1,
) -> Result<Vec<Vec<String>>, RadrootsTradeMutationError> {
    let canonical = canonical_trade_mutation_content(envelope.clone())
        .map_err(|_| RadrootsTradeMutationError::CanonicalContentMismatch)?;
    canonical_trade_mutation_tags(&canonical.envelope)
}

#[cfg(feature = "json")]
pub fn validate_trade_mutation_tags(
    envelope: &TradeMutationEnvelopeV1,
    tags: &[Vec<String>],
) -> Result<(), RadrootsTradeMutationError> {
    if tags.len() > MAX_TRADE_MUTATION_TAGS {
        return Err(RadrootsTradeMutationError::InvalidTagShape);
    }
    let proposal = envelope.mutation_kind() == TradeMutationKindV1::Proposal;
    if tags
        .iter()
        .any(|tag| tag.first().map(String::as_str) == Some("e"))
    {
        return Err(RadrootsTradeMutationError::LegacyParentEventTag);
    }
    let trade_count = count_named(tags, "d");
    if trade_count > 1 {
        return Err(RadrootsTradeMutationError::DuplicateTradeTag);
    }
    let mutation_count = count_marked(tags, "mutation");
    let root_count = count_marked(tags, "root");
    let parent_count = count_marked(tags, "parent");
    if count_named(tags, "x") != mutation_count + root_count + parent_count {
        return Err(RadrootsTradeMutationError::InvalidTagShape);
    }
    if mutation_count == 0 {
        return Err(RadrootsTradeMutationError::MissingMutationTag);
    }
    if mutation_count != 1 {
        return Err(RadrootsTradeMutationError::InvalidTagShape);
    }
    if proposal {
        if root_count != 0 {
            return Err(RadrootsTradeMutationError::UnexpectedRootTag);
        }
        if parent_count != 0 {
            return Err(RadrootsTradeMutationError::UnexpectedParentTag);
        }
    } else {
        if root_count == 0 {
            return Err(RadrootsTradeMutationError::MissingRootTag);
        }
        if root_count != 1 {
            return Err(RadrootsTradeMutationError::InvalidTagShape);
        }
        if parent_count == 0 {
            return Err(RadrootsTradeMutationError::MissingParentTag);
        }
        if parent_count > 4 {
            return Err(RadrootsTradeMutationError::InvalidTagShape);
        }
    }
    if trade_count == 0 || count_named(tags, "contract") != 1 || count_named(tags, "p") != 2 {
        return Err(RadrootsTradeMutationError::InvalidTagShape);
    }
    if tags.iter().any(|tag| {
        !matches!(
            tag.first().map(String::as_str),
            Some("contract" | "d" | "x" | "p")
        )
    }) {
        return Err(RadrootsTradeMutationError::UnexpectedTag);
    }

    let contract = exact_unmarked(tags.first(), "contract")?;
    if contract != envelope.contract_id {
        return Err(RadrootsTradeMutationError::ContractTagMismatch);
    }
    let trade = exact_unmarked(tags.get(1), "d")?;
    if canonical_trade_id(trade)? != envelope.trade_id.to_hex() {
        return Err(RadrootsTradeMutationError::TradeTagMismatch);
    }
    let mutation = exact_marked(tags.get(2), "mutation")?;
    if canonical_mutation_id(mutation)?
        != envelope
            .mutation_id
            .as_ref()
            .ok_or(RadrootsTradeMutationError::CanonicalContentMismatch)?
            .to_hex()
    {
        return Err(RadrootsTradeMutationError::MutationTagMismatch);
    }

    let mut cursor = 3;
    if !proposal {
        let root = exact_marked(tags.get(cursor), "root")?;
        if canonical_mutation_id(root)?
            != envelope
                .root_mutation_id
                .as_ref()
                .ok_or(RadrootsTradeMutationError::MissingRootTag)?
                .to_hex()
        {
            return Err(RadrootsTradeMutationError::RootTagMismatch);
        }
        cursor += 1;
    }
    let mut parsed_parents = Vec::with_capacity(parent_count);
    for tag in tags.iter().skip(cursor).take(parent_count) {
        let parent = canonical_mutation_id(exact_marked(Some(tag), "parent")?)?;
        parsed_parents.push(
            MutationId::parse(parent).map_err(|_| RadrootsTradeMutationError::InvalidIdentifier)?,
        );
    }
    if parsed_parents.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RadrootsTradeMutationError::NoncanonicalParentOrder);
    }
    if parsed_parents != envelope.parent_mutation_ids {
        return Err(RadrootsTradeMutationError::ParentTagMismatch);
    }
    cursor += parent_count;
    let buyer = exact_unmarked(tags.get(cursor), "p")?;
    let seller = exact_unmarked(tags.get(cursor + 1), "p")?;
    if canonical_public_key(buyer)? != envelope.buyer_pubkey.to_hex()
        || canonical_public_key(seller)? != envelope.seller_pubkey.to_hex()
    {
        return Err(RadrootsTradeMutationError::PartyTagOrderMismatch);
    }
    if cursor + 2 != tags.len() {
        return Err(RadrootsTradeMutationError::InvalidTagShape);
    }
    validate_party_binding(envelope)
}

#[cfg(feature = "json")]
fn canonical_trade_mutation_tags(
    envelope: &TradeMutationEnvelopeV1,
) -> Result<Vec<Vec<String>>, RadrootsTradeMutationError> {
    validate_party_binding(envelope)?;
    let mutation = envelope
        .mutation_id
        .as_ref()
        .ok_or(RadrootsTradeMutationError::CanonicalContentMismatch)?;
    let mut tags = Vec::with_capacity(5 + envelope.parent_mutation_ids.len());
    tags.push(vec!["contract".to_string(), envelope.contract_id.clone()]);
    tags.push(vec!["d".to_string(), envelope.trade_id.to_hex()]);
    tags.push(vec![
        "x".to_string(),
        mutation.to_hex(),
        "mutation".to_string(),
    ]);
    if let Some(root) = &envelope.root_mutation_id {
        tags.push(vec!["x".to_string(), root.to_hex(), "root".to_string()]);
    }
    for parent in &envelope.parent_mutation_ids {
        tags.push(vec!["x".to_string(), parent.to_hex(), "parent".to_string()]);
    }
    tags.push(vec!["p".to_string(), envelope.buyer_pubkey.to_hex()]);
    tags.push(vec!["p".to_string(), envelope.seller_pubkey.to_hex()]);
    Ok(tags)
}

#[cfg(feature = "json")]
fn validate_party_binding(
    envelope: &TradeMutationEnvelopeV1,
) -> Result<(), RadrootsTradeMutationError> {
    let expected_counterparty = if envelope.author_pubkey == envelope.buyer_pubkey {
        &envelope.seller_pubkey
    } else if envelope.author_pubkey == envelope.seller_pubkey {
        &envelope.buyer_pubkey
    } else {
        return Err(RadrootsTradeMutationError::AuthorMismatch);
    };
    if &envelope.counterparty_pubkey != expected_counterparty {
        return Err(RadrootsTradeMutationError::AuthorMismatch);
    }
    Ok(())
}

#[cfg(feature = "json")]
fn count_named(tags: &[Vec<String>], name: &str) -> usize {
    tags.iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(name))
        .count()
}

#[cfg(feature = "json")]
fn count_marked(tags: &[Vec<String>], marker: &str) -> usize {
    tags.iter()
        .filter(|tag| {
            tag.first().map(String::as_str) == Some("x")
                && tag.get(2).map(String::as_str) == Some(marker)
        })
        .count()
}

#[cfg(feature = "json")]
fn exact_unmarked<'a>(
    tag: Option<&'a Vec<String>>,
    name: &str,
) -> Result<&'a str, RadrootsTradeMutationError> {
    let tag = tag.ok_or(RadrootsTradeMutationError::InvalidTagShape)?;
    if tag.len() != 2 || tag.first().map(String::as_str) != Some(name) {
        return Err(RadrootsTradeMutationError::InvalidTagShape);
    }
    Ok(&tag[1])
}

#[cfg(feature = "json")]
fn exact_marked<'a>(
    tag: Option<&'a Vec<String>>,
    marker: &str,
) -> Result<&'a str, RadrootsTradeMutationError> {
    let tag = tag.ok_or(RadrootsTradeMutationError::InvalidTagShape)?;
    if tag.len() != 3
        || tag.first().map(String::as_str) != Some("x")
        || tag.get(2).map(String::as_str) != Some(marker)
    {
        return Err(RadrootsTradeMutationError::InvalidTagShape);
    }
    Ok(&tag[1])
}

#[cfg(feature = "json")]
fn canonical_trade_id(value: &str) -> Result<String, RadrootsTradeMutationError> {
    let parsed =
        TradeId::parse(value).map_err(|_| RadrootsTradeMutationError::InvalidIdentifier)?;
    let canonical = parsed.to_hex();
    if canonical != value {
        return Err(RadrootsTradeMutationError::InvalidIdentifier);
    }
    Ok(canonical)
}

#[cfg(feature = "json")]
fn canonical_mutation_id(value: &str) -> Result<String, RadrootsTradeMutationError> {
    let parsed =
        MutationId::parse(value).map_err(|_| RadrootsTradeMutationError::InvalidIdentifier)?;
    let canonical = parsed.to_hex();
    if canonical != value {
        return Err(RadrootsTradeMutationError::InvalidIdentifier);
    }
    Ok(canonical)
}

#[cfg(feature = "json")]
fn canonical_public_key(value: &str) -> Result<String, RadrootsTradeMutationError> {
    let parsed =
        PublicKey::from_hex(value).map_err(|_| RadrootsTradeMutationError::InvalidIdentifier)?;
    let canonical = parsed.to_hex();
    if canonical != value {
        return Err(RadrootsTradeMutationError::InvalidIdentifier);
    }
    Ok(canonical)
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use super::*;
    use radroots_event::{
        envelope::EventEnvelope,
        envelope::EventEnvelopeParts,
        id::{ClassifiedListingAddress, DTag, EventId, InventoryBinId, TradeId},
        trade::{
            FulfillmentProfileV1, RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
            RADROOTS_TRADE_SCHEMA_VERSION, TradeCancellationProfileV1, TradeCandidateLineV1,
            TradeCandidateTermsV1, TradeEconomicAdjustmentV1, TradeEconomicsProfileV1,
            TradeMutationBodyV1, TradeMutationEnvelopeV1,
        },
    };
    use radroots_identity::PublicKey;

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn hex_32(character: char) -> String {
        core::iter::repeat_n(character, 32).collect()
    }

    fn pubkey(character: char) -> PublicKey {
        PublicKey::from_hex(&crate::test_fixtures::fixture_public_key_hex(character)).unwrap()
    }

    fn event_id(character: char) -> EventId {
        EventId::parse(hex_64(character)).unwrap()
    }

    fn proposal() -> TradeMutationEnvelopeV1 {
        TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_PROPOSAL_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: TradeId::parse(hex_32('1')).unwrap(),
            root_mutation_id: None,
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: DTag::parse("farm-1").unwrap(),
            parent_mutation_ids: Vec::new(),
            author_pubkey: pubkey('a'),
            counterparty_pubkey: pubkey('b'),
            authored_at_unix_s: 1_799_000_000,
            body: TradeMutationBodyV1::Proposal {
                candidate: candidate(),
            },
        }
    }

    fn candidate() -> TradeCandidateTermsV1 {
        TradeCandidateTermsV1 {
            candidate_id: None,
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            base_candidate_id: None,
            supersession_intent: None,
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: DTag::parse("farm-1").unwrap(),
            lines: vec![TradeCandidateLineV1 {
                line_id: DTag::parse("line-1").unwrap(),
                listing_addr: ClassifiedListingAddress::parse(format!(
                    "30402:{}:listing-1",
                    pubkey('b').to_hex()
                ))
                .unwrap(),
                listing_event_id: event_id('c'),
                listing_snapshot_sha256: hex_64('d'),
                product_id: "carrots".to_string(),
                option_id: None,
                bin_id: InventoryBinId::parse("bin-1").unwrap(),
                quantity_mantissa: "2".to_string(),
                quantity_scale: 0,
                unit_code: "count".to_string(),
                unit_profile: "mvp-count".to_string(),
                unit_price_mantissa: "500".to_string(),
                currency_code: "USD".to_string(),
                line_subtotal_mantissa: "1000".to_string(),
                replaces_line_id: None,
            }],
            line_tombstones: Vec::new(),
            economics: TradeEconomicsProfileV1 {
                profile_id: "mvp-fixed".to_string(),
                currency_code: "USD".to_string(),
                currency_exponent: 2,
                rounding_profile: "half-even".to_string(),
                subtotal_mantissa: "1000".to_string(),
                discount_total_mantissa: "0".to_string(),
                adjustment_total_mantissa: "0".to_string(),
                total_mantissa: "1000".to_string(),
                adjustments: Vec::<TradeEconomicAdjustmentV1>::new(),
            },
            fulfillment: FulfillmentProfileV1 {
                profile_id: "market-pickup".to_string(),
                method: "pickup".to_string(),
                starts_at_unix_s: 1_800_000_000,
                ends_at_unix_s: 1_800_003_600,
                timezone: "America/New_York".to_string(),
                utc_offset_seconds: -18_000,
                fold: 0,
                location_class: "farmstand".to_string(),
                requires_private_terms: false,
            },
            cancellation: TradeCancellationProfileV1 {
                profile_id: "buyer-pre-agreement".to_string(),
                buyer_pre_agreement: true,
                post_agreement_cutoff_unix_s: None,
            },
            private_terms: None,
            proposal_expires_at_unix_s: 1_799_999_000,
        }
    }

    #[test]
    fn trade_mutation_event_build_roundtrips_canonical_content_and_tags() {
        let parts = trade_mutation_event_build(proposal()).unwrap();
        assert_eq!(
            parts.kind,
            radroots_event::envelope::kind::KIND_TRADE_PROPOSAL
        );
        assert_eq!(
            parts.tags[0],
            vec![
                "contract".to_string(),
                RADROOTS_TRADE_PROPOSAL_CONTRACT_ID.to_string()
            ]
        );
        let envelope = trade_mutation_from_event(
            &EventEnvelope::new(EventEnvelopeParts {
                id: hex_64('e'),
                author: pubkey('a').to_hex(),
                created_at: 1_799_000_000,
                kind: parts.kind,
                tags: parts.tags,
                content: parts.content,
                sig: core::iter::repeat_n('f', 128).collect(),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(envelope.contract_id, RADROOTS_TRADE_PROPOSAL_CONTRACT_ID);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn trade_mutation_codec_rejects_all_invalid_wire_shapes() {
        let canonical = canonical_trade_mutation_content(proposal())
            .unwrap()
            .envelope;
        let mut tags = trade_mutation_tags(&canonical).unwrap();
        *tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some("contract"))
            .unwrap() = vec!["contract".into(), "wrong-contract".into()];
        assert_eq!(
            validate_trade_mutation_tags(&canonical, &tags).unwrap_err(),
            RadrootsTradeMutationError::ContractTagMismatch
        );

        let mut tags = trade_mutation_tags(&canonical).unwrap();
        *tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some("d"))
            .unwrap() = vec!["d".into(), hex_32('9')];
        assert_eq!(
            validate_trade_mutation_tags(&canonical, &tags).unwrap_err(),
            RadrootsTradeMutationError::TradeTagMismatch
        );

        let mut tags = trade_mutation_tags(&canonical).unwrap();
        let party = tags.len() - 2;
        tags.swap(party, party + 1);
        assert_eq!(
            validate_trade_mutation_tags(&canonical, &tags).unwrap_err(),
            RadrootsTradeMutationError::PartyTagOrderMismatch
        );

        let mut legacy = trade_mutation_tags(&canonical).unwrap();
        legacy.push(vec!["e".into(), hex_64('9')]);
        assert_eq!(
            validate_trade_mutation_tags(&canonical, &legacy).unwrap_err(),
            RadrootsTradeMutationError::LegacyParentEventTag
        );

        let mut missing_mutation = trade_mutation_tags(&canonical).unwrap();
        missing_mutation.remove(2);
        assert_eq!(
            validate_trade_mutation_tags(&canonical, &missing_mutation).unwrap_err(),
            RadrootsTradeMutationError::MissingMutationTag
        );

        let mut duplicate_trade = trade_mutation_tags(&canonical).unwrap();
        duplicate_trade.push(vec!["d".into(), hex_32('9')]);
        assert_eq!(
            validate_trade_mutation_tags(&canonical, &duplicate_trade).unwrap_err(),
            RadrootsTradeMutationError::DuplicateTradeTag
        );

        assert_eq!(
            trade_mutation_event_build_with_extra_tags(
                proposal(),
                &[vec!["d".into(), hex_32('9')]],
            )
            .unwrap_err(),
            RadrootsTradeMutationError::CallerStructuralTagForbidden
        );
    }

    #[test]
    fn trade_event_parser_binds_kind_author_time_content_and_markers() {
        let built = trade_mutation_event_build(proposal()).expect("trade event");
        let event = |author: String, created_at, kind, tags: Vec<Vec<String>>, content: String| {
            EventEnvelope::new(EventEnvelopeParts {
                id: hex_64('e'),
                author,
                created_at,
                kind,
                tags,
                content,
                sig: core::iter::repeat_n('f', 128).collect(),
            })
            .expect("structural envelope")
        };
        assert_eq!(
            trade_mutation_from_event(&event(
                pubkey('a').to_hex(),
                1_799_000_000,
                1,
                built.tags.clone(),
                built.content.clone(),
            ))
            .unwrap_err(),
            RadrootsTradeMutationError::InvalidKind
        );
        assert_eq!(
            trade_mutation_from_event(&event(
                pubkey('c').to_hex(),
                1_799_000_000,
                built.kind,
                built.tags.clone(),
                built.content.clone(),
            ))
            .unwrap_err(),
            RadrootsTradeMutationError::AuthorMismatch
        );
        assert_eq!(
            trade_mutation_from_event(&event(
                pubkey('a').to_hex(),
                1_799_000_001,
                built.kind,
                built.tags.clone(),
                built.content.clone(),
            ))
            .unwrap_err(),
            RadrootsTradeMutationError::AuthoredAtMismatch
        );
        assert_eq!(
            trade_mutation_from_event(&event(
                pubkey('a').to_hex(),
                1_799_000_000,
                built.kind,
                built.tags.clone(),
                "{}".to_owned(),
            ))
            .unwrap_err(),
            RadrootsTradeMutationError::CanonicalContentMismatch
        );
        let mut unknown_marker = built.tags.clone();
        unknown_marker[2][2] = "unknown".to_owned();
        assert_eq!(
            trade_mutation_from_event(&event(
                pubkey('a').to_hex(),
                1_799_000_000,
                built.kind,
                unknown_marker,
                built.content,
            ))
            .unwrap_err(),
            RadrootsTradeMutationError::InvalidTagShape
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn trade_errors_have_fixed_redacted_diagnostics() {
        let errors = [
            RadrootsTradeMutationError::CallerStructuralTagForbidden,
            RadrootsTradeMutationError::DuplicateTradeTag,
            RadrootsTradeMutationError::LegacyParentEventTag,
            RadrootsTradeMutationError::MissingParentTag,
            RadrootsTradeMutationError::MissingMutationTag,
            RadrootsTradeMutationError::MissingRootTag,
            RadrootsTradeMutationError::NoncanonicalParentOrder,
            RadrootsTradeMutationError::PartyTagOrderMismatch,
            RadrootsTradeMutationError::UnexpectedParentTag,
            RadrootsTradeMutationError::UnexpectedRootTag,
            RadrootsTradeMutationError::InvalidKind,
            RadrootsTradeMutationError::AuthorMismatch,
            RadrootsTradeMutationError::AuthoredAtMismatch,
            RadrootsTradeMutationError::CanonicalContentMismatch,
            RadrootsTradeMutationError::InvalidIdentifier,
            RadrootsTradeMutationError::InvalidTagShape,
            RadrootsTradeMutationError::UnexpectedTag,
            RadrootsTradeMutationError::ContractTagMismatch,
            RadrootsTradeMutationError::TradeTagMismatch,
            RadrootsTradeMutationError::MutationTagMismatch,
            RadrootsTradeMutationError::RootTagMismatch,
            RadrootsTradeMutationError::ParentTagMismatch,
        ];
        for error in errors {
            let display = error.to_string();
            let debug = format!("{error:?}");
            assert!(!display.is_empty());
            assert!(!debug.contains(&hex_64('a')));
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}
