#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::{
    envelope::kind::KIND_POST,
    post::{
        AuthoredAsk, AuthoredPhotoUpdate, AuthoredPostImage, AuthoredUpdate,
        RADROOTS_ASK_MARKER_TAG_KEY, RADROOTS_ASK_MARKER_TAG_VALUE,
    },
    wire::Nip01EventWireParts,
};

/// Builds deterministic unsigned kind-1 wire parts for a strict Update.
pub fn authored_update_to_wire_parts(update: &AuthoredUpdate) -> Nip01EventWireParts {
    Nip01EventWireParts {
        kind: KIND_POST,
        content: update.content().to_string(),
        tags: Vec::new(),
    }
}

/// Builds deterministic unsigned kind-1 wire parts for a strict PhotoUpdate.
///
/// The caller must separately establish successful BUD-02 upload completion
/// for every image before passing these parts to a signing boundary.
pub fn authored_photo_update_to_wire_parts(photo: &AuthoredPhotoUpdate) -> Nip01EventWireParts {
    Nip01EventWireParts {
        kind: KIND_POST,
        content: photo.content().to_string(),
        tags: image_tags(photo.images()),
    }
}

/// Builds deterministic unsigned kind-1 wire parts for a strict Ask.
///
/// The exact Ask marker is emitted first. Optional media uses the same strict
/// NIP-92 profile as PhotoUpdate. Upload completion remains a separate runtime
/// precondition before signing.
pub fn authored_ask_to_wire_parts(ask: &AuthoredAsk) -> Nip01EventWireParts {
    let mut tags = Vec::with_capacity(1 + ask.images().len());
    tags.push(vec![
        RADROOTS_ASK_MARKER_TAG_KEY.to_string(),
        RADROOTS_ASK_MARKER_TAG_VALUE.to_string(),
    ]);
    tags.extend(image_tags(ask.images()));
    Nip01EventWireParts {
        kind: KIND_POST,
        content: ask.content().to_string(),
        tags,
    }
}

fn image_tags(images: &[AuthoredPostImage]) -> Vec<Vec<String>> {
    images
        .iter()
        .map(|image| image.imeta_tag().to_vec())
        .collect()
}
