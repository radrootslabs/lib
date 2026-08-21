//! Exact authored event bodies, plans, and semantic digests.

#[cfg(not(feature = "std"))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{borrow::ToOwned, string::String, vec::Vec};

use core::fmt;
use radroots_blossom::authorization::AuthoredUploadClaim;
use radroots_event::{
    GenericEventDraft,
    contract::ContractKey,
    draft::DraftError,
    id::EventId,
    wire::{CanonicalEventIdError, compute_canonical_nip01_event_id},
};
use radroots_identity::PublicKey;
use sha2::{Digest, Sha256};

pub const PLAN_WIRE_VERSION_V1: u32 = 1;
const PLAN_DIGEST_DOMAIN: &[u8] = b"radroots.authored_event_plan.v1";
const BLOSSOM_AUTHORIZATION_PLAN_DIGEST_DOMAIN: &[u8] =
    b"radroots.blossom_upload_authorization_plan.v1";

mod typed;
mod wire;

pub use typed::{AuthoredPlanError, REGISTRY_V7_TYPED_AUTHORING_CONTRACT_IDS};
pub use wire::{
    HistoricalPlanIntegrity, PLAN_WIRE_MAX_BYTES, PlanDecodeError, PlanRegistryRelation, PlanWireV1,
};

/// Exact contract-owned NIP-01 payload fields before author/time binding.
///
/// Fields are private and there is deliberately no arbitrary-parts
/// constructor. Bodies enter through generic validation, strict typed
/// conversion, or bounded historical reconstruction.
///
/// ```compile_fail
/// use radroots_event_codec::authoring::AuthoredEventBody;
///
/// let _ = AuthoredEventBody::new("radroots.social.geochat.v1", 20_000, vec![], "raw");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredEventBody {
    contract: ContractKey,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
}

impl AuthoredEventBody {
    fn from_generic(draft: &GenericEventDraft) -> Self {
        Self {
            contract: draft.contract().clone(),
            kind: draft.kind_u32(),
            tags: draft.tags_as_vec(),
            content: draft.content().to_owned(),
        }
    }

    #[must_use]
    pub const fn contract(&self) -> &ContractKey {
        &self.contract
    }

    #[must_use]
    pub const fn kind(&self) -> u32 {
        self.kind
    }

    #[must_use]
    pub fn tags(&self) -> &[Vec<String>] {
        &self.tags
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// Stable SHA-256 commitment to an exact authored event plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanDigest([u8; Self::BYTE_LENGTH]);

impl PlanDigest {
    pub const BYTE_LENGTH: usize = 32;

    #[must_use]
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LENGTH]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LENGTH] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn parse_hex(value: &str) -> Result<Self, PlanDigestError> {
        if value.len() != Self::BYTE_LENGTH * 2
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(PlanDigestError);
        }
        let mut bytes = [0_u8; Self::BYTE_LENGTH];
        hex::decode_to_slice(value, &mut bytes).map_err(|_| PlanDigestError)?;
        Ok(Self(bytes))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanDigestError;

impl fmt::Display for PlanDigestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("plan digest must be exactly 64 lowercase hexadecimal characters")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PlanDigestError {}

/// One immutable exact NIP-01 event plan, ready for authorization and signing.
///
/// ```compile_fail
/// use radroots_event_codec::authoring::AuthoredEventPlan;
///
/// let _ = AuthoredEventPlan::new();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredEventPlan {
    body: AuthoredEventBody,
    author: PublicKey,
    created_at: u64,
    expected_event_id: EventId,
    digest: PlanDigest,
}

/// Exact BUD-11 upload-authorization event plan for HTTP use only.
///
/// This type is deliberately distinct from [`AuthoredEventPlan`]. It cannot be
/// accepted by relay push APIs and therefore cannot accidentally publish a
/// short-lived Blossom authorization token as a Nostr event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlossomAuthorizationPlan {
    author: PublicKey,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    expected_event_id: EventId,
    digest: PlanDigest,
}

impl BlossomAuthorizationPlan {
    /// Binds one validated upload claim to the exact expected signer identity.
    pub fn for_upload(
        claim: &AuthoredUploadClaim,
        author: PublicKey,
    ) -> Result<Self, CanonicalEventIdError> {
        let wire = claim.wire_parts();
        let kind = u32::from(wire.kind());
        let tags = wire.tags().to_vec();
        let content = wire.content().to_owned();
        let expected_event_id = compute_canonical_nip01_event_id(
            author.to_hex().as_str(),
            wire.created_at(),
            kind,
            &tags,
            content.as_str(),
        )?;
        let digest = compute_blossom_authorization_plan_digest(
            &author,
            wire.created_at(),
            kind,
            &tags,
            content.as_str(),
            &expected_event_id,
        );
        Ok(Self {
            author,
            created_at: wire.created_at(),
            kind,
            tags,
            content,
            expected_event_id,
            digest,
        })
    }

    #[must_use]
    pub const fn author(&self) -> &PublicKey {
        &self.author
    }

    #[must_use]
    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    #[must_use]
    pub const fn kind(&self) -> u32 {
        self.kind
    }

    #[must_use]
    pub fn tags(&self) -> &[Vec<String>] {
        &self.tags
    }

    #[must_use]
    pub fn content(&self) -> &str {
        self.content.as_str()
    }

    #[must_use]
    pub const fn expected_event_id(&self) -> &EventId {
        &self.expected_event_id
    }

    #[must_use]
    pub const fn digest(&self) -> PlanDigest {
        self.digest
    }
}

impl AuthoredEventPlan {
    /// Converts the generic-only event input into its immutable authored plan.
    pub fn from_generic(draft: GenericEventDraft) -> Result<Self, DraftError> {
        draft.validate_for_authoring()?;
        let author = *draft.expected_pubkey();
        let created_at = draft.created_at_u64();
        let expected_event_id = *draft.expected_event_id();
        let body = AuthoredEventBody::from_generic(&draft);
        Ok(Self::from_validated_parts(
            body,
            author,
            created_at,
            expected_event_id,
        ))
    }

    pub(super) fn from_validated_parts(
        body: AuthoredEventBody,
        author: PublicKey,
        created_at: u64,
        expected_event_id: EventId,
    ) -> Self {
        let digest = compute_plan_digest(&body, &author, created_at, &expected_event_id);
        Self {
            body,
            author,
            created_at,
            expected_event_id,
            digest,
        }
    }

    #[must_use]
    pub const fn body(&self) -> &AuthoredEventBody {
        &self.body
    }

    #[must_use]
    pub const fn author(&self) -> &PublicKey {
        &self.author
    }

    #[must_use]
    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    #[must_use]
    pub const fn expected_event_id(&self) -> &EventId {
        &self.expected_event_id
    }

    #[must_use]
    pub const fn digest(&self) -> PlanDigest {
        self.digest
    }
}

fn compute_plan_digest(
    body: &AuthoredEventBody,
    author: &PublicKey,
    created_at: u64,
    expected_event_id: &EventId,
) -> PlanDigest {
    let mut encoder = DigestEncoder::new();
    encoder.bytes(PLAN_DIGEST_DOMAIN);
    encoder.u32(PLAN_WIRE_VERSION_V1);
    encoder.u32(body.contract.registry_version().get());
    encoder.bytes(body.contract.contract_id().as_str().as_bytes());
    encoder.bytes(author.as_bytes());
    encoder.u64(created_at);
    encoder.u32(body.kind);
    encoder.u32(body.tags.len() as u32);
    for tag in &body.tags {
        encoder.u32(tag.len() as u32);
        for element in tag {
            encoder.bytes(element.as_bytes());
        }
    }
    encoder.bytes(body.content.as_bytes());
    encoder.bytes(expected_event_id.as_bytes());
    encoder.finish()
}

fn compute_blossom_authorization_plan_digest(
    author: &PublicKey,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    expected_event_id: &EventId,
) -> PlanDigest {
    let mut encoder = DigestEncoder::new();
    encoder.bytes(BLOSSOM_AUTHORIZATION_PLAN_DIGEST_DOMAIN);
    encoder.bytes(author.as_bytes());
    encoder.u64(created_at);
    encoder.u32(kind);
    encoder.u32(tags.len() as u32);
    for tag in tags {
        encoder.u32(tag.len() as u32);
        for element in tag {
            encoder.bytes(element.as_bytes());
        }
    }
    encoder.bytes(content.as_bytes());
    encoder.bytes(expected_event_id.as_bytes());
    encoder.finish()
}

struct DigestEncoder(Sha256);

impl DigestEncoder {
    fn new() -> Self {
        Self(Sha256::new())
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        self.0.update(value);
    }

    fn finish(self) -> PlanDigest {
        PlanDigest::from_bytes(self.0.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_blossom::{
        Sha256 as BlossomSha256,
        authorization::{AuthorizationContent, ServerDomain},
    };
    use radroots_event::envelope::kind::KIND_GEOCHAT;

    const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const BOB: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    fn plan(
        author: &str,
        created_at: u64,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> AuthoredEventPlan {
        AuthoredEventPlan::from_generic(
            GenericEventDraft::new(
                "radroots.social.geochat.v1",
                KIND_GEOCHAT,
                created_at,
                tags,
                content,
                author,
            )
            .expect("generic draft"),
        )
        .expect("authored plan")
    }

    #[test]
    fn generic_input_becomes_an_exact_immutable_plan() {
        let plan = plan(ALICE, 1_700_000_000, Vec::new(), "hello");
        assert_eq!(
            plan.body().contract().contract_id().as_str(),
            "radroots.social.geochat.v1"
        );
        assert_eq!(plan.body().kind(), KIND_GEOCHAT);
        assert!(plan.body().tags().is_empty());
        assert_eq!(plan.body().content(), "hello");
        assert_eq!(plan.author().to_hex(), ALICE);
        assert_eq!(plan.created_at(), 1_700_000_000);
        assert_eq!(plan.digest().to_hex().len(), 64);
    }

    #[test]
    fn semantic_digest_has_a_stable_golden_vector() {
        let plan = plan(
            ALICE,
            1_700_000_000,
            vec![vec!["g".to_owned(), "u4pru".to_owned()]],
            "hello 🍓",
        );
        assert_eq!(
            plan.digest().to_hex(),
            "eb719fe792ef467b35810967c28a7fdc1c2fb8cebaefdf76efa878fe6dd4f1ec"
        );
    }

    #[test]
    fn plan_digest_parser_rejects_a_wrong_length_before_hex_decoding() {
        assert_eq!(PlanDigest::parse_hex("a"), Err(PlanDigestError));
    }

    #[test]
    fn every_bound_authoring_field_changes_the_plan_digest() {
        let base = plan(ALICE, 1_700_000_000, Vec::new(), "hello");
        let variants = [
            plan(BOB, 1_700_000_000, Vec::new(), "hello"),
            plan(ALICE, 1_700_000_001, Vec::new(), "hello"),
            plan(
                ALICE,
                1_700_000_000,
                vec![vec!["g".to_owned(), "u4pru".to_owned()]],
                "hello",
            ),
            plan(ALICE, 1_700_000_000, Vec::new(), "hello!"),
        ];
        for variant in variants {
            assert_ne!(variant.expected_event_id(), base.expected_event_id());
            assert_ne!(variant.digest(), base.digest());
        }
    }

    fn blossom_plan(
        author: &str,
        created_at: u64,
        content: &str,
        server: &str,
        payload: &[u8],
    ) -> BlossomAuthorizationPlan {
        let claim = AuthoredUploadClaim::new(
            AuthorizationContent::parse(content).expect("content"),
            ServerDomain::parse(server).expect("server"),
            BlossomSha256::digest(payload),
            created_at,
            60,
        )
        .expect("upload claim");
        BlossomAuthorizationPlan::for_upload(&claim, PublicKey::from_hex(author).expect("author"))
            .expect("authorization plan")
    }

    #[test]
    fn blossom_upload_plan_binds_every_http_authorization_field() {
        let base = blossom_plan(
            ALICE,
            1_700_000_000,
            "Upload exact image",
            "media.example",
            b"exact-image",
        );
        assert_eq!(base.kind(), 24_242);
        assert_eq!(base.author().to_hex(), ALICE);
        assert_eq!(base.created_at(), 1_700_000_000);
        assert_eq!(base.content(), "Upload exact image");
        assert_eq!(base.tags().len(), 4);
        assert_eq!(base.digest().to_hex().len(), 64);

        let variants = [
            blossom_plan(
                BOB,
                1_700_000_000,
                "Upload exact image",
                "media.example",
                b"exact-image",
            ),
            blossom_plan(
                ALICE,
                1_700_000_001,
                "Upload exact image",
                "media.example",
                b"exact-image",
            ),
            blossom_plan(
                ALICE,
                1_700_000_000,
                "Upload another image",
                "media.example",
                b"exact-image",
            ),
            blossom_plan(
                ALICE,
                1_700_000_000,
                "Upload exact image",
                "uploads.example",
                b"exact-image",
            ),
            blossom_plan(
                ALICE,
                1_700_000_000,
                "Upload exact image",
                "media.example",
                b"different-image",
            ),
        ];
        for variant in variants {
            assert_ne!(variant.expected_event_id(), base.expected_event_id());
            assert_ne!(variant.digest(), base.digest());
        }
    }
}
