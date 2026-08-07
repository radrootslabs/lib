use radroots_transport::{EventSink, EventSource};
use radroots_transport_nostr::{Config, NostrTransport, RelayProfile};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_profile(RelayProfile::public(["wss://relay.example.com"])?)
        .with_timeouts(5_000, 20_000, 2_000)?;
    let transport = NostrTransport::new(config);

    let source: &dyn EventSource = &transport;
    let sink: &dyn EventSink = &transport;
    drop(source.status());
    drop(sink.status());

    println!("configured a Nostr source and sink without opening a relay connection");
    Ok(())
}
