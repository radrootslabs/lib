#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    article::RadrootsArticle,
    farm::RadrootsFarmRef,
    kinds::{KIND_ARTICLE, KIND_FARM},
    social::RadrootsSocialFarmAnchor,
    tags::{TAG_A, TAG_D, TAG_IMAGE, TAG_PUBLISHED_AT, TAG_SUMMARY, TAG_T, TAG_TITLE},
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;
use crate::field_helpers::{
    parse_address_tag_with_kind, required_tag_value, tag_values, validate_non_empty_tag_value,
};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
use crate::social_helpers::{first_tag_value, location_from_tags};

const EXPECTED_KIND: &str = "30023";

pub fn article_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsArticle, EventParseError> {
    if kind != KIND_ARTICLE {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_KIND,
            got: kind,
        });
    }
    validate_non_empty_tag_value(content, "content")?;
    let d_tag = required_tag_value(tags, TAG_D)?;
    validate_d_tag_tag(&d_tag, TAG_D)?;
    let title = required_tag_value(tags, TAG_TITLE)?;
    let published_at = first_tag_value(tags, TAG_PUBLISHED_AT)
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|err| EventParseError::InvalidNumber(TAG_PUBLISHED_AT, err))
        })
        .transpose()?;
    let farm = parse_farm_anchor(tags)?;
    Ok(RadrootsArticle {
        d_tag,
        title,
        content: content.to_string(),
        summary: first_tag_value(tags, TAG_SUMMARY),
        image: first_tag_value(tags, TAG_IMAGE),
        published_at,
        farm,
        location: location_from_tags(tags),
        topics: non_empty_vec(tag_values(tags, TAG_T)?),
    })
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsArticle>, EventParseError> {
    let article = article_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        article,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsArticle>, EventParseError> {
    let data = data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        data,
    })
}

fn parse_farm_anchor(
    tags: &[Vec<String>],
) -> Result<Option<RadrootsSocialFarmAnchor>, EventParseError> {
    let Some(value) = first_tag_value(tags, TAG_A) else {
        return Ok(None);
    };
    let address = parse_address_tag_with_kind(&value, KIND_FARM, TAG_A)?;
    Ok(Some(RadrootsSocialFarmAnchor {
        farm: RadrootsFarmRef {
            pubkey: address.pubkey,
            d_tag: address.d_tag,
        },
        relays: None,
    }))
}

fn non_empty_vec(values: Vec<String>) -> Option<Vec<String>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}
