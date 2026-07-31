use radroots_nostr::draft_signing::radroots_nostr_sign_frozen_draft;
use radroots_nostr::types::RadrootsNostrKeys;
use radroots_signing::{
    Error, SignReceipt, SignRequest, Signer, SignerStatus, error::Kind, signer::BoxFuture,
};
use std::sync::LazyLock;

struct FixtureKeyMaterial {
    keys: RadrootsNostrKeys,
    pubkey: String,
}

impl FixtureKeyMaterial {
    fn generate() -> Self {
        let keys = RadrootsNostrKeys::generate();
        let pubkey = keys.public_key().to_hex();
        Self { keys, pubkey }
    }
}

static FIXTURE_ALICE: LazyLock<FixtureKeyMaterial> = LazyLock::new(FixtureKeyMaterial::generate);
static FIXTURE_BOB: LazyLock<FixtureKeyMaterial> = LazyLock::new(FixtureKeyMaterial::generate);

pub(crate) fn fixture_alice_pubkey() -> &'static str {
    FIXTURE_ALICE.pubkey.as_str()
}

pub(crate) fn fixture_bob_pubkey() -> &'static str {
    FIXTURE_BOB.pubkey.as_str()
}

pub struct FixtureSigner {
    keys: RadrootsNostrKeys,
}

impl FixtureSigner {
    pub fn new(pubkey: &str) -> Self {
        let material = match pubkey {
            pubkey if pubkey == fixture_alice_pubkey() => &*FIXTURE_ALICE,
            pubkey if pubkey == fixture_bob_pubkey() => &*FIXTURE_BOB,
            _ => panic!("unsupported fixture signer public key"),
        };
        Self {
            keys: material.keys.clone(),
        }
    }

    #[allow(dead_code)]
    pub fn sign_frozen_draft(
        &self,
        draft: &radroots_event::EventDraft,
    ) -> Result<radroots_event::SignedEvent, radroots_nostr::error::RadrootsNostrError> {
        radroots_nostr_sign_frozen_draft(&self.keys, draft)
    }
}

impl Signer for FixtureSigner {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
        Box::pin(async { Ok(SignerStatus::unavailable()) })
    }

    fn sign(&self, request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
        Box::pin(async move {
            let signed_event = radroots_nostr_sign_frozen_draft(&self.keys, request.draft())
                .map_err(|source| Error::with_source(Kind::AuthorizationDenied, source))?;
            SignReceipt::from_signed_event(&request, signed_event, 1_700_000_001)
        })
    }
}
