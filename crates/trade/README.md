# radroots_trade

`radroots_trade` is the portable domain-algorithm layer for Radroots trades.
It validates canonical trade inputs, represents reducer evidence, derives
deterministic conflict-aware projections, and prepares side-effect-free
workflow plans over the canonical `radroots_event` trade model.

The crate is pre-release and its Cargo version is frozen at `0.1.0-alpha`
until explicitly changed. Versioned trade schemas, reducer contracts, and
conformance vectors evolve independently from the Cargo package version.

## Canonical surface

New code should enter through these modules:

| Module | Responsibility |
| --- | --- |
| `evidence` | Immutable mutation, private-term, and attestation observations consumed by reduction. |
| `model` | Trade projection state and validated business identifiers. |
| `reducer` | Deterministic reduction, evidence precedence, and conflict reporting. |
| `validation` | Validation-error ownership for canonical trade inputs. |
| `workflow` | Validated plans describing host actions without executing them. |

The curated root exports are `Projection`, `ReductionInput`, `ReducerIssue`,
`WorkflowPlan`, `ValidationError`, and `Error`. The canonical protocol
`TradeId` remains owned by `radroots_event`; `model::OrderId` is a distinct
human or business-workflow identifier and has no conversion to or from
`TradeId`.

The pre-release tree still contains operational-listing, binding-generation,
and versioned-contract migration paths needed by integrated consumers. They
are not new package identities or authority to add host behavior. Their exact
consumers and final-removal checkpoint are recorded in
[`COMPATIBILITY.md`](COMPATIBILITY.md). The reviewed Rust surface is recorded
in the [public API baseline](../../docs/api/radroots_trade.txt).

## Deterministic reduction

`reducer::reduce_trade_records` is a pure projection function. It accepts a
`ReductionInput` containing the canonical trade ID plus explicitly supplied
mutation, private-term, attestation, and observation evidence. It normalizes
input order, handles duplicates deterministically, isolates unsupported
contract versions, reports conflicts as typed `ReducerIssue` values, and
computes a canonical projection digest.

```rust
use radroots_event::trade::TradeId;
use radroots_trade::{ReductionInput, reducer::reduce_trade_records};

let trade_id = TradeId::parse("0123456789abcdef".repeat(2))?;
let projection = reduce_trade_records(ReductionInput::new(trade_id));

assert_eq!(projection.trade_id(), &trade_id);
# Ok::<(), Box<dyn std::error::Error>>(())
```

A runnable version is available at
[`examples/reduce_trade.rs`](examples/reduce_trade.rs).

Reduction does not retrieve missing records or attestations. A projection's
evidence state and issues describe only the supplied input, not the existence
of additional records elsewhere.

## Workflow planning

`WorkflowPlan::prepare` validates a canonical proposal, decision, revision,
or cancellation mutation and returns its ordered required actions. Actions
describe private-term verification, signing, atomic persistence, and delivery;
the crate never performs those actions. Private-term plans expose only the
artifact identifier, schema identifier, commitment, and candidate needed by
the host to perform its own verification.

Creating or dropping a plan changes no external state. A successful plan is
not an authorization decision, signature, storage receipt, delivery receipt,
or proof that referenced private material exists.

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | Standard-library integration for the portable model and errors. |
| `serde` | yes | Serialization support for native and versioned trade values. |
| `serde_json` | yes | JSON validation receipts and executable JSON conformance vectors; enables `serde`. |
| `dto-bindgen` | no | Pre-release generated-binding support used by integrated consumers; enables `std` and `serde_json`. |

`--no-default-features` keeps the allocation-backed trade model, reducer, and
workflow planner available in `no_std` environments. Features are additive;
enabling one never selects a runtime, performs I/O, starts work, or weakens
validation. The `serde_json` and `dto-bindgen` names are migration-era
surfaces, not authority to expose code generation or implementation assembly
as permanent Release V1 capabilities.

## Serialization and versioning

Serde represents validated values; deserialization does not perform actor
authorization, signature verification, record lookup, or delivery. Canonical
JSON stability applies only to the explicitly governed reducer and workflow
conformance vectors and validation-receipt contracts. Rust data layout,
debug formatting, and the pre-release public API are not wire contracts.

Versioned `V1` names identify serialized or algorithm-contract generations.
They do not imply that the Cargo package is `1.0`, and changing a Cargo version
must not rewrite authenticated historical trade data.

## Security and trust boundaries

All mutation, evidence, identifier, and serialized inputs are untrusted until
their owning constructors or validators accept them. Deterministic reduction
does not establish actor authority, validate a cryptographic signature, prove
referenced-event existence, decrypt private terms, or decide business policy.
Workflow preparation validates shape and required host actions but deliberately
does not acquire signers, keys, stores, transports, clocks, or executors.

The crate owns no secret material and must not log private-term plaintext.
Artifact identifiers and ciphertext commitments are references for a host to
verify through an explicit secure boundary; they are not proof of retrieval,
decryption, durability, or confidentiality by themselves.

## Side effects, cancellation, and commit points

This crate performs no network, filesystem, database, keychain, signing,
outbox, scheduling, or process-global operations. Its public algorithms are
synchronous and deterministic for the same canonical inputs.

There is no asynchronous cancellation or deadline boundary and no durable
commit point. Abandoning reduction or dropping a workflow plan discards only
in-memory work. Storage, signing, transport, sync, SDK, RHI, and application
hosts own cancellation and must report success only after their own explicit
commit boundary succeeds.

## Intended consumers

Direct consumers are `radroots_storage`, `radroots_sync`, `radroots_sdk`, RHI,
and applications that need the deterministic trade boundary. Ordinary
applications should normally use the `radroots` or `radroots_sdk` front door
and depend on this crate directly only when they need its native model,
reducer, evidence, or workflow-plan contracts.

This package must not acquire actor authorization, signers, event-store or
SQL access, files, transport delivery, outbox mutation, process scheduling,
or application state. Those responsibilities belong to host SPI, adapter,
storage, orchestration, and front-door packages.

## Package charter

The authoritative Release V1 responsibility, dependency, feature, module,
root-export, and forbidden-scope contract is the
[Radroots crates Release V1 specification](../../docs/specs/radroots_crates_release_v1.md).
The baseline generation procedure and toolchain are documented in
[`docs/api/README.md`](../../docs/api/README.md).

## Copyright

Except as otherwise noted, all files in the `radroots_trade` distribution are
copyright (c) 2025 Tyson Lupul. See `LICENSE` for usage, redistribution, and
warranty terms.
