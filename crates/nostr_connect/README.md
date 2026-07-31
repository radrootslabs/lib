# radroots_nostr_connect

`radroots_nostr_connect` is the relay-independent Nostr Connect (NIP-46)
security-protocol package for Radroots. It owns bounded URIs, methods,
permissions, request and response envelopes, and explicit client and server
state machines. Hosts provide relay I/O, timeout policy, approval policy,
signing authority, persistence, and runtime execution.

The crate is pre-release and publication remains disabled. Its Cargo version
is frozen at `0.1.0-alpha` until the coordinated release contract explicitly
changes it.

## Canonical surface

| Module | Responsibility |
| --- | --- |
| `client` | Prepared/published/completed request state, host transport SPI, cancellation, and progress. |
| `error` | Normalized protocol, validation, correlation, replay, and transport errors. |
| `message` | Bounded requests, responses, envelopes, capabilities, and package-owned event payloads. |
| `method` | Standard and validated extension method identifiers. |
| `permission` | Canonical permission values and bounded permission sets. |
| `server` | Replay-aware request parsing and permission-evaluation inputs without policy ownership. |
| `uri` | Canonical `nostrconnect://` and `bunker://` parsing, validation, and rendering. |

The curated root exports are `Client`, `Server`, `Method`, `Permission`,
`Request`, `Response`, `BunkerUri`, `ClientUri`, and `Error`. Supporting types
remain in their owning modules so callers make protocol boundaries explicit.
The reviewed Rust surface is recorded in the
[public API baseline](../../docs/api/radroots_nostr_connect.txt).

## Preparing a client request

Preparing a request validates and encrypts it but does not publish it:

```rust
use radroots_identity::PublicKey;
use radroots_nostr_connect::{
    Client, Request,
    client::Target,
    message::RequestId,
    uri::RelayUrl,
};

let signer = PublicKey::from_hex(
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
)?;
let relay = RelayUrl::parse("wss://relay.example.com")?;
let client = Client::generate(Target::try_new(signer, vec![relay])?)?;
let operation = client.prepare(RequestId::parse("example-ping")?, Request::Ping)?;

assert!(operation.publication().is_ok());
# Ok::<(), Box<dyn std::error::Error>>(())
```

A runnable version is available at
[`examples/prepare_request.rs`](examples/prepare_request.rs).

`Client::execute` accepts a caller-owned `client::Transport`. The transport
publishes and receives package-owned `ClientEvent` values and reports timeout
or cancellation explicitly. The protocol crate never creates a relay pool,
executor, Tokio runtime, background worker, or global session.

## Server requests and approval

`Server` parses already-decrypted request JSON, rejects malformed or replayed
requests, and returns a `ServerRequest` containing the required protocol
permission. It does not decide whether that permission is granted. Approval
UI, actor authorization, durable grants, session selection, secret access,
event signing, encryption, and response publication belong to the host.

A host constructs a correlated plaintext `ServerResponse`, then performs its
own encryption, event signing, and transport commit. The package never treats
successful parsing as authorization or successful response construction as
delivery.

## Features and supported targets

| Feature | Default | Effect |
| --- | --- | --- |
| `serde` | yes | Propagates serialization support to Radroots protocol dependencies. |

The complete public feature vocabulary is exactly `serde`. No feature starts
work, selects a relay implementation, installs a runtime, opens storage, or
changes approval policy. The Release V1 package is standard-library based; its
default and no-default configurations also compile for
`wasm32-unknown-unknown` as a protocol library.

## Serialization and compatibility

Wire values are validated before use and have explicit size or count bounds.
Unknown canonical extension methods round-trip without relaxing validation.
Current NIP-46 lifecycle vectors live at
`contracts/conformance/vectors/nip46/current_session.v1.json`.

Serde compatibility applies to the documented NIP-46 messages, URIs,
permissions, and capabilities. Rust layout, debug output, and the pre-1.0 Rust
API are not wire contracts. Cargo package versions evolve independently from
versioned protocol or conformance artifacts.

## Security, side effects, and commit points

URI secrets, client keys, encrypted events, protocol payloads, and auth URLs
are redacted from diagnostics. Callers must still treat URI strings and
plaintext request/response JSON as sensitive and avoid logging them.

Constructing a client generates or imports in-memory key material. Preparing a
request performs validation, encryption, and event signing in memory; it does
not perform network or durable I/O. `Transport::publish` is the remote-exposure
commit point: after it succeeds, cancellation or dropping the future stops
local waiting but cannot retract signer-side work. Cancellation before
publication prevents exposure; cancellation after publication is reported as
a distinct phase. The host owns deadlines and cancellation wakeups.

Server parsing and response construction are in-memory protocol operations.
They do not persist approval, consume secrets, sign events, or claim delivery.

## Intended consumers

Direct consumers are `radroots_sdk`, Myc, and remote-signer tooling.
Applications should normally compose NIP-46 through `radroots_sdk`; direct
users of this package are hosts implementing protocol transports or signer
services.

This package must not acquire relay-pool implementation, secret persistence,
approval UI, global sessions, Tokio runtime ownership, or Myc-specific service
storage. Those responsibilities remain in adapters, security providers,
applications, and host runtimes.

## Package charter

The authoritative responsibility, dependency, feature, module, root-export,
and forbidden-scope contract is the
[Radroots crates Release V1 specification](../../docs/specs/radroots_crates_release_v1.md).
The baseline generation procedure and pinned toolchain are documented in
[`docs/api/README.md`](../../docs/api/README.md).

## Copyright

Except as otherwise noted, all files in the `radroots_nostr_connect`
distribution are copyright (c) 2025 Tyson Lupul. See `LICENSE` for usage,
redistribution, and warranty terms.
