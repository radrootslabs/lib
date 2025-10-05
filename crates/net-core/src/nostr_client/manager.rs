use std::sync::Arc;
use tokio::runtime::Handle;

use super::inner::Inner;

#[derive(Clone)]
pub struct NostrClientManager {
    pub(super) inner: Arc<Inner>,
}

impl NostrClientManager {
    pub fn new(keys: nostr::Keys, rt: Handle) -> Self {
        let inner = Inner::new(keys, rt);
        let this = Self {
            inner: inner.clone(),
        };
        this.spawn_status_watcher();
        this
    }
}
