# radroots

`radroots` is the ordinary Rust entry to shared Radroots values and execution.
The Client owns one coherent execution session and its actual store. Clones
share admission and shutdown. Source is in refactor; package release remains
separately governed.

## Explicit preview

The default `client` feature enables an in-memory preview. Construction opens
no file, network, keyring, runtime or worker. Its deterministic cache generation
is not an operation ID or stock authority.

```rust
# async fn run() -> radroots::Result<()> {
use radroots::{Client, StartMode};
let client = Client::preview();
client.start(StartMode::LocalOnly).await?;
client.close().await?;
# Ok(())
# }
```

With `native`, `Client::open(NativeConfig::public_read(options)).await` opens
the actual SQLite cache from explicit validated host paths, mode and source
generation. It performs no networking or automatic command replay. This public
cache scope does not open an authenticated participant workspace or create
seller stock authority. On Unix, explicitly adding an opened protected
`NativeWorkspace` with `NativeConfig::with_workspace` enables `Client::attach`.
Attachment verifies its persisted credential and exact selection before returning
`WorkspaceSession`. That session constructs opaque, immutable command contexts
without I/O or a clock read; admission must recheck current rights and expiry.
A context does not submit a command or prove
user consent; network pairing, seller authority, complete command receipts and
durable listing/chat operations remain separate implementation requirements.

`attach_signer` observes the concrete provider only after retaining its callback
scope. The first successful key freezes and later drift invalidates that scope.
Observation grants no workspace, command or stock authority. Failed/cancelled
attachment remains reachable through `signer_session` for explicit cleanup.

Close permanently stops admission, quiesces callback dispatch, drains work,
disconnects owned transports, disposes callbacks and closes storage last. Failed
or cancelled close retains its unfinished stage; await close again. An unpolled
future has no shutdown effect, and Drop supplies no completion receipt.
`CloseReceipt` reports actual resource closure, never business installation.
Native handles/futures retain thread guarantees; shared browser resource code
uses local handles/futures. Compilation does not qualify browser runtime hosts.

Errors come from `radroots_execution`: fixed codes, bounded redacted details,
explicit admission/retry disposition and optional exact operation identity.
Backend prose, private payloads and paths are not retained in the error.

## Values and retained capabilities

`radroots::domain` provides exact decimals and distinct order, thread, workspace,
executor and operation identities. `radroots::OperationId` is the 256-bit
retained identity. These values alone perform no authorization or transaction.

Farm, listing, trade, event, identity, transport, geographic and knowledge
modules retain their source-owned values/planners and advanced capability
paths. The advanced `radroots_sdk` still owns its existing explicit transport
composition while concrete ordinary network integration is unfinished; the
new Client does not wrap that engine or report its writes as new durable
commerce. Optional capabilities remain governed by their actual host/target
qualification and do not become available because a feature compiles.

The former root `ClientBuilder` and builder-returning memory/native helpers
are replaced by explicit Client preview/open methods. Advanced SDK callers
remain at their owning package. No wildcard or SDK namespace is exposed:

```compile_fail
use radroots::sdk;
```

```compile_fail
use radroots::ClientBuilder;
```

The reviewed Rust API lives in
[`contracts/api_baselines/radroots.txt`](../../contracts/api_baselines/radroots.txt).
The approved successor ownership and host contracts live in
[`shared_sdk_refactor.v1.json`](../../contracts/architecture/decisions/shared_sdk_refactor.v1.json).


The common `OperationReceipt<T>` types are exported by this facade. Their owned
observations preserve original identities and independent issuance, installation,
delivery and caller-wait dimensions. The advanced native journal can supply a
coherent T0/T1/S snapshot for this projection. Ordinary inbox job orchestration,
T2 business installation/outbox and exact delivery remain required before the
complete native submission workflow is available.
