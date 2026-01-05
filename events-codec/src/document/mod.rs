#![forbid(unsafe_code)]

pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::document::{RadrootsDocument, RadrootsDocumentSubject};
    use crate::document::encode::document_build_tags;

    #[test]
    fn document_tags_include_required_fields() {
        let document = RadrootsDocument {
            d_tag: "EAAAAAAAAAAAAAAAAAAAAA".to_string(),
            doc_type: "charter".to_string(),
            title: "Sierra Co-op Charter".to_string(),
            version: "1.0.0".to_string(),
            summary: None,
            effective_at: None,
            body_markdown: None,
            subject: RadrootsDocumentSubject {
                pubkey: "coop_pubkey".to_string(),
                address: Some("30360:coop_pubkey:BAAAAAAAAAAAAAAAAAAAAA".to_string()),
            },
            tags: Some(vec!["charter".to_string()]),
        };

        let tags = document_build_tags(&document).expect("tags");
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"d".to_string())));
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"p".to_string())));
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"a".to_string())));
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"t".to_string())));
    }
}
