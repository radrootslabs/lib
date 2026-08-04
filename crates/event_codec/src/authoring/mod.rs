//! Exact authored event bodies, plans, and semantic digests.

#[cfg(not(feature = "std"))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{borrow::ToOwned, string::String, vec::Vec};

use radroots_event::{GenericEventDraft, contract::ContractKey, draft::DraftError, id::EventId};
use radroots_identity::PublicKey;
use sha2::{Digest, Sha256};

pub const PLAN_WIRE_VERSION_V1: u32 = 1;
const PLAN_DIGEST_DOMAIN: &[u8] = b"radroots.authored_event_plan.v1";

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
}

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

    fn from_validated_parts(
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
}
