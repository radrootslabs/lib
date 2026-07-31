# radroots_nostr

`radroots_nostr` is the portable Nostr protocol adapter for Radroots. It
converts between canonical Radroots identity/event values and Nostr protocol
values, provides typed NIP helpers, and supplies an optional concrete local
implementation of the `radroots_signing` SPI.

This crate owns no relay client. It owns no sockets, HTTP client, relay pool,
database, account store, retry loop, scheduler, executor, or process-global
state. Live Nostr transport belongs in `radroots_transport_nostr`; application
composition belongs in `radroots_sdk` or an advanced host.

The authoritative package charter is the
[`radroots_nostr` section of the Release V1 specification](https://github.com/radrootslabs/lib/blob/master/docs/specs/radroots_crates_release_v1.md#10-radroots_nostr).

## Quick start

Convert a canonical Radroots public key to and from its NIP-19 `npub`
representation:

```rust
use radroots_identity::PublicKey;
use radroots_nostr::key::{public_key_from_npub, public_key_to_npub};

let public_key = PublicKey::from_hex(
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
)?;
let npub = public_key_to_npub(public_key)?;

assert_eq!(public_key_from_npub(&npub)?, public_key);
# Ok::<(), Box<dyn core::error::Error>>(())
```

The same flow is available as a standalone example:

```sh
cargo run -p radroots_nostr --example identity_conversion
```

## Public boundary

The durable public modules are organized by responsibility:

- `event` converts event identifiers, coordinates, and signed NIP-01 values.
- `filter` constructs explicit Nostr subscription filters.
- `key` converts public identities, provides NIP-19 helpers, and—with
  `signing`—owns opaque local secret-key handling and NIP-49 operations.
- `tag` converts ordered tag parts and exposes focused tag inspection helpers.
- `signing` implements `radroots_signing::Signer` for local Nostr keys.
- `nip17` wraps and unwraps typed Radroots message events with NIP-17/NIP-59.
- `blossom` signs and verifies BUD-11 HTTP authorization values.

`Error` is the only intended root export. Protocol representations that must
cross the adapter are exposed from the module that owns the conversion, rather
than through a root prelude or wildcard alias set.

## Event model and typed authoring

Canonical product drafts and verified events belong to `radroots_event`.
Encoding, signature verification, contract admission, and typed inbound
projection belong to `radroots_event_codec`. This crate performs the explicit
translation to or from Nostr protocol values.

With `events`, typed builders cover the supported Profile, Update,
PhotoUpdate, Ask, Reply, Comment, deletion-request, and FoodAvailability
authoring profiles. Sealed focused builders fix the event kind and canonical
tag model before signer access. Generic builders reject reserved typed kinds;
relaying an already signed event is a transport operation and does not grant a
typed Radroots authoring claim.

Conversion and verification are deterministic for their inputs. Inbound
admission proves the received event and its typed projection; it does not prove
that referenced events, authors, addresses, relays, or external resources
exist.

## Local signing

The `signing` feature provides an opaque `key::SecretKey` and a concrete local
signer adapter. Secret-bearing values are single-owner, are not serializable,
and always redact `Debug` output. Public identities are converted to the
canonical `radroots_identity::PublicKey` boundary before they leave the
adapter.

NIP-19 `nsec` export and NIP-49 encryption/decryption are explicit operations.
`secret_key_to_nsec` deliberately returns plaintext secret material; callers
must treat that string as a credential, avoid logs and serialization, and
zeroize or discard it promptly. Password, ciphertext, and plaintext failures
are normalized so error values do not retain caller-supplied secret text.

## Side effects, cancellation, and commit points

This crate performs in-memory parsing, validation, encoding, cryptography, and
local signing only. It never opens a network connection, writes a file or
database, selects an account, publishes an event, or installs a runtime.

Some cryptographic adapters are async because their upstream protocol
operations are async. Dropping one of those futures cancels local computation
and creates no external durable effect. The crate has no remote publication or
persistence commit point: the only successful result is the value returned to
the caller. A transport or host that later publishes or stores that value owns
its own cancellation and commit semantics.

The `blossom` module creates and verifies signed `Authorization: Nostr` values
but never sends an HTTP request. The `nip17` module creates and opens gift-wrap
events. It does not select relays, deliver events, retry operations, or persist
message state.

## Serialization contract

- Canonical durable Radroots data should be serialized through
  `radroots_event`, `radroots_event_codec`, or versioned `radroots_protocol`
  contracts.
- Explicit Nostr boundary aliases use the upstream NIP-01 JSON representation.
- `event::ExternalSigningRequest` serializes only as the standard
  unsigned Nostr event after reserved-kind and authoring-policy validation.
- Returned externally signed events are accepted only when author, canonical
  event ID, and the complete NIP-01 signature verify against the request.
- Secret-bearing key and local-signer values do not implement serialization.

Deserializing protocol data never establishes product admission, account
authority, upload completion, referenced-event existence, or relay trust.

## Security guidance

- Parse untrusted protocol data through the checked conversion/admission
  functions; do not infer Radroots product validity from a syntactically valid
  upstream event.
- Treat event content, tags, relay hints, Blossom claims, and NIP-17 plaintext
  as untrusted input even after signature verification.
- BUD-11 authorization events are ephemeral credentials for a specific HTTP
  operation. This crate does not transmit them or manage replay protection for
  a server.
- Typed media descriptors prove bytes and metadata, not successful BUD-02
  upload. The composing runtime must establish upload completion separately.
- NIP-49 protects exported key material at rest; callers still own password
  handling, ciphertext storage, memory hygiene, and access control.
- The crate forbids unsafe code and does not expose a live Nostr client or an
  ambient authority boundary.

## Features

| Feature | Default | Contract |
| --- | --- | --- |
| `std` | yes | Enables standard-library support required by selected upstream protocol operations; it adds no network, storage, runtime, or global initialization. |
| `events` | yes | Enables typed event builders, deterministic Radroots/Nostr event conversion, and verified event adapters. |
| `signing` | no | Adds opaque local secret handling, NIP-49 helpers, draft signing, and the concrete local `radroots_signing::Signer` adapter. |
| `nip17` | no | Adds focused NIP-17/NIP-59 typed message and message-file wrapping/unwrapping; no delivery or persistence. |
| `blossom` | no | Adds BUD-11 signed HTTP authorization value creation and verification; no HTTP client or endpoint operation. |

Features are additive. `--no-default-features` provides the portable
`no_std + alloc` conversion core.

## Intended consumers

- `radroots_nostr_connect` uses explicit Nostr conversion while owning NIP-46
  protocol state.
- `radroots_transport_nostr` performs live relay I/O behind the generic
  transport contracts.
- `radroots_sdk` composes local or remote signing, storage, and transport.
- Myc and `radrootsd` consume focused protocol adapters without moving their
  host authority into this crate.
- Advanced Rust hosts may use this package directly for offline conversion,
  verification, or a local signer adapter.

Applications that only need ordinary Radroots workflows should normally use
`radroots` or `radroots_sdk`.

## Copyright

Except as otherwise noted, all files in the `radroots_nostr` distribution are

Copyright (c) 2025 Tyson Lupul

For information on usage and redistribution, and for a DISCLAIMER OF ALL
WARRANTIES, see LICENSE included in the `radroots_nostr` distribution.
