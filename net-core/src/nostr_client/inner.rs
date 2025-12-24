use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use radroots_events::post::RadrootsPostEventMetadata;
use radroots_nostr::prelude::{
    RadrootsNostrClient,
    RadrootsNostrKeys,
    RadrootsNostrMonitor,
    RadrootsNostrRelayStatus,
    RadrootsNostrRelayUrl,
};
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

pub(super) struct Inner {
    pub client: RadrootsNostrClient,
    pub relays: Arc<Mutex<Vec<String>>>,
    pub statuses: Arc<Mutex<HashMap<RadrootsNostrRelayUrl, RadrootsNostrRelayStatus>>>,
    pub last_error: Arc<Mutex<Option<String>>>,
    pub rt: Handle,
    pub post_events_tx: broadcast::Sender<RadrootsPostEventMetadata>,
    pub post_events_stream: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Inner {
    pub fn new(keys: RadrootsNostrKeys, rt: Handle) -> Arc<Self> {
        let monitor = RadrootsNostrMonitor::new(2048);
        let client = RadrootsNostrClient::new_with_monitor(keys, monitor);
        let (tx, _) = broadcast::channel(2048);

        Arc::new(Self {
            client,
            relays: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(HashMap::new())),
            last_error: Arc::new(Mutex::new(None)),
            rt,
            post_events_tx: tx,
            post_events_stream: Arc::new(Mutex::new(None)),
        })
    }
}
