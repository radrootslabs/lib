# radroots_protocol

The chat profile's `GiftWrapBudget` measures the complete kind-1059 envelope
for the exact already obtained seal representation. Whitespace in retained
signed JSON still consumes encryption and event budget; canonical event
identity does not permit replacing those bytes during recovery.

Authored chat rumors carry the original full operation ID in a private
`radroots/operation` tag. Identical messages from distinct operations in one
second therefore have distinct NIP-01 message identities. Replaying the
original operation preserves its captured time, exact rumor and message ID;
the discriminator supplies no authorization.

`radroots_protocol` defines the passive, versioned wire contracts shared by
Radroots processes and language bindings. It is a `no_std + alloc` foundation
crate: it validates schema identities and contract catalogs, but performs no
network, storage, signing, scheduling, or process-lifecycle work.

The crate is pre-release and its Cargo version is frozen at `0.1.0-alpha` until
explicitly changed. Cargo package versions are independent from the version
numbers embedded in wire and schema contracts.

## Contract surface

| Module | Authority |
| --- | --- |
| `capability::v1` | transport capabilities, maturity, and availability |
| `error::v1` | stable, redaction-safe error reports and recovery guidance |
| `event::v1` | event-kind catalogs and the serialized trade-state vocabulary |
| `runtime::v1` | operation descriptors, risk, approval, idempotency, and effects |
| `radrootsd::transport_publish::v5` | daemon transport-publish request, job, outcome, and capability DTOs |
| `schema` | schema IDs, module generations, descriptors, and aggregate registries |
| `command` (`commands` feature) | bounded closed v1 inquiry/reply data codec and exact command/approval-intent hashes |
| `nip01` (`nip01` feature) | exact bounded canonical preimages, IDs and complete-event sizes |
| `intent` (`intent` feature) | exact v1 signing commitment data without execution authority |
| `chat` (`chat` feature) | bounded kind-14/13 data, complete immutable signing-plan codec and observed DM route descriptors |
| `listing` (`listing` feature) | typed quote-required input and bounded kind-30402/profile-v1 preimages without command or publication authority |

Versioned DTOs live below an explicit generation module. The command codec
implements its fixed v1 profile without exposing automatic serialization of
captured private commands. Parsed command data supplies no authority or consent.
The crate root
does not reexport individual DTOs, so consumers must name the contract version
they use.

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | implements standard-library error integration and enables `serde?/std` |
| `serde` | yes | derives serialization and deserialization for wire-contract types |
| `commands` | no | enables the portable closed chat codec, domain values, SHA-256 and zeroizing private command buffers |
| `nip01` | no | enables pure exact NIP-01 serialization and complete byte counting |
| `intent` | no | enables pure domain-separated v1 signing commitment data |
| `chat` | no | enables commands, NIP-01 and the bounded known-kind chat/plan data profile |
| `listing` | no | enables portable typed quote-required listing input and exact public profile/event production |

The listing producer emits one canonical `radroots.listing` version-1 profile,
explicit default units and no fabricated price. Paused/withdrawn projections
keep the same addressable coordinate, state non-availability in summary and
content, and omit a standard status that cannot represent that state. Complete
signed-event framing is bounded to 64 KiB before the canonical preimage is
allocated. Canonical profile ordering remains explicit when a consumer enables
JSON insertion-order preservation. The early producer supports unspecified
package contents, unknown/available public supply and empty media. Full prices,
registered currencies and the remaining profile modes require their own
validated implementation. Pure input/preimage construction supplies no workspace
permission, durable preparation, signature, installed record or delivery receipt.

Disabling default features leaves the catalog and structural validation APIs
available with `alloc` only. Enable `serde` without `std` for portable encoded
DTOs.

## Validate the aggregate contract

```rust
use radroots_protocol::{capability, event, runtime, schema};

capability::v1::validate_catalog(capability::v1::CATALOG).unwrap();
event::v1::validate_catalog(event::v1::CATALOG).unwrap();
event::v1::validate_trade_state_vocabulary(event::v1::TRADE_STATE_VOCABULARY)
    .unwrap();
runtime::v1::validate_catalog(runtime::v1::CATALOG).unwrap();

let registry = schema::protocol_v1_registry().unwrap();
assert!(!registry.is_empty());
```

For a runnable version, see
[`examples/inspect_contract.rs`](examples/inspect_contract.rs).

## Serialization and trust boundaries

Wire representations are stable only within their named generation. A caller
must deserialize into the intended version, run the module's structural
validation, and apply its own authorization and policy checks before acting on
the data. Successful deserialization alone does not make input trusted.

Stable error reports intentionally carry only safe messages and bounded detail
values. Use `error::v1::ErrorReport::redacted_from_source` at sensitive
boundaries; do not copy secrets, credentials, raw upstream errors, or private
payloads into protocol DTOs.

## Side effects, cancellation, and commit points

This crate has no side effects, asynchronous work, cancellation mechanism, or
commit point. Runtime operation descriptors describe those properties; they do
not execute operations. The owning SDK, daemon, storage, signing, or transport
implementation must define cancellation behavior and must not report a commit
until its own durable boundary has succeeded.

## Intended consumers

The direct consumers are Radroots domain crates, SDK and daemon runtimes,
transport/storage/signing boundaries, generated bindings, and conformance
tools. Applications should normally enter through `radroots` or
`radroots_sdk`, using this crate directly only when implementing or inspecting
a versioned boundary.

The package charter is the
[Release V1 specification](../../contracts/crates/release_v1/radroots_crates_release_v1.toml).
The reviewed Rust surface is recorded in the
[public API baseline](../../contracts/api_baselines/radroots_protocol.txt).

## Complete private signing-plan data

`chat::ChatSigningPlan` retains the exact sorted unique peer/self recipient set,
seal author/kind/time/empty tags/content/event ID/purpose, original signed DM
route-source bytes, selected targets, logical message ID, command commitment and
provider replay declaration. The closed binary profile is capped at 384 KiB;
its decoded recipient collections discard caller spare capacity. Original route
sources are each at most 64 KiB, selected URIs at most 2048 bytes, and the
combined session selection at most sixteen distinct relays.

Construction and decoding validate descriptors only. They verify no signature,
MAC, grant, consent, route readiness or provider capability and create no durable
commit. The executor must verify source evidence, approved targets and current
access, then retain this complete plan through its qualified protected journal
before any business signing invocation. A stored replay declaration alone never
permits uncertain reissue.
