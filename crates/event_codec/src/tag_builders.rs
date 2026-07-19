#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg(feature = "knowledge")]
use radroots_event::knowledge::{
    RadrootsContributionAttestation, RadrootsEvidenceBounty, RadrootsKnowledgeChangeProposal,
    RadrootsKnowledgeClaim, RadrootsKnowledgeFieldReport, RadrootsKnowledgeRelation,
    RadrootsKnowledgeReview, RadrootsKnowledgeSource, RadrootsWikiArticle,
    RadrootsWikiMergeRequest, RadrootsWikiRedirect,
};
use radroots_event::{
    app_data::RadrootsAppData, coop::RadrootsCoop, document::RadrootsDocument, farm::RadrootsFarm,
    follow::RadrootsFollow, geochat::RadrootsGeoChat, gift_wrap::RadrootsGiftWrap,
    job_feedback::RadrootsJobFeedback, job_request::RadrootsJobRequest,
    job_result::RadrootsJobResult, list::RadrootsList, list_set::RadrootsListSet,
    message::RadrootsMessage, message_file::RadrootsMessageFile,
    operational_listing::RadrootsOperationalListing, plot::RadrootsPlot,
    reaction::RadrootsReaction, resource_area::RadrootsResourceArea,
    resource_cap::RadrootsResourceHarvestCap, seal::RadrootsSeal,
};

use crate::app_data::encode::app_data_build_tags;
use crate::coop::encode::coop_build_tags;
use crate::document::encode::document_build_tags;
use crate::error::EventEncodeError;
use crate::farm::encode::farm_build_tags;
use crate::follow::encode::follow_build_tags;
use crate::geochat::encode::geochat_build_tags;
use crate::gift_wrap::encode::gift_wrap_build_tags;
use crate::job::encode::JobEncodeError;
use crate::job::feedback::encode::job_feedback_build_tags;
use crate::job::request::encode::job_request_build_tags;
use crate::job::result::encode::job_result_build_tags;
#[cfg(feature = "knowledge")]
use crate::knowledge::encode::{
    contribution_attestation_build_tags, evidence_bounty_build_tags,
    knowledge_change_proposal_build_tags, knowledge_claim_build_tags,
    knowledge_field_report_build_tags, knowledge_relation_build_tags, knowledge_review_build_tags,
    knowledge_source_build_tags, wiki_article_build_tags, wiki_merge_request_build_tags,
    wiki_redirect_build_tags,
};
use crate::list::encode::list_build_tags;
use crate::list_set::encode::list_set_build_tags;
use crate::message::encode::message_build_tags;
use crate::message_file::encode::message_file_build_tags;
use crate::operational_listing::tags::operational_listing_tags;
use crate::plot::encode::plot_build_tags;
use crate::reaction::encode::reaction_build_tags;
use crate::resource_area::encode::resource_area_build_tags;
use crate::resource_cap::encode::resource_harvest_cap_build_tags;
use crate::seal::encode::seal_build_tags;

pub trait RadrootsEventTagBuilder {
    type Error;
    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error>;
}

impl RadrootsEventTagBuilder for RadrootsOperationalListing {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        operational_listing_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsAppData {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        app_data_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsReaction {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        reaction_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsMessage {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        message_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsMessageFile {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        message_file_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsGeoChat {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        geochat_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsFollow {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        follow_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsFarm {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        farm_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsResourceArea {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        resource_area_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsResourceHarvestCap {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        resource_harvest_cap_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsCoop {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        coop_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsDocument {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        document_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsList {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        list_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsListSet {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        list_set_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsPlot {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        plot_build_tags(self)
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

impl RadrootsEventTagBuilder for RadrootsSeal {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        seal_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for RadrootsGiftWrap {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        gift_wrap_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsWikiArticle {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        wiki_article_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsWikiRedirect {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        wiki_redirect_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsWikiMergeRequest {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        wiki_merge_request_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsKnowledgeSource {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_source_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsKnowledgeClaim {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_claim_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsKnowledgeRelation {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_relation_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsKnowledgeReview {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_review_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsKnowledgeFieldReport {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_field_report_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsEvidenceBounty {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        evidence_bounty_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsKnowledgeChangeProposal {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_change_proposal_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for RadrootsContributionAttestation {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        contribution_attestation_build_tags(self)
    }
}
