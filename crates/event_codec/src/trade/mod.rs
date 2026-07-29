#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

#[cfg(feature = "serde_json")]
use radroots_event::{
    RadrootsEventEnvelope,
    ids::RadrootsTradeMutationId,
    kinds::is_trade_mutation_event_kind,
    tags::{TAG_D, TAG_E},
    trade::{
        RadrootsTradeMutationEnvelopeV1, RadrootsTradeProtocolError,
        canonical_trade_mutation_content, trade_mutation_from_canonical_content,
    },
    wire::RadrootsNip01EventWireParts,
};

#[cfg(feature = "serde_json")]
use crate::error::EventEncodeError;

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeMutationParseError {
    InvalidKind(u32),
    MissingTag(&'static str),
    InvalidTag(&'static str),
    ContractTagMismatch,
    TradeIdTagMismatch,
    CounterpartyTagMismatch,
    ParentTagsMismatch,
    KindContractMismatch,
    Canonical(RadrootsTradeProtocolError),
}

#[cfg(feature = "serde_json")]
impl core::fmt::Display for RadrootsTradeMutationParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind(kind) => write!(f, "invalid trade mutation kind: {kind}"),
            Self::MissingTag(tag) => write!(f, "missing trade mutation tag: {tag}"),
            Self::InvalidTag(tag) => write!(f, "invalid trade mutation tag: {tag}"),
            Self::ContractTagMismatch => write!(f, "trade mutation contract tag mismatch"),
            Self::TradeIdTagMismatch => write!(f, "trade mutation trade id tag mismatch"),
            Self::CounterpartyTagMismatch => write!(f, "trade mutation counterparty tag mismatch"),
            Self::ParentTagsMismatch => write!(f, "trade mutation parent tags mismatch"),
            Self::KindContractMismatch => write!(f, "trade mutation kind contract mismatch"),
            Self::Canonical(error) => write!(f, "{error}"),
        }
    }
}

#[cfg(all(feature = "std", feature = "serde_json"))]
impl std::error::Error for RadrootsTradeMutationParseError {}

#[cfg(feature = "serde_json")]
impl From<RadrootsTradeProtocolError> for RadrootsTradeMutationParseError {
    fn from(value: RadrootsTradeProtocolError) -> Self {
        Self::Canonical(value)
    }
}

#[cfg(feature = "serde_json")]
pub fn trade_mutation_event_build(
    envelope: RadrootsTradeMutationEnvelopeV1,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    let canonical = canonical_trade_mutation_content(envelope)
        .map_err(map_trade_protocol_error_to_encode_error)?;
    let tags = trade_mutation_tags(&canonical.envelope)?;
    Ok(RadrootsNip01EventWireParts {
        kind: canonical.envelope.mutation_kind().nostr_kind(),
        content: canonical.content,
        tags,
    })
}

#[cfg(feature = "serde_json")]
pub fn trade_mutation_from_event(
    event: &RadrootsEventEnvelope,
) -> Result<RadrootsTradeMutationEnvelopeV1, RadrootsTradeMutationParseError> {
    if !is_trade_mutation_event_kind(event.kind_u32()) {
        return Err(RadrootsTradeMutationParseError::InvalidKind(
            event.kind_u32(),
        ));
    }
    let envelope = trade_mutation_from_canonical_content(event.content())?;
    if envelope.mutation_kind().nostr_kind() != event.kind_u32() {
        return Err(RadrootsTradeMutationParseError::KindContractMismatch);
    }
    validate_trade_mutation_tags(&envelope, &event.tags_as_vec())?;
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn trade_mutation_tags(
    envelope: &RadrootsTradeMutationEnvelopeV1,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::with_capacity(3 + envelope.parent_mutation_ids.len());
    push_tag(&mut tags, "contract", envelope.contract_id.clone())?;
    push_tag(&mut tags, TAG_D, envelope.trade_id.to_string())?;
    push_tag(&mut tags, "p", envelope.counterparty_pubkey.to_string())?;
    for parent in &envelope.parent_mutation_ids {
        push_tag(&mut tags, TAG_E, parent.to_string())?;
    }
    Ok(tags)
}

#[cfg(feature = "serde_json")]
fn validate_trade_mutation_tags(
    envelope: &RadrootsTradeMutationEnvelopeV1,
    tags: &[Vec<String>],
) -> Result<(), RadrootsTradeMutationParseError> {
    let contract = required_tag_value(tags, "contract")?;
    if contract != envelope.contract_id {
        return Err(RadrootsTradeMutationParseError::ContractTagMismatch);
    }
    let trade_id = required_tag_value(tags, TAG_D)?;
    if trade_id != envelope.trade_id.to_hex() {
        return Err(RadrootsTradeMutationParseError::TradeIdTagMismatch);
    }
    let counterparty = required_tag_value(tags, "p")?;
    if counterparty != envelope.counterparty_pubkey.to_hex() {
        return Err(RadrootsTradeMutationParseError::CounterpartyTagMismatch);
    }
    let mut parents = Vec::new();
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(TAG_E))
    {
        let value = tag
            .get(1)
            .map(String::as_str)
            .ok_or(RadrootsTradeMutationParseError::InvalidTag(TAG_E))?;
        let parent = RadrootsTradeMutationId::parse(value)
            .map_err(|_| RadrootsTradeMutationParseError::InvalidTag(TAG_E))?;
        parents.push(parent);
    }
    if parents != envelope.parent_mutation_ids {
        return Err(RadrootsTradeMutationParseError::ParentTagsMismatch);
    }
    Ok(())
}

#[cfg(feature = "serde_json")]
fn required_tag_value<'a>(
    tags: &'a [Vec<String>],
    name: &'static str,
) -> Result<&'a str, RadrootsTradeMutationParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some(name))
        .ok_or(RadrootsTradeMutationParseError::MissingTag(name))?;
    let value = tag
        .get(1)
        .map(String::as_str)
        .ok_or(RadrootsTradeMutationParseError::InvalidTag(name))?;
    if value.trim().is_empty() {
        return Err(RadrootsTradeMutationParseError::InvalidTag(name));
    }
    Ok(value)
}

#[cfg(feature = "serde_json")]
fn push_tag(
    tags: &mut Vec<Vec<String>>,
    name: &'static str,
    value: String,
) -> Result<(), EventEncodeError> {
    if value.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField(name));
    }
    tags.push(vec![name.to_string(), value]);
    Ok(())
}

#[cfg(feature = "serde_json")]
fn map_trade_protocol_error_to_encode_error(error: RadrootsTradeProtocolError) -> EventEncodeError {
    match error {
        RadrootsTradeProtocolError::EmptyField(field) => {
            EventEncodeError::EmptyRequiredField(field)
        }
        RadrootsTradeProtocolError::ContractMismatch { .. }
        | RadrootsTradeProtocolError::InvalidField(_)
        | RadrootsTradeProtocolError::InvalidIdentifier { .. }
        | RadrootsTradeProtocolError::InvalidInitialParents
        | RadrootsTradeProtocolError::MissingParentMutation
        | RadrootsTradeProtocolError::TooManyParents { .. }
        | RadrootsTradeProtocolError::UnsortedParents
        | RadrootsTradeProtocolError::DuplicateParent
        | RadrootsTradeProtocolError::SelfParent
        | RadrootsTradeProtocolError::MissingLines
        | RadrootsTradeProtocolError::TooManyLines { .. }
        | RadrootsTradeProtocolError::TooManyAdjustments { .. }
        | RadrootsTradeProtocolError::UnsupportedNumber
        | RadrootsTradeProtocolError::ContentTooLarge { .. }
        | RadrootsTradeProtocolError::InvalidTimeRange
        | RadrootsTradeProtocolError::MissingReservationCommitments
        | RadrootsTradeProtocolError::MissingCancellationTarget
        | RadrootsTradeProtocolError::CandidateIdMismatch { .. }
        | RadrootsTradeProtocolError::MutationIdMismatch { .. }
        | RadrootsTradeProtocolError::InvalidSchemaVersion { .. } => {
            EventEncodeError::InvalidField("trade_mutation")
        }
        RadrootsTradeProtocolError::DuplicateKey(_)
        | RadrootsTradeProtocolError::InvalidJson(_)
        | RadrootsTradeProtocolError::NonCanonicalJson => EventEncodeError::Json,
    }
}

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::*;
    use radroots_event::{
        RadrootsEventEnvelope, RadrootsEventEnvelopeParts,
        ids::{
            RadrootsClassifiedListingAddress, RadrootsDTag, RadrootsEventId,
            RadrootsInventoryBinId, RadrootsTradeId,
        },
        trade::{
            RADROOTS_TRADE_PROPOSAL_CONTRACT_ID, RADROOTS_TRADE_SCHEMA_VERSION,
            RadrootsFulfillmentProfileV1, RadrootsTradeCancellationProfileV1,
            RadrootsTradeCandidateLineV1, RadrootsTradeCandidateTermsV1,
            RadrootsTradeEconomicAdjustmentV1, RadrootsTradeEconomicsProfileV1,
            RadrootsTradeMutationBodyV1, RadrootsTradeMutationEnvelopeV1,
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

    fn event_id(character: char) -> RadrootsEventId {
        RadrootsEventId::parse(hex_64(character)).unwrap()
    }

    fn proposal() -> RadrootsTradeMutationEnvelopeV1 {
        RadrootsTradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_PROPOSAL_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: RadrootsTradeId::parse(hex_32('1')).unwrap(),
            root_mutation_id: None,
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: RadrootsDTag::parse("farm-1").unwrap(),
            parent_mutation_ids: Vec::new(),
            author_pubkey: pubkey('a'),
            counterparty_pubkey: pubkey('b'),
            authored_at_unix_s: 1_799_000_000,
            body: RadrootsTradeMutationBodyV1::Proposal {
                candidate: candidate(),
            },
        }
    }

    fn candidate() -> RadrootsTradeCandidateTermsV1 {
        RadrootsTradeCandidateTermsV1 {
            candidate_id: None,
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            base_candidate_id: None,
            supersession_intent: None,
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: RadrootsDTag::parse("farm-1").unwrap(),
            lines: vec![RadrootsTradeCandidateLineV1 {
                line_id: RadrootsDTag::parse("line-1").unwrap(),
                listing_addr: RadrootsClassifiedListingAddress::parse(format!(
                    "30402:{}:listing-1",
                    pubkey('b').to_hex()
                ))
                .unwrap(),
                listing_event_id: event_id('c'),
                listing_snapshot_sha256: hex_64('d'),
                product_id: "carrots".to_string(),
                option_id: None,
                bin_id: RadrootsInventoryBinId::parse("bin-1").unwrap(),
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
            economics: RadrootsTradeEconomicsProfileV1 {
                profile_id: "mvp-fixed".to_string(),
                currency_code: "USD".to_string(),
                currency_exponent: 2,
                rounding_profile: "half-even".to_string(),
                subtotal_mantissa: "1000".to_string(),
                discount_total_mantissa: "0".to_string(),
                adjustment_total_mantissa: "0".to_string(),
                total_mantissa: "1000".to_string(),
                adjustments: Vec::<RadrootsTradeEconomicAdjustmentV1>::new(),
            },
            fulfillment: RadrootsFulfillmentProfileV1 {
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
            cancellation: RadrootsTradeCancellationProfileV1 {
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
        assert_eq!(parts.kind, radroots_event::kinds::KIND_TRADE_PROPOSAL);
        assert_eq!(
            parts.tags[0],
            vec![
                "contract".to_string(),
                RADROOTS_TRADE_PROPOSAL_CONTRACT_ID.to_string()
            ]
        );
        let envelope = trade_mutation_from_event(
            &RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
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
        let parse_errors = [
            RadrootsTradeMutationParseError::InvalidKind(1),
            RadrootsTradeMutationParseError::MissingTag("contract"),
            RadrootsTradeMutationParseError::InvalidTag("contract"),
            RadrootsTradeMutationParseError::ContractTagMismatch,
            RadrootsTradeMutationParseError::TradeIdTagMismatch,
            RadrootsTradeMutationParseError::CounterpartyTagMismatch,
            RadrootsTradeMutationParseError::ParentTagsMismatch,
            RadrootsTradeMutationParseError::KindContractMismatch,
            RadrootsTradeMutationParseError::Canonical(RadrootsTradeProtocolError::MissingLines),
        ];
        for error in parse_errors {
            assert!(!error.to_string().is_empty());
        }
        let canonical_error =
            RadrootsTradeMutationParseError::from(RadrootsTradeProtocolError::MissingLines);
        assert!(matches!(
            canonical_error,
            RadrootsTradeMutationParseError::Canonical(_)
        ));

        let mut tags = trade_mutation_tags(&proposal()).unwrap();
        *tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some("contract"))
            .unwrap() = vec!["contract".into(), "wrong-contract".into()];
        assert_eq!(
            validate_trade_mutation_tags(&proposal(), &tags).unwrap_err(),
            RadrootsTradeMutationParseError::ContractTagMismatch
        );

        let mut tags = trade_mutation_tags(&proposal()).unwrap();
        *tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(TAG_D))
            .unwrap() = vec![TAG_D.into(), "other-trade".into()];
        assert_eq!(
            validate_trade_mutation_tags(&proposal(), &tags).unwrap_err(),
            RadrootsTradeMutationParseError::TradeIdTagMismatch
        );

        let mut tags = trade_mutation_tags(&proposal()).unwrap();
        *tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some("p"))
            .unwrap() = vec!["p".into(), pubkey('c').to_hex()];
        assert_eq!(
            validate_trade_mutation_tags(&proposal(), &tags).unwrap_err(),
            RadrootsTradeMutationParseError::CounterpartyTagMismatch
        );

        let mut missing_parent_value = trade_mutation_tags(&proposal()).unwrap();
        missing_parent_value.push(vec![TAG_E.into()]);
        assert_eq!(
            validate_trade_mutation_tags(&proposal(), &missing_parent_value).unwrap_err(),
            RadrootsTradeMutationParseError::InvalidTag(TAG_E)
        );

        let mut invalid_parent = trade_mutation_tags(&proposal()).unwrap();
        invalid_parent.push(vec![TAG_E.into(), "not-an-event-id".into()]);
        assert_eq!(
            validate_trade_mutation_tags(&proposal(), &invalid_parent).unwrap_err(),
            RadrootsTradeMutationParseError::InvalidTag(TAG_E)
        );

        let parent = RadrootsTradeMutationId::parse(hex_64('9')).unwrap();
        let mut parent_envelope = proposal();
        parent_envelope.parent_mutation_ids.push(parent);
        let parent_tags = trade_mutation_tags(&parent_envelope).unwrap();
        validate_trade_mutation_tags(&parent_envelope, &parent_tags).unwrap();

        let mut mismatched_parent = trade_mutation_tags(&proposal()).unwrap();
        mismatched_parent.push(vec![TAG_E.into(), parent.to_string()]);
        assert_eq!(
            validate_trade_mutation_tags(&proposal(), &mismatched_parent).unwrap_err(),
            RadrootsTradeMutationParseError::ParentTagsMismatch
        );

        assert_eq!(
            required_tag_value(&[], "contract").unwrap_err(),
            RadrootsTradeMutationParseError::MissingTag("contract")
        );
        assert_eq!(
            required_tag_value(&[vec!["contract".into()]], "contract").unwrap_err(),
            RadrootsTradeMutationParseError::InvalidTag("contract")
        );
        assert_eq!(
            required_tag_value(&[vec!["contract".into(), " ".into()]], "contract",).unwrap_err(),
            RadrootsTradeMutationParseError::InvalidTag("contract")
        );
        assert!(matches!(
            push_tag(&mut Vec::new(), "contract", " ".into()).unwrap_err(),
            EventEncodeError::EmptyRequiredField("contract")
        ));

        let built = trade_mutation_event_build(proposal()).unwrap();
        let invalid_kind = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: hex_64('e'),
            author: pubkey('a').to_hex(),
            created_at: 1_799_000_000,
            kind: 1,
            tags: built.tags.clone(),
            content: built.content.clone(),
            sig: core::iter::repeat_n('f', 128).collect(),
        })
        .unwrap();
        assert_eq!(
            trade_mutation_from_event(&invalid_kind).unwrap_err(),
            RadrootsTradeMutationParseError::InvalidKind(1)
        );

        let kind_contract_mismatch = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: hex_64('e'),
            author: pubkey('a').to_hex(),
            created_at: 1_799_000_000,
            kind: radroots_event::kinds::KIND_TRADE_DECISION,
            tags: built.tags,
            content: built.content,
            sig: core::iter::repeat_n('f', 128).collect(),
        })
        .unwrap();
        assert_eq!(
            trade_mutation_from_event(&kind_contract_mismatch).unwrap_err(),
            RadrootsTradeMutationParseError::KindContractMismatch
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn trade_protocol_errors_map_to_stable_encode_categories() {
        let id_error = RadrootsTradeMutationId::parse("invalid").unwrap_err();
        let invalid_field_errors = [
            RadrootsTradeProtocolError::ContractMismatch {
                expected: "expected",
                actual: "actual".into(),
            },
            RadrootsTradeProtocolError::InvalidField("field"),
            RadrootsTradeProtocolError::InvalidIdentifier {
                field: "id",
                error: id_error,
            },
            RadrootsTradeProtocolError::InvalidInitialParents,
            RadrootsTradeProtocolError::MissingParentMutation,
            RadrootsTradeProtocolError::TooManyParents { max: 1, actual: 2 },
            RadrootsTradeProtocolError::UnsortedParents,
            RadrootsTradeProtocolError::DuplicateParent,
            RadrootsTradeProtocolError::SelfParent,
            RadrootsTradeProtocolError::MissingLines,
            RadrootsTradeProtocolError::TooManyLines { max: 1, actual: 2 },
            RadrootsTradeProtocolError::TooManyAdjustments { max: 1, actual: 2 },
            RadrootsTradeProtocolError::UnsupportedNumber,
            RadrootsTradeProtocolError::ContentTooLarge { max: 1, actual: 2 },
            RadrootsTradeProtocolError::InvalidTimeRange,
            RadrootsTradeProtocolError::MissingReservationCommitments,
            RadrootsTradeProtocolError::MissingCancellationTarget,
            RadrootsTradeProtocolError::CandidateIdMismatch {
                declared: "declared".into(),
                computed: "computed".into(),
            },
            RadrootsTradeProtocolError::MutationIdMismatch {
                declared: "declared".into(),
                computed: "computed".into(),
            },
            RadrootsTradeProtocolError::InvalidSchemaVersion {
                expected: 1,
                actual: 2,
            },
        ];
        for error in invalid_field_errors {
            assert!(matches!(
                map_trade_protocol_error_to_encode_error(error),
                EventEncodeError::InvalidField("trade_mutation")
            ));
        }

        assert!(matches!(
            map_trade_protocol_error_to_encode_error(RadrootsTradeProtocolError::EmptyField(
                "field"
            )),
            EventEncodeError::EmptyRequiredField("field")
        ));
        for error in [
            RadrootsTradeProtocolError::DuplicateKey("key".into()),
            RadrootsTradeProtocolError::InvalidJson("json".into()),
            RadrootsTradeProtocolError::NonCanonicalJson,
        ] {
            assert!(matches!(
                map_trade_protocol_error_to_encode_error(error),
                EventEncodeError::Json
            ));
        }
    }
}
