use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nostr_sdk::prelude::*;
use tokio::runtime::Handle;

pub(super) struct Inner {
    pub client: Client,
    pub relays: Arc<Mutex<Vec<String>>>,
    pub statuses: Arc<Mutex<HashMap<RelayUrl, RelayStatus>>>,
    pub last_error: Arc<Mutex<Option<String>>>,
    pub rt: Handle,
}

impl Inner {
    pub fn new(keys: nostr::Keys, rt: Handle) -> Arc<Self> {
        let monitor = Monitor::new(2048);
        let client = Client::builder().signer(keys).monitor(monitor).build();

        Arc::new(Self {
            client,
            relays: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(HashMap::new())),
            last_error: Arc::new(Mutex::new(None)),
            rt,
        })
    }
}
