#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::social::{
    RadrootsSocialFarmAnchor, RadrootsSocialLocation, RadrootsSocialMediaMetadata,
    RadrootsSocialTarget,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsPost {
    pub content: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub farm: Option<RadrootsSocialFarmAnchor>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub address_refs: Option<Vec<RadrootsSocialTarget>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub location: Option<RadrootsSocialLocation>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub topics: Option<Vec<String>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub quote_refs: Option<Vec<RadrootsSocialTarget>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub media: Option<Vec<RadrootsSocialMediaMetadata>>,
}

#[cfg(all(test, feature = "std", feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn content_only_post_deserializes_without_social_metadata() {
        let post: RadrootsPost =
            serde_json::from_str(r#"{"content":"farm update"}"#).expect("post");

        assert_eq!(post.content, "farm update");
        assert!(post.farm.is_none());
        assert!(post.address_refs.is_none());
        assert!(post.location.is_none());
        assert!(post.topics.is_none());
        assert!(post.quote_refs.is_none());
        assert!(post.media.is_none());
    }

    #[test]
    fn content_only_post_serializes_without_null_social_metadata() {
        let post = RadrootsPost {
            content: "farm update".to_string(),
            farm: None,
            address_refs: None,
            location: None,
            topics: None,
            quote_refs: None,
            media: None,
        };

        let json = serde_json::to_string(&post).expect("json");
        assert_eq!(json, r#"{"content":"farm update"}"#);
    }
}
