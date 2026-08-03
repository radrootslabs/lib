//! Client construction and lifecycle.

/// Cloneable handle to a composed Radroots client.
///
/// Construction is intentionally introduced by the ordered builder
/// composition step; this type establishes the durable root identity only.
#[derive(Clone, Debug)]
pub struct Client {
    _private: (),
}

/// Explicit composition boundary for a [`Client`].
///
/// The lower capability fields and construction methods are added by the
/// ordered composition step.
#[derive(Debug)]
pub struct ClientBuilder {
    _private: (),
}
