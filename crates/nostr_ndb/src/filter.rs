use crate::error::RadrootsNostrNdbError;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct RadrootsNostrNdbFilterSpec {
    event_ids_hex: Vec<String>,
    authors_hex: Vec<String>,
    kinds: Vec<u16>,
    since_unix: Option<u64>,
    until_unix: Option<u64>,
    limit: Option<u64>,
    search: Option<String>,
}

impl RadrootsNostrNdbFilterSpec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text_notes(limit: Option<u64>, since_unix: Option<u64>) -> Self {
        let mut filter = Self::new().with_kind(1);
        if let Some(limit) = limit {
            filter = filter.with_limit(limit);
        }
        if let Some(since_unix) = since_unix {
            filter = filter.with_since_unix(since_unix);
        }
        filter
    }

    pub fn with_event_id_hex(mut self, id_hex: impl Into<String>) -> Self {
        self.event_ids_hex.push(id_hex.into());
        self
    }

    pub fn with_author_hex(mut self, author_hex: impl Into<String>) -> Self {
        self.authors_hex.push(author_hex.into());
        self
    }

    pub fn with_kind(mut self, kind: u16) -> Self {
        self.kinds.push(kind);
        self
    }

    pub fn with_since_unix(mut self, since_unix: u64) -> Self {
        self.since_unix = Some(since_unix);
        self
    }

    pub fn with_until_unix(mut self, until_unix: u64) -> Self {
        self.until_unix = Some(until_unix);
        self
    }

    pub fn with_limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_search(mut self, search: impl Into<String>) -> Self {
        self.search = Some(search.into());
        self
    }

    pub fn event_ids_hex(&self) -> &[String] {
        &self.event_ids_hex
    }

    pub fn authors_hex(&self) -> &[String] {
        &self.authors_hex
    }

    pub fn kinds(&self) -> &[u16] {
        &self.kinds
    }

    pub fn since_unix(&self) -> Option<u64> {
        self.since_unix
    }

    pub fn until_unix(&self) -> Option<u64> {
        self.until_unix
    }

    pub fn limit(&self) -> Option<u64> {
        self.limit
    }

    pub fn search(&self) -> Option<&str> {
        self.search.as_deref()
    }

    pub(crate) fn to_ndb_filter(&self) -> Result<nostrdb::Filter, RadrootsNostrNdbError> {
        let mut builder = nostrdb::Filter::new();

        if !self.event_ids_hex.is_empty() {
            let event_ids = self
                .event_ids_hex
                .iter()
                .map(|hex_value| parse_hex_32(hex_value, "event_id"))
                .collect::<Result<Vec<_>, _>>()?;
            builder = builder.ids(event_ids.iter());
        }

        if !self.authors_hex.is_empty() {
            let authors = self
                .authors_hex
                .iter()
                .map(|hex_value| parse_hex_32(hex_value, "author"))
                .collect::<Result<Vec<_>, _>>()?;
            builder = builder.authors(authors.iter());
        }

        if !self.kinds.is_empty() {
            builder = builder.kinds(self.kinds.iter().map(|kind| *kind as u64));
        }

        if let Some(since_unix) = self.since_unix {
            builder = builder.since(since_unix);
        }

        if let Some(until_unix) = self.until_unix {
            builder = builder.until(until_unix);
        }

        if let Some(limit) = self.limit {
            builder = builder.limit(limit);
        }

        if let Some(search) = self.search() {
            builder = builder.search(search);
        }

        Ok(builder.build())
    }
}

pub(crate) fn parse_hex_32(
    value: &str,
    field: &'static str,
) -> Result<[u8; 32], RadrootsNostrNdbError> {
    let bytes = hex::decode(value).map_err(|source| RadrootsNostrNdbError::InvalidHex {
        field,
        reason: source.to_string(),
    })?;

    if bytes.len() != 32 {
        return Err(RadrootsNostrNdbError::InvalidHexLength {
            field,
            expected: 32,
            actual: bytes.len(),
        });
    }

    let mut out = [0u8; 32];
    out.copy_from_slice(bytes.as_slice());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_hex_32(value: u8) -> String {
        format!("{value:02x}").repeat(32)
    }

    #[test]
    fn filter_spec_builders_and_accessors_round_trip() {
        let event_id = valid_hex_32(0x11);
        let author = valid_hex_32(0x22);

        let empty_notes = RadrootsNostrNdbFilterSpec::text_notes(None, None);
        assert_eq!(empty_notes.kinds(), &[1]);
        assert_eq!(empty_notes.limit(), None);
        assert_eq!(empty_notes.since_unix(), None);

        let spec = RadrootsNostrNdbFilterSpec::text_notes(Some(50), Some(100))
            .with_event_id_hex(event_id.clone())
            .with_author_hex(author.clone())
            .with_kind(30023)
            .with_since_unix(200)
            .with_until_unix(300)
            .with_limit(10)
            .with_search("coffee");

        assert_eq!(spec.event_ids_hex(), &[event_id.clone()]);
        assert_eq!(spec.authors_hex(), &[author.clone()]);
        assert_eq!(spec.kinds(), &[1, 30023]);
        assert_eq!(spec.since_unix(), Some(200));
        assert_eq!(spec.until_unix(), Some(300));
        assert_eq!(spec.limit(), Some(10));
        assert_eq!(spec.search(), Some("coffee"));
        let _ = spec.to_ndb_filter().expect("ndb filter");

        let empty = RadrootsNostrNdbFilterSpec::new();
        let _ = empty.to_ndb_filter().expect("empty ndb filter");
    }

    #[test]
    fn parse_hex_32_validates_input() {
        let valid = parse_hex_32(valid_hex_32(0xab).as_str(), "value").expect("valid");
        assert_eq!(valid, [0xab; 32]);

        let invalid_hex = parse_hex_32("zz", "value");
        assert!(matches!(
            invalid_hex,
            Err(RadrootsNostrNdbError::InvalidHex { field: "value", .. })
        ));

        let invalid_len = parse_hex_32("abcd", "value");
        assert!(matches!(
            invalid_len,
            Err(RadrootsNostrNdbError::InvalidHexLength {
                field: "value",
                expected: 32,
                ..
            })
        ));
    }

    #[test]
    fn to_ndb_filter_rejects_invalid_event_id_and_author_hex() {
        let bad_event_id = RadrootsNostrNdbFilterSpec::new().with_event_id_hex("not-hex");
        let bad_event_result = bad_event_id.to_ndb_filter();
        assert!(matches!(
            bad_event_result,
            Err(RadrootsNostrNdbError::InvalidHex {
                field: "event_id",
                ..
            })
        ));

        let bad_author = RadrootsNostrNdbFilterSpec::new().with_author_hex("not-hex");
        let bad_author_result = bad_author.to_ndb_filter();
        assert!(matches!(
            bad_author_result,
            Err(RadrootsNostrNdbError::InvalidHex {
                field: "author",
                ..
            })
        ));
    }
}
