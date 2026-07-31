use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nostr::Keys as RadrootsNostrKeys;
use radroots_event_codec::{parsed::RadrootsParsedData, post::decode::LegacyPost};
use radroots_transport_nostr::{
    RadrootsNostrClient, RadrootsNostrClientKey, RadrootsNostrMonitor, RadrootsNostrRelayStatus,
    RelayUrl,
};
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

pub(super) struct Inner {
    pub client: RadrootsNostrClient,
    pub keys: RadrootsNostrKeys,
    pub relays: Arc<Mutex<Vec<String>>>,
    pub statuses: Arc<Mutex<HashMap<RelayUrl, RadrootsNostrRelayStatus>>>,
    pub last_error: Arc<Mutex<Option<String>>>,
    pub rt: Handle,
    pub post_events_tx: broadcast::Sender<RadrootsParsedData<LegacyPost>>,
    pub post_events_stream: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Inner {
    pub fn new(keys: RadrootsNostrKeys, rt: Handle) -> Arc<Self> {
        let monitor = RadrootsNostrMonitor::new(2048);
        let client_key =
            RadrootsNostrClientKey::from_secret_key_bytes(keys.secret_key().to_secret_bytes())
                .expect("an existing Nostr key remains valid at the transport boundary");
        let client = RadrootsNostrClient::new_with_monitor(client_key, monitor);
        let (tx, _) = broadcast::channel(2048);

        Arc::new(Self {
            client,
            keys,
            relays: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(HashMap::new())),
            last_error: Arc::new(Mutex::new(None)),
            rt,
            post_events_tx: tx,
            post_events_stream: Arc::new(Mutex::new(None)),
        })
    }
}
