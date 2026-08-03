#[test]
fn approved_module_skeleton_is_public() {
    #[allow(unused_imports)]
    use radroots::{
        client, event, farm, identity, knowledge, listing, signing, storage, sync, trade, transport,
    };
}

#[test]
fn root_exports_only_the_client_boundary() {
    fn assert_client<T: Clone + Send + Sync>() {}
    fn assert_builder<T: Send + Sync>() {}
    fn assert_error<T: std::error::Error + Send + Sync + 'static>() {}

    assert_client::<radroots::Client>();
    assert_builder::<radroots::ClientBuilder>();
    assert_error::<radroots::Error>();
    let result: radroots::Result<()> = Ok(());
    assert!(result.is_ok());
}

#[test]
fn canonical_domain_paths_compile() {
    #[allow(unused_imports)]
    use radroots::{
        event::{Event, EventDraft, EventId},
        farm::{Farm, FarmPublicLocation, Plan as FarmPlan, PrepareRequest as FarmRequest},
        identity::{AccountId, PublicKey, Username},
        listing::{EditV1, OperationalListing, Plan as ListingPlan},
        signing::{Mode, Provider},
        storage::{IntegrityStatus, Operations as StorageOperations, Status},
        trade::{Money, MutationV1, Plan as TradePlan, Projection},
        transport::{Profile, SatisfactionPolicy, Target, TargetSet, TransportId},
    };
}

#[cfg(feature = "client")]
#[test]
fn ordinary_memory_and_local_only_construction_is_inert() {
    let client = radroots::client::memory().build().expect("memory client");
    assert!(!client.is_closed());
    assert!(radroots::client::local_only().is_local_only());
}

#[cfg(feature = "knowledge")]
#[test]
fn canonical_knowledge_paths_compile() {
    #[allow(unused_imports)]
    use radroots::knowledge::{KnowledgeClaim, WikiArticle, normalize_wiki_d_tag};
}
