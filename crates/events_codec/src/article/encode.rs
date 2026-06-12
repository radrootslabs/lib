#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use radroots_events::{
    article::RadrootsArticle,
    kinds::KIND_ARTICLE,
    tags::{TAG_D, TAG_IMAGE, TAG_PUBLISHED_AT, TAG_SUMMARY, TAG_T, TAG_TITLE},
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;
use crate::field_helpers::{push_optional_tag, push_tag, validate_non_empty_field};
use crate::social_helpers::push_location_tags;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_ARTICLE;

pub fn article_build_tags(article: &RadrootsArticle) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_article(article)?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, article.d_tag.as_str());
    push_tag(&mut tags, TAG_TITLE, article.title.as_str());
    push_optional_tag(&mut tags, TAG_SUMMARY, article.summary.as_deref());
    push_optional_tag(&mut tags, TAG_IMAGE, article.image.as_deref());
    if let Some(published_at) = article.published_at {
        push_tag(&mut tags, TAG_PUBLISHED_AT, published_at.to_string());
    }
    if let Some(farm) = article.farm.as_ref() {
        crate::social_helpers::push_farm_anchor(&mut tags, farm);
    }
    if let Some(location) = article.location.as_ref() {
        push_location_tags(&mut tags, location);
    }
    if let Some(topics) = article.topics.as_ref() {
        for topic in topics {
            push_optional_tag(&mut tags, TAG_T, Some(topic.as_str()));
        }
    }
    Ok(tags)
}

pub fn to_wire_parts(article: &RadrootsArticle) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(article, DEFAULT_KIND)
}

pub fn to_wire_parts_with_kind(
    article: &RadrootsArticle,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != DEFAULT_KIND {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(WireEventParts {
        kind,
        content: article.content.clone(),
        tags: article_build_tags(article)?,
    })
}

fn validate_article(article: &RadrootsArticle) -> Result<(), EventEncodeError> {
    validate_d_tag(&article.d_tag, "d_tag")?;
    validate_non_empty_field(&article.title, "title")?;
    validate_non_empty_field(&article.content, "content")?;
    Ok(())
}
