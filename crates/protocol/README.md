# radroots_protocol

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

All serialized DTOs live below an explicit generation module. The crate root
does not reexport individual DTOs, so consumers must name the contract version
they use.

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | implements standard-library error integration and enables `serde?/std` |
| `serde` | yes | derives serialization and deserialization for wire-contract types |

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
