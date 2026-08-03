//! Curated knowledge-domain entry points.

#[cfg(any(feature = "knowledge", feature = "full"))]
pub use radroots_event::knowledge::{
    AddressableRef, KnowledgeClaim, KnowledgeFieldReport, KnowledgeRelation, KnowledgeReview,
    KnowledgeSource, KnowledgeValidationError, WikiArticle, WikiDTagError, WikiMergeRequest,
    WikiRedirect, normalize_wiki_d_tag, validate_wiki_d_tag,
};
