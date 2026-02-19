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

    #[error("runtime error: {0}")]
    Runtime(String),
}
