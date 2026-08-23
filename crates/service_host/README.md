# radroots_service_host

`radroots_service_host` is the unpublished, native host-mechanism crate for
Radroots services. It provides narrow, reusable building blocks for validated
service identity, deterministic configuration loading, injected time and
entropy, lifecycle supervision, permissioned local administration, and cached
operations surfaces.

The crate owns mechanisms only. Service-specific configuration, policy,
database schema, domain routes, readiness decisions, process CLI parsing,
runtime creation, global logging, and signal installation remain with the
consuming service or binary boundary. Authoritative tasks may not be detached,
and this crate must not become a broad lifecycle framework.

All service-host modules are private. Consumers use the deliberate crate-root
surface, including its re-exported [`ServiceId`](crate::ServiceId) and
[`InstanceId`](crate::InstanceId) identity types.

The host boundary is governed by the
[`services_hardening_host.v1` machine decision](../../contracts/architecture/decisions/services_hardening_host.v1.json).
The reviewed Rust surface is recorded in the
[public API baseline](../../contracts/api_baselines/radroots_service_host.txt).

## Strict configuration values

Configuration parsing is inert except for the one explicitly selected file.
Schema identity is checked before service-owned typed deserialization, and
common leaves use strict canonical units:

```rust
use radroots_service_host::{
    BoundedCount, ByteLimit, LoggingFormat, OptionalOperationsBind,
    PositiveDuration,
};

let grace: PositiveDuration = "30s".parse()?;
let response_limit: ByteLimit = "1MiB".parse()?;
let workers = BoundedCount::<64>::new(8)?;
let logging: LoggingFormat = "json".parse()?;
let operations: OptionalOperationsBind = "127.0.0.1:9100".parse()?;

assert_eq!(grace.to_string(), "30s");
assert_eq!(response_limit.bytes(), 1_048_576);
assert_eq!(workers.value(), 8);
assert_eq!(logging.to_string(), "json");
assert!(operations.is_enabled());
# Ok::<(), Box<dyn std::error::Error>>(())
```

An enabled non-loopback operations address does not authorize public binding;
conversion to [`OperationsListenerConfig`](crate::OperationsListenerConfig)
still requires [`OperationsBindPolicy::Public`](crate::OperationsBindPolicy::Public).
Configuration errors redact selected paths and raw parser input from ordinary
Display, Debug, and standard error chains.

## Cached state and explicit cancellation

Status and operations reads consume the latest validated cached value. They do
not perform an active probe. Cancellation is explicit, cloneable, and
directional from parent to child:

```rust
use radroots_service_host::{
    CachedServiceState, CancellationToken, Readiness, ReasonCodes,
    ServiceOperationalState, ServicePhase, cached_service_state,
};

let operational = ServiceOperationalState::new(
    ServicePhase::Ready,
    Readiness::READY,
    ReasonCodes::empty(),
)?;
let (_publisher, reader) = cached_service_state(CachedServiceState::new(operational, ()));
assert_eq!(reader.snapshot().operational().phase(), ServicePhase::Ready);

let parent = CancellationToken::new();
let child = parent.child_token();
parent.cancel();
assert!(child.is_cancelled());
# Ok::<(), radroots_service_host::StatusContractError>(())
```

## Local administration and operations

Detailed status and mutations use the bounded HTTP/1.1 Unix administration
models and server. Linux admission requires peer credentials; macOS v1 relies
on the exact owner-only filesystem posture. The TCP operations server exposes
only cached `GET /livez`, `GET /readyz`, and `GET /metrics`; it has no route
extension point for detailed status or mutation authority.

Request and response bodies, headers, queries, connection concurrency,
deadlines, task names, metric snapshots, reason collections, and shutdown
phases have explicit bounds. Safe Display and ordinary Debug projections
exclude raw paths, SQL, credentials, payloads, and provider or relay errors.
Where a host error retains an original cause for trusted inspection, that cause
is not rendered by its safe projection.

Reason-code iterators and wire arrays stop at the first item beyond their fixed
maximum. Administration payloads are traversed directly from the bounded input
or streamed directly into the capped response writer; validation does not
materialize an intermediate `serde_json::Value` tree. Recursive null rejection
and duplicate or unknown field rejection remain fail closed.

Every bounded text constructor validates borrowed UTF-8 before it creates the
retained string. Bounded wire strings use validating Serde visitors, so the
host does not create a second prevalidation copy of identifiers, safe messages,
reason codes, task names, routes, or metric vocabulary.

Administration operation and correlation identifiers are closed ASCII values
of 1 through 128 bytes. Their first byte is alphanumeric; later bytes are
alphanumeric or `.`, `_`, `:`, or `-`. Ordinary `Debug` is redacted and no
`Display` implementation exposes the retained value. Trusted protocol code
uses the explicit borrowed `as_str` accessor for serialization.

## Process and runtime ownership

The crate does not parse a CLI, read configuration from environment variables,
install an operating-system signal handler, initialize tracing, create a Tokio
runtime, call `process::exit`, choose service paths, open a service database,
or own service-domain policy. Process binaries inject clocks and entropy,
normalize signals through [`ProcessSignalAdapter`], register every authoritative
task with [`TaskSupervisor`], and execute [`GracefulShutdown`] explicitly.

## Supported targets and publication

The generic host/config/status/lifecycle surfaces support the workspace's
qualified Rust targets. Unix administration and its client/server are exported
only on Linux and macOS. Linux peer credentials are required and fail closed;
macOS makes no peer-credential equivalence claim.

Publication is disabled. The package is not part of the public Radroots crate
release closure.
