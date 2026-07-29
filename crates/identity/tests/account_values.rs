use radroots_identity::{
    AccountId, Error, PublicIdentity, PublicKey,
    account::{Record, Status},
};

const ALICE: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const BOB: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

fn public_identity(value: &str) -> PublicIdentity {
    PublicIdentity::new(PublicKey::from_hex(value).expect("valid public key"))
}

#[test]
fn public_account_values_preserve_identity_and_readiness() {
    let identity = public_identity(ALICE);
    let record = Record::new(identity.clone(), Some("primary".into()), 10);
    let status = Status::Ready {
        account: record.clone(),
    };

    assert_eq!(record.id(), AccountId::from_public_identity(&identity));
    assert_eq!(record.public_identity(), &identity);
    assert_eq!(record.label(), Some("primary"));
    assert_eq!(status.account(), Some(&record));
    assert!(status.is_ready());
}

#[test]
fn decoded_account_parts_cannot_change_identity_or_reverse_time() {
    let identity = public_identity(ALICE);
    let wrong_id = AccountId::from_public_identity(&public_identity(BOB));

    assert!(matches!(
        Record::try_from_parts(wrong_id, identity.clone(), None, 10, 10),
        Err(Error::AccountIdMismatch)
    ));
    assert!(matches!(
        Record::try_from_parts(
            AccountId::from_public_identity(&identity),
            identity,
            None,
            10,
            9,
        ),
        Err(Error::AccountUpdatedBeforeCreated {
            created_at_unix: 10,
            updated_at_unix: 9,
        })
    ));
}

#[test]
fn public_identity_values_are_send_sync_without_host_state() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<AccountId>();
    assert_send_sync::<PublicIdentity>();
    assert_send_sync::<PublicKey>();
    assert_send_sync::<Record>();
    assert_send_sync::<Status>();
}
