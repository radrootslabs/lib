# radroots_nostr

The `public_ingress` feature retains one original authenticated listing or
capability head per explicit scope. It applies frozen NIP-01 replacement order
before profile, expiry or caller-injected clock evaluation. Newer authentic
unsupported, invalid or future heads prevent older actionable fallback. Closed
known profiles retain validated values; observed support remains a participant
claim. This boundary performs no IO, publication, stock or permission decision.
The successor public reader uses explicit64KiB event/tag budgets; legacy defaults
and private profiles remain separately governed. The owning contract is
[`public_ingress.v1.json`](../../contracts/protocol/public_ingress.v1.json).

`radroots_nostr` is the portable Nostr protocol adapter for Radroots. It
converts between canonical Radroots identity/event values and Nostr protocol
values, provides typed NIP helpers, and supplies an optional concrete local
implementation of the `radroots_signing` SPI.

With `signing`, `LocalSigner` also reports its actual participant identity.
With `nip17` or `private_ingress`, it decrypts bounded NIP-44 v2 payloads and can encrypt after
explicit nonce-source injection through `with_nonce_source`. Native hosts may
choose `OsNonceSource`; deterministic sources belong only in controlled tests.
Call identity/deadline/cancellation checks precede effects. Private text is
owned and redacted, and obtained outputs are not discarded by a later clock
or cancellation check. These are crypto primitives: the composing executor
authenticates carriers, enforces profile budgets and retains preimages before
signing or transport. No private key is exported or message published.

With both `signing` and `nip17`, `signing::gift_wrap::wrap_retained_seal`
authenticates an already obtained kind-13 seal and wraps its exact original
bytes. It accepts an explicit positive time and qualified entropy source,
creates a separate temporary key, and never invokes the participant signer.
The complete nested budget is checked before entropy; randomized timestamps,
nonce generation and signing auxiliary randomness use the injected source.
`PendingGiftWrap` retains the exact outer bytes and temporary key for independent
protected-capture verification, with redacted diagnostics and no cloning or
automatic serialization. The caller must retain the envelope and its original
operation, recipient and route binding before transport. This primitive grants
no scheduling authority and proves no durable capture or relay observation.

Run `cargo extbuild run -- cargo run -p radroots_nostr --example
participant_private --features signing,nip17 --locked` for the native OS-entropy
smoke. It uses published synthetic vector identities, encrypts twice and
decrypts both results without persistence or network effects.

This crate owns no relay client. It owns no sockets, HTTP client, relay pool,
database, account store, retry loop, scheduler, executor, or process-global
state. Live Nostr transport belongs in `radroots_transport_nostr`; application
composition belongs in `radroots_sdk` or an advanced host.

The authoritative package charter is the
[`radroots_nostr` section of the Release V1 specification](../../contracts/crates/release_v1/radroots_crates_release_v1.toml).

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
PhotoUpdate, Ask, Reply, Comment, deletion-request, NIP-52 date/time Event, and
FoodAvailability authoring profiles. Sealed focused builders fix the event kind and canonical
tag model before signer access. Generic builders reject reserved typed kinds;
relaying an already signed event is a transport operation and does not grant a
typed Radroots authoring claim.

Conversion and verification are deterministic for their inputs. Inbound
admission proves the received event and its typed projection; it does not prove
that referenced events, authors, addresses, relays, or external resources
exist.

`event::EventAdapter` preserves the original numeric wire kind in its borrowed
and owned job conversions, including named Nostr kinds such as feedback kind
7000. The job decoder separately enforces its supported kind; conversion does
not establish event authenticity or job admission.

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
operations are async. Local key-backed work returns an in-memory result, while
an injected upstream signer may invoke external custody. Dropping its observer
does not prove that no artifact was obtained remotely. The protocol-only
multi-recipient helper is not a durable issuance journal: an earlier recipient
may already have an obtained seal when a later request fails. The composing
executor must retain each disposition and exact obtained result. Transport and
storage owners supply their own publication and durable commit semantics.

The `blossom` module converts exact, verified signer output into and from
signed `Authorization: Nostr` values but never sends an HTTP request. BUD-11
plans remain distinct from relay-authored plans. The `nip17` module creates and
opens gift-wrap events. It does not select relays, deliver events, retry
operations, or persist message state.

Opening verifies the bounded outer event and its single recipient before any
signer callback, then authenticates a closed empty-tag kind13 seal. It checks
NIP-44 v2 framing, identity continuity, complete24KiB rumors and depth16, raw
u16 kinds before upstream conversion, required canonical IDs and author/recipient
bindings. The original rumor ID is recomputed rather than trusted. Peer and
sender self copies retain the same logical message. Decrypted frames and the
temporary parsed rumor are cleared on drop; returned legacy message/file DTOs
remain ordinary owned application data. This helper supplies no commerce,
durability, consent, stock or current-history authority. Its typed outer input
does not recover malformed raw fields already lost by an earlier decoder.
Authentic unknown rumor kinds return unsupported after header/ID/author checks;
their content and tags are not interpreted as known chat data.

With `private_ingress`, `private_ingress::open` takes the original raw outer
JSON and a captured `ParticipantProvider` call. It shares the exact signed
reader with the existing adapter, verifies each carrier before decryption,
and retains the original outer, protected seal and identity-checked rumor.
Provider-owned custody, MAC validation, deadline and cancellation remain
required; the composing host qualifies its actual provider implementation.

With `commerce`, `commerce::open` additionally checks complete closed kind8890
record data, its separate content hash, empty rumor tags, exact author/time,
and the actual recipient's buyer/seller membership. Known records and protected
authenticated unsupported or rejected outcomes remain distinct. Unknown
profiles cannot become known records through a fallback. These values supply
cryptographic provenance without granting business authority or executing a
durable transition. The owning successor contract is
[`commerce_ingress.v1.json`](../../contracts/protocol/commerce_ingress.v1.json).
The optional pinned Node fixture producer under `tools/protocol_oracle` is
development-only; standalone Rust tests consume checked-in synthetic vectors.

The local participant adapter implements separately qualified exact-intent
recovery through `recover_exact`. It recomputes only the complete original
preimage while the original first-issuance window remains open and the current
recovery-call budget permits work. A longer call budget cannot renew the original
window: the earlier claim may have preceded an invocation that never began.
Identity/clock/cancellation refusal retains original uncertainty, and post-effect
clock failure cannot discard obtained bytes. This path performs no encryption,
route replacement, network call or legacy fallback. After-expiry durable signer
retrieval is unavailable. Legacy replay and first-dispatch behavior are unchanged.

`radroots_signing::commerce::VerifiedCommerceSigningPlan::verify` checks both exact recipient
ciphertexts against the original rumor through the selected participant
provider, verifies original signed DM advertisements and their selected subsets,
and checks the provider's actual replay declaration. It performs no signing or
store transaction. Current consent, approved network targets, durable T1 and
dispatch permission remain executor obligations.

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
| `private_ingress` | no | Authenticates original raw NIP-59 carriers through participant-owned custody and retains protected evidence. |
| `commerce` | no | Adds complete known commerce record admission and protected unsupported/rejected outcomes. |
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
