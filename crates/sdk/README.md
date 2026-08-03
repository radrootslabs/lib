# radroots_sdk

`radroots_sdk` is the host-neutral asynchronous client engine for Radroots.
It composes the canonical event, trade, signing, transport, storage, and sync
crates without installing a runtime, starting workers, opening files, probing
the network, selecting an account, or choosing fallback transports.

The crate root intentionally exports only `Client`, `ClientBuilder`, `Error`,
and `Result`. Advanced operations live in the `farm`, `listing`, `trade`,
`signing`, `transport`, `storage`, `sync`, `diagnostics`, and `capability`
modules.

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
