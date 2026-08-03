//! Constructs the ordinary, safe, local-only Radroots front door.

fn main() -> radroots::Result<()> {
    let client = radroots::client::memory().build()?;
    let profile = radroots::client::local_only();

    assert!(!client.is_closed());
    assert!(profile.is_local_only());
    Ok(())
}
