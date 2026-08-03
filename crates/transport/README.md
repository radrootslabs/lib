# radroots_transport

`radroots_transport` is the transport-neutral host SPI for moving verified
Radroots events between explicit targets. It owns extensible transport and
target identities, separate source and sink capabilities, bounded fetch and
delivery requests, provenance, partial outcomes, and normalized errors.

The crate is `no_std` with `alloc`. It does not own network clients, storage,
outbox claiming, retry loops, schedulers, timers, or fallback policy. Concrete
adapters such as `radroots_transport_nostr` implement the SPI; applications
compose those adapters in `radroots_sdk` or their own host layer.

The authoritative package charter is the
[`radroots_transport` section of the Release V1 specification](https://github.com/radrootslabs/lib/blob/master/docs/specs/radroots_crates_release_v1.md#9-radroots_transport).

## Typical flow

1. A host selects a validated [`TransportId`] and constructs one or more
   canonical [`Target`] values.
2. [`TargetSet`] rejects an empty, oversized, or duplicate-fingerprint set.
3. The caller creates a bounded [`FetchRequest`] or [`DeliveryRequest`] with a
   request identity and absolute deadline. A fetch may carry a validated
   [`FetchSelector`] for exact kinds, authors, and inclusive event-time bounds.
4. A dyn-compatible [`EventSource`] or [`EventSink`] implementation performs
   only the requested operation.
5. The caller validates [`FetchPage`] or [`DeliveryReceipt`] against the
   originating request and decides whether normalized retryable outcomes merit
   another explicit operation.

[`TransportId`]: crate::TransportId
[`Target`]: crate::Target
[`TargetSet`]: crate::TargetSet
[`FetchRequest`]: crate::FetchRequest
[`FetchSelector`]: crate::source::FetchSelector
[`DeliveryRequest`]: crate::DeliveryRequest
[`EventSource`]: crate::EventSource
[`EventSink`]: crate::EventSink
[`FetchPage`]: crate::FetchPage
[`DeliveryReceipt`]: crate::DeliveryReceipt

```rust
use radroots_transport::{Error, Target, TargetSet, TransportId};

let transport_id = TransportId::parse("example")?;
let target = Target::new(transport_id, "https://transport.example/events")?;
let targets = TargetSet::new(vec![target])?;

assert_eq!(targets.len(), 1);
# Ok::<(), Error>(())
```

The complete externally implementable SPI example is
[`examples/host_transport.rs`](examples/host_transport.rs).

## Host SPI contract

`EventSource` and `EventSink` are independent, externally implementable,
dyn-compatible `Send + Sync` traits. An adapter may implement either or both.
Their methods return boxed `Future + Send` values so this package does not
select an async runtime or require an async-trait macro.

- `status` is observational and must not begin fetch or delivery work.
- `fetch` returns one bounded page plus per-target outcomes and explicit
  continuation state. Returned events must satisfy the request selector;
  request-bound page validation rejects adapter drift.
- `deliver` returns one receipt for the exact request and every requested
  target; it never performs an implicit retry.
- Implementations must not install an executor or spawn hidden workers. The
  composing host owns polling, scheduling, retries, and process lifecycle.
- Concrete backend failures are normalized to `radroots_transport::Error`.
  Native sources may be retained by adapters, but public outcome codes and
  messages must remain bounded and secret-safe.

## Targets and extensible identity

`TransportId` is a validated open identity, not a closed enum. The built-in
`LOCAL`, `NOSTR`, `RETICULUM`, and `RADROOTSD` constants are conveniences;
future adapters can use validated custom IDs without changing this crate.

Targets contain a transport ID, canonical endpoint URI, optional scope and
human label, and a derived SHA-256 fingerprint. The label is descriptive and
does not participate in identity. The transport ID, endpoint, and scope do.
Target-set construction preserves caller order and rejects duplicate
fingerprints instead of silently deduplicating them.

Adapter-specific endpoint policy belongs in the adapter. This generic package
does not export relay URL types, Reticulum constants, network clients, or
private-network exceptions.

## Bounds, deadlines, cancellation, and commit points

Request IDs, endpoint values, cursors, target sets, outcome details, and page
sizes are bounded by public constants. Fetch and delivery requests carry an
absolute Unix-millisecond deadline; constructors validate the deadline but do
not read a clock.

Dropping a returned future requests cancellation. Before an adapter publishes
or commits a remote operation, cancellation must leave no durable effect.
After that boundary, cancellation does not imply rollback: the adapter must
preserve and report the final remote state when it can be observed. Each
concrete adapter documents its exact publication or commit point.

## Outcomes, partial success, and retry

Fetch pages and delivery receipts preserve target-local results. Delivery
satisfaction is explicit (`any`, `all`, quorum, or required fingerprints) and
is evaluated against the originating request. Retryability and terminality are
normalized data on outcomes, never an automatic loop. Authentication,
unavailability, rejection, cancellation, partial progress, and unknown remote
state must not be rewritten as success or silent fallback.

## Serialization and provenance

With `serde`, passive identities, requests, pages, receipts, status values, and
outcomes use validated representations. Deserialization rechecks canonical
identity, bounds, request/receipt cardinality, and provenance rather than
trusting serialized fingerprints or counts.

Inbound events carry the transport ID, exact target fingerprint, observation
time, and optional continuation cursor that produced them. A page cannot claim
events or outcomes for targets outside its request.

## Security and side effects

- Target endpoint syntax is validated before an adapter receives a request;
  adapters remain responsible for scheme, DNS/IP, TLS, and private-network
  policy before connection.
- Delivery accepts a validated signed event, not arbitrary bytes.
- Receipt and page constructors reject missing, duplicate, unexpected, or
  forged target evidence.
- This crate forbids unsafe code and contains no socket, filesystem, database,
  global state, timer, executor, or retry implementation.
- No transport may silently route through another transport.

## Features

| Feature | Default | Contract |
| --- | --- | --- |
| `std` | yes | Enables standard-library support in canonical dependency values; it adds no I/O, runtime, or global initialization. |
| `serde` | yes | Adds validated serialization for passive identities, requests, status, provenance, pages, receipts, policies, and outcomes. |

Features are additive. `--no-default-features` provides the `no_std + alloc`
core, and `serde` is supported independently of `std`.

## Intended consumers

- `radroots_transport_nostr` provides the concrete native Nostr adapter.
- `radroots_storage` persists transport-neutral outbox and evidence values.
- `radroots_sync` plans bounded operations and applies explicit retry policy.
- `radroots_sdk` composes storage, signing, source, and sink implementations.
- Service and future adapter hosts implement one or both SPIs and own runtime,
  network, cancellation, and lifecycle behavior.

Applications that only need ordinary Radroots operations should normally use
`radroots` or `radroots_sdk`; implement this package directly when providing a
new transport adapter or advanced host composition.

## Copyright

Except as otherwise noted, all files in the `radroots_transport` distribution are

 Copyright (c) 2025 Tyson Lupul

For information on usage and redistribution, and for a DISCLAIMER OF ALL
WARRANTIES, see LICENSE included in the `radroots_transport` distribution.
