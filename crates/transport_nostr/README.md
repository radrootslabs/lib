# radroots_transport_nostr

`radroots_transport_nostr` is the concrete, signer-free Nostr implementation
of the generic `radroots_transport` event source and sink interfaces.

The crate validates relay configuration and network policy, performs bounded
fetch and delivery attempts, exposes explicit host-mediated NIP-42
authentication, and normalizes relay outcomes and passive status. It does not
own event ingestion, persistence, outbox claiming, projection refresh, retry
scheduling, SDK profiles, or a process runtime. Those policies belong to
`radroots_sync` and host applications.

Publication remains disabled during the `0.1.0-alpha` refactor.
