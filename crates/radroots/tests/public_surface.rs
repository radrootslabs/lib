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
