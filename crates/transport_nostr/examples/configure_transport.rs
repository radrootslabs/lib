use radroots_transport::{EventSink, EventSource};
use radroots_transport_nostr::{
    Config, NostrTransport, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind,
    RelayUrlPolicy,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = RelayEndpoint::new(
        "wss://relay.example.com",
        RelayUrlPolicy::Public,
        RelayAccess::ReadWrite,
    )?;
    let profile = RelayProfile::explicit(RelayProfileKind::Public, [endpoint])?;
    let config = Config::from_profile(profile).with_timeouts(5_000, 20_000, 2_000)?;
    let transport = NostrTransport::new(config);

    let source: &dyn EventSource = &transport;
    let sink: &dyn EventSink = &transport;
    drop(source.status());
    drop(sink.status());

    println!("configured a Nostr source and sink without opening a relay connection");
    Ok(())
}
