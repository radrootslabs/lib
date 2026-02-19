use alloc::string::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsNostrSubscriptionPolicy {
    Streaming,
    OneShotOnEose,
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrSubscriptionSpec {
    pub name: Option<String>,
    pub policy: RadrootsNostrSubscriptionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RadrootsNostrSubscriptionHandle {
    pub id: String,
}

#[derive(Debug, Clone)]
pub enum RadrootsNostrRuntimeEvent {
    SubscriptionOpened { id: String },
    SubscriptionClosed { id: String },
    Note { id: String, relay: String },
    Notice { relay: String, message: String },
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsNostrTrafficLight {
    Red,
    Yellow,
    Green,
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrConnectionSnapshot {
    pub light: RadrootsNostrTrafficLight,
    pub connected: usize,
    pub connecting: usize,
    pub last_error: Option<String>,
}
