use core::num::NonZeroU16;
use radroots_signing::provider::{
    ParticipantProvider, ProviderError, ProviderState, SignerCallbacks, SignerHandle,
};
use std::{
    boxed::Box,
    sync::{Mutex, MutexGuard},
};

use super::{HostFuture, Session, SessionHost, SessionState, host::Shared};

struct Binding {
    provider: Shared<dyn ParticipantProvider>,
    callbacks: Shared<dyn SignerCallbacks>,
}

impl Binding {
    fn shared_copy(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            callbacks: self.callbacks.clone(),
        }
    }
}

struct ResourceInner {
    binding: Mutex<Option<Binding>>,
    observed: Mutex<Observation>,
}

#[derive(Clone, Copy)]
enum Observation {
    None,
    Current(ProviderState),
    IdentityChanged(ProviderState),
}

impl ResourceInner {
    fn binding(&self) -> MutexGuard<'_, Option<Binding>> {
        self.binding
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn observed(&self) -> MutexGuard<'_, Observation> {
        self.observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn retain_binding(&self) -> Option<Binding> {
        self.binding().as_ref().map(Binding::shared_copy)
    }
}

struct Resources(Shared<ResourceInner>);

impl SessionHost for Resources {
    fn start(&self) -> HostFuture<'_, crate::Result<()>> {
        // This scope owns callbacks, not workers or a transport connection.
        Box::pin(async { Ok(()) })
    }

    fn quiesce(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            let binding = self.0.retain_binding();
            if let Some(binding) = binding {
                binding
                    .callbacks
                    .quiesce()
                    .await
                    .map_err(|_| crate::Error::signer_unavailable())?;
            }
            Ok(())
        })
    }

    fn disconnect(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn dispose_callbacks(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            let binding = self.0.retain_binding();
            if let Some(binding) = binding {
                binding
                    .callbacks
                    .close()
                    .await
                    .map_err(|_| crate::Error::signer_unavailable())?;
                let retired = { self.0.binding().take() };
                // Host-owned destructors and callback code never run under
                // our bookkeeping mutex, including after successful closure.
                drop(retired);
            }
            Ok(())
        })
    }

    fn close_storage(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

/// A revocable provider observation scope with one frozen participant identity.
///
/// It supplies no workspace grant, command context, stock authority or provider
/// qualification. The owning Client must retain this scope before awaiting its
/// first observation so cancellation cannot lose callback cleanup ownership.
pub struct SignerSession {
    resources: Shared<ResourceInner>,
    session: Session<Resources>,
}

impl Clone for SignerSession {
    fn clone(&self) -> Self {
        Self {
            resources: self.resources.clone(),
            session: self.session.clone(),
        }
    }
}

impl SignerSession {
    /// Owns the handle without invoking it. Observations serialize through a
    /// single admitted resource permit; excess concurrent observations refuse.
    pub fn new(handle: SignerHandle) -> Self {
        let (provider, callbacks) = handle.into_parts();
        let resources = Shared::new(ResourceInner {
            binding: Mutex::new(Some(Binding {
                provider,
                callbacks,
            })),
            observed: Mutex::new(Observation::None),
        });
        Self {
            session: Session::new(Resources(resources.clone()), NonZeroU16::MIN),
            resources,
        }
    }

    /// Returns resource state without invoking a provider or claiming authority.
    pub fn state(&self) -> SessionState {
        self.session.state()
    }

    /// Returns the last identity-bound observation. Cached capabilities may be
    /// stale and never authorize a later effect or override retained uncertainty.
    pub fn observed(&self) -> Option<ProviderState> {
        match *self.resources.observed() {
            Observation::None => None,
            Observation::Current(state) | Observation::IdentityChanged(state) => Some(state),
        }
    }

    /// Reports a latched identity change. Returning to the original key does
    /// not reauthorize this scope; explicit replacement is required.
    pub fn identity_changed(&self) -> bool {
        matches!(*self.resources.observed(), Observation::IdentityChanged(_))
    }

    /// Invokes the actual provider and freezes its first successfully observed
    /// key. Later capability changes may be observed; another key is refused.
    pub async fn observe(&self) -> Result<ProviderState, ProviderError> {
        self.session
            .start()
            .await
            .map_err(|_| ProviderError::CapabilityUnavailable)?;
        let _permit = self
            .session
            .admit()
            .map_err(|_| ProviderError::CapabilityUnavailable)?;
        if self.identity_changed() {
            return Err(ProviderError::IdentityChanged);
        }
        let binding = self
            .resources
            .retain_binding()
            .ok_or(ProviderError::CapabilityUnavailable)?;
        let current = binding.provider.provider_state().await?;
        let mut observed = self.resources.observed();
        if let Observation::Current(old) = *observed
            && old.identity() != current.identity()
        {
            *observed = Observation::IdentityChanged(old);
            return Err(ProviderError::IdentityChanged);
        }
        *observed = Observation::Current(current);
        Ok(current)
    }

    /// Permanently stops observation admission, drains admitted observations,
    /// revokes/drains host callbacks and releases provider roots. Failed or
    /// cancelled close retains its unfinished stage and must be resumed.
    pub async fn close(&self) -> crate::Result<()> {
        self.session.close().await
    }
}
