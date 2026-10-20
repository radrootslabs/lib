//! Fixed-memory admission of decrypted ingress before WebSocket decoding.

use radroots_transport::host::websocket::Admission;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

const MAX_UPGRADE_BYTES: usize = 16 * 1024;
const READ_CHUNK_BYTES: usize = 4096;

/// Counts HTTP response and frame overhead conservatively in the same raw-byte
/// allowance as payloads. No message or payload is retained by this layer.
pub(crate) struct MeteredIo<S> {
    inner: S,
    ingress: Arc<dyn Admission>,
    parser: WireParser,
    failed: bool,
}

impl<S> MeteredIo<S> {
    pub(crate) fn new(inner: S, ingress: Arc<dyn Admission>, max_message_bytes: usize) -> Self {
        Self {
            inner,
            ingress,
            parser: WireParser::new(max_message_bytes),
            failed: false,
        }
    }

    fn admitted(&self, context: &Context<'_>) -> io::Result<()> {
        if self.failed || !self.ingress.admitted_read(context) {
            Err(denied())
        } else {
            Ok(())
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for MeteredIo<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if let Err(error) = this.admitted(context) {
            return Poll::Ready(Err(error));
        }
        if output.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let mut scratch = [0; READ_CHUNK_BYTES];
        let length = output.remaining().min(scratch.len());
        let mut input = ReadBuf::new(&mut scratch[..length]);
        match Pin::new(&mut this.inner).poll_read(context, &mut input) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => {
                this.failed = true;
                Poll::Ready(Err(error))
            }
            Poll::Ready(Ok(())) => {
                let bytes = input.filled();
                let result = if !this.ingress.charge(bytes.len(), 0, 0) {
                    Err(denied())
                } else if bytes.is_empty() {
                    this.parser.eof()
                } else {
                    this.parser
                        .consume(bytes, |data| this.ingress.charge(0, 1, usize::from(data)))
                };
                if let Err(error) = result {
                    this.failed = true;
                    return Poll::Ready(Err(error));
                }
                // Cancellation may race a ready read. Revalidate before any
                // plaintext becomes visible to the WebSocket implementation.
                if let Err(error) = this.admitted(context) {
                    this.failed = true;
                    return Poll::Ready(Err(error));
                }
                output.put_slice(bytes);
                Poll::Ready(Ok(()))
            }
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for MeteredIo<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.failed || !this.ingress.admitted_write(context) {
            return Poll::Ready(Err(denied()));
        }
        Pin::new(&mut this.inner).poll_write(context, bytes)
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.failed || !this.ingress.admitted_write(context) {
            return Poll::Ready(Err(denied()));
        }
        Pin::new(&mut this.inner).poll_flush(context)
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Closing the underlying transport remains possible after revocation.
        Pin::new(&mut self.get_mut().inner).poll_shutdown(context)
    }
}

#[derive(Default)]
struct WireParser {
    max_message_bytes: usize,
    upgrade_bytes: usize,
    delimiter: u32,
    upgraded: bool,
    header: [u8; 10],
    header_len: usize,
    header_needed: usize,
    payload_remaining: usize,
}

impl WireParser {
    fn new(max_message_bytes: usize) -> Self {
        Self {
            max_message_bytes,
            ..Self::default()
        }
    }

    fn consume(
        &mut self,
        mut bytes: &[u8],
        mut admit_frame: impl FnMut(bool) -> bool,
    ) -> io::Result<()> {
        while let Some((&byte, rest)) = bytes.split_first() {
            if !self.upgraded {
                self.upgrade_bytes += 1;
                if self.upgrade_bytes > MAX_UPGRADE_BYTES {
                    return Err(invalid("WebSocket upgrade response exceeds its limit"));
                }
                self.delimiter = (self.delimiter << 8) | u32::from(byte);
                self.upgraded = self.delimiter == u32::from_be_bytes(*b"\r\n\r\n");
                bytes = rest;
            } else if self.payload_remaining != 0 {
                let consumed = self.payload_remaining.min(bytes.len());
                self.payload_remaining -= consumed;
                bytes = &bytes[consumed..];
            } else {
                if self.header_len == 0 {
                    let opcode = byte & 0x0f;
                    if !admit_frame(matches!(opcode, 1 | 2)) {
                        return Err(denied());
                    }
                    if byte & 0x70 != 0 || !matches!(opcode, 0 | 1 | 2 | 8 | 9 | 10) {
                        return Err(invalid("unsupported WebSocket frame flags or opcode"));
                    }
                    if opcode >= 8 && byte & 0x80 == 0 {
                        return Err(invalid("fragmented WebSocket control frame"));
                    }
                    self.header_needed = 2;
                }
                self.header[self.header_len] = byte;
                self.header_len += 1;
                bytes = rest;
                if self.header_len == 2 {
                    if byte & 0x80 != 0 {
                        return Err(invalid("masked server WebSocket frame"));
                    }
                    self.header_needed = match byte {
                        126 => 4,
                        127 => 10,
                        _ => 2,
                    };
                }
                if self.header_len == self.header_needed {
                    self.payload_remaining = self.payload_length()?;
                    self.header_len = 0;
                }
            }
        }
        Ok(())
    }

    fn payload_length(&self) -> io::Result<usize> {
        let (length, minimum) = match self.header[1] {
            126 => (
                u64::from(u16::from_be_bytes([self.header[2], self.header[3]])),
                126,
            ),
            127 => {
                let mut extended = [0; 8];
                extended.copy_from_slice(&self.header[2..10]);
                (u64::from_be_bytes(extended), 65536)
            }
            length => (u64::from(length), 0),
        };
        if length < minimum || length > self.max_message_bytes as u64 {
            return Err(invalid(
                "WebSocket frame length is noncanonical or exceeds its limit",
            ));
        }
        if self.header[0] & 0x0f >= 8 && length > 125 {
            return Err(invalid("WebSocket control frame exceeds its limit"));
        }
        usize::try_from(length).map_err(|_| invalid("WebSocket frame length cannot be represented"))
    }

    fn eof(&self) -> io::Result<()> {
        if !self.upgraded || self.header_len != 0 || self.payload_remaining != 0 {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated WebSocket ingress",
            ))
        } else {
            Ok(())
        }
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::ConnectionAborted,
        "relay ingress budget or generation revoked",
    )
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
