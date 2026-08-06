//! Selects local-only behavior without constructing or probing a transport.

use radroots_sdk::transport::Profile;

fn main() {
    let profile = Profile::local_only();

    assert!(profile.is_local_only());
    assert!(profile.targets().is_none());
    assert!(profile.satisfaction().is_none());
}
