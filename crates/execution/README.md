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

`ClientSession::inquire` and `publish_listing` capture a validated command from this client's own
authenticated context, calls the actual journal admission and queries its
coherent retained graph. It does not preempt original-operation lookup with a
fresh deadline check. Known failure, uncertain initial commit and a failure
after confirmed admission keep distinct original-ID dispositions. T0 itself
performs no signing or network work. An explicitly online `CommandRuntime`
continues accepted work through original planning, exact signing, protected
capture, installation and publication. Public listing work uses public targets
without private routing, encryption or wrapping; private inquiry retains those
requirements. `operation` reads original progress
through an explicitly selected client-owned scope, independently of the old
first-issue deadline. The qualified concrete bounded worker owns accepted native
admission through observer loss; its close must drain accepted work.

`ClientSession` routes explicit private inventory creation, corrections, mapping,
original-effect reads and proven-unissued cleanup through the supplied inventory
port and this client's exact owned attachment. These calls require a bounded
`TaskHost`; no inline fallback starts them. The existing resource permit and live
byte reservation move into each accepted task until the store settles, including
after observer loss. Close drains these tasks before disposal. The concrete store
must still authenticate current scope, independently retained minima and actual
protected effects. A custom port or a missing query result proves no authority,
safe nonissuance or commit. These paths construct no signer or transport.
An accepted admission task lost before replying reports unknown initial admission
with its original operation ID and manual reconciliation, never known failure.

Package upload is not authorized. Full commerce authority, complete listing revision
behavior, final browser qualification and new bindings remain separate
requirements. Revalidating a context is not durable admission;
the authoritative transaction must recheck its scope.


`OperationReceipt<T>` owns the original captured scope, command and transaction
identities, issuance window, authority checkpoint and coherent progress digest.
It separates installation, per-recipient issuance, delivery and caller waiting.
Its diagnostics contain no private command text or ciphertext. The optional
`receipts` feature projects a Storage operation snapshot without I/O; the
concrete adapter must qualify the underlying authenticated coherent query.
The native T2 profile projects an owned bounded `ChatMessage` or `Listing` when
the concrete owner reports the matching installed result. `OperationStatus`
uses an explicit `OperationValue`; a public listing never projects as a message.
It preserves the original installation
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

`ClientSession::prepare_stock_acceptance` captures the existing seller command
from its exact client-owned scope and preserves the supplied independent stock,
private-source and listing minima. The bounded target task retains captured
inputs, proof, resource permit and byte reservation through store settlement
after observer loss; close drains accepted work. Missing or refused task hosts
execute no fallback. Loss of an accepted admission task preserves the original
ID with unknown T0 and manual reconciliation. The result is preparation only;
the concrete store still authenticates consent and actual atomic holds. This
low-level method does not supply complete ordinary commerce orchestration.

`SignerSession::recover_exact` uses the already attached frozen identity and
rechecks the actual recovery capability before calling the qualified method.
Unobserved, unavailable, closed or changed scopes return original-bound
indeterminate state without legacy or first-signing fallback. Active calls retain
the provider and callback scope until settlement; close waits for those calls.
Foreign callback bindings are rejected with bounded forensic identity. Correctly
bound obtained bytes and nullable observation survive unchanged. The composing
executor must still own the complete task and durable capture, authenticate the
original store claim and enforce retained replay policy. This scope grants no
stock authority, renewed deadline or proof of original nonissuance.

`ClientSession::capture_stock_evidence` owns a bounded copy of borrowed original
evidence through the selected shared port. Accepted tasks outlive their caller's
wait. Errors, unknown appends and mismatched receipts retain exact bytes, original
credentials and resource/byte leases. A real storage failure keeps its error code
and requires manual reconciliation. `drain_stock_evidence` retries that same
material; it performs no signing, T0, installation or delivery. Close drains the
owner before callback/key/storage disposal and remains resumable on failure.
Fresh inventory preparation/mutation and new command first-signature calls
refuse while material remains unpersisted; independent reads remain available.
Borrowed originals survive unsupported/refused hosts. Scope checks preserve the
immutable namespace and actor; host observation metadata supplies no permission
or renewed issuance window. Matching host reports remain a trust boundary, not
cryptographic transaction proofs. Full ordinary commerce orchestration is a
separate requirement.

Capture drain calls explicit `InventoryStore::reconcile_stock_evidence`; a blind
append retry cannot resolve a concrete host's uncertain connection. The qualified
native host retains writer ownership while authenticating and reopening that
same namespace. Core keeps the original material and leases until a matching
confirmed result, including through a refused preflight or lost observer.

With `commerce`, `SignerSession::verify_commerce_plan` qualifies the complete
original plan through the attached provider's frozen observed identity and
existing per-call admission. Changed identity latches a refusal, and missing
capabilities or a closing scope cannot confirm a plan. Active decryption retains
provider/callback roots until settlement; each observation and decryption uses
the existing single-call capacity. No legacy or first-signing method is invoked.
The shared Signing proof preserves original bytes and deadline without granting
T1 or stock authority. The composing executor still owns the complete task and
subsequent durable boundaries after caller cancellation.

`ClientSession::accept_stock_order` owns captured acceptance through actual T0,
complete original peer/self route and encryption checks, T1, confirmed claims,
exact scoped signing, retained S and initial T2. The shared `CommandRuntime`
contains this algorithm; `session::commerce::AcceptanceStore` supplies typed
target transactions and authenticated read-only snapshots. Host adapters select
no new business inputs or policy. Native facade views use the same configured
owner as inventory, with no second database or executor.

The peer commitment runs first and confirmed peer S installs the agreement
before self-history work. Missing IO or signing leaves explicit pending facts.
`resume_stock_acceptance` uses only original inputs; existing non-replayable
claims never obtain a first-dispatch permit, and qualified local recovery needs
both retained policy and a current concrete permit. Obtained bytes enter the
retained evidence owner before persistence. Verified signer bytes enter that
owner before fallible request conversion; conversion refusal retains exact bytes,
credentials and the original byte lease. Accepted tasks outlive caller waits,
and close retains callback roots through settlement and durable capture. Query
facts keep preparation, recipient evidence, installation and actual abort
separate. This path performs no outer wrapping, relay publication, stock release
or proof of original nonissuance; full commerce/accounting closure remains open.

Observed rejected signer output returns `SignerTampering` with the original
confirmed operation identity. Effectful commands return `ExecutionResult`;
its allocation-free Copy `ExecutionFailure` retains the compact primitive Error
plus optional fixed Signing rejection through `signer_rejection()`, including
exact prefix coverage. The primitive Error keeps its 64-byte budget and the
separate command failure is bounded to 128 bytes; neither
failure type owns an untrusted raw payload. Ordinary acceptance
and the generic chat/listing loop stop further issuance on rejection. Already
captured peer evidence remains retained and matching installation completes
before reporting a self-copy rejection. A later installation failure preserves
its actual error code together with signer diagnostics and manual recovery.
Unknown installation stops delivery. These diagnostics are caller observations,
not durable forensic records, authenticated business output or proof that the
original signer never issued. Missing/unsupported/uncertain outcomes keep their
existing conservative claims; no rejection authorizes another first dispatch,
stock release or renewed deadline.

Generic chat/listing execution registers the original obtained artifact and
attempt with prepared capture credentials and its resource lease before
fallible storage conversion. Conversion or append failure retains the original
bytes, blocks further issuance and permits only original capture reconciliation.
`CommandRuntime::drain` returns the compact execution `Error`, preserving the
actual store code, original confirmed operation and manual reconciliation
through resume and close, including when another operation encounters that
retained obligation. Observation append failures likewise retain their original
operation. A failed conversion is cached without discarding its signed input;
neither a new caller nor close may relabel it as a successful capture.
