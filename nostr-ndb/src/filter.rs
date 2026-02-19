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

fn parse_hex_32(value: &str, field: &'static str) -> Result<[u8; 32], RadrootsNostrNdbError> {
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
