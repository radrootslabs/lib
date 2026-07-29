#[cfg(not(feature = "std"))]
extern crate alloc;

use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
#[cfg(not(feature = "std"))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};
use radroots_event::{
    social::job_feedback::JobFeedback, social::job_request::JobRequest,
    social::job_result::JobResult,
};

use crate::job::{
    error::JobParseError,
    feedback::decode::{
        data_from_event as feedback_metadata_from_event,
        parsed_from_event as feedback_index_from_event,
    },
    request::decode::{
        data_from_event as request_metadata_from_event,
        parsed_from_event as request_index_from_event,
    },
    result::decode::{
        data_from_event as result_metadata_from_event, parsed_from_event as result_index_from_event,
    },
};

pub trait JobEventLike {
    fn raw_id(&self) -> String;
    fn raw_author(&self) -> String;
    fn raw_published_at(&self) -> u64;
    fn raw_kind(&self) -> u32;
    fn raw_content(&self) -> String;
    fn raw_tags(&self) -> Vec<Vec<String>>;
    fn raw_sig(&self) -> String;

    fn to_job_request_metadata(&self) -> Result<RadrootsParsedData<JobRequest>, JobParseError> {
        request_metadata_from_event(
            self.raw_id(),
            self.raw_author(),
            self.raw_published_at(),
            self.raw_kind(),
            self.raw_tags(),
        )
    }

    fn to_job_request_event_index(&self) -> Result<RadrootsParsedEvent<JobRequest>, JobParseError> {
        request_index_from_event(
            self.raw_id(),
            self.raw_author(),
            self.raw_published_at(),
            self.raw_kind(),
            self.raw_content(),
            self.raw_tags(),
            self.raw_sig(),
        )
    }

    fn to_job_result_metadata(&self) -> Result<RadrootsParsedData<JobResult>, JobParseError> {
        result_metadata_from_event(
            self.raw_id(),
            self.raw_author(),
            self.raw_published_at(),
            self.raw_kind(),
            self.raw_content(),
            self.raw_tags(),
        )
    }

    fn to_job_result_event_index(&self) -> Result<RadrootsParsedEvent<JobResult>, JobParseError> {
        result_index_from_event(
            self.raw_id(),
            self.raw_author(),
            self.raw_published_at(),
            self.raw_kind(),
            self.raw_content(),
            self.raw_tags(),
            self.raw_sig(),
        )
    }

    fn to_job_feedback_metadata(&self) -> Result<RadrootsParsedData<JobFeedback>, JobParseError> {
        feedback_metadata_from_event(
            self.raw_id(),
            self.raw_author(),
            self.raw_published_at(),
            self.raw_kind(),
            self.raw_content(),
            self.raw_tags(),
        )
    }

    fn to_job_feedback_event_index(
        &self,
    ) -> Result<RadrootsParsedEvent<JobFeedback>, JobParseError> {
        feedback_index_from_event(
            self.raw_id(),
            self.raw_author(),
            self.raw_published_at(),
            self.raw_kind(),
            self.raw_content(),
            self.raw_tags(),
            self.raw_sig(),
        )
    }
}

pub trait JobEventBorrow<'a> {
    fn raw_id(&'a self) -> String;
    fn raw_author(&'a self) -> String;
    fn raw_content(&'a self) -> &'a str;
    fn raw_kind(&'a self) -> u32;
}

#[derive(Clone)]
pub struct BorrowedEventAdapter<'a, E: JobEventBorrow<'a>> {
    inner: &'a E,
    published_at: u64,
    tags: &'a [Vec<String>],
    sig: String,
}

impl<'a, E: JobEventBorrow<'a>> BorrowedEventAdapter<'a, E> {
    pub fn new(
        inner: &'a E,
        published_at: u64,
        tags: &'a [Vec<String>],
        sig: impl Into<String>,
    ) -> Self {
        Self {
            inner,
            published_at,
            tags,
            sig: sig.into(),
        }
    }
}

impl<'a, E: JobEventBorrow<'a>> JobEventLike for BorrowedEventAdapter<'a, E> {
    #[inline]
    fn raw_id(&self) -> String {
        self.inner.raw_id()
    }
    #[inline]
    fn raw_author(&self) -> String {
        self.inner.raw_author()
    }
    #[inline]
    fn raw_published_at(&self) -> u64 {
        self.published_at
    }
    #[inline]
    fn raw_kind(&self) -> u32 {
        self.inner.raw_kind()
    }
    #[inline]
    fn raw_content(&self) -> String {
        self.inner.raw_content().to_owned()
    }
    #[inline]
    fn raw_tags(&self) -> Vec<Vec<String>> {
        self.tags.to_vec()
    }
    #[inline]
    fn raw_sig(&self) -> String {
        self.sig.clone()
    }
}

impl<'a> JobEventBorrow<'a> for radroots_event::envelope::EventEnvelope {
    #[inline]
    fn raw_id(&'a self) -> String {
        self.id_hex()
    }
    #[inline]
    fn raw_author(&'a self) -> String {
        self.author().to_hex()
    }
    #[inline]
    fn raw_content(&'a self) -> &'a str {
        self.content()
    }
    #[inline]
    fn raw_kind(&'a self) -> u32 {
        self.kind_u32()
    }
}
