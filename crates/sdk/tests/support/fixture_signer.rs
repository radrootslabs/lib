use radroots_authority::{RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity};
use radroots_event::draft::{RadrootsEventDraft, RadrootsSignedEvent};
use radroots_nostr::prelude::{RadrootsNostrKeys, radroots_nostr_sign_frozen_draft};
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

#[derive(Clone)]
pub struct FixtureSigner {
    identity: RadrootsSignerIdentity,
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
            identity: RadrootsSignerIdentity::new(pubkey).expect("identity"),
            keys: material.keys.clone(),
        }
    }
}

impl RadrootsEventSigner for FixtureSigner {
    fn pubkey(&self) -> &radroots_event::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsEventDraft,
    ) -> Result<RadrootsSignedEvent, RadrootsSignerError> {
        if self.pubkey().as_str() != draft.expected_pubkey_str() {
            return Err(RadrootsSignerError::SigningFailed {
                message: "wrong fixture signer".to_owned(),
            });
        }
        radroots_nostr_sign_frozen_draft(&self.keys, draft).map_err(|error| {
            RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            }
        })
    }
}
