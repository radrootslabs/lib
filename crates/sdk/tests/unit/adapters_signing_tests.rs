use super::{event_builder_from_parts, sign_parts_with_identity};
use crate::identity::RadrootsIdentity;
use radroots_event::wire::RadrootsNip01EventWireParts;

#[test]
fn event_builder_from_parts_preserves_kind_and_content() {
    let builder = event_builder_from_parts(RadrootsNip01EventWireParts {
        kind: 30402,
        content: "hello".into(),
        tags: vec![vec!["x".into(), "y".into()]],
    })
    .expect("builder");
    let identity = RadrootsIdentity::generate();
    let event = builder.build(identity.keys().public_key());

    assert_eq!(u16::from(event.kind), 30402);
    assert_eq!(event.content, "hello");
}

#[test]
fn sign_parts_with_identity_signs_event() {
    let identity = RadrootsIdentity::generate();
    let event = sign_parts_with_identity(
        &identity,
        RadrootsNip01EventWireParts {
            kind: 30402,
            content: "hello".into(),
            tags: vec![],
        },
    )
    .expect("signed event");

    assert_eq!(event.pubkey.to_hex(), identity.public_key_hex());
}
