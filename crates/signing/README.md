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
[`radroots_signing` section of the Release V1 specification](../../contracts/crates/release_v1/radroots_crates_release_v1.toml).

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

The same SPI supports a purpose-tagged, HTTP-only
`BlossomAuthorizationPlan`. `SigningPurpose` keeps that BUD-11 credential
distinct from registry-authored relay events, while receipt verification still
binds the exact author, timestamp, kind, tags, content, event ID, deadline, and
cancellation signal.

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

The `provider` module adds a participant identity observation and separate
NIP-44 encryption/decryption capabilities. `ParticipantProvider` retains key
custody and exposes no secret key. `ProviderCall` freezes the observed key,
deadline and cancellation input without claiming authenticated command admission.
Private text is bounded before copying, zeroized when dropped and redacted in
diagnostics. The caller authenticates message carriers and applies the stricter
chat, record, rumor and complete-event limits before using these primitives.
Capability and replay observations do not replace actual provider qualification
or permit reissue of retained uncertain work.

`ParticipantProvider::recover_exact` is a separate original-intent boundary.
`ExactRecoveryRequest` retains the complete original intent and a distinct
recovery-call budget. The executor must check captured T1 replay policy, current
authorization and the actual adapter's documented method. Unsupported adapters
return original-bound indeterminate state without calling legacy or first-issue
signing. `ExactRecoveryOutcome` has no proven-unissued variant: refusal, expiry
or cancellation of this invocation proves nothing about the earlier invocation.
Obtained bytes are independently verified against the original fields/binding
and must remain owned through append-only durable capture. Provider capability
observations, request construction and outcome metadata confer no authority.

`SignerHandle` pairs a provider with its exact `SignerCallbacks` host scope.
The lower signing port owns this contract so adapters do not depend on an
executor. A composing lifecycle owner must quiesce dispatch, drain observers,
await callback closure and release roots on the required host thread. Neither
construction nor an identity observation grants a workspace or command role.

```compile_fail
use radroots_signing::provider::PrivateText;
let text = PrivateText::new("private inquiry").unwrap();
let duplicate = text.clone();
```

```compile_fail
use radroots_signing::provider::PrivateText;
let text = PrivateText::new("private inquiry").unwrap();
let wire = serde_json::to_string(&text);
```

`Signer` is externally implementable and object-safe. Native hosts retain
`Send + Sync` and boxed `Future + Send`; WASM hosts and futures may remain local.
`host::Shared` selects native `Arc` or browser `Rc` for signer and progress
callback ownership. These target bounds let the host choose its executor;
the SPI does not select an async runtime or require an async-trait macro.

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

## Retained authored evidence

`AuthoredSignEvidence` records a cryptographically verified authored event even
when an already-started signer finishes after the caller's deadline or
cancellation. It binds exact event bytes, operation, artifact and signer request
identity to a positive injected observation time. That time records observation,
not proof of the instant of signing. `revalidate` checks the retained request
identity and all cryptographic fields again with the caller's own clock.

`Signer::sign_authored_evidence` is an optional hook. Its default delegates to
`sign`, verifies receipt identity and promotes the receipt; already-cancelled
requests are rejected before invoking the adapter. Existing adapters
compile unchanged. Adapters that discard late output need an override to retain
it. Overrides must still prevent new work after deadline or cancellation. The
host owns polling, lifecycle and durable reconciliation; the SPI adds no worker.

Evidence is a fact, not an active success receipt or authority to schedule new
signing, admission or delivery. `SignReceipt` remains deadline/cancellation
checked. The evidence constructor and default hook reject Blossom requests;
expiring BUD-11 authorization continues to use the strict active receipt path.

## Serialization contract

Native `Actor` and `SignRequest` values are runtime-local and are not
serializable. `SignReceipt` and `AuthoredSignEvidence` can be serialized with
`serde` but cannot be
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
- Callers at a persistence or HTTP boundary must revalidate an untrusted host
  signer's returned event against their retained request and locally observed
  completion time before committing or transmitting it.
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
| `chat` | no | Adds bounded chat-seal conversion and authenticated DM advertisements. |
| `commerce` | no | Adds exact structured-commerce seal conversion and authenticated DM advertisements. |
| `artifacts` | no | Adds captured chat/public-listing artifact conversion. |

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

## Exact issuance and obtained results

`issuance::SigningIntent` binds the canonical 256-bit domain operation ID,
command digest, exact authored preimage, role/recipient and captured deadline.
`issuance::ArtifactRole` and `issuance::IntentBinding` reexport the portable
Protocol data definitions; the exact v1 commitment bytes remain unchanged.
Constructing a binding proves no retained claim or dispatch authority.
Its generic complete-event limit is 64 KiB, including escaped content, tags,
keys and signature. Executor chat/record/rumor limits remain tighter. Values and
bindings are neither authenticated command contexts nor durable admission.

`ParticipantProvider::sign_exact` has no fallback to the legacy signer method.
An unsupported provider reports unavailable. `ProviderState::exact_issuance`
is observed separately from legacy replay metadata. The concrete local adapter
reports non-replayable exact issuance; no durable retrieval or safe uncertain
reissue is inferred from custody of the key or a stable event ID.

`SigningOutcome` binds every classification to the original intent. Obtained
bytes are verified against every signed field and signature, own their original
representation, and carry optional observation time. A failed, expired or
cancelled completion observation cannot discard valid bytes. Rejected output
has fixed forensic metadata; prefix digests report their actual coverage. No
classification claims that T0/T1/S/T2/D ran, and rejection is not proof that
issuance never happened. The executor owns polling and durable capture through
its safe journal boundary; dropping an observer cannot erase that responsibility.

The legacy request/evidence API retains its existing callers while the
successor workers and providers migrate. Its sixteen-byte identities and
mandatory observation clock do not qualify the new execution contract.


With `chat`, `issuance::VerifiedDmRelayAdvertisement` checks bounded exact
kind-10050 fields, event ID, Schnorr signature and expected participant while
retaining original bytes. Capturing an explicit advertised subset preserves
that source observation; it establishes no global freshness, configured or
available transport, workspace grant, consent or signing permission. Missing DM
information supplies no fallback target. Exact seal conversion also compares the
originating captured operation and command commitment before creating an intent.
