//! Public, transport-neutral account value types.

use alloc::string::String;

use crate::{Error, IdentityId, PublicIdentity, key::define_identifier};

define_identifier! {
    /// A canonical public account identifier.
    pub struct AccountId;
}

impl AccountId {
    /// Derives the account identifier from its public identity identifier.
    #[must_use]
    pub const fn from_identity_id(identity_id: IdentityId) -> Self {
        Self::from_validated_bytes(identity_id.into_bytes())
    }

    /// Derives the account identifier from a public identity.
    #[must_use]
    pub const fn from_public_identity(public_identity: &PublicIdentity) -> Self {
        Self::from_identity_id(public_identity.id())
    }
}

impl From<IdentityId> for AccountId {
    fn from(value: IdentityId) -> Self {
        Self::from_identity_id(value)
    }
}

impl From<&PublicIdentity> for AccountId {
    fn from(value: &PublicIdentity) -> Self {
        Self::from_public_identity(value)
    }
}

/// A portable public account record.
///
/// The account identifier is always derived from `public_identity`. Secret
/// material, persistence location, account selection, and signer state are
/// intentionally absent.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    account_id: AccountId,
    public_identity: PublicIdentity,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    label: Option<String>,
    created_at_unix: u64,
    updated_at_unix: u64,
}

impl Record {
    /// Creates a public account record at the supplied Unix timestamp.
    #[must_use]
    pub fn new(
        public_identity: PublicIdentity,
        label: Option<String>,
        created_at_unix: u64,
    ) -> Self {
        Self {
            account_id: AccountId::from_public_identity(&public_identity),
            public_identity,
            label,
            created_at_unix,
            updated_at_unix: created_at_unix,
        }
    }

    /// Validates an account record assembled from separately decoded parts.
    pub fn try_from_parts(
        account_id: AccountId,
        public_identity: PublicIdentity,
        label: Option<String>,
        created_at_unix: u64,
        updated_at_unix: u64,
    ) -> Result<Self, Error> {
        if account_id != AccountId::from_public_identity(&public_identity) {
            return Err(Error::AccountIdMismatch);
        }
        if updated_at_unix < created_at_unix {
            return Err(Error::AccountUpdatedBeforeCreated {
                created_at_unix,
                updated_at_unix,
            });
        }
        Ok(Self {
            account_id,
            public_identity,
            label,
            created_at_unix,
            updated_at_unix,
        })
    }

    /// Returns the canonical account identifier.
    #[must_use]
    pub const fn id(&self) -> AccountId {
        self.account_id
    }

    /// Borrows the public identity represented by this account.
    #[must_use]
    pub const fn public_identity(&self) -> &PublicIdentity {
        &self.public_identity
    }

    /// Borrows the optional host-facing label.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Replaces the optional host-facing label without changing identity.
    pub fn set_label(&mut self, label: Option<String>) {
        self.label = label;
    }

    /// Returns the record creation timestamp as Unix seconds.
    #[must_use]
    pub const fn created_at_unix(&self) -> u64 {
        self.created_at_unix
    }

    /// Returns the latest record update timestamp as Unix seconds.
    #[must_use]
    pub const fn updated_at_unix(&self) -> u64 {
        self.updated_at_unix
    }

    /// Advances the record update timestamp without permitting time reversal.
    pub fn touch_updated(&mut self, updated_at_unix: u64) -> Result<(), Error> {
        if updated_at_unix < self.updated_at_unix {
            return Err(Error::AccountUpdateRegressed {
                current_updated_at_unix: self.updated_at_unix,
                proposed_updated_at_unix: updated_at_unix,
            });
        }
        self.updated_at_unix = updated_at_unix;
        Ok(())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Record {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RecordRepr {
            account_id: AccountId,
            public_identity: PublicIdentity,
            #[serde(default)]
            label: Option<String>,
            created_at_unix: u64,
            updated_at_unix: u64,
        }

        let value = RecordRepr::deserialize(deserializer)?;
        Self::try_from_parts(
            value.account_id,
            value.public_identity,
            value.label,
            value.created_at_unix,
            value.updated_at_unix,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// Publicly observable account readiness without secret or signer details.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Status {
    /// No account has been configured by the composing host.
    #[default]
    NotConfigured,
    /// The account can be observed but has no available signing capability.
    PublicOnly { account: Record },
    /// The composing host reports that the account can sign.
    Ready { account: Record },
}

impl Status {
    /// Borrows the configured account, when present.
    #[must_use]
    pub const fn account(&self) -> Option<&Record> {
        match self {
            Self::NotConfigured => None,
            Self::PublicOnly { account } | Self::Ready { account } => Some(account),
        }
    }

    /// Reports whether the composing host marked the account ready to sign.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "serde")]
    use alloc::format;
    use alloc::string::ToString;
    use core::str::FromStr;

    use super::*;

    const ALICE: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const BOB: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    #[test]
    fn account_ids_round_trip_through_identity_ids() {
        let identity_id = IdentityId::from_hex(ALICE).expect("valid identity ID");
        let account_id = AccountId::from(identity_id);

        assert_eq!(account_id.to_hex(), ALICE);
        assert_eq!(account_id.as_bytes(), identity_id.as_bytes());
        assert_eq!(AccountId::from_str(ALICE).unwrap(), account_id);
    }

    #[test]
    fn account_ids_validate_and_order_canonical_bytes() {
        let alice = AccountId::from_hex(ALICE).expect("alice account");
        let bob = AccountId::from_hex(BOB).expect("bob account");

        assert_eq!(AccountId::from_bytes(alice.into_bytes()).unwrap(), alice);
        assert!(alice < bob);
        assert!(AccountId::from_hex("not-an-account").is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn account_ids_serde_as_validated_canonical_hex() {
        let account_id = AccountId::from_hex(ALICE).expect("valid account ID");
        let encoded = serde_json::to_string(&account_id).expect("serialize account ID");

        assert_eq!(encoded, format!("\"{ALICE}\""));
        assert_eq!(
            serde_json::from_str::<AccountId>(&encoded).expect("deserialize account ID"),
            account_id
        );
        assert!(serde_json::from_str::<AccountId>("\"not-an-account\"").is_err());
    }

    fn public_identity(value: &str) -> PublicIdentity {
        PublicIdentity::new(crate::PublicKey::from_hex(value).expect("public key"))
    }

    #[test]
    fn records_derive_identity_and_enforce_monotonic_timestamps() {
        let identity = public_identity(ALICE);
        let mut record = Record::new(identity.clone(), Some("primary".to_string()), 10);

        assert_eq!(record.id(), AccountId::from_public_identity(&identity));
        assert_eq!(record.public_identity(), &identity);
        assert_eq!(record.label(), Some("primary"));
        assert_eq!(record.created_at_unix(), 10);
        assert_eq!(record.updated_at_unix(), 10);
        assert!(matches!(
            record.touch_updated(9),
            Err(Error::AccountUpdateRegressed {
                current_updated_at_unix: 10,
                proposed_updated_at_unix: 9,
            })
        ));
        assert_eq!(record.updated_at_unix(), 10);

        record.touch_updated(12).expect("advance timestamp");
        assert!(matches!(
            record.touch_updated(11),
            Err(Error::AccountUpdateRegressed {
                current_updated_at_unix: 12,
                proposed_updated_at_unix: 11,
            })
        ));
        record.set_label(None);
        assert_eq!(record.updated_at_unix(), 12);
        assert_eq!(record.label(), None);

        let wrong_id = AccountId::from_identity_id(IdentityId::from_hex(BOB).unwrap());
        assert!(matches!(
            Record::try_from_parts(wrong_id, identity, None, 10, 10),
            Err(Error::AccountIdMismatch)
        ));
    }

    #[test]
    fn status_exposes_only_public_account_readiness() {
        let record = Record::new(public_identity(ALICE), None, 10);
        let not_configured = Status::default();
        let public_only = Status::PublicOnly {
            account: record.clone(),
        };
        let ready = Status::Ready {
            account: record.clone(),
        };

        assert!(not_configured.account().is_none());
        assert_eq!(public_only.account(), Some(&record));
        assert!(!public_only.is_ready());
        assert_eq!(ready.account(), Some(&record));
        assert!(ready.is_ready());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn records_and_status_serde_revalidate_public_identity_and_timestamps() {
        let record = Record::new(public_identity(ALICE), Some("primary".to_string()), 10);
        let status = Status::Ready {
            account: record.clone(),
        };
        let encoded_record = serde_json::to_string(&record).expect("serialize record");
        let encoded_status = serde_json::to_string(&status).expect("serialize status");

        assert_eq!(
            serde_json::from_str::<Record>(&encoded_record).expect("deserialize record"),
            record
        );
        assert_eq!(
            serde_json::from_str::<Status>(&encoded_status).expect("deserialize status"),
            status
        );
        assert!(
            serde_json::from_str::<Record>(&encoded_record.replace(
                &format!("\"account_id\":\"{ALICE}\""),
                &format!("\"account_id\":\"{BOB}\""),
            ))
            .is_err()
        );
        assert!(
            serde_json::from_str::<Record>(
                &encoded_record.replace("\"created_at_unix\":10", "\"created_at_unix\":11")
            )
            .is_err()
        );
        assert!(!encoded_record.contains("secret"));
        assert!(!encoded_status.contains("signer"));
    }
}
