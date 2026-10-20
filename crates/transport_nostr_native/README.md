# radroots_transport_nostr_native

Native DNS, TCP, original-host TLS, WebSocket framing and plaintext ingress
metering for the shared Radroots Nostr session. The host exchanges owned frames
through `radroots_transport::host::websocket`; it owns no protocol relay pool,
signer, storage, outbox, retries or application executor.

`create_transport` composes the existing shared protocol session with this
provider. Construction is inert. The caller supplies an explicit validated
profile and a Tokio executor; the package creates neither a runtime nor a
second relay pool. Browser hosts use their separately governed provider.
