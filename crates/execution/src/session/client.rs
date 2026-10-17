use super::{HostFuture, Session, SessionHost, SessionState, SignerSession, host::Shared};
use core::num::NonZeroU16;
use radroots_signing::provider::{ProviderError, SignerHandle};
use std::{
    boxed::Box,
    sync::{Mutex, MutexGuard},
};

struct Resources<H> {
    host: H,
    signer: Shared<Mutex<Option<SignerSession>>>,
    host_quiesced: Mutex<bool>,
}
fn signer_lock(value: &Mutex<Option<SignerSession>>) -> MutexGuard<'_, Option<SignerSession>> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
impl<H: SessionHost> SessionHost for Resources<H> {
    fn start(&self) -> HostFuture<'_, crate::Result<()>> {
        self.host.start()
    }
    fn quiesce(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            let quiesced = *self
                .host_quiesced
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !quiesced {
                self.host.quiesce().await?;
                *self
                    .host_quiesced
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            }
            let signer = signer_lock(&self.signer).clone();
            if let Some(signer) = signer {
                signer.quiesce().await?;
            }
            Ok(())
        })
    }
    fn disconnect(&self) -> HostFuture<'_, crate::Result<()>> {
        self.host.disconnect()
    }
    fn dispose_callbacks(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            let signer = signer_lock(&self.signer).clone();
            if let Some(signer) = signer {
                signer.close().await?;
            }
            self.host.dispose_callbacks().await
        })
    }
    fn close_storage(&self) -> HostFuture<'_, crate::Result<()>> {
        self.host.close_storage()
    }
}

/// Shared client resource ownership and one explicitly attached signer scope.
///
/// No observed identity establishes workspace, command or stock authority.
/// The concrete facade supplies its actual resources through `SessionHost`.
pub struct ClientSession<H: SessionHost> {
    session: Session<Resources<H>>,
    signer: Shared<Mutex<Option<SignerSession>>>,
}
impl<H: SessionHost> Clone for ClientSession<H> {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            signer: self.signer.clone(),
        }
    }
}
impl<H: SessionHost> ClientSession<H> {
    /// Owns resources without starting workers, observing a signer or networking.
    pub fn new(host: H, maximum_in_flight: NonZeroU16) -> Self {
        let signer = Shared::new(Mutex::new(None));
        Self {
            session: Session::new(
                Resources {
                    host,
                    signer: signer.clone(),
                    host_quiesced: Mutex::new(false),
                },
                maximum_in_flight,
            ),
            signer,
        }
    }
    /// Reports resource lifecycle independently of business authority.
    pub fn state(&self) -> SessionState {
        self.session.state()
    }
    /// Starts the explicitly configured resource host on the caller's executor.
    pub async fn start(&self) -> crate::Result<()> {
        self.session.start().await
    }
    /// Retains the provider before awaiting its first actual observation.
    /// Failed or cancelled attachment remains owned and may be inspected/closed
    /// through `signer_session`; replacing a live scope is explicitly refused.
    pub async fn attach_signer(&self, handle: SignerHandle) -> crate::Result<SignerSession> {
        let _permit = self.session.retain_resource()?;
        let (scope, retired) = {
            let mut current = signer_lock(&self.signer);
            // Insertion and the close hook share this lock. Rechecking resource
            // admission here prevents an attachment being missed by quiesce.
            if matches!(self.state(), SessionState::Closing | SessionState::Closed) {
                return Err(crate::Error::signer_unavailable());
            }
            if current
                .as_ref()
                .is_some_and(|scope| scope.state() != SessionState::Closed)
            {
                return Err(crate::Error::signer_unavailable());
            }
            let scope = SignerSession::new(handle);
            let retired = current.replace(scope.clone());
            (scope, retired)
        };
        drop(retired);
        scope.observe().await.map_err(|error| {
            if error == ProviderError::IdentityChanged {
                crate::Error::identity_changed()
            } else {
                crate::Error::signer_unavailable()
            }
        })?;
        Ok(scope)
    }
    /// Returns the owned scope, including failed/cancelled first observations.
    /// It grants no workspace access and cannot reopen closed admission.
    pub fn signer_session(&self) -> Option<SignerSession> {
        signer_lock(&self.signer).clone()
    }
    /// Stops admission, quiesces callbacks, drains work, disconnects transports,
    /// disposes callbacks and closes storage last. Cancellation is resumable.
    pub async fn close(&self) -> crate::Result<()> {
        self.session.close().await
    }
}
