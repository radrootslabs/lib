//! Public account value types.

use crate::{IdentityId, key::define_identifier};

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
}

impl From<IdentityId> for AccountId {
    fn from(value: IdentityId) -> Self {
        Self::from_identity_id(value)
    }
}

#[cfg(test)]
mod tests {
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
}
