#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::social::{RadrootsSocialFarmAnchor, RadrootsSocialLocation};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsArticle {
    pub d_tag: String,
    pub title: String,
    pub content: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub summary: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub image: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub published_at: Option<u64>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub farm: Option<RadrootsSocialFarmAnchor>,
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
}

#[cfg(all(test, feature = "std", feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn article_represents_required_nip23_fields() {
        let article = RadrootsArticle {
            d_tag: "soil-notes".to_string(),
            title: "soil notes".to_string(),
            content: "# soil notes".to_string(),
            summary: None,
            image: None,
            published_at: Some(1_700_000_000),
            farm: None,
            location: Some(RadrootsSocialLocation {
                name: Some("field edge".to_string()),
                geohash: Some("c23nb62w20st".to_string()),
            }),
            topics: Some(vec!["soil".to_string(), "cover-crops".to_string()]),
        };

        assert_eq!(article.d_tag, "soil-notes");
        assert_eq!(article.title, "soil notes");
        assert_eq!(article.published_at, Some(1_700_000_000));
        assert_eq!(article.topics.as_ref().expect("topics").len(), 2);
    }
}
