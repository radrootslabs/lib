#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use radroots_blossom::{
    RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl, RadrootsBlossomMediaType,
    RadrootsBlossomSha256,
};
use radroots_event::{
    RadrootsAuthoredImage,
    post::{
        RadrootsAuthoredAsk, RadrootsAuthoredPhotoUpdate, RadrootsAuthoredPostImage,
        RadrootsAuthoredUpdate, RadrootsPostImageDimensions,
    },
};
use radroots_nostr::{
    events::post::{
        radroots_nostr_build_ask_event, radroots_nostr_build_photo_update_event,
        radroots_nostr_build_update_event,
    },
    types::RadrootsNostrPublicKey,
};

#[test]
fn typed_post_builders_preserve_strict_wire_profiles() {
    let author =
        RadrootsNostrPublicKey::from_hex(test_fixtures::FIXTURE_ALICE_PUBLIC_KEY_HEX).unwrap();
    let update = RadrootsAuthoredUpdate::new("Farm update").unwrap();
    let event = radroots_nostr_build_update_event(&update)
        .unwrap()
        .build(author);
    assert_eq!(event.kind.as_u16(), 1);
    assert!(event.tags.is_empty());

    let image = authored_image();
    let photo =
        RadrootsAuthoredPhotoUpdate::new(format!("Harvest {}", image.url()), vec![image.clone()])
            .unwrap();
    let event = radroots_nostr_build_photo_update_event(&photo)
        .unwrap()
        .build(author);
    assert_eq!(event.tags.len(), 1);
    assert_eq!(event.tags.iter().next().unwrap().as_slice()[0], "imeta");

    let ask =
        RadrootsAuthoredAsk::new(format!("Is this ready? {}", image.url()), vec![image]).unwrap();
    let event = radroots_nostr_build_ask_event(&ask).unwrap().build(author);
    assert_eq!(event.tags.len(), 2);
    let tags = event.tags.iter().collect::<Vec<_>>();
    assert_eq!(tags[0].as_slice(), ["t", "radroots-ask"]);
    assert_eq!(tags[1].as_slice()[0], "imeta");
}

fn authored_image() -> RadrootsAuthoredPostImage {
    let bytes = b"strawberries";
    let hash = RadrootsBlossomSha256::digest(bytes);
    let media_type = RadrootsBlossomMediaType::parse("image/webp").unwrap();
    let descriptor = RadrootsBlossomBlobDescriptor::new(
        RadrootsBlossomBlobUrl::parse(&format!("https://media.example/{hash}.webp")).unwrap(),
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
    RadrootsAuthoredPostImage::new(
        RadrootsAuthoredImage::try_from(descriptor).unwrap(),
        RadrootsPostImageDimensions::new(1200, 900).unwrap(),
        "Harvest",
    )
    .unwrap()
}
