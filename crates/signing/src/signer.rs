//! Object-safe signer service-provider interface.

use core::{future::Future, pin::Pin};

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

use crate::{Error, SignReceipt, SignRequest, SignerStatus};

/// A boxed, dynamically dispatched signer future.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Protocol-neutral signing service-provider interface.
///
/// The owned request and boxed futures keep this trait dyn-compatible for
/// local, remote, and host-mediated implementations without choosing an async
/// runtime. Implementations must document the point at which a request creates
/// a durable remote side effect. Dropping the future before that point must
/// leave no durable effect; dropping it afterward does not imply rollback.
pub trait Signer: Send + Sync {
    /// Reports current capabilities and progress without creating a signing
    /// request or another durable side effect.
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>>;

    /// Signs one already-authorized request.
    ///
    /// The request's deadline and cancellation policy remain authoritative
    /// throughout the operation. Implementations must create successful output
    /// with [`SignReceipt::from_signed_event`], which rejects any drift from the
    /// frozen draft. They must not install an executor, spawn hidden workers,
    /// or convert cancellation into silent success.
    fn sign(&self, request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Actor,
        actor::ActorSource,
        request::{CancellationPolicy, SignPolicy},
    };
    use core::sync::atomic::{AtomicUsize, Ordering};
    use radroots_event::envelope::kind::KIND_TRADE_PROPOSAL;
    use radroots_event::{EventDraft, contract::AuthorRole};
    use radroots_identity::PublicKey;
    use radroots_protocol::runtime::v1::OperationId;

    #[cfg(not(feature = "std"))]
    use alloc::{borrow::ToOwned, vec, vec::Vec};
    #[cfg(feature = "std")]
    use std::{borrow::ToOwned, vec, vec::Vec};

    const PUBLIC_KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER_PUBLIC_KEY: &str =
        "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    struct LocalSigner;
    struct RemoteSigner;
    struct CountingSigner(AtomicUsize);

    impl Signer for LocalSigner {
        fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
            Box::pin(async { Ok(SignerStatus::unavailable()) })
        }

        fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
            Box::pin(async { Err(Error) })
        }
    }

    impl Signer for RemoteSigner {
        fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
            Box::pin(async { Ok(SignerStatus::unavailable()) })
        }

        fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
            Box::pin(async { Err(Error) })
        }
    }

    impl Signer for CountingSigner {
        fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
            Box::pin(async { Ok(SignerStatus::unavailable()) })
        }

        fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Err(Error) })
        }
    }

    fn assert_dyn_signer(signer: &dyn Signer) {
        drop(signer.status());
    }

    #[test]
    fn local_and_remote_implementations_are_dyn_compatible() {
        assert_dyn_signer(&LocalSigner);
        assert_dyn_signer(&RemoteSigner);
    }

    #[test]
    fn trait_objects_remain_send_and_sync() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<dyn Signer>();
    }

    #[test]
    fn rejected_request_cannot_invoke_counting_signer() {
        let key_drift_draft = EventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            1_700_000_000,
            Vec::new(),
            "frozen-content",
            PUBLIC_KEY,
        )
        .expect("draft");
        let key_drift_actor = Actor::new(
            PublicKey::from_hex(OTHER_PUBLIC_KEY).expect("public key"),
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .expect("actor");
        let role_drift_draft = EventDraft::new(
            "radroots.trade.proposal.v1",
            KIND_TRADE_PROPOSAL,
            1_700_000_000,
            vec![
                vec![
                    "contract".to_owned(),
                    "radroots.trade.proposal.v1".to_owned(),
                ],
                vec![
                    "d".to_owned(),
                    "11111111111111111111111111111111".to_owned(),
                ],
                vec!["p".to_owned(), PUBLIC_KEY.to_owned()],
            ],
            r#"{"contract_id":"radroots.trade.proposal.v1"}"#,
            PUBLIC_KEY,
        )
        .expect("draft");
        let role_drift_actor = Actor::new(
            PublicKey::from_hex(PUBLIC_KEY).expect("public key"),
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Seller],
        )
        .expect("actor");
        let policy = SignPolicy::new(1_700_000_100, CancellationPolicy::PreservePublishedRequest)
            .expect("policy");
        let signer = CountingSigner(AtomicUsize::new(0));

        let requests = [
            SignRequest::new(
                OperationId::SyncPush,
                key_drift_actor,
                key_drift_draft,
                policy,
            ),
            SignRequest::new(
                OperationId::SyncPush,
                role_drift_actor,
                role_drift_draft,
                policy,
            ),
        ];
        for request in requests {
            assert!(request.is_err());
            if let Ok(request) = request {
                drop(signer.sign(request));
            }
        }

        assert_eq!(signer.0.load(Ordering::Relaxed), 0);
    }
}
