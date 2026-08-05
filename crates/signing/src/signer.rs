//! Object-safe signer service-provider interface.

use core::{future::Future, pin::Pin};

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, sync::Arc};
#[cfg(feature = "std")]
use std::{boxed::Box, sync::Arc};

use crate::{Error, SignReceipt, SignRequest, SignerStatus};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Protocol-neutral, caller-driven signing service-provider interface.
///
/// Implementations must document their durable remote-effect point. Dropping
/// a future after that point does not imply rollback. Replay behavior is
/// advertised through signer status and every successful result must use the
/// verified receipt constructor.
pub trait Signer: Send + Sync {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>>;

    /// Signs one already-authorized exact plan.
    ///
    /// Implementations must observe the request's millisecond deadline and
    /// cancellation signal throughout the operation, preserve its stable
    /// signer request ID for remote replay, and create success only through
    /// [`SignReceipt::from_signed_event`].
    fn sign(&self, request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>>;
}

/// Shared signer handle used by composing hosts without selecting a runtime.
pub type DynSigner = Arc<dyn Signer>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Kind;

    struct Stub;

    impl Signer for Stub {
        fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
            Box::pin(async { Ok(SignerStatus::unavailable()) })
        }

        fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
            Box::pin(async { Err(Error::new(Kind::SignerUnavailable)) })
        }
    }

    #[test]
    fn signer_remains_dyn_send_and_sync() {
        fn assert_dyn(_: &dyn Signer) {}
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_dyn(&Stub);
        assert_send_sync::<dyn Signer>();
    }
}
