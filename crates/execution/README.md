# Radroots execution

Typed command execution and recovery boundaries. The current implementation
contains allocation-free errors with canonical codes, fixed diagnostic enums,
four explicit retry classes and a validated operation/admission relationship.
The default package supports native Rust and `no_std` targets without a runtime.
The optional `session` feature adds host resource ownership without creating
an async runtime or detached tasks. Native handles and futures are thread safe;
WASM handles use `Rc` and accept thread-local host futures.

The optional `signer_session` feature owns a provider together with its scoped
host callbacks. Construction is inert. Explicit observation freezes the first
participant key and rejects later identity drift; cached capabilities supply
no workspace grant or stock authority. Close stops admission, quiesces callback
dispatch, drains observations, awaits host disposal and releases roots. Failed
or cancelled disposal must be resumed. The Client must retain this scope before
awaiting first observation, including when attachment fails or loses its waiter.

The optional `client_session` feature composes this scope with an actual
`SessionHost`. Attachment is retained before its first await, including before
worker startup. Quiesce permanently stops admission and host callback dispatch;
the owner then drains work, disconnects transports, disposes callbacks and closes
storage. A failed or cancelled attachment remains reachable for explicit cleanup.
No lifecycle observation authenticates a workspace or admits a command.

A proposed operation ID is not durable admission. Unknown signing or commit
outcomes keep the original identity and require exact-operation continuation
or manual reconciliation. Uncertain initial admission remains explicitly
`Admission::Unknown`; it is neither confirmed admission nor known failure.
Signing and delivery results require confirmed durable preparation.
Same-operation continuation means
resuming or querying retained state; it never authorizes fresh preparation or
blind signing. An executor may construct a retained report only after its
actual durable store confirms admission. This reporting value grants no
authority and performs no transaction.

```rust
use radroots_domain::OperationId;
use radroots_execution::{Admission, Code, Error, OperationContext, RetryClass};

let operation = OperationId::from_bytes([7; 32]);
let error = Error::new(
    Code::InvalidInput,
    OperationContext::proposed(Some(operation)),
    RetryClass::Never,
    None,
)?;
assert_eq!(error.context().operation_id(), Some(operation));
assert_eq!(error.context().admission(), Admission::NotAdmitted);
# Ok::<(), radroots_execution::InvalidErrorContext>(())
```

Details accept fixed field/reason categories, never private payloads, paths or
backend prose. Display contains only a canonical code and fixed message;
errors have no retained underlying cause. Bindings must preserve the typed
operation ID and explicit admission/retry disposition.

```compile_fail
use radroots_execution::{Detail, Field};

let _ = Detail::new(Field::Signer, String::from("private signer response"));
```

```compile_fail
use radroots_execution::{Code, Error, RetryClass};

let _ = Error {
    code: Code::CommitUnknown,
    retry: RetryClass::Never,
};
```

With `session`, `Session::new` takes explicit resources and a nonzero in-flight
limit. Construction performs no work. Awaiting `start` starts the host's bounded
workers; only successful startup enables resource admission. Cloned handles
share the same gate. `WorkPermit` retains resources and capacity for owned
executor work, independently of its caller's observation. It is not durable
command admission. Executors must retain it through result/uncertainty storage.

On its first poll, `close` permanently closes admission and interrupts pending
startup. It then quiesces workers, drains permits, disconnects transports/auth,
disposes callbacks and closes storage last. Successful shutdown stages are
retained across cancellation or known failure. Awaiting close again resumes the
unfinished host hook. Hosts must independently preserve their own partial hook
progress and obtained results. Dropping an unpolled close performs no shutdown;
dropping handles or permits never claims durable completion.

With `workspace_session`, explicit attachment authenticates a protected
credential through the selected host. `WorkspaceSession::command_context`
purely freezes the captured attachment and all explicit context fields without
I/O or a clock read. Current grant, revision and expiry require revalidation.
The opaque `CommandContext` has no public field constructor or JSON deserializer.
Closing the attachment invalidates its contexts. A custom adapter is an explicit
host trust boundary and requires its own qualification; its observation cannot
establish native SQL authority. An approval descriptor is not consent proof.

`ClientSession::inquire` captures a validated inquiry from this client's own
authenticated context, calls the actual journal admission and queries its
coherent retained graph. It does not preempt original-operation lookup with a
fresh deadline check. Known failure, uncertain initial commit and a failure
after confirmed admission keep distinct original-ID dispositions. No signer or
network operation occurs in this T0 boundary. `operation` reads original progress
through an explicitly selected client-owned scope, independently of the old
first-issue deadline. The qualified concrete bounded worker owns accepted native
admission through observer loss; its close must drain accepted work.

Package upload is not authorized. Owned signing/install/delivery continuation,
inventory authority, listing execution, browser attachment and new bindings
remain separate requirements. Revalidating a context is not durable admission;
the authoritative transaction must recheck its scope.


`OperationReceipt<T>` owns the original captured scope, command and transaction
identities, issuance window, authority checkpoint and coherent progress digest.
It separates installation, per-recipient issuance, delivery and caller waiting.
Its diagnostics contain no private command text or ciphertext. The optional
`receipts` feature projects a Storage operation snapshot without I/O; the
concrete adapter must qualify the underlying authenticated coherent query.
The native T2 profile projects an owned bounded `ChatMessage` when the concrete
owner reports a matching installed result. It preserves the original installation
transaction/time separately from later recipient publication-work transactions.
Pending wrapping/publication work does not establish transport acknowledgement.
Newly captured recipient evidence may require T2 continuation while the original
message remains installed; unknown issuance remains independently unknown.
Expired remaining first issuance is blocked while obtained and unknown facts
remain retained. Changing a wait observation cannot change those facts.
Default receipt values remain usable without an async runtime on noStd targets;
the Storage-backed projection is a separately selected host feature.

The D projection retains original outer/target intent summaries and independent
target observations, including the full request ID, target fingerprint and
endpoint attempt number. Unobserved retained potential invocations remain
unknown; observed rejection or timeout does not become acceptance. Accepted
evidence is evaluated with the shared original target policy, independently for
each publication work item and across its independently selected retries,
without erasing another recipient's pending state or weakening the full policy.
These projections perform no transport or participant signer invocation and
never renew a scheduling lease.
