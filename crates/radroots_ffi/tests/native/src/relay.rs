use crate::{FixtureError, FixturePolicy, now};
use futures::{SinkExt, StreamExt};
use nostr::JsonUtil;
use radroots_transport::{
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    target::Target,
};
use radroots_transport_nostr::{
    Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
};
use serde_json::{Value, value::RawValue};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tokio::{
    net::TcpListener,
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};
use tokio_tungstenite::{
    accept_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

pub(crate) struct Relay {
    pub url: String,
    pub policy: SatisfactionPolicy,
    pub events: Arc<Mutex<Vec<String>>>,
    pub reads: Arc<AtomicU64>,
    pub accepting: Arc<AtomicBool>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    task: Mutex<Option<JoinHandle<()>>>,
}
impl Relay {
    pub async fn new(selected: FixturePolicy) -> Result<Self, FixtureError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| FixtureError::Unavailable)?;
        let url = format!(
            "ws://{}",
            listener
                .local_addr()
                .map_err(|_| FixtureError::Unavailable)?
        );
        let sources = Arc::new([advertisement(1, &url)?, advertisement(2, &url)?]);
        let policy = match selected {
            FixturePolicy::All => TargetPolicy::all(),
            FixturePolicy::Any => TargetPolicy::any(),
            FixturePolicy::Quorum => {
                TargetPolicy::quorum(1).map_err(|_| FixtureError::Unavailable)?
            }
            FixturePolicy::Required => TargetPolicy::required(vec![
                Target::nostr_relay(&url)
                    .map_err(|_| FixtureError::Unavailable)?
                    .fingerprint()
                    .clone(),
            ])
            .map_err(|_| FixtureError::Unavailable)?,
        };
        let policy = SatisfactionPolicy::new(SatisfactionClass::Accepted, policy);
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = events.clone();
        let reads = Arc::new(AtomicU64::new(0));
        let requests = reads.clone();
        let accepting = Arc::new(AtomicBool::new(true));
        let accepted = accepting.clone();
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    _ = clients.join_next(), if !clients.is_empty() => {},
                    connection = listener.accept() => {
                        let Ok((tcp, _)) = connection else { break };
                        if clients.len() >= 16 { continue; }
                        let sources = sources.clone(); let events = received.clone(); let reads = requests.clone(); let accepting = accepted.clone();
                        clients.spawn(async move {
                            let config = WebSocketConfig::default().max_message_size(Some(65_600)).max_frame_size(Some(65_600));
                            let Ok(mut socket) = accept_async_with_config(tcp, Some(config)).await else { return };
                            while let Some(Ok(message)) = socket.next().await {
                                let Message::Text(raw) = message else { continue };
                                let Ok(frame) = serde_json::from_str::<Vec<&RawValue>>(&raw) else { break };
                                match frame.first().map(|v| v.get()) {
                                    Some("\"REQ\"") if frame.len() >= 3 => {
                                        reads.fetch_add(1, Ordering::SeqCst);
                                        let Ok(filter) = serde_json::from_str::<Value>(frame[2].get()) else { break };
                                        let author = filter["authors"][0].as_str();
                                        for source in sources.iter() {
                                            let event: Value = serde_json::from_str(source).expect("synthetic event");
                                            if event["pubkey"].as_str() == author && socket.send(Message::Text(format!("[\"EVENT\",{},{}]", frame[1].get(), source).into())).await.is_err() { return; }
                                        }
                                        if socket.send(Message::Text(format!("[\"EOSE\",{}]", frame[1].get()).into())).await.is_err() { return; }
                                    }
                                    Some("\"EVENT\"") if frame.len() == 2 => {
                                        let original = frame[1].get();
                                        let Ok(event) = nostr::Event::from_json(original) else { break };
                                        if event.verify().is_err() { break; }
                                        {
                                            let mut retained = events.lock().expect("fixture events");
                                            let bytes: usize = retained.iter().map(String::len).sum();
                                            if bytes + original.len() > 4 * 1024 * 1024 { return; }
                                            retained.push(original.to_owned());
                                        }
                                        let accepted = accepting.load(Ordering::SeqCst);
                                        let reason = if accepted { "" } else { "blocked: measured loopback rejection" };
                                        let ack = serde_json::json!(["OK", event.id.to_hex(), accepted, reason]);
                                        if socket.send(Message::Text(ack.to_string().into())).await.is_err() { return; }
                                    }
                                    _ => {}
                                }
                            }
                        });
                    }
                }
            }
            clients.abort_all();
            while clients.join_next().await.is_some() {}
        });
        Ok(Self {
            url,
            policy,
            events,
            reads,
            accepting,
            stop: Mutex::new(Some(stop)),
            task: Mutex::new(Some(task)),
        })
    }
    pub fn config(&self) -> Result<Config, FixtureError> {
        let endpoint = RelayEndpoint::new(&self.url, RelayUrlPolicy::Local, RelayAccess::ReadWrite)
            .map_err(|_| FixtureError::Unavailable)?;
        let profile = RelayProfile::explicit(RelayProfileKind::Simulator, [endpoint])
            .map_err(|_| FixtureError::Unavailable)?;
        Config::from_profile(profile)
            .with_timeouts(2000, 2000, 2000)
            .map_err(|_| FixtureError::Unavailable)
    }
    pub async fn stop(&self) -> Result<(), FixtureError> {
        if let Some(stop) = self
            .stop
            .lock()
            .map_err(|_| FixtureError::Unavailable)?
            .take()
        {
            let _ = stop.send(());
        }
        let task = self
            .task
            .lock()
            .map_err(|_| FixtureError::Unavailable)?
            .take();
        if let Some(task) = task {
            task.await.map_err(|_| FixtureError::Unavailable)?;
        }
        Ok(())
    }
}
fn advertisement(last: u8, url: &str) -> Result<String, FixtureError> {
    let mut scalar = [0; 32];
    scalar[31] = last;
    let keys = nostr::Keys::new(
        nostr::SecretKey::from_slice(&scalar).map_err(|_| FixtureError::Unavailable)?,
    );
    nostr::UnsignedEvent {
        id: None,
        pubkey: keys.public_key(),
        created_at: nostr::Timestamp::from_secs(now()?.saturating_sub(1)),
        kind: nostr::Kind::Custom(10050),
        tags: nostr::Tags::from_list(vec![
            nostr::Tag::parse(["relay", url]).map_err(|_| FixtureError::Unavailable)?,
        ]),
        content: String::new(),
    }
    .sign_with_keys(&keys)
    .map(|v| v.as_json())
    .map_err(|_| FixtureError::Unavailable)
}
