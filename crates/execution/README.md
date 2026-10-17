# Radroots execution

Typed command execution and recovery boundaries. The current implementation
contains allocation-free errors with canonical codes, fixed diagnostic enums,
four explicit retry classes and a validated operation/admission relationship.
The default package supports native Rust and `no_std` targets without a runtime.
The optional `session` feature adds host resource ownership without creating
an async runtime or detached tasks. Native handles and futures are thread safe;
WASM handles use `Rc` and accept thread-local host futures.

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

The package is unpublished while the shared facade is migrated. The session
coordinator is tested with controlled hosts; concrete journal/network/browser
hosts still require qualification. It does not yet implement durable commands,
signing, outbox workers, inventory authority, public listing/private chat
execution, participant attachment or bindings.
