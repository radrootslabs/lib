# radroots_signing

`radroots_signing` is the protocol-neutral host SPI for authorizing and signing
exact Radroots authored-event plans. It owns actor provenance, signer capabilities and
status, bounded signing requests, verified receipts, progress, and normalized
secret-safe errors.

The crate is `no_std` with `alloc`. It does not own secret keys, keyrings,
network clients, NIP-46 sessions, SQL, UI prompts, or an async executor.
Concrete local Nostr signing belongs in `radroots_nostr`; remote protocol state
belongs in `radroots_nostr_connect`; applications compose those adapters in
`radroots_sdk` or their own host layer.

The authoritative package charter is the
[`radroots_signing` section of the Release V1 specification](https://github.com/radrootslabs/lib/blob/master/docs/specs/radroots_crates_release_v1.md#8-radroots_signing).

## Typical flow

1. A host resolves public actor provenance and roles into an [`Actor`].
2. Event-codec code creates an immutable `AuthoredEventPlan`.
3. The host combines stable operation/artifact identity, actor, plan, deadline, and
   cancellation policy into a [`SignRequest`]. Construction validates the
   current authorization, required author role, public key, and provenance.
4. A dyn-compatible [`Signer`] implementation signs locally, delegates to a
   remote device/service, or mediates explicit host interaction.
5. The implementation creates a [`SignReceipt`] from the originating request.
   Receipt construction rejects plan drift and invalid Schnorr signatures.

[`Actor`]: crate::Actor
[`Signer`]: crate::Signer
[`SignRequest`]: crate::SignRequest
[`SignReceipt`]: crate::SignReceipt

```rust
use radroots_event::{GenericEventDraft, contract::AuthorRole};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_identity::PublicKey;
use radroots_protocol::runtime::v1::OperationId;
use radroots_signing::{
    Actor, AuthoredArtifactId, SignRequest, SigningIntentId, SigningOperationId,
    actor::ActorSource,
    request::{CancellationPolicy, SignPolicy},
};

let public_key = PublicKey::from_hex(
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
)
.expect("canonical public key");
let actor = Actor::new(
    public_key,
    ActorSource::ExplicitPublicKey,
    [AuthorRole::Any],
)
.expect("validated actor");
let draft = GenericEventDraft::new(
    "radroots.social.geochat.v1",
    20_000,
    1_700_000_000,
    Vec::new(),
    "hello from Radroots",
    public_key.to_hex(),
)
.expect("validated generic draft");
let plan = AuthoredEventPlan::from_generic(draft).expect("exact plan");
let intent_id = SigningIntentId::new(
    SigningOperationId::new([1; 16]).expect("operation ID"),
    AuthoredArtifactId::new([2; 16]).expect("artifact ID"),
);
let policy = SignPolicy::new(
    1_700_000_030_000,
    CancellationPolicy::PreservePublishedRequest,
)
.expect("bounded policy");
let request = SignRequest::new(OperationId::SyncPush, intent_id, actor, plan, policy)
    .expect("authorized signing request");

assert_eq!(request.operation_kind(), OperationId::SyncPush);
```

The complete externally implementable SPI example is
[`examples/host_signer.rs`](examples/host_signer.rs).

## Host SPI contract

`Signer` is intentionally externally implementable, object-safe, `Send`, and
`Sync`. Its methods return boxed `Future + Send` values so the SPI does not
select an async runtime or require an async-trait macro.

- `status` is observational and must not create a signing request or another
  durable side effect.
- `sign` receives an owned, already-authorized request. Implementations must
  honor its deadline and cancellation policy for the entire operation and must
  create success only through `SignReceipt::from_signed_event`.
- Implementations must not install an executor or spawn hidden workers. The
  composing host owns polling, scheduling, and process lifecycle.
- Every concrete adapter must document the exact point at which a request
  becomes durable. Dropping the future before that point must leave no durable
  effect; dropping it afterward does not imply rollback.
- Concrete backend failures are normalized to `radroots_signing::Error`.
  Native sources may be retained with `std`, but display, debug, and versioned
  protocol reports never copy source text.

## Deadlines, cancellation, and commit points

`SignPolicy` carries an absolute Unix-millisecond deadline. A signer must reject work once
that deadline is reached; request construction does not read a clock.

`CancellationPolicy::LocalCooperative` is for local-only work that may stop
when cancellation is observed. `PreservePublishedRequest` is for operations
that may publish a durable remote request: cancellation before publication may
stop the operation, while cancellation after publication must preserve and
report the final remote state explicitly. Cancellation never silently converts
an unknown or committed remote state into success or rollback.

The SPI defines those rules but performs no I/O itself. Commit points belong to
the concrete adapter and must be visible in that adapter's documentation and
status/progress behavior.

## Serialization contract

Native `Actor` and `SignRequest` values are runtime-local and are not
serializable. `SignReceipt` can be serialized with `serde` but cannot be
deserialized without the originating request; this prevents callers from
bypassing authorization and exact-draft verification.

The `serde` feature provides stable representations for passive policies,
capabilities, status, progress, challenges, and receipts. Versioned
cross-process error data is produced through `Error::to_report` and owned by
`radroots_protocol`. An authentication challenge URI is deliberately
host-displayable and serializable, so adapters must not embed credentials or
secret material in it.

## Security and side effects

- Request construction separates current authorization from historical plan integrity before role, key, and provenance
  authorization and never invokes a signer on failure.
- A successful receipt proves exact equality of author, timestamp, kind, tags,
  content, and event ID with the exact request plan, plus a valid signature.
- `Error` display/debug output and protocol reports are redacted. Under `std`,
  a caller may explicitly inspect a preserved native error source locally.
- `AuthChallenge` debug output redacts its URI; the value accepts only bounded
  control-free HTTPS URIs and remains untrusted navigation input for hosts.
- This crate forbids unsafe code and contains no secret-key type, persistence,
  network transport, global state, timer, or runtime initialization.

## Features

| Feature | Default | Contract |
| --- | --- | --- |
| `std` | yes | Uses `std` collections and permits preserved native error sources; no runtime, I/O, or global initialization is added. |
| `serde` | yes | Adds serialization for explicitly passive public values and deserialization only where validated reconstruction is safe. |

Features are additive. `--no-default-features` provides the `no_std + alloc`
core, and `serde` is supported independently of `std`.

## Intended consumers

- `radroots_nostr` implements concrete local signing.
- `radroots_nostr_connect` supplies remote protocol state without owning this
  SPI's host composition.
- `radroots_sync` and `radroots_sdk` orchestrate authorized requests and
  consume verified receipts.
- CLI, Studio, mobile/FFI, and service hosts resolve actor provenance, choose
  adapters, display authentication challenges, and own cancellation/runtime
  behavior.

Applications that only need ordinary Radroots operations should normally use
`radroots` or `radroots_sdk`; implement this package directly when providing a
new signer adapter or advanced host composition.
