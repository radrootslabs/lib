#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::{
    kinds::KIND_POST,
    post::{
        RADROOTS_ASK_MARKER_TAG_KEY, RADROOTS_ASK_MARKER_TAG_VALUE, RadrootsAuthoredAsk,
        RadrootsAuthoredPhotoUpdate, RadrootsAuthoredPostImage, RadrootsAuthoredUpdate,
    },
    wire::RadrootsNip01EventWireParts,
};

/// Builds deterministic unsigned kind-1 wire parts for a strict Update.
pub fn authored_update_to_wire_parts(
    update: &RadrootsAuthoredUpdate,
) -> RadrootsNip01EventWireParts {
    RadrootsNip01EventWireParts {
        kind: KIND_POST,
        content: update.content().to_string(),
        tags: Vec::new(),
    }
}

/// Builds deterministic unsigned kind-1 wire parts for a strict PhotoUpdate.
///
/// The caller must separately establish successful BUD-02 upload completion
/// for every image before passing these parts to a signing boundary.
pub fn authored_photo_update_to_wire_parts(
    photo: &RadrootsAuthoredPhotoUpdate,
) -> RadrootsNip01EventWireParts {
    RadrootsNip01EventWireParts {
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
pub fn authored_ask_to_wire_parts(ask: &RadrootsAuthoredAsk) -> RadrootsNip01EventWireParts {
    let mut tags = Vec::with_capacity(1 + ask.images().len());
    tags.push(vec![
        RADROOTS_ASK_MARKER_TAG_KEY.to_string(),
        RADROOTS_ASK_MARKER_TAG_VALUE.to_string(),
    ]);
    tags.extend(image_tags(ask.images()));
    RadrootsNip01EventWireParts {
        kind: KIND_POST,
        content: ask.content().to_string(),
        tags,
    }
}

fn image_tags(images: &[RadrootsAuthoredPostImage]) -> Vec<Vec<String>> {
    images
        .iter()
        .map(|image| image.imeta_tag().to_vec())
        .collect()
}
