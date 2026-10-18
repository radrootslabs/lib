use super::*;
use futures::{SinkExt, StreamExt};
use nostr::JsonUtil;
use radroots::{Client, NativeConfig, StartMode, inbox::InquireInput};
use radroots_domain::ThreadId;
use radroots_nostr::{
    key::parse_secret_key,
    signing::{LocalSigner, OsNonceSource},
};
use radroots_signing::{
    Signer,
    provider::{ParticipantProvider, ProviderError, SignerCallbacks, SignerHandle},
    signer::BoxFuture,
};
use radroots_storage::event::SourceGeneration;
use radroots_storage::workspace::WorkspaceStore;
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths};
use radroots_transport::policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy};
use radroots_transport_nostr::{
    Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc as channel, oneshot},
    task::JoinSet,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[path = "receipt_workflow_faults.rs"]
mod faults;

struct Callbacks;
impl SignerCallbacks for Callbacks {
    fn quiesce(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async { Ok(()) })
    }
    fn close(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async { Ok(()) })
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
fn route_key(last: u8) -> nostr::Keys {
    let mut bytes = [0; 32];
    bytes[31] = last;
    nostr::Keys::new(nostr::SecretKey::from_slice(&bytes).unwrap())
}
fn advertisement(last: u8, url: &str) -> String {
    let keys = route_key(last);
    nostr::UnsignedEvent {
        id: None,
        pubkey: keys.public_key(),
        created_at: nostr::Timestamp::from_secs(now() - 1),
        kind: nostr::Kind::Custom(10050),
        tags: nostr::Tags::from_list(vec![
            nostr::Tag::parse(["relay", url]).unwrap(),
            nostr::Tag::parse(["client", "ordinary-test"]).unwrap(),
        ]),
        content: String::new(),
    }
    .sign_with_keys(&keys)
    .unwrap()
    .as_json()
}
struct Relay {
    url: String,
    events: channel::UnboundedReceiver<String>,
    count: Arc<AtomicU64>,
    accepting: Arc<AtomicBool>,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}
impl Relay {
    async fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let sources = Arc::new([advertisement(1, &url), advertisement(2, &url)]);
        let count = Arc::new(AtomicU64::new(0));
        let counter = count.clone();
        let accepting = Arc::new(AtomicBool::new(true));
        let acceptance = accepting.clone();
        let (send, events) = channel::unbounded_channel();
        let (stop, mut stopping) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopping => break,
                    accepted = listener.accept() => {
                        let (tcp, _) = accepted.unwrap();
                        let sources = sources.clone(); let send = send.clone(); let count = counter.clone(); let acceptance = acceptance.clone();
                        clients.spawn(async move {
                            let mut socket = accept_async(tcp).await.unwrap();
                            while let Some(Ok(message)) = socket.next().await {
                                let Message::Text(raw) = message else {continue};
                                let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
                                match value[0].as_str().unwrap() {
                                    "REQ" => {
                                        let author = value[2]["authors"][0].as_str().unwrap();
                                        for source in sources.iter() {
                                            let event: serde_json::Value = serde_json::from_str(source).unwrap();
                                            if event["pubkey"].as_str() == Some(author) {
                                                socket.send(Message::Text(format!("[\"EVENT\",{}, {} ]", value[1], source).into())).await.unwrap();
                                            }
                                        }
                                        socket.send(Message::Text(json!(["EOSE", value[1]]).to_string().into())).await.unwrap();
                                    }
                                    "EVENT" => {
                                        let event = raw.strip_prefix("[\"EVENT\",").and_then(|raw| raw.strip_suffix(']')).unwrap().to_owned();
                                        nostr::Event::from_json(&event).unwrap().verify().unwrap();
                                        count.fetch_add(1, Ordering::SeqCst);
                                        send.send(event).unwrap();
                                        socket.send(Message::Text(json!(["OK", value[1]["id"], acceptance.load(Ordering::SeqCst), ""]).to_string().into())).await.unwrap();
                                    }
                                    "CLOSE" => {},
                                    other => panic!("unexpected relay command {other}"),
                                }
                            }
                        });
                    }
                }
            }
            clients.shutdown().await;
        });
        Self {
            url,
            events,
            count,
            accepting,
            stop,
            task,
        }
    }
    fn config(&self) -> Config {
        Config::from_profile(
            RelayProfile::explicit(
                RelayProfileKind::Simulator,
                [
                    RelayEndpoint::new(&self.url, RelayUrlPolicy::Local, RelayAccess::ReadWrite)
                        .unwrap(),
                ],
            )
            .unwrap(),
        )
        .with_timeouts(2_000, 2_000, 2_000)
        .unwrap()
    }
}

#[tokio::test]
async fn ordinary_online_inquiry_runs_t0_t1_sign_install_and_exact_peer_self_delivery_then_reopens()
{
    let mut relay = Relay::new().await;
    let fixture = Fixture::new();
    fixture.clock.live.store(true, Ordering::SeqCst);
    let (store, input, proof, _) = fixture.create_with_expiry(now() + 600).await;
    let cache = fixture.path.parent().unwrap().join("ordinary-cache");
    std::fs::create_dir(&cache).unwrap();
    let options = OpenOptions::new(Paths::from_directory(cache).unwrap(), OpenMode::Create)
        .with_source_generation(SourceGeneration::new([18; 32]).unwrap(), 1)
        .unwrap();
    let policy = SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all());
    let client = Client::open(
        NativeConfig::public_read(options)
            .with_workspace(store.as_ref().clone())
            .with_nostr(relay.config(), policy)
            .unwrap(),
    )
    .await
    .unwrap();
    let scope = client.attach(input.clone(), copy(&proof)).await.unwrap();
    let signer = LocalSigner::new(
        parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
            .unwrap(),
    )
    .unwrap()
    .with_nonce_source(Arc::new(OsNonceSource));
    client
        .attach_signer(SignerHandle::new(Arc::new(signer), Arc::new(Callbacks)))
        .await
        .unwrap();
    client.start(StartMode::Online).await.unwrap();
    assert_eq!(relay.count.load(Ordering::SeqCst), 0);
    let operation = OperationId::from_bytes([201; 32]);
    let not_after = now() + 120;
    let context = || {
        scope
            .command_context_with_correlation(
                operation,
                None,
                None,
                UnixSeconds::new(not_after).unwrap(),
                Some(CorrelationId::new("ordinary-online").unwrap()),
            )
            .unwrap()
    };
    let inquire = || {
        InquireInput::new(
            ThreadId::from_bytes([33; 32]),
            PublicKey::from_bytes(route_key(2).public_key().to_bytes()).unwrap(),
            None,
            "ordinary complete inquiry",
        )
        .unwrap()
    };
    let receipt = client.inbox().inquire(context(), inquire()).await.unwrap();
    assert!(receipt.durable());
    assert!(receipt.business_result().is_some(), "{receipt:?}");
    assert_eq!(relay.count.load(Ordering::SeqCst), 2);
    let mut recipients = Vec::new();
    for _ in 0..2 {
        let raw = tokio::time::timeout(Duration::from_secs(5), relay.events.recv())
            .await
            .unwrap()
            .unwrap();
        let outer = nostr::Event::from_json(raw).unwrap();
        let target = outer
            .tags
            .iter()
            .find(|tag| tag.as_slice()[0] == "p")
            .unwrap()
            .as_slice()[1]
            .clone();
        let key = if target == route_key(1).public_key().to_hex() {
            1
        } else {
            2
        };
        let message = nostr::nips::nip59::extract_rumor(&route_key(key), &outer)
            .await
            .unwrap();
        assert_eq!(message.sender, route_key(1).public_key());
        assert_eq!(message.rumor.content, "ordinary complete inquiry");
        assert_eq!(
            message.rumor.id.unwrap().as_bytes(),
            receipt.business_result().unwrap().id().as_bytes()
        );
        recipients.push(key);
    }
    recipients.sort_unstable();
    assert_eq!(recipients, [1, 2]);
    let snapshot = store
        .operation_snapshot(scope.binding().clone(), operation, None, copy(&proof))
        .await
        .unwrap()
        .unwrap();
    assert!(
        snapshot
            .recipients()
            .iter()
            .all(|recipient| recipient.obtained().is_some())
    );
    assert_eq!(snapshot.publications().len(), 2);
    assert_eq!(snapshot.delivery_attempts().len(), 2);
    assert!(
        snapshot
            .publications()
            .iter()
            .all(|work| snapshot.publication_accepted(*work).unwrap())
    );
    let original = receipt.operation_checkpoint();
    let replay = client.inbox().inquire(context(), inquire()).await.unwrap();
    assert_eq!(replay.operation_checkpoint(), original);
    assert_eq!(relay.count.load(Ordering::SeqCst), 2);
    client.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    let snapshot = reopened
        .operation_snapshot(scope.binding().clone(), operation, None, copy(&proof))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        radroots_execution::OperationReceipt::from_snapshot(&snapshot).operation_checkpoint(),
        original
    );
    reopened.close_workspace().await.unwrap();
    relay.stop.send(()).unwrap();
    relay.task.await.unwrap();
}
