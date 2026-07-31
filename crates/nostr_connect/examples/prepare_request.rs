use radroots_identity::PublicKey;
use radroots_nostr_connect::{Client, Request, client::Target, message::RequestId, uri::RelayUrl};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signer =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let relay = RelayUrl::parse("wss://relay.example.com")?;
    let client = Client::generate(Target::try_new(signer, vec![relay])?)?;
    let operation = client.prepare(RequestId::parse("example-ping")?, Request::Ping)?;

    assert!(operation.publication().is_ok());
    println!("prepared one encrypted NIP-46 request without publishing it");
    Ok(())
}
