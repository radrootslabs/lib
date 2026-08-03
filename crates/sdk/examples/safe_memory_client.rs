//! Builds and explicitly closes the safe in-process client.

use radroots_sdk::{ClientBuilder, capability::CapabilityId};
use radroots_storage::event::SourceGeneration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let generation = SourceGeneration::new([1; 32])?;
    let client = ClientBuilder::memory(generation).build()?;

    assert!(
        client
            .capabilities()
            .get(CapabilityId::CANONICAL_STORAGE)
            .is_some()
    );

    client.close().await?;
    Ok(())
}
