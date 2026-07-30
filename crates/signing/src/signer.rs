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
    /// throughout the operation. Implementations must not install an executor,
    /// spawn hidden workers, or convert cancellation into silent success.
    fn sign(&self, request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct LocalSigner;
    struct RemoteSigner;

    impl Signer for LocalSigner {
        fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
            Box::pin(async { Ok(SignerStatus) })
        }

        fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
            Box::pin(async { Ok(SignReceipt) })
        }
    }

    impl Signer for RemoteSigner {
        fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
            Box::pin(async { Ok(SignerStatus) })
        }

        fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
            Box::pin(async { Err(Error) })
        }
    }

    fn assert_dyn_signer(signer: &dyn Signer) {
        drop(signer.status());
        drop(signer.sign(SignRequest));
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
}
