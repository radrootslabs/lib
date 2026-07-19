use core::fmt;

use radroots_event::{RadrootsEventEnvelope, contract::RadrootsEventContract};

use crate::{
    deletion::inbound::{
        RadrootsInboundNip09DeletionProjection, RadrootsNip09DeletionProjectionError,
        project_verified_nip09_deletion_request_event,
    },
    verification::{
        RadrootsNip01VerificationError, RadrootsSignatureVerifiedEvent, verify_nip01_event,
    },
};

/// A signature-and-id verified kind-5 event admitted as a NIP-09 request.
///
/// Admission establishes only the request contract. It does not establish that
/// any requested deletion effect is authorized or applicable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAdmittedNip09DeletionRequestEvent {
    verified_event: RadrootsSignatureVerifiedEvent,
    projection: RadrootsInboundNip09DeletionProjection,
}

impl RadrootsAdmittedNip09DeletionRequestEvent {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn event(&self) -> &RadrootsEventEnvelope {
        self.verified_event.event()
    }

    pub const fn projection(&self) -> &RadrootsInboundNip09DeletionProjection {
        &self.projection
    }

    pub fn contract(&self) -> &'static RadrootsEventContract {
        radroots_event::contract::event_contract(self.projection.contract_id())
            .expect("NIP-09 deletion request contract is registry-owned")
    }

    pub fn into_parts(
        self,
    ) -> (
        RadrootsSignatureVerifiedEvent,
        RadrootsInboundNip09DeletionProjection,
    ) {
        (self.verified_event, self.projection)
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip09DeletionAdmissionError {
    Nip01Verification(RadrootsNip01VerificationError),
    Projection(RadrootsNip09DeletionProjectionError),
}

impl RadrootsNip09DeletionAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Nip01Verification(error) => error.code(),
            Self::Projection(error) => error.code(),
        }
    }
}

impl fmt::Display for RadrootsNip09DeletionAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nip01Verification(error) => write!(formatter, "{error}"),
            Self::Projection(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip09DeletionAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Nip01Verification(error) => Some(error),
            Self::Projection(error) => Some(error),
        }
    }
}

impl From<RadrootsNip01VerificationError> for RadrootsNip09DeletionAdmissionError {
    fn from(value: RadrootsNip01VerificationError) -> Self {
        Self::Nip01Verification(value)
    }
}

impl From<RadrootsNip09DeletionProjectionError> for RadrootsNip09DeletionAdmissionError {
    fn from(value: RadrootsNip09DeletionProjectionError) -> Self {
        Self::Projection(value)
    }
}

pub fn admit_verified_nip09_deletion_request_event(
    verified_event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedNip09DeletionRequestEvent, RadrootsNip09DeletionAdmissionError> {
    let projection = project_verified_nip09_deletion_request_event(&verified_event)?;
    Ok(RadrootsAdmittedNip09DeletionRequestEvent {
        verified_event,
        projection,
    })
}

pub fn verify_and_admit_nip09_deletion_request_event(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsAdmittedNip09DeletionRequestEvent, RadrootsNip09DeletionAdmissionError> {
    admit_verified_nip09_deletion_request_event(verify_nip01_event(event)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "nostr")]
    #[test]
    fn admitted_contract_resolves_registry_entry() {
        use nostr::secp256k1::Message;
        use nostr::{Keys, SECP256K1};
        use radroots_event::{
            RadrootsEventEnvelopeParts, kinds::KIND_DELETION_REQUEST,
            wire::compute_canonical_nip01_event_id,
        };
        use radroots_test_fixtures::FIXTURE_ALICE_SECRET_KEY_HEX;

        let keys =
            Keys::parse(FIXTURE_ALICE_SECRET_KEY_HEX).expect("fixed fixture secret key must parse");
        let author = keys.public_key().to_string();
        let created_at = 1_800_000_300;
        let tags = vec![vec!["e".to_string(), "a".repeat(64)]];
        let content = "superseded";
        let id = compute_canonical_nip01_event_id(
            author.as_str(),
            created_at,
            KIND_DELETION_REQUEST,
            &tags,
            content,
        )
        .expect("canonical deletion request id");
        let nostr_id = nostr::EventId::from_hex(id.as_str()).expect("Nostr event id");
        let message = Message::from_digest(nostr_id.to_bytes());
        let signature = SECP256K1.sign_schnorr_no_aux_rand(&message, keys.key_pair(SECP256K1));
        let event = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: id.into_string(),
            author,
            created_at,
            kind: KIND_DELETION_REQUEST,
            tags,
            content: content.to_string(),
            sig: signature.to_string(),
        })
        .expect("valid deletion request envelope");

        let admitted = verify_and_admit_nip09_deletion_request_event(event)
            .expect("admitted deletion request");

        assert_eq!(
            admitted.contract().id,
            "radroots.social.deletion_request.v1"
        );
        assert_eq!(admitted.contract().kind, KIND_DELETION_REQUEST);
    }

    #[test]
    fn stable_error_codes_delegate_to_verification_and_projection() {
        let verification = RadrootsNip09DeletionAdmissionError::Nip01Verification(
            RadrootsNip01VerificationError::SignatureInvalid,
        );
        let projection = RadrootsNip09DeletionAdmissionError::Projection(
            RadrootsNip09DeletionProjectionError::TargetMissing,
        );
        assert_eq!(verification.code(), "signature_invalid");
        assert_eq!(projection.code(), "deletion_target_missing");
        assert!(!verification.to_string().is_empty());
        assert!(!projection.to_string().is_empty());
    }
}
