use radroots_identity::PublicKey;
use radroots_nostr::key::{public_key_from_npub, public_key_to_npub};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let public_key =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let npub = public_key_to_npub(public_key)?;

    assert_eq!(public_key_from_npub(&npub)?, public_key);
    println!("{npub}");
    Ok(())
}
