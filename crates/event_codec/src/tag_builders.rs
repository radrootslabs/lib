#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg(feature = "knowledge")]
use radroots_event::knowledge::{
    ContributionAttestation, EvidenceBounty, KnowledgeChangeProposal, KnowledgeClaim,
    KnowledgeFieldReport, KnowledgeRelation, KnowledgeReview, KnowledgeSource, WikiArticle,
    WikiMergeRequest, WikiRedirect,
};
use radroots_event::{
    farm::Farm, farm::coop::Coop, farm::plot::Plot, farm::resource_area::ResourceArea,
    farm::resource_cap::ResourceHarvestCap, listing::operational::OperationalListing,
    post::document::Document, post::reaction::Reaction, social::app_data::AppData,
    social::follow::Follow, social::geochat::GeoChat, social::gift_wrap::GiftWrap,
    social::job_feedback::JobFeedback, social::job_request::JobRequest,
    social::job_result::JobResult, social::list::List, social::list_set::ListSet,
    social::message::Message, social::message_file::MessageFile, social::seal::Seal,
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

impl RadrootsEventTagBuilder for OperationalListing {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        operational_listing_tags(self)
    }
}

impl RadrootsEventTagBuilder for AppData {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        app_data_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for Reaction {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        reaction_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for Message {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        message_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for MessageFile {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        message_file_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for GeoChat {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        geochat_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for Follow {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        follow_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for Farm {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        farm_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for ResourceArea {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        resource_area_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for ResourceHarvestCap {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        resource_harvest_cap_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for Coop {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        coop_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for Document {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        document_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for List {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        list_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for ListSet {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        list_set_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for Plot {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        plot_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for JobRequest {
    type Error = JobEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        if self.encrypted && self.providers.is_empty() {
            return Err(JobEncodeError::MissingProvidersForEncrypted);
        }
        Ok(job_request_build_tags(self))
    }
}

impl RadrootsEventTagBuilder for JobResult {
    type Error = JobEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        Ok(job_result_build_tags(self))
    }
}

impl RadrootsEventTagBuilder for JobFeedback {
    type Error = JobEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        Ok(job_feedback_build_tags(self))
    }
}

impl RadrootsEventTagBuilder for Seal {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        seal_build_tags(self)
    }
}

impl RadrootsEventTagBuilder for GiftWrap {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        gift_wrap_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for WikiArticle {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        wiki_article_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for WikiRedirect {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        wiki_redirect_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for WikiMergeRequest {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        wiki_merge_request_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for KnowledgeSource {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_source_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for KnowledgeClaim {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_claim_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for KnowledgeRelation {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_relation_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for KnowledgeReview {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_review_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for KnowledgeFieldReport {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_field_report_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for EvidenceBounty {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        evidence_bounty_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for KnowledgeChangeProposal {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        knowledge_change_proposal_build_tags(self)
    }
}

#[cfg(feature = "knowledge")]
impl RadrootsEventTagBuilder for ContributionAttestation {
    type Error = EventEncodeError;

    fn build_tags(&self) -> Result<Vec<Vec<String>>, Self::Error> {
        contribution_attestation_build_tags(self)
    }
}
