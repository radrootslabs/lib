use core::num::NonZeroU64;
use nostr::JsonUtil;
use radroots_signing::{
    issuance::{SigningIntent, SigningOutcome},
    provider::ProviderError,
    signer::BoxFuture,
};

use super::LocalSigner;

impl LocalSigner {
    pub(super) fn issue_exact(&self, intent: SigningIntent) -> BoxFuture<'_, SigningOutcome> {
        Box::pin(async move {
            let binding = intent.binding();
            if intent.author() != self.public_key {
                return SigningOutcome::not_dispatched(binding, ProviderError::IdentityChanged);
            }
            // Conversion/representation errors occur before cryptography.
            let unsigned = match intent.with_plan(crate::plan_signing::unsigned_event_from_plan) {
                Ok(unsigned) => unsigned,
                Err(_) => {
                    return SigningOutcome::not_dispatched(binding, ProviderError::InvalidInput);
                }
            };
            let now = match (self.clock)() {
                Ok(now) => now,
                Err(_) => {
                    return SigningOutcome::not_dispatched(binding, ProviderError::ProviderFailure);
                }
            };
            if let Err(reason) = intent.check_before_effect(self.public_key, now) {
                return SigningOutcome::not_dispatched(binding, reason);
            }
            // After invoking cryptography, an error is uncertain. Never claim
            // proven non-issuance or fall through to a new captured preimage.
            let event = match unsigned.sign_with_keys(&self.keys) {
                Ok(event) => event,
                Err(_) => return SigningOutcome::unknown(binding, ProviderError::ProviderFailure),
            };
            let observed = (self.clock)().ok().and_then(NonZeroU64::new);
            // No deadline/cancellation check or fallible clock can discard the
            // obtained event. Capture verifies its exact fields and signature.
            SigningOutcome::capture(&intent, event.as_json(), observed)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::parse_secret_key;
    use radroots_event::GenericEventDraft;
    use radroots_event_codec::authoring::AuthoredEventPlan;
    use radroots_signing::{
        Error as SigningError,
        error::Kind,
        issuance::{ArtifactRole, OutcomeKind},
        provider::{ParticipantProvider, ProviderCall},
        recovery::ReplayCapability,
        request::CancellationSignal,
    };
    use std::cell::RefCell;
    const AUTHOR: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const CREATED: u64 = 1_700_000_000;
    const DEADLINE: u64 = CREATED * 1000 + 10_000;
    #[derive(Default)]
    struct ClockState {
        calls: usize,
        mode: u8,
        cancellation: Option<CancellationSignal>,
    }
    thread_local! { static CLOCK: RefCell<ClockState> = RefCell::default(); }
    fn clock() -> Result<u64, SigningError> {
        CLOCK.with(|state| {
            let mut state = state.borrow_mut();
            state.calls += 1;
            match (state.mode, state.calls) {
                (1, 2) | (3, 1) => Err(SigningError::new(Kind::InternalError)),
                (2, 2) => Ok(DEADLINE + 1),
                (4, 1) | (7, 2) => Ok(0),
                (5, 1) => Ok(DEADLINE),
                (6, 2) => {
                    state.cancellation.as_ref().unwrap().cancel();
                    Ok(CREATED * 1000 + 2)
                }
                _ => Ok(CREATED * 1000 + 1),
            }
        })
    }
    fn setup(mode: u8) -> (LocalSigner, SigningIntent) {
        // Published synthetic interoperability identity, never custody material.
        let signer = LocalSigner::with_clock(
            parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
            clock,
        )
        .unwrap();
        let cancellation = CancellationSignal::new();
        CLOCK.with(|state| {
            *state.borrow_mut() = ClockState {
                calls: 0,
                mode,
                cancellation: Some(cancellation.clone()),
            }
        });
        let plan = AuthoredEventPlan::from_generic(
            GenericEventDraft::new(
                "radroots.social.geochat.v1",
                20_000,
                CREATED,
                vec![],
                "synthetic unchanged exact preimage",
                AUTHOR,
            )
            .unwrap(),
        )
        .unwrap();
        let intent = SigningIntent::new(
            "ab".repeat(32).parse().unwrap(),
            [9; 32],
            plan,
            ArtifactRole::PublicEvent,
            None,
            ProviderCall::new(
                signer.public_key(),
                NonZeroU64::new(DEADLINE).unwrap(),
                cancellation,
            ),
        )
        .unwrap();
        (signer, intent)
    }
    #[tokio::test]
    async fn exact_native_signatures_survive_missing_late_and_cancelled_completion_observation() {
        for mode in [0, 1, 2, 6, 7] {
            let (signer, intent) = setup(mode);
            let expected = intent.binding();
            let length = intent.signed_json_bytes();
            let state = signer.provider_state().await.unwrap();
            assert_eq!(
                state.exact_issuance(),
                Some(ReplayCapability::NonReplayable)
            );
            let outcome = signer.sign_exact(intent).await;
            assert_eq!(outcome.binding(), expected);
            let OutcomeKind::Obtained(artifact) = outcome.kind() else {
                panic!("obtained native signature lost: {outcome:?}");
            };
            artifact.with_json(|raw| {
                assert_eq!(raw.len(), length);
                let event = nostr::Event::from_json(raw).unwrap();
                event.verify().unwrap();
                assert_eq!(event.pubkey.to_hex(), AUTHOR);
                assert_eq!(event.content, "synthetic unchanged exact preimage");
            });
            assert_eq!(
                artifact.observed_at_unix_ms().is_none(),
                matches!(mode, 1 | 7)
            );
            if mode == 2 {
                assert!(artifact.observed_at_unix_ms().unwrap().get() > DEADLINE);
            }
            CLOCK.with(|state| assert_eq!(state.borrow().calls, 2));
        }
    }
    #[tokio::test]
    async fn failed_zero_and_expired_admission_clocks_do_not_dispatch_a_signature() {
        for mode in [3, 4, 5] {
            let (signer, intent) = setup(mode);
            let outcome = signer.sign_exact(intent).await;
            assert!(matches!(
                outcome.kind(),
                OutcomeKind::NotDispatched(
                    ProviderError::ProviderFailure | ProviderError::DeadlineExceeded
                )
            ));
            CLOCK.with(|state| assert_eq!(state.borrow().calls, 1));
        }
    }
    #[tokio::test]
    async fn wrong_actual_key_and_preexisting_cancellation_refuse_before_crypto() {
        let (_, intent) = setup(3);
        let other = LocalSigner::with_clock(
            parse_secret_key("0000000000000000000000000000000000000000000000000000000000000002")
                .unwrap(),
            clock,
        )
        .unwrap();
        // An untrusted caller cannot even construct a different captured key.
        assert!(matches!(
            SigningIntent::new(
                intent.binding().operation_id(),
                [9; 32],
                intent.with_plan(Clone::clone),
                ArtifactRole::PublicEvent,
                None,
                ProviderCall::new(
                    other.public_key(),
                    NonZeroU64::new(DEADLINE).unwrap(),
                    CancellationSignal::new()
                ),
            ),
            Err(ProviderError::InvalidInput)
        ));
        let outcome = other.sign_exact(intent).await;
        assert!(matches!(
            outcome.kind(),
            OutcomeKind::NotDispatched(ProviderError::IdentityChanged)
        ));
        CLOCK.with(|state| assert_eq!(state.borrow().calls, 0));
        let (signer, intent) = setup(0);
        CLOCK.with(|state| state.borrow().cancellation.as_ref().unwrap().cancel());
        let outcome = signer.sign_exact(intent).await;
        assert!(matches!(
            outcome.kind(),
            OutcomeKind::NotDispatched(ProviderError::Cancelled)
        ));
        CLOCK.with(|state| assert_eq!(state.borrow().calls, 1));
    }
}
