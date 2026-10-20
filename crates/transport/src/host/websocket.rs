//! Byte I/O for a shared protocol session. Hosts own sockets; protocol adapters
//! own sessions, destination authority, ingress budgets and exact publication.

use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use core::{fmt, future::Future, net::IpAddr, pin::Pin, task::Context, time::Duration};

/// Owned channel futures may cross threads. Browser implementations keep DOM
/// objects in their origin-thread actor and return only safe message handles.
/// This bound does not apply to application or browser session futures.
pub type IoFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Bounded failures retain no provider errors, destinations or wire content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// Explicit destination or ingress authority refused the effect.
    Policy,
    /// DNS, TCP, TLS or the WebSocket handshake could not complete.
    Unavailable,
    /// The original connection deadline elapsed.
    Timeout,
    /// The connection no longer admits I/O.
    Closed,
    /// The peer or caller supplied an invalid or oversized frame.
    Protocol,
    /// This host cannot perform the requested operation.
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Policy => "socket authority denied",
            Self::Unavailable => "socket connection unavailable",
            Self::Timeout => "socket connection deadline elapsed",
            Self::Closed => "socket connection closed",
            Self::Protocol => "socket frame refused",
            Self::Unsupported => "socket operation unsupported",
        })
    }
}

impl std::error::Error for Error {}

/// Original close code and reason. Debug formatting omits peer text.
#[derive(Clone, Eq, PartialEq)]
pub struct CloseFrame {
    /// WebSocket close code.
    pub code: u16,
    /// Uninterpreted peer close reason.
    pub reason: String,
}

impl fmt::Debug for CloseFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CloseFrame")
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}

/// Original message bytes; the host never parses or reserializes protocol data.
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum Frame {
    /// UTF-8 message bytes, including original whitespace and member order.
    Text(String),
    /// Original binary message bytes.
    Binary(Vec<u8>),
    /// Native control frame; browsers do not expose this operation.
    Ping(Vec<u8>),
    /// Native control frame; browsers do not expose this operation.
    Pong(Vec<u8>),
    /// Peer close information, when available.
    Close(Option<CloseFrame>),
}

impl fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Text(_) => "Frame::Text([redacted])",
            Self::Binary(_) => "Frame::Binary([redacted])",
            Self::Ping(_) => "Frame::Ping([redacted])",
            Self::Pong(_) => "Frame::Pong([redacted])",
            Self::Close(_) => "Frame::Close([redacted])",
        })
    }
}

/// One connection's original destination and revocable ingress authority.
/// Implementations must not create fresh grants or reset a shared budget.
pub trait Admission: Send + Sync {
    /// Checks each DNS result before any address is connected.
    fn permits_address(&self, address: IpAddr) -> bool;
    /// Checks read authority and registers the current reader for revocation.
    fn admitted_read(&self, context: &Context<'_>) -> bool;
    /// Checks write authority and registers the writer for revocation.
    fn admitted_write(&self, context: &Context<'_>) -> bool;
    /// Charges actual bytes, frame starts and data-message starts before decode.
    /// A refusal is sticky within this connection's original shared budget.
    fn charge(&self, bytes: usize, frames: usize, data_messages: usize) -> bool;
}

/// A concrete host connection. The shared protocol adapter owns serialization,
/// session membership, reconnect policy and acknowledgement correlation.
pub trait Socket: Send + Sync {
    /// Sends the original frame under the connection's frozen limits.
    fn send(&self, frame: Frame) -> IoFuture<'_, Result<(), Error>>;
    /// Receives one frame, or confirms the end of this connection's stream.
    fn receive(&self) -> IoFuture<'_, Result<Option<Frame>, Error>>;
    /// Closes this original connection; it never switches to a replacement.
    fn close(&self) -> IoFuture<'_, Result<(), Error>>;
}

/// Supplies concrete I/O to one existing protocol-session owner. Construction
/// and unpolled connection futures must be inert. A failed or cancelled attempt
/// must dispose its own socket and callbacks without authorizing a retry.
pub trait Connector: fmt::Debug + Send + Sync {
    /// Whether the host exposes native ping/pong control frames.
    fn supports_ping(&self) -> bool;
    /// Revalidates the endpoint and every resolved address before I/O. Native
    /// hosts meter plaintext before framing; browser hosts must describe their
    /// actual platform admission boundary rather than claiming native controls.
    fn connect<'a>(
        &'a self,
        endpoint: &'a str,
        timeout: Duration,
        max_message_bytes: core::num::NonZeroUsize,
        admission: Arc<dyn Admission>,
    ) -> IoFuture<'a, Result<Arc<dyn Socket>, Error>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn frame_diagnostics_never_retain_or_print_wire_credentials_or_peer_text() {
        let secret = "private_wire_value_that_must_not_appear";
        let close = CloseFrame {
            code: 1000,
            reason: secret.into(),
        };
        assert!(!format!("{close:?}").contains(secret));
        for frame in [
            Frame::Text(secret.into()),
            Frame::Binary(secret.as_bytes().into()),
            Frame::Ping(secret.as_bytes().into()),
            Frame::Pong(secret.as_bytes().into()),
            Frame::Close(Some(close)),
            Frame::Close(None),
        ] {
            let debug = format!("{frame:?}");
            assert!(!debug.contains(secret));
            assert!(debug.contains("[redacted]"));
            assert!(debug.len() < 64);
        }
    }

    #[test]
    fn io_failures_have_bounded_diagnostics_and_no_provider_error_chain() {
        use std::error::Error as _;
        for error in [
            Error::Policy,
            Error::Unavailable,
            Error::Timeout,
            Error::Closed,
            Error::Protocol,
            Error::Unsupported,
        ] {
            assert!(error.source().is_none());
            let text = format!("{error}");
            assert!(!text.is_empty() && text.len() < 64);
        }
    }
}
