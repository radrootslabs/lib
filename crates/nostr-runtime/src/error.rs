use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrRuntimeError {
    #[error("runtime not started")]
    RuntimeNotStarted,

    #[error("runtime already started")]
    RuntimeAlreadyStarted,

    #[error("runtime shutdown")]
    RuntimeShutdown,

    #[error("missing required runtime configuration: {0}")]
    MissingConfig(&'static str),

    #[error("invalid runtime configuration: {0}")]
    InvalidConfig(&'static str),

    #[error("nostr client error: {0}")]
    Client(String),

    #[error("subscription not found: {0}")]
    SubscriptionNotFound(String),

    #[error("runtime error: {0}")]
    Runtime(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_variants_render_messages() {
        assert_eq!(
            RadrootsNostrRuntimeError::RuntimeNotStarted.to_string(),
            "runtime not started"
        );
        assert_eq!(
            RadrootsNostrRuntimeError::RuntimeAlreadyStarted.to_string(),
            "runtime already started"
        );
        assert_eq!(
            RadrootsNostrRuntimeError::RuntimeShutdown.to_string(),
            "runtime shutdown"
        );
        assert_eq!(
            RadrootsNostrRuntimeError::MissingConfig("keys").to_string(),
            "missing required runtime configuration: keys"
        );
        assert_eq!(
            RadrootsNostrRuntimeError::InvalidConfig("queue_capacity").to_string(),
            "invalid runtime configuration: queue_capacity"
        );
        assert_eq!(
            RadrootsNostrRuntimeError::Client("client failure".into()).to_string(),
            "nostr client error: client failure"
        );
        assert_eq!(
            RadrootsNostrRuntimeError::SubscriptionNotFound("sub-1".into()).to_string(),
            "subscription not found: sub-1"
        );
        assert_eq!(
            RadrootsNostrRuntimeError::Runtime("runtime failure".into()).to_string(),
            "runtime error: runtime failure"
        );
    }
}
