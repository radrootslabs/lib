#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};
use radroots_event::{
    media::AuthoredImage,
    post::{
        AuthoredAsk, AuthoredPhotoUpdate, AuthoredPostImage, AuthoredUpdate, PostImageDimensions,
    },
};
use radroots_nostr::{
    event::Timestamp as RadrootsNostrTimestamp,
    events::post::{
        radroots_nostr_build_ask_event, radroots_nostr_build_photo_update_event,
        radroots_nostr_build_update_event,
    },
    types::{RadrootsNostrKeys, RadrootsNostrSecretKey},
};

#[test]
fn typed_post_builders_preserve_strict_wire_profiles() {
    let keys = RadrootsNostrKeys::new(
        RadrootsNostrSecretKey::from_hex(test_fixtures::FIXTURE_ALICE_SECRET_KEY_HEX).unwrap(),
    );
    let created_at = RadrootsNostrTimestamp::from_secs(1_784_347_200);
    let update = AuthoredUpdate::new("Farm update").unwrap();
    let event = radroots_nostr_build_update_event(&update)
        .unwrap()
        .custom_created_at(created_at)
        .sign_with_keys(&keys)
        .unwrap();
    assert_eq!(event.kind.as_u16(), 1);
    assert!(event.tags.is_empty());
    assert_eq!(event.created_at, created_at);
    assert!(event.verify().is_ok());

    let image = authored_image();
    let photo =
        AuthoredPhotoUpdate::new(format!("Harvest {}", image.url()), vec![image.clone()]).unwrap();
    let event = radroots_nostr_build_photo_update_event(&photo)
        .unwrap()
        .custom_created_at(created_at)
        .sign_with_keys(&keys)
        .unwrap();
    assert_eq!(event.tags.len(), 1);
    assert_eq!(event.tags.iter().next().unwrap().as_slice()[0], "imeta");
    assert!(event.verify().is_ok());

    let ask = AuthoredAsk::new(format!("Is this ready? {}", image.url()), vec![image]).unwrap();
    let event = radroots_nostr_build_ask_event(&ask)
        .unwrap()
        .custom_created_at(created_at)
        .sign_with_keys(&keys)
        .unwrap();
    assert_eq!(event.tags.len(), 2);
    let tags = event.tags.iter().collect::<Vec<_>>();
    assert_eq!(tags[0].as_slice(), ["t", "radroots-ask"]);
    assert_eq!(tags[1].as_slice()[0], "imeta");
    assert!(event.verify().is_ok());
}

fn authored_image() -> AuthoredPostImage {
    let bytes = b"strawberries";
    let hash = Sha256::digest(bytes);
    let media_type = MediaType::parse("image/webp").unwrap();
    let descriptor = BlobDescriptor::new(
        BlobUrl::parse(&format!("https://media.example/{hash}.webp")).unwrap(),
        hash,
        bytes.len() as u64,
        media_type.clone(),
        1_784_347_200,
    )
    .unwrap()
    .approve_reference()
    .unwrap()
    .verify_bytes(bytes, &media_type)
    .unwrap();
    AuthoredPostImage::new(
        AuthoredImage::try_from(descriptor).unwrap(),
        PostImageDimensions::new(1200, 900).unwrap(),
        "Harvest",
    )
    .unwrap()
}
