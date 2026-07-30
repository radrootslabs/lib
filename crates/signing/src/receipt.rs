//! Signing receipts.

use core::fmt;
use radroots_event::{SignedEvent, draft::validate_signed_nostr_event_matches_draft};
use radroots_protocol::runtime::v1::OperationId;

use crate::{Error, SignRequest};

/// Successful signer output with portable operation provenance.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, PartialEq, Eq)]
pub struct SignReceipt {
    operation_id: OperationId,
    signed_event: SignedEvent,
    completed_at_unix: u64,
}

impl fmt::Debug for SignReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignReceipt")
            .field("operation_id", &self.operation_id)
            .field("signed_event_id", &self.signed_event.id_str())
            .field("completed_at_unix", &self.completed_at_unix)
            .finish()
    }
}

impl SignReceipt {
    /// Validates signer output against the exact request draft and creates its
    /// receipt. This is the only public receipt constructor.
    pub fn from_signed_event(
        request: &SignRequest,
        signed_event: SignedEvent,
        completed_at_unix: u64,
    ) -> Result<Self, Error> {
        validate_signed_nostr_event_matches_draft(&signed_event, request.draft())
            .map_err(|_| Error)?;
        Ok(Self {
            operation_id: request.operation_id(),
            signed_event,
            completed_at_unix,
        })
    }

    /// Returns the originating runtime operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    /// Borrows the invariant-checked signed event.
    #[must_use]
    pub const fn signed_event(&self) -> &SignedEvent {
        &self.signed_event
    }

    /// Returns the host-supplied completion timestamp.
    #[must_use]
    pub const fn completed_at_unix(&self) -> u64 {
        self.completed_at_unix
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Actor,
        actor::ActorSource,
        request::{CancellationPolicy, SignPolicy},
    };
    use radroots_event::{EventDraft, contract::AuthorRole, wire::Nip01EventWire};
    use radroots_identity::PublicKey;

    #[cfg(not(feature = "std"))]
    use alloc::{borrow::ToOwned, string::String, vec, vec::Vec};
    #[cfg(feature = "std")]
    use std::{borrow::ToOwned, string::String, vec, vec::Vec};

    const PUBLIC_KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER_PUBLIC_KEY: &str =
        "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    fn request() -> SignRequest {
        let actor = Actor::new(
            PublicKey::from_hex(PUBLIC_KEY).expect("public key"),
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .expect("actor");
        let draft = EventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            1_700_000_000,
            Vec::new(),
            "frozen-content",
            PUBLIC_KEY,
        )
        .expect("draft");
        SignRequest::new(
            OperationId::SyncPush,
            actor,
            draft,
            SignPolicy::new(1_700_000_100, CancellationPolicy::PreservePublishedRequest)
                .expect("policy"),
        )
        .expect("request")
    }

    fn signed_event(
        pubkey: &str,
        created_at: u64,
        kind: u32,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> SignedEvent {
        let mut wire = Nip01EventWire {
            id: String::new(),
            pubkey: pubkey.to_owned(),
            created_at,
            kind,
            tags,
            content: content.to_owned(),
            sig: core::iter::repeat_n('f', 128).collect(),
            extra: Default::default(),
        };
        wire.id = wire.computed_event_id().expect("event id").into_string();
        let raw_json = serde_json::json!({
            "id": wire.id,
            "pubkey": wire.pubkey,
            "created_at": wire.created_at,
            "kind": wire.kind,
            "tags": wire.tags,
            "content": wire.content,
            "sig": wire.sig,
        })
        .to_string();
        SignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn matching_event(request: &SignRequest) -> SignedEvent {
        let draft = request.draft();
        signed_event(
            PUBLIC_KEY,
            draft.created_at_u64(),
            draft.kind_u32(),
            draft.tags_as_vec(),
            draft.content(),
        )
    }

    #[test]
    fn exact_signed_event_creates_receipt_with_request_operation() {
        let request = request();
        let receipt = SignReceipt::from_signed_event(&request, matching_event(&request), 42)
            .expect("receipt");

        assert_eq!(receipt.operation_id(), OperationId::SyncPush);
        assert_eq!(receipt.completed_at_unix(), 42);
        assert_eq!(receipt.signed_event().content(), "frozen-content");
        assert!(!format!("{receipt:?}").contains("frozen-content"));
    }

    #[test]
    fn every_publicly_constructible_signed_event_drift_is_rejected() {
        let request = request();
        let draft = request.draft();
        let cases = [
            signed_event(
                OTHER_PUBLIC_KEY,
                draft.created_at_u64(),
                draft.kind_u32(),
                draft.tags_as_vec(),
                draft.content(),
            ),
            signed_event(
                PUBLIC_KEY,
                draft.created_at_u64() + 1,
                draft.kind_u32(),
                draft.tags_as_vec(),
                draft.content(),
            ),
            signed_event(
                PUBLIC_KEY,
                draft.created_at_u64(),
                draft.kind_u32() + 1,
                draft.tags_as_vec(),
                draft.content(),
            ),
            signed_event(
                PUBLIC_KEY,
                draft.created_at_u64(),
                draft.kind_u32(),
                vec![vec!["changed".to_owned()]],
                draft.content(),
            ),
            signed_event(
                PUBLIC_KEY,
                draft.created_at_u64(),
                draft.kind_u32(),
                draft.tags_as_vec(),
                "changed-content",
            ),
        ];

        for event in cases {
            assert_eq!(
                SignReceipt::from_signed_event(&request, event, 42),
                Err(Error)
            );
        }
    }
}
