#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::RadrootsNostrEvent;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsParsedData<T> {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub data: T,
}

impl<T> RadrootsParsedData<T> {
    #[inline]
    pub fn new(id: String, author: String, published_at: u32, kind: u32, data: T) -> Self {
        Self {
            id,
            author,
            published_at,
            kind,
            data,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsParsedEvent<T> {
    pub event: RadrootsNostrEvent,
    pub data: RadrootsParsedData<T>,
}

impl<T> RadrootsParsedEvent<T> {
    #[inline]
    pub fn new(event: RadrootsNostrEvent, data: RadrootsParsedData<T>) -> Self {
        Self { event, data }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: String,
        author: String,
        published_at: u32,
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
        sig: String,
        data: T,
    ) -> Self {
        Self {
            event: RadrootsNostrEvent {
                id: id.clone(),
                author: author.clone(),
                created_at: published_at,
                kind,
                tags,
                content,
                sig,
            },
            data: RadrootsParsedData::new(id, author, published_at, kind, data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RadrootsParsedData, RadrootsParsedEvent};
    use radroots_events::RadrootsNostrEvent;

    #[test]
    fn parsed_data_constructor_maps_fields() {
        let out = RadrootsParsedData::new(
            "id".to_string(),
            "author".to_string(),
            10,
            30402,
            "payload".to_string(),
        );
        assert_eq!(out.id, "id");
        assert_eq!(out.author, "author");
        assert_eq!(out.published_at, 10);
        assert_eq!(out.kind, 30402);
        assert_eq!(out.data, "payload");
    }

    #[test]
    fn parsed_event_constructor_maps_event_and_data() {
        let event = RadrootsNostrEvent {
            id: "id".to_string(),
            author: "author".to_string(),
            created_at: 22,
            kind: 1,
            tags: vec![vec!["k".to_string(), "v".to_string()]],
            content: "content".to_string(),
            sig: "sig".to_string(),
        };
        let data = RadrootsParsedData::new(
            "id".to_string(),
            "author".to_string(),
            22,
            1,
            "payload".to_string(),
        );

        let out = RadrootsParsedEvent::new(event.clone(), data.clone());
        assert_eq!(out.event.id, event.id);
        assert_eq!(out.event.author, event.author);
        assert_eq!(out.event.created_at, event.created_at);
        assert_eq!(out.event.kind, event.kind);
        assert_eq!(out.event.tags, event.tags);
        assert_eq!(out.event.content, event.content);
        assert_eq!(out.event.sig, event.sig);
        assert_eq!(out.data, data);
    }

    #[test]
    fn parsed_event_from_parts_builds_consistent_structs() {
        let out = RadrootsParsedEvent::from_parts(
            "id".to_string(),
            "author".to_string(),
            77,
            1111,
            "hello".to_string(),
            vec![vec!["e".to_string(), "root".to_string()]],
            "sig".to_string(),
            "payload".to_string(),
        );
        assert_eq!(out.event.id, "id");
        assert_eq!(out.event.author, "author");
        assert_eq!(out.event.created_at, 77);
        assert_eq!(out.event.kind, 1111);
        assert_eq!(out.event.content, "hello");
        assert_eq!(
            out.event.tags,
            vec![vec!["e".to_string(), "root".to_string()]]
        );
        assert_eq!(out.event.sig, "sig");
        assert_eq!(out.data.id, "id");
        assert_eq!(out.data.author, "author");
        assert_eq!(out.data.published_at, 77);
        assert_eq!(out.data.kind, 1111);
        assert_eq!(out.data.data, "payload");
    }
}
