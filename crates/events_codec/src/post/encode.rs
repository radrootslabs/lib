#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_events::{
    kinds::{KIND_FARM, KIND_POST},
    post::RadrootsPost,
    social::{RadrootsSocialFarmAnchor, RadrootsSocialMediaMetadata, RadrootsSocialTarget},
    tags::{TAG_A, TAG_IMETA, TAG_Q, TAG_T},
};

use crate::error::EventEncodeError;
use crate::field_helpers::{parse_address_tag, validate_lowercase_hex_64};
use crate::social_helpers::{dimensions_tag, push_location_tags};
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_POST;

pub fn post_build_tags(post: &RadrootsPost) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::new();
    if let Some(farm) = post.farm.as_ref() {
        push_farm_anchor(&mut tags, farm)?;
    }
    if let Some(refs) = post.address_refs.as_ref() {
        for target in refs {
            push_address_ref(&mut tags, target)?;
        }
    }
    if let Some(location) = post.location.as_ref() {
        push_location_tags(&mut tags, location);
    }
    if let Some(topics) = post.topics.as_ref() {
        for topic in topics {
            if !topic.trim().is_empty() {
                tags.push(vec![TAG_T.to_string(), topic.clone()]);
            }
        }
    }
    if let Some(quote_refs) = post.quote_refs.as_ref() {
        for target in quote_refs {
            push_quote_ref(&mut tags, target)?;
        }
    }
    if let Some(media) = post.media.as_ref() {
        for item in media {
            push_media_tags(&mut tags, item)?;
        }
    }
    Ok(tags)
}

pub fn to_wire_parts(post: &RadrootsPost) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(post, DEFAULT_KIND)
}

pub fn to_wire_parts_with_kind(
    post: &RadrootsPost,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != DEFAULT_KIND {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    if post.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    let tags = post_build_tags(post)?;
    Ok(WireEventParts {
        kind,
        content: post.content.clone(),
        tags,
    })
}

fn push_farm_anchor(
    tags: &mut Vec<Vec<String>>,
    farm: &RadrootsSocialFarmAnchor,
) -> Result<(), EventEncodeError> {
    if farm.farm.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.pubkey"));
    }
    if farm.farm.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.d_tag"));
    }
    let address = format!("{}:{}:{}", KIND_FARM, farm.farm.pubkey, farm.farm.d_tag);
    parse_address_tag(&address, "farm").map_err(|_| EventEncodeError::InvalidField("farm"))?;
    let mut tag = Vec::with_capacity(2 + farm.relays.as_ref().map_or(0, Vec::len));
    tag.push(TAG_A.to_string());
    tag.push(address);
    if let Some(relays) = farm.relays.as_ref() {
        tag.extend(relays.iter().cloned());
    }
    tags.push(tag);
    Ok(())
}

fn push_address_ref(
    tags: &mut Vec<Vec<String>>,
    target: &RadrootsSocialTarget,
) -> Result<(), EventEncodeError> {
    let RadrootsSocialTarget::Address {
        address,
        author,
        event_kind,
        relays,
    } = target
    else {
        return Err(EventEncodeError::InvalidField("address_refs"));
    };
    let parsed = parse_address_tag(address, "address_refs")
        .map_err(|_| EventEncodeError::InvalidField("address_refs"))?;
    if parsed.kind == KIND_FARM {
        return Err(EventEncodeError::InvalidField("address_refs"));
    }
    if let Some(kind) = event_kind {
        if *kind != parsed.kind {
            return Err(EventEncodeError::InvalidField("address_refs"));
        }
    }
    if let Some(author) = author.as_deref() {
        if author != parsed.pubkey {
            return Err(EventEncodeError::InvalidField("address_refs"));
        }
    }
    let mut tag = Vec::with_capacity(2 + relays.as_ref().map_or(0, Vec::len));
    tag.push(TAG_A.to_string());
    tag.push(format!(
        "{}:{}:{}",
        parsed.kind, parsed.pubkey, parsed.d_tag
    ));
    if let Some(relays) = relays {
        tag.extend(relays.iter().cloned());
    }
    tags.push(tag);
    Ok(())
}

fn push_quote_ref(
    tags: &mut Vec<Vec<String>>,
    target: &RadrootsSocialTarget,
) -> Result<(), EventEncodeError> {
    match target {
        RadrootsSocialTarget::Event { id, relays, .. } => {
            validate_lowercase_hex_64(id, "quote_refs")?;
            let mut tag = Vec::with_capacity(2 + relays.as_ref().map_or(0, Vec::len));
            tag.push(TAG_Q.to_string());
            tag.push(id.clone());
            if let Some(relays) = relays {
                tag.extend(relays.iter().cloned());
            }
            tags.push(tag);
            Ok(())
        }
        RadrootsSocialTarget::Address {
            address,
            event_kind,
            relays,
            ..
        } => {
            let parsed = parse_address_tag(address, "quote_refs")
                .map_err(|_| EventEncodeError::InvalidField("quote_refs"))?;
            if let Some(kind) = event_kind {
                if *kind != parsed.kind {
                    return Err(EventEncodeError::InvalidField("quote_refs"));
                }
            }
            let mut tag = Vec::with_capacity(2 + relays.as_ref().map_or(0, Vec::len));
            tag.push(TAG_Q.to_string());
            tag.push(format!(
                "{}:{}:{}",
                parsed.kind, parsed.pubkey, parsed.d_tag
            ));
            if let Some(relays) = relays {
                tag.extend(relays.iter().cloned());
            }
            tags.push(tag);
            Ok(())
        }
        RadrootsSocialTarget::External { .. } => Err(EventEncodeError::InvalidField("quote_refs")),
    }
}

fn push_media_tags(
    tags: &mut Vec<Vec<String>>,
    media: &RadrootsSocialMediaMetadata,
) -> Result<(), EventEncodeError> {
    if let Some(raw_tags) = media.imeta.as_ref() {
        for raw in raw_tags {
            if raw.is_empty() || raw.iter().any(|value| value.trim().is_empty()) {
                return Err(EventEncodeError::InvalidField("imeta"));
            }
            let mut tag = Vec::with_capacity(1 + raw.len());
            tag.push(TAG_IMETA.to_string());
            tag.extend(raw.iter().cloned());
            tags.push(tag);
        }
        return Ok(());
    }

    let mut fields = Vec::new();
    push_imeta_field(&mut fields, "url", media.url.as_deref());
    push_imeta_field(&mut fields, "m", media.mime_type.as_deref());
    push_imeta_field(&mut fields, "x", media.sha256.as_deref());
    push_imeta_field(&mut fields, "ox", media.original_sha256.as_deref());
    if let Some(size) = media.size {
        fields.push(format!("size {size}"));
    }
    if let Some(dimensions) = media.dimensions.as_ref() {
        fields.push(format!("dim {}", dimensions_tag(dimensions)));
    }
    push_imeta_field(&mut fields, "blurhash", media.blurhash.as_deref());
    if let Some(thumbnails) = media.thumbnails.as_ref() {
        for thumbnail in thumbnails {
            if thumbnail.url.trim().is_empty() {
                return Err(EventEncodeError::InvalidField("imeta"));
            }
            fields.push(format!("thumb {}", thumbnail.url));
            if let Some(dimensions) = thumbnail.dimensions.as_ref() {
                fields.push(format!("dim {}", dimensions_tag(dimensions)));
            }
        }
    }
    push_imeta_field(&mut fields, "image", media.image.as_deref());
    push_imeta_field(&mut fields, "summary", media.summary.as_deref());
    push_imeta_field(&mut fields, "alt", media.alt.as_deref());
    push_imeta_field(&mut fields, "fallback", media.fallback.as_deref());
    push_imeta_field(&mut fields, "magnet", media.magnet.as_deref());
    if let Some(values) = media.content_hashes.as_ref() {
        for value in values {
            push_imeta_field(&mut fields, "i", Some(value.as_str()));
        }
    }
    if let Some(values) = media.services.as_ref() {
        for value in values {
            push_imeta_field(&mut fields, "service", Some(value.as_str()));
        }
    }
    if !fields.is_empty() {
        let mut tag = Vec::with_capacity(1 + fields.len());
        tag.push(TAG_IMETA.to_string());
        tag.extend(fields);
        tags.push(tag);
    }
    Ok(())
}

fn push_imeta_field(fields: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        fields.push(format!("{key} {value}"));
    }
}
