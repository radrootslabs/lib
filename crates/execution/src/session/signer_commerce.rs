//! Portable crypto qualification through the existing observed signer scope.
use super::*;
use radroots_protocol::commerce::{CommerceSigningPlan, EncodedCommerceRumor};
use radroots_signing::{
    SignReceipt, SignRequest, Signer, SignerStatus, commerce::VerifiedCommerceSigningPlan,
    signer::BoxFuture,
};

// Each observation/decryption uses the scope's existing single-call admission.
// A second surrounding permit would consume that capacity before IO can start.
struct ScopedProvider<'a>(&'a SignerSession);
impl Signer for ScopedProvider<'_> {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, radroots_signing::Error>> {
        Box::pin(async { Ok(SignerStatus::unavailable()) })
    }
    fn sign(
        &self,
        _request: SignRequest,
    ) -> BoxFuture<'_, Result<SignReceipt, radroots_signing::Error>> {
        Box::pin(async {
            Err(radroots_signing::Error::new(
                radroots_signing::error::Kind::SignerUnavailable,
            ))
        })
    }
}
impl ParticipantProvider for ScopedProvider<'_> {
    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
        Box::pin(async move {
            if self.0.state() != SessionState::Running {
                return Err(ProviderError::CapabilityUnavailable);
            }
            self.0.observe().await
        })
    }
    fn encrypt_private(
        &self,
        call: ProviderCall,
        recipient: radroots_domain::PublicKey,
        plaintext: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
        Box::pin(self.0.encrypt_private(call, recipient, plaintext))
    }
    fn decrypt_private(
        &self,
        call: ProviderCall,
        sender: radroots_domain::PublicKey,
        ciphertext: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
        Box::pin(self.0.decrypt_private(call, sender, ciphertext))
    }
}
impl SignerSession {
    /// Checks the exact original peer/self ciphertexts and signed routes through
    /// this attached scope. It signs nothing and grants no T1, dispatch, stock
    /// or network authority. The composing executor owns the complete task.
    pub async fn verify_commerce_plan(
        &self,
        call: ProviderCall,
        rumor: &EncodedCommerceRumor,
        plan: CommerceSigningPlan,
    ) -> Result<VerifiedCommerceSigningPlan, ProviderError> {
        if self.state() != SessionState::Running {
            return Err(ProviderError::CapabilityUnavailable);
        }
        if self.identity_changed() {
            return Err(ProviderError::IdentityChanged);
        }
        let observed = self
            .observed()
            .ok_or(ProviderError::CapabilityUnavailable)?;
        if observed.identity() != rumor.author() || call.identity() != observed.identity() {
            return Err(ProviderError::IdentityChanged);
        }
        VerifiedCommerceSigningPlan::verify(&ScopedProvider(self), call, rumor, plan).await
    }
}
