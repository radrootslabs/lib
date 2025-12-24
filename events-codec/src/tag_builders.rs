#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use core::convert::Infallible;

use radroots_events::{
    comment::RadrootsComment,
    follow::RadrootsFollow,
    job_feedback::RadrootsJobFeedback,
    job_request::RadrootsJobRequest,
    job_result::RadrootsJobResult,
    listing::RadrootsListing,
    post::RadrootsPost,
    profile::RadrootsProfile,
    reaction::RadrootsReaction,
};

use crate::comment::encode::comment_build_tags;
use crate::error::EventEncodeError;
use crate::follow::encode::follow_build_tags;
use crate::job::encode::JobEncodeError;
use crate::job::feedback::encode::job_feedback_build_tags;
use crate::job::request::encode::job_request_build_tags;
use crate::job::result::encode::job_result_build_tags;
use crate::listing::tags::listing_tags;
use crate::reaction::encode::reaction_build_tags;

pub trait RadrootsEventTagBuilder {
    type Error;
    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error>;
}

impl RadrootsEventTagBuilder for RadrootsListing {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        listing_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsComment {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        comment_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsReaction {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        reaction_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsFollow {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        follow_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsJobRequest {
    type Error = JobEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        if self.encrypted && self.providers.is_empty() {
            return Err(JobEncodeError::MissingProvidersForEncrypted);
        }
        Ok(job_request_build_tags(self))
    }
}

impl RadrootsEventTagBuilder for RadrootsJobResult {
    type Error = JobEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        Ok(job_result_build_tags(self))
    }
}

impl RadrootsEventTagBuilder for RadrootsJobFeedback {
    type Error = JobEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        Ok(job_feedback_build_tags(self))
    }
}

impl RadrootsEventTagBuilder for RadrootsProfile {
    type Error = Infallible;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        Ok(Vec::new())
    }
}

impl RadrootsEventTagBuilder for RadrootsPost {
    type Error = Infallible;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        Ok(Vec::new())
    }
}
