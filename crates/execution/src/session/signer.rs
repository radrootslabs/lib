use core::num::NonZeroU16;
use radroots_signing::provider::{
    ParticipantProvider, PrivateCiphertext, PrivateCryptoCapability, PrivateText, ProviderCall,
    ProviderError, ProviderState, SignerCallbacks, SignerHandle,
};
use std::{
    boxed::Box,
    sync::{Mutex, MutexGuard},
};

use super::{HostFuture, Session, SessionHost, SessionState, host::Shared};
#[cfg(feature = "commerce")]
#[path = "signer_commerce.rs"]
mod commerce;

struct Binding {
    provider: Shared<dyn ParticipantProvider>,
    callbacks: Shared<dyn SignerCallbacks>,
}

impl radroots_signing::issuance::ExactIssuer for SignerSession {
    fn issue_exact(
        &self,
        intent: radroots_signing::issuance::SigningIntent,
    ) -> radroots_signing::signer::BoxFuture<'_, radroots_signing::issuance::SigningOutcome> {
        Box::pin(self.sign_exact(intent))
    }
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
    /// Recovers an original intent only through the current qualified method.
    /// This scope supplies no store authority or captured replay policy. Its
    /// caller must own the complete task and durable capture after observer loss.
    /// Refusal leaves original issuance indeterminate and renews no deadline.
    pub async fn recover_exact(
        &self,
        request: radroots_signing::issuance::ExactRecoveryRequest,
    ) -> radroots_signing::issuance::ExactRecoveryOutcome {
        use radroots_signing::issuance::{ExactRecoveryCapability, ExactRecoveryOutcome};
        let binding = request.binding();
        let refuse = |reason| ExactRecoveryOutcome::indeterminate(binding, reason);
        if self.state() != SessionState::Running {
            return refuse(ProviderError::CapabilityUnavailable);
        }
        if self.identity_changed() {
            return refuse(ProviderError::IdentityChanged);
        }
        let Some(observed) = self.observed() else {
            return refuse(ProviderError::CapabilityUnavailable);
        };
        if observed.identity() != request.original().author() {
            return refuse(ProviderError::IdentityChanged);
        }
        if observed.exact_recovery() != ExactRecoveryCapability::LocalExactReplay {
            return refuse(ProviderError::CapabilityUnavailable);
        }
        let observed = match self.observe().await {
            Ok(observed) => observed,
            Err(error) => return refuse(error),
        };
        if observed.identity() != request.original().author() {
            return refuse(ProviderError::IdentityChanged);
        }
        if observed.exact_recovery() != ExactRecoveryCapability::LocalExactReplay {
            return refuse(ProviderError::CapabilityUnavailable);
        }
        let Ok(_permit) = self.session.admit() else {
            return refuse(ProviderError::CapabilityUnavailable);
        };
        let Some(provider) = self.resources.retain_binding() else {
            return refuse(ProviderError::CapabilityUnavailable);
        };
        provider
            .provider
            .recover_exact(request)
            .await
            .enforce_binding(binding)
    }

    async fn private_provider(
        &self,
        call: &ProviderCall,
        encrypt: bool,
    ) -> Result<Binding, ProviderError> {
        if self.state() != SessionState::Running || self.observed().is_none() {
            return Err(ProviderError::CapabilityUnavailable);
        }
        let observed = self.observe().await?;
        if observed.identity() != call.identity() {
            return Err(ProviderError::IdentityChanged);
        }
        let capability = if encrypt {
            observed.encryption()
        } else {
            observed.decryption()
        };
        if capability != PrivateCryptoCapability::Nip44V2 {
            return Err(ProviderError::CapabilityUnavailable);
        }
        self.resources
            .retain_binding()
            .ok_or(ProviderError::CapabilityUnavailable)
    }

    /// Encrypts only with the already attached participant's current NIP-44 v2
    /// provider. It reobserves identity/capability before dispatch and retains
    /// the callback scope through the call. No signing or persistence occurs.
    pub async fn encrypt_private(
        &self,
        call: ProviderCall,
        recipient: radroots_domain::PublicKey,
        plaintext: PrivateText,
    ) -> Result<PrivateCiphertext, ProviderError> {
        let provider = self.private_provider(&call, true).await?;
        let _permit = self
            .session
            .admit()
            .map_err(|_| ProviderError::CapabilityUnavailable)?;
        provider
            .provider
            .encrypt_private(call, recipient, plaintext)
            .await
    }

    /// Decrypts through the current attached NIP-44 v2 provider. The composing
    /// executor must first authenticate the carrier and bind its actual author.
    /// Plaintext remains zeroizing and grants no history/command authority.
    pub async fn decrypt_private(
        &self,
        call: ProviderCall,
        sender: radroots_domain::PublicKey,
        ciphertext: PrivateCiphertext,
    ) -> Result<PrivateText, ProviderError> {
        let provider = self.private_provider(&call, false).await?;
        let _permit = self
            .session
            .admit()
            .map_err(|_| ProviderError::CapabilityUnavailable)?;
        provider
            .provider
            .decrypt_private(call, sender, ciphertext)
            .await
    }
    /// Issues only through an already observed, live exact-intent provider.
    /// The executor must retain its own work permit through durable capture;
    /// this scoped future owns resources but supplies no admission receipt.
    pub async fn sign_exact(
        &self,
        intent: radroots_signing::issuance::SigningIntent,
    ) -> radroots_signing::issuance::SigningOutcome {
        use radroots_signing::issuance::SigningOutcome;
        let binding = intent.binding();
        if self.state() != SessionState::Running {
            return SigningOutcome::not_dispatched(binding, ProviderError::CapabilityUnavailable);
        }
        if self.identity_changed() {
            return SigningOutcome::not_dispatched(binding, ProviderError::IdentityChanged);
        }
        let Some(observed) = self.observed() else {
            return SigningOutcome::not_dispatched(binding, ProviderError::CapabilityUnavailable);
        };
        if observed.identity() != intent.author() {
            return SigningOutcome::not_dispatched(binding, ProviderError::IdentityChanged);
        }
        if observed.exact_issuance().is_none() {
            return SigningOutcome::not_dispatched(binding, ProviderError::CapabilityUnavailable);
        }
        let observed = match self.observe().await {
            Ok(observed) => observed,
            Err(error) => return SigningOutcome::not_dispatched(binding, error),
        };
        if observed.identity() != intent.author() || observed.exact_issuance().is_none() {
            return SigningOutcome::not_dispatched(binding, ProviderError::CapabilityUnavailable);
        }
        let Ok(_permit) = self.session.admit() else {
            return SigningOutcome::not_dispatched(binding, ProviderError::CapabilityUnavailable);
        };
        let Some(provider) = self.resources.retain_binding() else {
            return SigningOutcome::not_dispatched(binding, ProviderError::CapabilityUnavailable);
        };
        provider
            .provider
            .sign_exact(intent)
            .await
            .enforce_binding(binding)
    }

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

    /// Stops observation admission and host dispatch without releasing roots.
    /// The composing Client drains its work and transports before awaiting
    /// close, which disposes these callbacks. Cancellation is resumable.
    pub async fn quiesce(&self) -> crate::Result<()> {
        self.session.quiesce().await
    }
}
