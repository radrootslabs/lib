#![forbid(unsafe_code)]

use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(feature = "std")]
use std::string::String;

#[derive(Debug, PartialEq, Eq)]
pub enum RadrootsAuthorityError {
    InvalidActorPubkey,

    InvalidActorAccountIdEmpty,

    InvalidActorAccountIdUntrimmed,

    InvalidActorAccountIdControlCharacter,

    InvalidActorAccountIdTooLong {
        max_len: usize,
    },

    InvalidSignerPubkey,

    UnknownContract {
        contract_id: String,
    },

    DraftKindMismatch {
        contract_id: String,
        expected_kind: u32,
        actual_kind: u32,
    },

    ActorRoleUnsatisfied {
        contract_id: String,
        required_role: radroots_events::contract::RadrootsActorRole,
    },

    ActorPubkeyMismatch {
        expected_pubkey: String,
        actor_pubkey: String,
    },

    SignerPubkeyMismatch {
        expected_pubkey: String,
        signer_pubkey: String,
    },

    SignedEventPubkeyMismatch {
        expected_pubkey: String,
        actual_pubkey: String,
    },

    SignedEventIdMismatch {
        expected_event_id: String,
        actual_event_id: String,
    },

    SignedEventCreatedAtMismatch {
        expected_created_at: u32,
        actual_created_at: u32,
    },

    SignedEventKindMismatch {
        expected_kind: u32,
        actual_kind: u32,
    },

    SignedEventTagsMismatch {
        expected_len: usize,
        actual_len: usize,
    },

    SignedEventContentMismatch {
        expected_len: usize,
        actual_len: usize,
    },

    SignedEventComputedIdInvalid {
        message: String,
    },

    SignedEventComputedIdMismatch {
        expected_event_id: String,
        computed_event_id: String,
    },

    Signer(RadrootsSignerError),
}

impl fmt::Display for RadrootsAuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidActorPubkey => write!(f, "invalid actor public key"),
            Self::InvalidActorAccountIdEmpty => write!(f, "invalid actor account id: empty"),
            Self::InvalidActorAccountIdUntrimmed => {
                write!(
                    f,
                    "invalid actor account id: contains leading or trailing whitespace"
                )
            }
            Self::InvalidActorAccountIdControlCharacter => {
                write!(f, "invalid actor account id: contains a control character")
            }
            Self::InvalidActorAccountIdTooLong { max_len } => {
                write!(
                    f,
                    "invalid actor account id: longer than {max_len} characters"
                )
            }
            Self::InvalidSignerPubkey => write!(f, "invalid signer public key"),
            Self::UnknownContract { contract_id } => {
                write!(f, "unknown event contract `{contract_id}`")
            }
            Self::DraftKindMismatch {
                contract_id,
                expected_kind,
                actual_kind,
            } => write!(
                f,
                "event contract `{contract_id}` expects kind {expected_kind}, got {actual_kind}"
            ),
            Self::ActorRoleUnsatisfied {
                contract_id,
                required_role,
            } => write!(
                f,
                "actor does not satisfy role {required_role:?} for contract `{contract_id}`"
            ),
            Self::ActorPubkeyMismatch {
                expected_pubkey,
                actor_pubkey,
            } => write!(
                f,
                "actor pubkey mismatch: expected {expected_pubkey}, got {actor_pubkey}"
            ),
            Self::SignerPubkeyMismatch {
                expected_pubkey,
                signer_pubkey,
            } => write!(
                f,
                "signer pubkey mismatch: expected {expected_pubkey}, got {signer_pubkey}"
            ),
            Self::SignedEventPubkeyMismatch {
                expected_pubkey,
                actual_pubkey,
            } => write!(
                f,
                "signed event pubkey mismatch: expected {expected_pubkey}, got {actual_pubkey}"
            ),
            Self::SignedEventIdMismatch {
                expected_event_id,
                actual_event_id,
            } => write!(
                f,
                "signed event id mismatch: expected {expected_event_id}, got {actual_event_id}"
            ),
            Self::SignedEventCreatedAtMismatch {
                expected_created_at,
                actual_created_at,
            } => write!(
                f,
                "signed event created_at mismatch: expected {expected_created_at}, got {actual_created_at}"
            ),
            Self::SignedEventKindMismatch {
                expected_kind,
                actual_kind,
            } => write!(
                f,
                "signed event kind mismatch: expected {expected_kind}, got {actual_kind}"
            ),
            Self::SignedEventTagsMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "signed event tags mismatch: expected {expected_len} tags, got {actual_len} tags"
            ),
            Self::SignedEventContentMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "signed event content mismatch: expected {expected_len} bytes, got {actual_len} bytes"
            ),
            Self::SignedEventComputedIdInvalid { message } => {
                write!(
                    f,
                    "signed event computed id could not be derived: {message}"
                )
            }
            Self::SignedEventComputedIdMismatch {
                expected_event_id,
                computed_event_id,
            } => write!(
                f,
                "signed event computed id mismatch: expected {expected_event_id}, computed {computed_event_id}"
            ),
            Self::Signer(error) => write!(f, "signer error: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsAuthorityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Signer(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RadrootsSignerError> for RadrootsAuthorityError {
    fn from(error: RadrootsSignerError) -> Self {
        Self::Signer(error)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RadrootsSignerError {
    Unavailable,

    Rejected,

    SigningFailed { message: String },
}

impl fmt::Display for RadrootsSignerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(f, "signer unavailable"),
            Self::Rejected => write!(f, "signer rejected draft"),
            Self::SigningFailed { message } => write!(f, "signing failed: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSignerError {}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_events::contract::RadrootsActorRole;
    use std::error::Error as _;

    #[test]
    fn authority_error_display_uses_contract_messages() {
        let cases = [
            (
                RadrootsAuthorityError::InvalidActorPubkey,
                "invalid actor public key",
            ),
            (
                RadrootsAuthorityError::InvalidActorAccountIdEmpty,
                "invalid actor account id: empty",
            ),
            (
                RadrootsAuthorityError::InvalidActorAccountIdUntrimmed,
                "invalid actor account id: contains leading or trailing whitespace",
            ),
            (
                RadrootsAuthorityError::InvalidActorAccountIdControlCharacter,
                "invalid actor account id: contains a control character",
            ),
            (
                RadrootsAuthorityError::InvalidActorAccountIdTooLong { max_len: 128 },
                "invalid actor account id: longer than 128 characters",
            ),
            (
                RadrootsAuthorityError::InvalidSignerPubkey,
                "invalid signer public key",
            ),
            (
                RadrootsAuthorityError::UnknownContract {
                    contract_id: "radroots.unknown.v1".to_owned(),
                },
                "unknown event contract `radroots.unknown.v1`",
            ),
            (
                RadrootsAuthorityError::DraftKindMismatch {
                    contract_id: "radroots.social.post.v1".to_owned(),
                    expected_kind: 1,
                    actual_kind: 2,
                },
                "event contract `radroots.social.post.v1` expects kind 1, got 2",
            ),
            (
                RadrootsAuthorityError::ActorRoleUnsatisfied {
                    contract_id: "radroots.listing.published.v1".to_owned(),
                    required_role: RadrootsActorRole::Seller,
                },
                "actor does not satisfy role Seller for contract `radroots.listing.published.v1`",
            ),
            (
                RadrootsAuthorityError::ActorPubkeyMismatch {
                    expected_pubkey: "expected".to_owned(),
                    actor_pubkey: "actor".to_owned(),
                },
                "actor pubkey mismatch: expected expected, got actor",
            ),
            (
                RadrootsAuthorityError::SignerPubkeyMismatch {
                    expected_pubkey: "expected".to_owned(),
                    signer_pubkey: "signer".to_owned(),
                },
                "signer pubkey mismatch: expected expected, got signer",
            ),
            (
                RadrootsAuthorityError::SignedEventPubkeyMismatch {
                    expected_pubkey: "expected".to_owned(),
                    actual_pubkey: "actual".to_owned(),
                },
                "signed event pubkey mismatch: expected expected, got actual",
            ),
            (
                RadrootsAuthorityError::SignedEventIdMismatch {
                    expected_event_id: "expected".to_owned(),
                    actual_event_id: "actual".to_owned(),
                },
                "signed event id mismatch: expected expected, got actual",
            ),
            (
                RadrootsAuthorityError::SignedEventCreatedAtMismatch {
                    expected_created_at: 1,
                    actual_created_at: 2,
                },
                "signed event created_at mismatch: expected 1, got 2",
            ),
            (
                RadrootsAuthorityError::SignedEventKindMismatch {
                    expected_kind: 1,
                    actual_kind: 2,
                },
                "signed event kind mismatch: expected 1, got 2",
            ),
            (
                RadrootsAuthorityError::SignedEventTagsMismatch {
                    expected_len: 2,
                    actual_len: 1,
                },
                "signed event tags mismatch: expected 2 tags, got 1 tags",
            ),
            (
                RadrootsAuthorityError::SignedEventContentMismatch {
                    expected_len: 17,
                    actual_len: 2,
                },
                "signed event content mismatch: expected 17 bytes, got 2 bytes",
            ),
            (
                RadrootsAuthorityError::SignedEventComputedIdInvalid {
                    message: "invalid event id".to_owned(),
                },
                "signed event computed id could not be derived: invalid event id",
            ),
            (
                RadrootsAuthorityError::SignedEventComputedIdMismatch {
                    expected_event_id: "expected".to_owned(),
                    computed_event_id: "computed".to_owned(),
                },
                "signed event computed id mismatch: expected expected, computed computed",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
            assert!(error.source().is_none());
        }
    }

    #[test]
    fn signer_error_display_and_source_are_stable() {
        assert_eq!(
            RadrootsSignerError::Unavailable.to_string(),
            "signer unavailable"
        );
        assert_eq!(
            RadrootsSignerError::Rejected.to_string(),
            "signer rejected draft"
        );

        let signer_error = RadrootsSignerError::SigningFailed {
            message: "deterministic failure".to_owned(),
        };
        assert_eq!(
            signer_error.to_string(),
            "signing failed: deterministic failure"
        );

        let authority_error = RadrootsAuthorityError::from(signer_error);
        assert_eq!(
            authority_error.to_string(),
            "signer error: signing failed: deterministic failure"
        );
        assert_eq!(
            authority_error.source().expect("signer source").to_string(),
            "signing failed: deterministic failure"
        );
    }
}
