pub mod decode;
pub mod encode;

use crate::d_tag::is_d_tag_base64url;

fn list_set_requires_base64(d_tag: &str) -> bool {
    d_tag.starts_with("farm:") || d_tag.starts_with("coop:") || d_tag.starts_with("resource:")
}

fn list_set_base64_id_is_valid(d_tag: &str) -> bool {
    if !list_set_requires_base64(d_tag) {
        return true;
    }
    let mut parts = d_tag.splitn(3, ':');
    let _ = parts.next();
    let id = parts.next().unwrap_or("");
    let suffix = parts.next().unwrap_or("");
    !id.trim().is_empty() && !suffix.trim().is_empty() && is_d_tag_base64url(id)
}

#[cfg(test)]
mod tests {
    use super::{decode::list_set_from_tags, encode::list_set_build_tags};
    use crate::error::{EventEncodeError, EventParseError};
    use radroots_events::{
        kinds::KIND_LIST_SET_FOLLOW, list::RadrootsListEntry, list_set::RadrootsListSet,
    };

    #[test]
    fn list_set_tags_round_trip() {
        let list = RadrootsListSet {
            d_tag: "members.owners".to_string(),
            content: "".to_string(),
            entries: vec![
                RadrootsListEntry {
                    tag: "p".to_string(),
                    values: vec!["owner_pubkey".to_string()],
                },
                RadrootsListEntry {
                    tag: "p".to_string(),
                    values: vec!["worker_pubkey".to_string(), "wss://relay".to_string()],
                },
            ],
            title: Some("Owners".to_string()),
            description: None,
            image: None,
        };
        let tags = list_set_build_tags(&list).expect("build tags");
        let parsed = list_set_from_tags(KIND_LIST_SET_FOLLOW, list.content.clone(), &tags)
            .expect("parse list set");
        assert_eq!(parsed.d_tag, list.d_tag);
        assert_eq!(parsed.title, list.title);
        assert_eq!(parsed.entries.len(), list.entries.len());
        assert_eq!(parsed.entries[0].values[0], "owner_pubkey");
    }

    #[test]
    fn list_set_rejects_invalid_farm_d_tag_on_encode() {
        let list = RadrootsListSet {
            d_tag: "farm:invalid:members".to_string(),
            content: "".to_string(),
            entries: vec![RadrootsListEntry {
                tag: "p".to_string(),
                values: vec!["pubkey".to_string()],
            }],
            title: None,
            description: None,
            image: None,
        };
        let err = list_set_build_tags(&list).expect_err("expected invalid d_tag");
        assert!(matches!(err, EventEncodeError::InvalidField("d_tag")));
    }

    #[test]
    fn list_set_rejects_invalid_farm_d_tag_on_decode() {
        let tags = vec![
            vec!["d".to_string(), "farm:invalid:members".to_string()],
            vec!["p".to_string(), "pubkey".to_string()],
        ];
        let err = list_set_from_tags(KIND_LIST_SET_FOLLOW, "".to_string(), &tags)
            .expect_err("expected invalid d_tag");
        assert!(matches!(err, EventParseError::InvalidTag("d")));
    }
}
