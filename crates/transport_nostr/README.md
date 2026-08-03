# radroots_transport_nostr

`radroots_transport_nostr` is the concrete, signer-free Nostr implementation
of the generic [`radroots_transport::EventSource`] and
[`radroots_transport::EventSink`] interfaces. It validates relay configuration
and network policy, performs bounded fetch and delivery attempts, exposes
explicit host-mediated NIP-42 authentication, and normalizes relay outcomes
and passive status.

The crate does not own event ingestion, persistence, outbox claiming,
projection refresh, retry scheduling, SDK profiles, or a process runtime.
Those policies belong to `radroots_sync` and host applications. Publication
remains disabled during the `0.1.0-alpha` refactor.

The authoritative package charter is the
[`radroots_transport_nostr` section of the Release V1 specification](https://github.com/radrootslabs/lib/blob/master/docs/specs/radroots_crates_release_v1.md#15-radroots_transport_nostr).
The reviewed Rust surface is recorded in the
[public API baseline](../../docs/api/radroots_transport_nostr.txt).

## Configure without connecting

Configuration is explicit, validated, and inert. Constructing
[`NostrTransport`] creates no socket and performs no DNS lookup:

```rust
use radroots_transport::{EventSink, EventSource};
use radroots_transport_nostr::{Config, NostrTransport, RelayUrlPolicy};

let config = Config::new(
    RelayUrlPolicy::Public,
    ["wss://relay.example.com"],
)?.with_timeouts(5_000, 20_000, 2_000)?;
let transport = NostrTransport::new(config);

let source: &dyn EventSource = &transport;
let sink: &dyn EventSink = &transport;
drop(source.status());
drop(sink.status());
# Ok::<(), Box<dyn std::error::Error>>(())
```

A runnable version is available at
[`examples/configure_transport.rs`](examples/configure_transport.rs).
The composing host constructs bounded `FetchRequest` and `DeliveryRequest`
values from `radroots_transport`, polls the returned futures on its executor,
and applies any retry or scheduling policy outside this crate.

## Public surface

- [`Config`] validates a non-empty, duplicate-free relay set plus bounded
  connection, request, status, and concurrency limits.
- [`RelayUrl`] is a canonical Nostr relay URL that converts to and from the
  generic `radroots_transport::Target` model.
- [`RelayUrlPolicy`] selects public-Internet, exact-loopback, or explicitly
  trusted private-network destination rules.
- [`NostrTransport`] implements both transport SPIs and exposes explicit
  NIP-42 challenge lifecycle methods.
- [`Error`] contains only package-owned validation and authentication errors;
  upstream failures are normalized before crossing the public boundary.

All source modules are private implementation details. The crate root contains
only the five reviewed exports above and does not expose an upstream client,
relay pool, Tokio handle, signer, storage handle, or retry worker.

## Relay and network security

`RelayUrlPolicy::Public` accepts TLS WebSocket URLs with public hostnames or
global addresses. `Local` accepts exact loopback destinations and permits
plaintext WebSocket only for that class. `PrivateNetwork` accepts explicit
trusted private or public destinations but still requires TLS.

Before opening a socket, the live connector resolves at most 32 addresses,
validates the entire answer set against the selected policy, and connects to a
validated address directly. The original hostname remains the TLS SNI and
certificate-verification identity. Proxy and Tor connection modes are denied;
there is no certificate, hostname, DNS-policy, or fallback bypass.

Callers that resolve addresses outside the adapter may use
[`RelayUrl::validate_resolved_addresses`] to apply the same destination-class
check before handing control to another network boundary.

## Fetch, delivery, and outcome behavior

Fetch accepts only configured Nostr targets, applies the request page bound,
deduplicates events by event ID, preserves per-relay provenance, and emits an
opaque versioned cursor when more results remain. Malformed relay events are
ignored and reported as a partial target outcome rather than admitted.

Delivery converts an already validated signed Radroots event to Nostr, attempts
each configured target once, and returns one normalized receipt entry per
requested target. Relay rejection, authentication requirements, rate limits,
timeouts, connection failures, missing results, and partial acceptance remain
explicit; this crate never retries, falls back to another transport, or
rewrites an unknown result as success.

Source and sink status are passive in-memory observations. Reading status does
not connect to a relay, refresh DNS, or begin fetch or delivery work.

## Deadlines, cancellation, and commit points

The absolute deadline in each generic request bounds the complete operation.
The configured connection and request timeouts are upper bounds within that
remaining budget. An already-expired request performs no relay work.

Dropping an unpolled fetch or delivery future performs no I/O. Once polled,
cancellation is best effort at the socket boundary. For delivery, submission
to a relay is the remote commit point: after a relay accepts the event, dropping
the future cannot retract it. A missing final response is therefore reported
as unavailable or unknown evidence, never as proof that no publication
occurred. Fetch is observational and has no local durable commit point.

NIP-42 authentication is explicit. [`NostrTransport::begin_authentication`]
records one bounded relay challenge and returns the exact host signing input.
The host signs outside this crate, then calls
[`NostrTransport::complete_authentication`] once. Relay submission is the AUTH
commit point. Rejecting a challenge consumes it without network access, and a
challenge is never retried or silently replaced.

## Serialization and diagnostics

This package defines no public serialization feature or stable serialized
configuration format. Persist relay profiles in a host-owned, versioned
contract and reconstruct [`Config`] through its validating constructors.
Generic requests, pages, receipts, targets, and status values follow the
serialization contract of `radroots_transport`.

Public diagnostics are bounded and secret-safe. Raw upstream client errors,
relay challenge payloads, signed authentication events, credentials, and
transport internals are not retained in public status or normalized outcomes.
Applications should still avoid logging relay authentication inputs or signed
event JSON.

## Features and runtime requirements

The package has no Cargo features. It is a standard-library native adapter;
Tokio and the upstream Nostr relay client are private implementation choices.
The crate never creates an executor, installs a runtime, spawns a background
worker, installs a tracing subscriber, or owns process lifecycle. The host must
poll operations from a compatible executor and provide clock/deadline policy
through the generic requests.

## Intended consumers

- `radroots_sync` composes this source and sink with verification, storage,
  projection, outbox, and explicit retry decisions.
- `radroots_sdk` selects and configures the adapter for advanced applications.
- Native services may compose it directly behind the generic transport SPIs.

Ordinary applications should normally use `radroots` or `radroots_sdk`.
Adapter authors and advanced hosts should depend on `radroots_transport` for
the generic contract and use this package only when Nostr relay I/O is needed.

## Copyright

Except as otherwise noted, all files in the `radroots_transport_nostr`
distribution are copyright (c) 2025 Tyson Lupul. See `LICENSE` for usage,
redistribution, and warranty terms.
