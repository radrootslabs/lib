//! Native, nonpublishing provider smoke using published synthetic vector keys.
//! No persistent store, relay or real participant is used.

use core::num::NonZeroU64;
use radroots_nostr::{
    key::parse_secret_key,
    signing::{LocalSigner, OsNonceSource},
};
use radroots_signing::{
    host::Shared,
    provider::{ParticipantProvider, PrivateText, ProviderCall},
    request::CancellationSignal,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Public NIP-44 interoperability identities; never use these for custody.
    let alice = LocalSigner::new(parse_secret_key(
        "0000000000000000000000000000000000000000000000000000000000000001",
    )?)?
    .with_nonce_source(Shared::new(OsNonceSource));
    let bob = LocalSigner::new(parse_secret_key(
        "0000000000000000000000000000000000000000000000000000000000000002",
    )?)?;
    let now = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let deadline = NonZeroU64::new(now.checked_add(10_000).ok_or("deadline overflow")?)
        .ok_or("deadline unavailable")?;
    let call = |signer: &LocalSigner| {
        ProviderCall::new(signer.public_key(), deadline, CancellationSignal::new())
    };
    let text = "synthetic local food inquiry";
    let first = alice
        .encrypt_private(call(&alice), bob.public_key(), PrivateText::new(text)?)
        .await?;
    let second = alice
        .encrypt_private(call(&alice), bob.public_key(), PrivateText::new(text)?)
        .await?;
    assert_ne!(first.as_str(), second.as_str());
    for ciphertext in [first, second] {
        let opened = bob
            .decrypt_private(call(&bob), alice.public_key(), ciphertext)
            .await?;
        assert_eq!(opened.expose(), text);
    }
    println!("Native participant provider: OS entropy and two decryptions passed");
    Ok(())
}
