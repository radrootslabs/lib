#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSdkEventReference {
    pub event_id: String,
    pub pubkey: String,
    pub kind: u32,
    pub created_at: u32,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSdkLocalMutationReceipt {
    pub event: RadrootsSdkEventReference,
    pub stored: bool,
    pub queued: bool,
    pub outbox_event_id: Option<i64>,
    pub idempotency_key_digest_prefix: Option<String>,
}
