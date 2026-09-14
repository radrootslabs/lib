//! Object-safe signer service-provider interface.

use core::{future::Future, pin::Pin};

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, sync::Arc};
#[cfg(feature = "std")]
use std::{boxed::Box, sync::Arc};

use crate::{
    AuthoredSignEvidence, Error, SignReceipt, SignRequest, SignerStatus, error::Kind,
    receipt::verify_identity,
};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Protocol-neutral, caller-driven signing service-provider interface.
///
/// Implementations must document their durable remote-effect point. Dropping
/// a future after that point does not imply rollback. Replay behavior is
/// advertised through signer status and every successful result must use a
/// verified receipt or authored-evidence constructor.
pub trait Signer: Send + Sync {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>>;

    /// Signs one already-authorized exact plan.
    ///
    /// Implementations must observe the request's millisecond deadline and
    /// cancellation signal throughout the operation, preserve its stable
    /// signer request ID for remote replay, and create success only through
    /// [`SignReceipt::from_signed_event`].
    fn sign(&self, request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>>;

    /// Signs an authored plan while retaining any verified result of started work.
    ///
    /// An override must honor deadline and cancellation before starting work,
    /// but may return exact evidence received afterward. Such evidence is not
    /// permission to resume stopped work. The composing host owns polling and
    /// durable reconciliation; dropping this future does not undo a signature.
    ///
    /// The default delegates to [`Self::sign`] and preserves existing adapters.
    /// It cannot recover evidence that the adapter discards. Blossom requests
    /// and already-cancelled requests are rejected before invoking that adapter.
    fn sign_authored_evidence(
        &self,
        request: SignRequest,
    ) -> BoxFuture<'_, Result<AuthoredSignEvidence, Error>> {
        Box::pin(async move {
            if request.authored_plan().is_none() {
                return Err(Error::new(Kind::InvalidArgument));
            }
            if request.cancellation_signal().is_cancelled() {
                return Err(Error::new(Kind::SignerCancelled));
            }
            let receipt = self.sign(request.clone()).await?;
            verify_identity(
                receipt.operation_kind(),
                receipt.intent_id(),
                receipt.signer_request_id(),
                &request,
            )?;
            AuthoredSignEvidence::from_signed_event(
                &request,
                receipt.signed_event().clone(),
                receipt.completed_at_unix_ms(),
            )
        })
    }
}

/// Shared signer handle used by composing hosts without selecting a runtime.
pub type DynSigner = Arc<dyn Signer>;

#[cfg(test)]
mod tests {
    use super::*;

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
