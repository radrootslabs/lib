# radroots_sdk

`radroots_sdk` is the host-neutral asynchronous client engine for Radroots.
It composes the canonical event, trade, signing, transport, storage, and sync
crates without installing a runtime, starting workers, opening files, probing
the network, selecting an account, or choosing fallback transports.

The crate root intentionally exports only `Client`, `ClientBuilder`, `Error`,
and `Result`. Advanced operations live in the `farm`, `listing`, `trade`,
`signing`, `transport`, `storage`, `sync`, `diagnostics`, and `capability`
modules.

The package charter is the normative [`radroots_sdk` crate
specification](../../docs/specs/radroots_crates_release_v1.md#18-radroots_sdk).
This advanced front door is intended for CLI, Studio, FFI/mobile, and native
applications that need to compose storage, signing, transport, or sync
capabilities directly. Ordinary Rust applications should use the curated
`radroots` facade.

## Getting started

The default `memory` feature provides an inert, in-process backend. The caller
supplies its source generation; building the client creates no files, opens no
network connection, installs no runtime, and starts no worker.

```rust
use radroots_sdk::{ClientBuilder, capability::CapabilityId};
use radroots_storage::event::SourceGeneration;

let generation = SourceGeneration::new([1; 32])?;
let client = ClientBuilder::memory(generation).build()?;
let storage = client
    .capabilities()
    .get(CapabilityId::CANONICAL_STORAGE);
assert!(storage.is_some());

# Ok::<(), Box<dyn std::error::Error>>(())
```

The complete executable form, including explicit asynchronous shutdown, lives
in [`examples/safe_memory_client.rs`](examples/safe_memory_client.rs). The
side-effect-free transport-selection example lives in
[`examples/transport_profile.rs`](examples/transport_profile.rs).

## Feature contract

The complete public feature vocabulary is:

| Feature | Capability |
| --- | --- |
| `memory` | deterministic in-process reference storage; the default feature |
| `sqlite` | explicit canonical SQLite storage construction |
| `sync` | composition with a caller-supplied canonical sync engine |
| `nostr` | Nostr conversion and concrete source/sink adapters; implies `sync` |
| `nip46` | NIP-46 signer provider; implies `nostr` |
| `local-signing` | explicit local signing and secret-provider adapters |
| `radrootsd` | explicitly invoked private daemon execution adapter; implies `sync` |
| `geonames` | concrete GeoNames provider integration |
| `knowledge` | deterministic knowledge event contracts/codecs |
| `native` | `sqlite`, `sync`, and `local-signing` |
| `full` | every supported production capability |

There are no `runtime`, `local-runtime`, `signer-adapters`,
`transport-nostr-runtime`, `transport-nostr-client`, or fixture features.
Features compile capabilities; they do not perform I/O. Optional dependencies
are activated only by their owning feature.

The supported qualification matrix is:

```sh
cargo check -p radroots_sdk --all-targets --no-default-features
cargo check -p radroots_sdk --all-targets
cargo check -p radroots_sdk --all-targets --no-default-features --features memory
cargo check -p radroots_sdk --all-targets --no-default-features --features sqlite
cargo check -p radroots_sdk --all-targets --no-default-features --features sync
cargo check -p radroots_sdk --all-targets --no-default-features --features nostr
cargo check -p radroots_sdk --all-targets --no-default-features --features nip46
cargo check -p radroots_sdk --all-targets --no-default-features --features local-signing
cargo check -p radroots_sdk --all-targets --no-default-features --features radrootsd
cargo check -p radroots_sdk --all-targets --no-default-features --features geonames
cargo check -p radroots_sdk --all-targets --no-default-features --features knowledge
cargo check -p radroots_sdk --all-targets --no-default-features --features native
cargo check -p radroots_sdk --all-targets --no-default-features --features full
cargo check -p radroots_sdk --all-targets --all-features
```

## Explicit composition

`ClientBuilder` requires a storage capability. `ClientBuilder::memory(...)`
and `ClientBuilder::sqlite(...)` are explicit constructors; merely enabling a
feature or constructing an empty builder creates no resource. Signers, event
sources, event sinks, and the sync engine are injected separately.

Transport profiles are explicit. `Profile::local_only()` contains no target.
`Profile::delivery(...)` retains the exact canonical target set and
satisfaction policy. Preview transports report unavailable and never
substitute Nostr, daemon, local persistence, or another route.

Farm, listing, and trade preparation is deterministic and side-effect-free.
Commit operations accept native operation and idempotency identities,
cancellation policy, and an explicit transport profile, then return the
canonical sync receipt. Repeating the same idempotent request is the supported
resume/replay path.

Preparation has no commit point. During commit, durable local acceptance is
the first commit point; a cancellation observed before that point returns
without claiming acceptance. Cancellation after durable acceptance cannot
roll the accepted event back: the returned error/receipt identifies the safe
resume path, and retrying with the same idempotency identity must not create a
second logical operation. Dropping `Client::close` before completion leaves
the shared client in a retry-required closing state; call `close` again.

The SDK does not define a second wire model. Canonical lower-crate domain and
protocol types own serialization, validation, versioning, and size limits.
SDK request, plan, and receipt structs are constructor-led orchestration types;
their private representation is not a persistence or interchange format.

## Reliability and privacy

Backup, restore, integrity, and status operations delegate to
`radroots_storage::StorageReliability` and return its native versioned plans,
manifests, revisions, stages, and status values. Restore is staged and must be
explicitly finalized. Client shutdown is explicit and asynchronous.

Public farm/listing events contain only the coarse locality represented by the
canonical event model. Exact coordinates, private trade terms, protected
content, and key references remain behind the private-artifact and secrets
SPIs. Diagnostics contain only capability and canonical storage status. Public
errors and daemon failures use stable, redacted classifications while retaining
private source chains for local diagnostics.

Signer material and bearer credentials are caller-owned capabilities. The SDK
does not generate keys, read a keyring, persist secrets, or include credentials
in `Debug`, `Display`, diagnostics, receipts, or public error text. Hosts remain
responsible for protecting source chains and any lower-level logs they choose
to expose.

## Daemon execution

The `radrootsd` feature compiles a private HTTP/RPC adapter using the versioned
`radroots_protocol::radrootsd::transport_publish::v5` contract. Constructing
`transport::DaemonDelivery` is inert. Network contact occurs only when the host
invokes `deliver`; bearer credentials are redacted, HTTP error bodies are not
surfaced, and the response must match the signed event and requested policies.

## Release posture

This package remains `publish = false` until the complete package-realistic
release qualification and separately authorized publication step. The crate is
licensed under `MIT OR Apache-2.0`.

The reviewed all-features public API baseline is recorded at
[`docs/api/radroots_sdk-0.1.0-alpha.txt`](../../docs/api/radroots_sdk-0.1.0-alpha.txt).
