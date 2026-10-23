use radroots_identity::PublicKey;
use radroots_nostr::{
    commerce::{self, RecordOpening, RejectedReason, UnsupportedReason},
    key::SecretKey,
    private_ingress::OpeningError,
    signing::LocalSigner,
};
use radroots_protocol::{commerce::EncodedCommerceRumor, private_rumor::DecodedRumor};
use radroots_signing::{
    SignReceipt, SignRequest, Signer, SignerStatus,
    provider::{
        ParticipantProvider, PrivateCiphertext, PrivateCryptoCapability, PrivateText, ProviderCall,
        ProviderError, ProviderState,
    },
    request::CancellationSignal,
    signer::BoxFuture,
};
use serde::Deserialize;
use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Deserialize)]
struct Corpus {
    positives: Vec<Vector>,
    negatives: Vec<Vector>,
}
#[derive(Deserialize)]
struct Vector {
    name: String,
    recipient_scalar: u64,
    recipient: String,
    rumor_id: String,
    raw_rumor: String,
    raw_seal: String,
    raw_wrap: String,
    record_id: Option<String>,
    expected: Expected,
}
#[derive(Deserialize)]
struct Expected {
    outcome: String,
    reason: Option<String>,
}
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("fixtures/commerce_envelopes.v1.json")).unwrap()
}
fn signer(scalar: u64) -> LocalSigner {
    LocalSigner::with_clock(SecretKey::parse(&format!("{scalar:064x}")).unwrap(), || {
        Ok(1000)
    })
    .unwrap()
}
fn call(key: PublicKey) -> ProviderCall {
    ProviderCall::new(
        key,
        NonZeroU64::new(2000).unwrap(),
        CancellationSignal::new(),
    )
}

fn reason(value: RejectedReason) -> &'static str {
    match value {
        RejectedReason::InvalidProfile => "invalid_profile",
        RejectedReason::LimitExceeded => "limit_exceeded",
        RejectedReason::RecordIdentityMismatch => "record_identity_mismatch",
        RejectedReason::CarrierMismatch => "carrier_mismatch",
        RejectedReason::RecipientMismatch => "recipient_mismatch",
    }
}

#[tokio::test]
async fn independent_nested_vectors_open_all_families_peer_self_and_exact_budgets() {
    let rows = corpus().positives;
    assert_eq!(rows.len(), 80);
    for row in &rows {
        let provider = signer(row.recipient_scalar);
        assert_eq!(provider.public_key().to_hex(), row.recipient);
        let result = commerce::open(&provider, call(provider.public_key()), &row.raw_wrap)
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", row.name));
        let RecordOpening::Known(value) = result else {
            panic!("{}: {result:?}", row.name)
        };
        assert_eq!(
            value.record().record_id().to_string(),
            *row.record_id.as_ref().unwrap(),
            "{}",
            row.name
        );
        let carrier = value.carrier();
        let outer: serde_json::Value = serde_json::from_str(&row.raw_wrap).unwrap();
        let seal: serde_json::Value = serde_json::from_str(&row.raw_seal).unwrap();
        assert_eq!(
            carrier.envelope_id().to_string(),
            outer["id"].as_str().unwrap()
        );
        assert_eq!(carrier.seal_id().to_string(), seal["id"].as_str().unwrap());
        assert_eq!(carrier.rumor().event_id().to_string(), row.rumor_id);
        assert_eq!(carrier.protected_outer(), row.raw_wrap);
        assert_eq!(carrier.protected_seal(), row.raw_seal);
        assert_eq!(carrier.protected_rumor(), row.raw_rumor);
        assert_eq!(carrier.recipient(), provider.public_key());
        assert!(!carrier.rumor().has_tags());
        let encoded = EncodedCommerceRumor::capture(value.record().content()).unwrap();
        assert_eq!(encoded.record_id(), value.record().record_id());
        assert_eq!(encoded.event_id(), carrier.rumor().event_id());
        let decoded = DecodedRumor::decode(encoded.protected_wire().as_bytes()).unwrap();
        assert_eq!(
            decoded.protected_content(),
            carrier.rumor().protected_content()
        );
        assert_eq!(decoded.author(), carrier.rumor().author());
        assert_eq!(decoded.created_at(), carrier.rumor().created_at());
        assert!(!format!("{value:?}").contains("private"));
    }
    for pair in rows[..78].as_chunks::<2>().0 {
        assert_eq!(pair[0].rumor_id, pair[1].rumor_id);
        assert_eq!(pair[0].record_id, pair[1].record_id);
        assert_ne!(pair[0].raw_seal, pair[1].raw_seal);
        assert_ne!(pair[0].raw_wrap, pair[1].raw_wrap);
        assert_ne!(pair[0].recipient, pair[1].recipient);
    }
    assert_eq!(rows[78].raw_rumor.len(), 24_576);
    assert_eq!(rows[79].raw_wrap.len(), 65_536);
}

#[tokio::test]
async fn independent_nested_failures_and_unsupported_evidence_have_exact_dispositions() {
    let rows = corpus().negatives;
    assert_eq!(rows.len(), 36);
    for row in rows {
        let provider = signer(row.recipient_scalar);
        let result = commerce::open(&provider, call(provider.public_key()), &row.raw_wrap).await;
        let (outcome, actual_reason) = match result {
            Err(error) => ("error", error.to_string()),
            Ok(RecordOpening::Unsupported(value)) => {
                assert_eq!(value.carrier().protected_rumor(), row.raw_rumor);
                assert_eq!(value.carrier().protected_seal(), row.raw_seal);
                assert_eq!(value.carrier().protected_outer(), row.raw_wrap);
                (
                    "unsupported",
                    match value.reason() {
                        UnsupportedReason::CarrierKind => "carrier_kind",
                        UnsupportedReason::Profile => "profile",
                    }
                    .into(),
                )
            }
            Ok(RecordOpening::Rejected(value)) => {
                assert_eq!(value.carrier().protected_rumor(), row.raw_rumor);
                ("rejected", reason(value.reason()).into())
            }
            Ok(value) => panic!("{} unexpectedly admitted: {value:?}", row.name),
        };
        assert_eq!(
            (outcome, actual_reason.as_str()),
            (
                row.expected.outcome.as_str(),
                row.expected.reason.as_deref().unwrap()
            ),
            "{}",
            row.name
        );
    }
}

struct ObservedProvider {
    inner: LocalSigner,
    observations: AtomicUsize,
    decryptions: AtomicUsize,
    change_at: usize,
    available: bool,
}
impl ObservedProvider {
    fn new() -> Self {
        Self {
            inner: signer(2),
            observations: AtomicUsize::new(0),
            decryptions: AtomicUsize::new(0),
            change_at: usize::MAX,
            available: true,
        }
    }
}
impl Signer for ObservedProvider {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, radroots_signing::Error>> {
        self.inner.status()
    }
    fn sign(&self, _: SignRequest) -> BoxFuture<'_, Result<SignReceipt, radroots_signing::Error>> {
        panic!("opening must never sign")
    }
}
impl ParticipantProvider for ObservedProvider {
    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
        Box::pin(async {
            let observed = self.observations.fetch_add(1, Ordering::SeqCst);
            let identity = if observed >= self.change_at {
                signer(3).public_key()
            } else {
                self.inner.public_key()
            };
            let support = if self.available {
                PrivateCryptoCapability::Nip44V2
            } else {
                PrivateCryptoCapability::Unavailable
            };
            Ok(ProviderState::new(
                identity,
                PrivateCryptoCapability::Unavailable,
                support,
                radroots_signing::recovery::ReplayCapability::NonReplayable,
            ))
        })
    }
    fn encrypt_private(
        &self,
        _: ProviderCall,
        _: PublicKey,
        _: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
        panic!("opening must never encrypt")
    }
    fn decrypt_private(
        &self,
        call: ProviderCall,
        sender: PublicKey,
        value: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
        self.decryptions.fetch_add(1, Ordering::SeqCst);
        self.inner.decrypt_private(call, sender, value)
    }
}

#[tokio::test]
async fn original_outer_and_recipient_are_admitted_before_any_provider_callback() {
    let rows = corpus().negatives;
    for name in [
        "outer_id",
        "outer_signature",
        "outer_recipient",
        "outer_duplicate_recipient",
        "outer_kind",
        "outer_unknown_field",
        "outer_encryption_version",
        "outer_limit_plus_one",
    ] {
        let row = rows.iter().find(|r| r.name == name).unwrap();
        let provider = ObservedProvider::new();
        assert!(
            commerce::open(&provider, call(provider.inner.public_key()), &row.raw_wrap)
                .await
                .is_err(),
            "{name}"
        );
        assert_eq!(provider.observations.load(Ordering::SeqCst), 0, "{name}");
        assert_eq!(provider.decryptions.load(Ordering::SeqCst), 0, "{name}");
    }
    for name in ["seal_id", "seal_signature", "seal_kind", "seal_tags"] {
        let row = rows.iter().find(|r| r.name == name).unwrap();
        let provider = ObservedProvider::new();
        assert!(
            commerce::open(&provider, call(provider.inner.public_key()), &row.raw_wrap)
                .await
                .is_err()
        );
        assert_eq!(provider.decryptions.load(Ordering::SeqCst), 1, "{name}");
    }
}

#[tokio::test]
async fn identity_support_deadline_and_cancellation_remain_provider_bound() {
    let row = corpus().positives.remove(0);
    for (change_at, expected_decryptions) in [(0, 0), (1, 1), (2, 2)] {
        let mut provider = ObservedProvider::new();
        provider.change_at = change_at;
        assert_eq!(
            commerce::open(&provider, call(provider.inner.public_key()), &row.raw_wrap)
                .await
                .unwrap_err(),
            OpeningError::Provider(ProviderError::IdentityChanged)
        );
        assert_eq!(
            provider.decryptions.load(Ordering::SeqCst),
            expected_decryptions
        );
    }
    let mut provider = ObservedProvider::new();
    provider.available = false;
    assert_eq!(
        commerce::open(&provider, call(provider.inner.public_key()), &row.raw_wrap)
            .await
            .unwrap_err(),
        OpeningError::Provider(ProviderError::CapabilityUnavailable)
    );
    assert_eq!(provider.decryptions.load(Ordering::SeqCst), 0);
    let provider = signer(2);
    let cancelled = CancellationSignal::new();
    cancelled.cancel();
    for (deadline, signal, expected) in [
        (2000, cancelled, ProviderError::Cancelled),
        (
            1000,
            CancellationSignal::new(),
            ProviderError::DeadlineExceeded,
        ),
    ] {
        let call = ProviderCall::new(
            provider.public_key(),
            NonZeroU64::new(deadline).unwrap(),
            signal,
        );
        assert_eq!(
            commerce::open(&provider, call, &row.raw_wrap)
                .await
                .unwrap_err(),
            OpeningError::Provider(expected)
        );
    }
}
