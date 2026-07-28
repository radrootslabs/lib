#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::error::EventParseError;
use radroots_event::{RadrootsEventEnvelope, RadrootsEventEnvelopeParts};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsParsedData<T> {
    pub id: String,
    pub author: String,
    pub published_at: u64,
    pub kind: u32,
    pub data: T,
}

impl<T> RadrootsParsedData<T> {
    #[inline]
    pub fn new(id: String, author: String, published_at: u64, kind: u32, data: T) -> Self {
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
    pub event: RadrootsEventEnvelope,
    pub data: RadrootsParsedData<T>,
}

impl<T> RadrootsParsedEvent<T> {
    #[inline]
    pub fn new(event: RadrootsEventEnvelope, data: RadrootsParsedData<T>) -> Self {
        Self { event, data }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: String,
        author: String,
        published_at: u64,
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
        sig: String,
        data: T,
    ) -> Result<Self, EventParseError> {
        let parsed_data =
            RadrootsParsedData::new(id.clone(), author.clone(), published_at, kind, data);
        Self::from_event_parts(
            id,
            author,
            published_at,
            kind,
            content,
            tags,
            sig,
            parsed_data,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_event_parts(
        id: String,
        author: String,
        published_at: u64,
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
        sig: String,
        data: RadrootsParsedData<T>,
    ) -> Result<Self, EventParseError> {
        let event = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id,
            author,
            created_at: published_at,
            kind,
            tags,
            content,
            sig,
        })?;
        Ok(Self { event, data })
    }
}

#[cfg(test)]
mod tests {
    use super::{RadrootsParsedData, RadrootsParsedEvent};
    use radroots_event::{RadrootsEventEnvelope, RadrootsEventEnvelopeParts};

    const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const AUTHOR: &str = crate::test_fixtures::FIXTURE_ALICE_PUBLIC_KEY_HEX;
    const SIG: &str = concat!(
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
    );

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
        let event = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: EVENT_ID.to_string(),
            author: AUTHOR.to_string(),
            created_at: 22,
            kind: 1,
            tags: vec![vec!["k".to_string(), "v".to_string()]],
            content: "content".to_string(),
            sig: SIG.to_string(),
        })
        .unwrap();
        let data = RadrootsParsedData::new(
            EVENT_ID.to_string(),
            AUTHOR.to_string(),
            22,
            1,
            "payload".to_string(),
        );

        let out = RadrootsParsedEvent::new(event.clone(), data.clone());
        assert_eq!(out.event.id(), event.id());
        assert_eq!(out.event.author(), event.author());
        assert_eq!(out.event.created_at(), event.created_at());
        assert_eq!(out.event.kind(), event.kind());
        assert_eq!(out.event.tags(), event.tags());
        assert_eq!(out.event.content(), event.content());
        assert_eq!(out.event.sig(), event.sig());
        assert_eq!(out.data, data);
    }

    #[test]
    fn parsed_event_from_parts_builds_consistent_structs() {
        let out = RadrootsParsedEvent::from_parts(
            EVENT_ID.to_string(),
            AUTHOR.to_string(),
            77,
            1111,
            "hello".to_string(),
            vec![vec!["e".to_string(), "root".to_string()]],
            SIG.to_string(),
            "payload".to_string(),
        )
        .unwrap();
        assert_eq!(out.event.id_hex(), EVENT_ID);
        assert_eq!(out.event.author().to_hex(), AUTHOR);
        assert_eq!(out.event.created_at_u64(), 77);
        assert_eq!(out.event.kind_u32(), 1111);
        assert_eq!(out.event.content(), "hello");
        assert_eq!(
            out.event.tags_as_vec(),
            vec![vec!["e".to_string(), "root".to_string()]]
        );
        assert_eq!(out.event.signature_hex(), SIG);
        assert_eq!(out.data.id, EVENT_ID);
        assert_eq!(out.data.author, AUTHOR);
        assert_eq!(out.data.published_at, 77);
        assert_eq!(out.data.kind, 1111);
        assert_eq!(out.data.data, "payload");
    }
}
