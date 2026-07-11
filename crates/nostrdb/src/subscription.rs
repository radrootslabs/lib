use crate::filter::RadrootsNostrdbFilterSpec;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct RadrootsNostrdbSubscriptionHandle {
    id: u64,
}

impl RadrootsNostrdbSubscriptionHandle {
    pub(crate) fn new(id: u64) -> Self {
        Self { id }
    }

    pub fn id(self) -> u64 {
        self.id
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct RadrootsNostrdbNoteKey {
    key: u64,
}

impl RadrootsNostrdbNoteKey {
    pub(crate) fn new(key: u64) -> Self {
        Self { key }
    }

    pub fn as_u64(self) -> u64 {
        self.key
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RadrootsNostrdbSubscriptionSpec {
    filters: Vec<RadrootsNostrdbFilterSpec>,
}

impl RadrootsNostrdbSubscriptionSpec {
    pub fn new(filters: Vec<RadrootsNostrdbFilterSpec>) -> Self {
        Self { filters }
    }

    pub fn single(filter: RadrootsNostrdbFilterSpec) -> Self {
        Self {
            filters: vec![filter],
        }
    }

    pub fn text_notes(limit: Option<u64>, since_unix: Option<u64>) -> Self {
        Self::single(RadrootsNostrdbFilterSpec::text_notes(limit, since_unix))
    }

    pub fn filters(&self) -> &[RadrootsNostrdbFilterSpec] {
        &self.filters
    }
}

#[cfg(feature = "rt")]
pub struct RadrootsNostrdbSubscriptionStream {
    pub(crate) inner: nostrdb::SubscriptionStream,
}

#[cfg(feature = "rt")]
impl futures::Stream for RadrootsNostrdbSubscriptionStream {
    type Item = Vec<RadrootsNostrdbNoteKey>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.inner)
            .poll_next(cx)
            .map(|note_keys| {
                note_keys.map(|keys| {
                    keys.into_iter()
                        .map(|note_key| RadrootsNostrdbNoteKey::new(note_key.as_u64()))
                        .collect()
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::RadrootsNostrdbFilterSpec;

    #[test]
    fn subscription_types_expose_builders_and_accessors() {
        let handle = RadrootsNostrdbSubscriptionHandle::new(42);
        assert_eq!(handle.id(), 42);

        let note_key = RadrootsNostrdbNoteKey::new(7);
        assert_eq!(note_key.as_u64(), 7);

        let filter = RadrootsNostrdbFilterSpec::new().with_kind(1);
        let from_new = RadrootsNostrdbSubscriptionSpec::new(vec![filter.clone()]);
        assert_eq!(from_new.filters(), std::slice::from_ref(&filter));

        let from_single = RadrootsNostrdbSubscriptionSpec::single(filter.clone());
        assert_eq!(from_single.filters(), std::slice::from_ref(&filter));

        let text_notes = RadrootsNostrdbSubscriptionSpec::text_notes(Some(10), Some(123));
        assert_eq!(text_notes.filters().len(), 1);
    }
}
