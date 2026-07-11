use crate::filter::RadrootsNostrdbFilterSpec;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RadrootsNostrdbQuerySpec {
    filters: Vec<RadrootsNostrdbFilterSpec>,
    max_results: u32,
}

impl RadrootsNostrdbQuerySpec {
    pub fn new(filters: Vec<RadrootsNostrdbFilterSpec>, max_results: u32) -> Self {
        Self {
            filters,
            max_results: max_results.max(1),
        }
    }

    pub fn single(filter: RadrootsNostrdbFilterSpec, max_results: u32) -> Self {
        Self::new(vec![filter], max_results)
    }

    pub fn text_notes(limit: Option<u64>, since_unix: Option<u64>, max_results: u32) -> Self {
        Self::single(
            RadrootsNostrdbFilterSpec::text_notes(limit, since_unix),
            max_results,
        )
    }

    pub fn filters(&self) -> &[RadrootsNostrdbFilterSpec] {
        &self.filters
    }

    pub fn max_results(&self) -> u32 {
        self.max_results
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RadrootsNostrdbNote {
    pub note_key: u64,
    pub id_hex: String,
    pub author_hex: String,
    pub kind: u32,
    pub created_at_unix: u64,
    pub content: String,
    pub json: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RadrootsNostrdbProfile {
    pub profile_key: Option<u64>,
    pub pubkey_hex: String,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub website: Option<String>,
    pub nip05: Option<String>,
    pub lud16: Option<String>,
}
