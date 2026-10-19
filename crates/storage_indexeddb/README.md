# radroots_storage_indexeddb

An explicit per-connection IndexedDB host for bounded opaque records. Namespace
identity comes from a supplied source generation. Creating a namespace and
opening an existing namespace are distinct operations; missing data never
silently creates a replacement. Importing the package performs no host work.

Atomic compare/exchange checks every original value and writes the complete
batch in one real transaction. Dependent requests are issued from active
IndexedDB callbacks. Success is returned only after `complete`; request success
alone never acknowledges a commit. Accepted work remains owned after its
observer is dropped. Closing stops admission and waits for accepted work.

The default private host stores opaque bytes. The opt-in `workspace` feature
adds `workspace::BrowserWorkspace`: an authenticated protected local buyer
outbox using the shared storage enrollment/grant rules and an explicitly
selected nonvolatile provider. It creates only a fresh unrelated outbox;
opening existing data requires its retained selection and protected proof.
It implements the shared `OperationJournal` for this protected local buyer
outbox. Participant custody, general storage SPI and stock authority remain
separate.

The workspace also executes immutable buyer operation admission and lookup.
It retains the original command, transaction identity and first-issue window;
an exact replay returns that preparation before acquiring entropy or applying
a fresh command deadline. A different command under the same ID refuses.
Preparation uses the shared native codec, split into at most four 16 KiB clear
parts and an authenticated manifest. Each part binds its workspace, operation,
position and full preparation digest. The grant comparison and all record slots
commit atomically, with current access, writer and fresh deadline checks inside
active callbacks. Unknown completion retains the original attempt and stops
admission until exact reopening. Queries compare the complete witnessed operation record set;
missing, orphaned or damaged parts never trigger replacement. These methods
provide T0; they do not issue signatures, install messages or deliver them.

The owner also binds and queries the original complete T1 signing plan, using
the shared native v1 manifest codec. The 384 KiB plan uses at most twenty-four
16 KiB clear parts. Every part binds the workspace, operation, position and
plan digest. One real 31-record transaction compares the grant, all five T0
slots and all twenty-five T1 slots. Fresh binding rechecks the original issue
window after asynchronous protection; exact retained replay keeps the original
transaction and observation without fresh entropy. Missing, orphaned, foreign
or damaged state refuses, including T0 absence with retained plan records.
Unknown completion retains the original attempt and stops admission. T1 stores
complete captured signing data; participant issuance remains a separate phase.

The v2 owner claims original per-recipient signing attempts and appends already
obtained, verified signed seals. A retained claim authorizes no redispatch.
Each claim binds its original preparation, plan, intent, recipient, deadline and
transaction. Captured output retains its exact bytes and optional original
observation in at most four 16 KiB parts plus an authenticated manifest. Actual
signing happens outside storage. Capture requires retained proof and the actual
original claim; expiry, revocation or an unavailable clock cannot discard already
obtained output. Explicit capture-only opening permits this append and closure,
and denies current authentication and fresh work. `capture_evidence` borrows
the original proof and output; accepted work retains its bounded owned copy
through caller cancellation. `obtained_evidence` reads original exact output.

The owner installs an original logical chat once from actual obtained evidence,
then appends distinct publication work for later obtained recipients. Exact
replay preserves the message, basis and transaction before new entropy. The
original first-issue deadline limits signing, while installation of already
obtained bytes requires current grant/proof, lease and clock. Shared checked
installation/publication models validate relationships; host-private v2 metadata
is a closed 468-byte record under independent protection contexts. Bodies stay
in the original command. `operation_snapshot` verifies the complete retained
T0/T1/S/T2/D graph and reconstructs its original message through the shared model.
All facts are witnessed and compared in the actual transaction before results
are returned. Missing predecessors with retained descendants refuse false
absence and new issuance.

`prepare_delivery_attempt` retains exact obtained outer bytes and selected target
intent before a potential transport invocation. It independently verifies the
temporary wrapping key, distinct author, signature, original contained seal,
actual publication work and captured signed relay routes. The complete original
target set and publication policy remain fixed. Selected endpoints spend at most
twelve contiguous invocation ordinals each. Retained attempts authorize no
redispatch; a new explicit attempt can reuse original exact bytes without another
participant signature. Replay preserves the original transaction and deadline.
Fresh writes recheck current grant/proof, lease, monotonic time and the original
delivery deadline after crypto, inside the actual transaction callback.

A private frame contains at most 393,216 bytes in at most twenty-four 16 KiB
parts, a 200-byte manifest and an independent 100-byte request index. Both rows
must agree under separately expected contexts. At most twenty-six new records
plus the existing six grant/preparation comparisons fit the 32-record atomic
limit. The complete witnessed operation prefix fences every write. A coherent
snapshot and `delivery_attempt` read original retained data without dispatch.
Partial or missing predecessors, manifest, index and parts refuse replacement
and false absence. Actual uncertain completion preserves the original operation,
request and optional transaction anchor. Borrowed candidates remain caller-owned;
accepted work retains its bounded copy through observer loss and cancelled close.
`record_delivery_observation` appends an already obtained normalized target
fact against the actual original attempt and retained proof. Each of six classes
has one immutable first observation per selected target and attempt, including
absent time and normalized details. Later classes append without erasing earlier
facts. Relay acceptance does not imply recipient delivery. Expiry, revocation
and an unavailable clock do not discard obtained evidence; capture-only opening
allows retention while denying current queries and fresh work. Independent v2
contexts bind the original work, request, outer representation, transaction,
target and class. Each clear record is at most 16 KiB, with at most 2,304 reports
per operation. Reports decode while the original attempt frame is available,
without retaining every raw outer body. The complete prefix and original grant
are compared in the actual retention transaction. Uncertain completion returns
the original observation and stops admission. `delivery_observations` and coherent
snapshots require current access and do not dispatch. Browser transport and the
ordinary client remain separate phases.

All shared journal ports delegate to this actual owner. Uncertainty stops
ordinary admission while retaining the original namespace and writer lease.
Already obtained original seals and target observations may reconcile under
the retained proof. Recovery authenticates the actual grant before connection
disposal, confirms closure, opens only the same existing namespace/schema and
rechecks the current grant and actual original claim or attempt. Cached original
grant authorization survives a failed reopen without retaining raw proof in the
handle. Fresh connection-bound witnesses are acquired after reopening; no old
witness or `RefCell` borrow crosses the asynchronous replacement boundary.

Only confirmation matching the original uncertain capture identity clears
admission. Confirming an unrelated older fact does not resolve another pending
operation. General or multiple unrelated uncertainty requires explicit closure
and reopening. Recovery grants no fresh command, grant, clock, signing, network
or redispatch permission and never initializes, migrates or repairs a namespace.
Accepted recovery stays owned through lost observation; final close drains it
before releasing the writer.

An owner-bound read snapshot witnesses at most 16,384 ordered opaque keys,
lengths and SHA-256 digests, using a conservative 2 MiB scan reservation. Every
decoded value must match its witness. The complete set is checked again in the
same real transaction as bounded mutations, detecting insertion, deletion and
replacement. Only synchronous bounded comparisons run in transaction callbacks;
providers and signing stay outside. The 32-mutation and 4 MiB live-input limits
remain unchanged. Exact-key cursor lookup distinguishes absent keys from foreign
stored `undefined` values. Readwrite transactions request and check `strict`
durability, which is a browser hint rather than a physical power-loss guarantee.
Observed asynchronous request errors remain failures even if their default abort
is prevented and the browser subsequently completes the transaction.

The workspace holds an actual exclusive origin Web Lock. Unavailable or
contended acquisition refuses; no in-memory substitute is selected. Accepted
jobs remain Rust-owned and serial through cancelled observation. Closing stops
admission, drains the jobs, closes IndexedDB, then observes actual lock release.
Its workspace queue reserves a conservative 4 MiB for one accepted job,
including a complete T1 plan and atomic comparisons. Grant-only setup remains
separately bounded to eight jobs at 512 KiB each; close retains at most 32 weak
observations.
These limits cover owned inputs, not browser-internal or whole-process memory.
Clear grants are bounded to 16 KiB and their protected representation to 32 KiB.

`compare_exchange_checked` runs a trusted host's synchronous eligibility check
in active write callbacks, after asynchronous crypto completes. Refusal aborts
the whole transaction. The workspace uses it to recheck current expiry/clock
and writer ownership before grant writes. Browser workspace v2 has its own
namespace and independently expected grant context. Earlier namespaces are
preserved and refused without migration or replacement. Native grant wire and
context, shared private codecs and generic authenticated envelope v2 remain
unchanged.
No JavaScript object or key handle is exposed by the workspace API. The Web
Lock follows the [standard holding and released promise lifecycle](https://www.w3.org/TR/web-locks/#locks).
Browser storage can be evicted; opening an absent namespace reports its absence.
No SQL global, memory fallback, network, implicit migration or automatic replay
is used. The transaction behavior follows the
[IndexedDB transaction lifecycle](https://w3c.github.io/IndexedDB/#transaction-lifecycle).

The standalone fixture at `tests/browser/Cargo.toml` builds the real host to
WASM. Generate its web bindings with the `wasm-bindgen` version matching that
fixture's lockfile, using an external output directory and the output name
`fixture`. Run `tests/browser/run.mjs` with an installed Playwright package
directory, that output directory, `chromium`, `firefox`, or `webkit`, and the
preserved v1 fixture package directory for actual cross-version refusal checks.
The optional final argument selects `delivery`, `delivery_limits`,
`delivery_corruption`, `delivery_faults`, `observations`, `observation_faults`,
`observation_corruption`, `journal`, `journal_observations`, or `journal_reopen`;
omission runs the existing base
regressions. Additional profiles preserve that base's bounded deadline.
Synthetic normalized observation inputs prove concrete storage and linkage
behavior; they do not establish remote relay execution or acceptance.
Both the page and dedicated worker import those generated bytes. The runner
uses isolated loopback origins and bounds each expanded run to 180 seconds.
This harness ceiling is not a product deadline or performance acceptance. Route
builds, generation and runner execution through `cargo extbuild run --`.
