use crate::filter::RadrootsNostrNdbFilterSpec;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct RadrootsNostrNdbSubscriptionHandle {
    id: u64,
}

impl RadrootsNostrNdbSubscriptionHandle {
    pub(crate) fn new(id: u64) -> Self {
        Self { id }
    }

    pub fn id(self) -> u64 {
        self.id
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct RadrootsNostrNdbNoteKey {
    key: u64,
}

impl RadrootsNostrNdbNoteKey {
    pub(crate) fn new(key: u64) -> Self {
        Self { key }
    }

    pub fn as_u64(self) -> u64 {
        self.key
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RadrootsNostrNdbSubscriptionSpec {
    filters: Vec<RadrootsNostrNdbFilterSpec>,
}

impl RadrootsNostrNdbSubscriptionSpec {
    pub fn new(filters: Vec<RadrootsNostrNdbFilterSpec>) -> Self {
        Self { filters }
    }

    pub fn single(filter: RadrootsNostrNdbFilterSpec) -> Self {
        Self {
            filters: vec![filter],
        }
    }

    pub fn text_notes(limit: Option<u64>, since_unix: Option<u64>) -> Self {
        Self::single(RadrootsNostrNdbFilterSpec::text_notes(limit, since_unix))
    }

    pub fn filters(&self) -> &[RadrootsNostrNdbFilterSpec] {
        &self.filters
    }
}

#[cfg(feature = "rt")]
pub struct RadrootsNostrNdbSubscriptionStream {
    pub(crate) inner: nostrdb::SubscriptionStream,
}

#[cfg(feature = "rt")]
impl futures::Stream for RadrootsNostrNdbSubscriptionStream {
    type Item = Vec<RadrootsNostrNdbNoteKey>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.inner)
            .poll_next(cx)
            .map(|note_keys| {
                note_keys.map(|keys| {
                    keys.into_iter()
                        .map(|note_key| RadrootsNostrNdbNoteKey::new(note_key.as_u64()))
                        .collect()
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::RadrootsNostrNdbFilterSpec;

    #[test]
    fn subscription_types_expose_builders_and_accessors() {
        let handle = RadrootsNostrNdbSubscriptionHandle::new(42);
        assert_eq!(handle.id(), 42);

        let note_key = RadrootsNostrNdbNoteKey::new(7);
        assert_eq!(note_key.as_u64(), 7);

        let filter = RadrootsNostrNdbFilterSpec::new().with_kind(1);
        let from_new = RadrootsNostrNdbSubscriptionSpec::new(vec![filter.clone()]);
        assert_eq!(from_new.filters(), &[filter.clone()]);

        let from_single = RadrootsNostrNdbSubscriptionSpec::single(filter.clone());
        assert_eq!(from_single.filters(), &[filter.clone()]);

        let text_notes = RadrootsNostrNdbSubscriptionSpec::text_notes(Some(10), Some(123));
        assert_eq!(text_notes.filters().len(), 1);
    }
}
