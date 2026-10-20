# radroots_transport_nostr_native

Native DNS, TCP, original-host TLS, WebSocket framing and plaintext ingress
metering for the shared Radroots Nostr session. The host exchanges owned frames
through `radroots_transport::host::websocket`; it owns no protocol relay pool,
signer, storage, outbox, retries or application executor.

This package begins private while its concrete I/O boundary is verified. The
shared session's ordinary client has not switched to this provider yet.
