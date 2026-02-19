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
        match std::pin::Pin::new(&mut self.inner).poll_next(cx) {
            std::task::Poll::Ready(Some(note_keys)) => std::task::Poll::Ready(Some(
                note_keys
                    .into_iter()
                    .map(|note_key| RadrootsNostrNdbNoteKey::new(note_key.as_u64()))
                    .collect(),
            )),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
