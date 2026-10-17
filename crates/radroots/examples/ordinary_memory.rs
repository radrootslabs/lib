//! Constructs the ordinary, safe, local-only Radroots front door.

fn main() -> radroots::Result<()> {
    futures::executor::block_on(async {
        let client = radroots::Client::preview();
        assert_eq!(
            client.state(),
            radroots_execution::session::SessionState::Ready
        );
        client.start(radroots::StartMode::LocalOnly).await?;
        client.close().await?;
        Ok(())
    })
}
