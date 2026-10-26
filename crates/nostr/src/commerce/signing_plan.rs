//! Crypto qualification for exact structured-commerce T1 data. No signing.
use alloc::string::String;
use core::fmt;
use radroots_protocol::{
    commerce::{CommerceSigningPlan, EncodedCommerceRumor},
    intent::SigningReplay,
};
use radroots_signing::{
    issuance::VerifiedDmRelayAdvertisement,
    provider::{
        ParticipantProvider, PrivateCiphertext, PrivateCryptoCapability, ProviderCall,
        ProviderError,
    },
    recovery::ReplayCapability,
};

/// Exact preimages whose encryption and original signed route advertisements
/// were checked through the selected participant provider. This proves no
/// current store consent, target network policy, dispatch or T1 commit.
pub struct VerifiedCommerceSigningPlan {
    plan: CommerceSigningPlan,
    deadline_ms: u64,
}
impl VerifiedCommerceSigningPlan {
    /// Verifies all peer/self ciphertexts against the unchanged original rumor
    /// before any business signing. Identity/capability changes fail closed.
    /// The caller owns any encryption prompting/attempt lifecycle separately.
    pub async fn verify(
        provider: &dyn ParticipantProvider,
        call: ProviderCall,
        rumor: &EncodedCommerceRumor,
        plan: CommerceSigningPlan,
    ) -> Result<Self, ProviderError> {
        let original = CommerceSigningPlan::parse(
            plan.operation_id(),
            plan.command_digest(),
            rumor,
            plan.protected_wire(),
        )
        .map_err(|_| ProviderError::InvalidInput)?;
        for (seal, route) in original.seals().iter().zip(original.routes()) {
            let verified =
                VerifiedDmRelayAdvertisement::verify(route.raw_json(), seal.recipient())?;
            let targets: alloc::vec::Vec<String> = route
                .selected_relays()
                .iter()
                .map(|value| String::from(value.as_ref()))
                .collect();
            let checked = verified.capture_route(&targets)?;
            if checked.source_id() != route.source_id() {
                return Err(ProviderError::AuthenticationFailed);
            }
        }
        for seal in original.seals() {
            state(provider, &call, rumor, original.replay()).await?;
            let ciphertext = PrivateCiphertext::parse(seal.fields().content)?;
            let clear = provider
                .decrypt_private(call.clone(), seal.recipient(), ciphertext)
                .await?;
            state(provider, &call, rumor, original.replay()).await?;
            if clear.expose() != rumor.protected_wire() {
                return Err(ProviderError::AuthenticationFailed);
            }
        }
        Ok(Self {
            plan: original,
            deadline_ms: call.deadline_unix_ms().get(),
        })
    }
    pub const fn plan(&self) -> &CommerceSigningPlan {
        &self.plan
    }
    pub const fn deadline_unix_ms(&self) -> u64 {
        self.deadline_ms
    }
    pub fn into_plan(self) -> CommerceSigningPlan {
        self.plan
    }
}
async fn state(
    provider: &dyn ParticipantProvider,
    call: &ProviderCall,
    rumor: &EncodedCommerceRumor,
    replay: SigningReplay,
) -> Result<(), ProviderError> {
    let state = provider.provider_state().await?;
    let supported = match replay {
        SigningReplay::NonReplayable => {
            state.exact_issuance() == Some(ReplayCapability::NonReplayable)
        }
        SigningReplay::LocalReplaySafe => {
            state.exact_issuance().is_some()
                && state.exact_recovery()
                    == radroots_signing::issuance::ExactRecoveryCapability::LocalExactReplay
        }
        SigningReplay::ExactReplayByRequestId => {
            state.exact_issuance() == Some(ReplayCapability::ExactReplayByRequestId)
        }
    };
    if call.identity() != rumor.author()
        || state.identity() != rumor.author()
        || state.encryption() != PrivateCryptoCapability::Nip44V2
        || state.decryption() != PrivateCryptoCapability::Nip44V2
        || !supported
    {
        return Err(ProviderError::CapabilityUnavailable);
    }
    Ok(())
}
impl fmt::Debug for VerifiedCommerceSigningPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VerifiedCommerceSigningPlan(<redacted>)")
    }
}
