# radroots_storage_sqlite

The separate execution-workspace database creates exact outbox schema generation 11.
Chat and initial `listing.publish` share the operation journal, protected
preparation and owned bounded worker. Seller authentication and a retained T0
coordinate decision precede public issuance. A conflicting initial publish
cannot replace that decision or reach a new signature. T2 rechecks the original
T0/T1/claim/obtained graph, then atomically retains its exact signed listing
revision and pending direct-publication work. It creates no private message,
recipient, wrapping identity, stock allocation or relay acknowledgement.

Exact generation 7/8/9 catalogs preserve their original encrypted records and
private continuation without catalog rewrites. Original public T1/S remains
readable where supported. Unsupported new public admission, binding, issuance
or installation refuses explicitly; retained claims never permit reissue.
Actual transport dispatch and ordinary listing execution are separate stages.

SQLite storage backend for Radroots.

Startup and migration preserve typed `SpaceInsufficient` for numeric SQLite
capacity failures and typed capacity I/O failures. Existing corruption and
validation errors remain distinct. Capacity does not prove rollback: inspect
the original store before retrying, and retain pending or ambiguous work.

Projection document inventory uses the existing canonical projection table.
Each page releases its read snapshot before returning a continuation. Metadata
is bounded before decoding; the 16 MiB payload budget is charged before each
value is loaded, including values that fail digest verification. Corrupt
payloads retain an explicit locator, while unpageable corrupt keys or
generations fail the query. Cross-page mutation fencing remains caller-owned.

Runtime schema v15 permits late verified authored signatures to remain durable
alongside a signing stop. The logical snapshot preserves cancelled or terminal
state; a separate SQL stop column preserves the original signed/raw constraint.
Prior snapshots, receipt identities, migration bytes and checksums remain
unchanged. Older schema policies reject the new version. An update guard prevents
replacement of the first signed bytes or removal of the retained signing stop.

Recording a late fact verifies the backend's immutable original claim receipt
inside the same transaction as the artifact, eligible delivery bindings and new
receipt. Expiry does not invalidate evidence, and evidence does not renew work
authority. Explicitly stopped delivery plans remain stopped. A success receipt
is returned only after the actual SQLite COMMIT.

The backend owns separate `runtime.sqlite` and `private.sqlite` files. Writable
opens hold a process advisory lock, use WAL with bounded busy handling, and
apply only the governed forward migrations. Fresh stores require a
host-supplied `SourceGeneration` and creation timestamp; the crate never reads
hidden entropy or a wall clock.

Runtime schema v14 adds generic schema/scope metadata and an index for bounded
draft-head queries. Migration preserves every original revision snapshot,
including corrupt historical evidence. Known foreign schemas and scopes are
filtered before decoding; unknown historical schemas expose only an opaque
author-bound corruption locator. Each page releases its read snapshot before
returning a continuation and reads at most 16 MiB of serialized snapshots.

Draft submission uses the existing WAL/FULL writer and authored tables. One
transaction writes the immutable intent, operation, artifacts, delivery plans,
ordinary Prepare receipt and source-association receipt. Exact replay precedes
source-head CAS, including after reopen or later editing. Existing atomic
receipt snapshots retain their 4 MiB limit; oversized submissions fail without
partial writes. No additional public database or connection API is introduced.

Every connection in both owned pools uses `synchronous=FULL` and requests
`fullfsync=ON`; writable stores retain WAL. Authored append acknowledges only
after a successful SQLite COMMIT, or an exact replay of an already committed
revision. Connection-policy validation rejects a missing full-sync request.
The existing owner supplies this policy without a second database or journal.

Qualification covers actual deferred-constraint COMMIT failure, bounded SQLite
page-capacity exhaustion, lock contention, denied writes, closed/read-only
stores, and child process termination before and after acknowledgment. Failed writes retain the
previous revision and return a typed failure without a success receipt. These
tests do not fill the host disk or establish physical power-loss survival.
SQLite requests `F_FULLFSYNC` where its VFS supports it and may fall back to
`fsync`; actual filesystem, device and power-cut behavior remains a separate
host qualification. Native protected-data policy remains the host's concern.

Backend status and the last integrity result are passive. Hosts invoke
`check_integrity` explicitly with their own positive timestamp when they want
full SQLite and foreign-key validation across both owned files. `close` drains
both pools and releases writable authority explicitly and idempotently.

Backup capture requires an explicit existing host-owned root configured with
`OpenOptions::with_backup_root`. V1 capture creates a deterministic staging
bundle, uses SQLite's online snapshot mechanism for each policy-selected
member, synchronizes captured files and directories, and returns exact lengths
and SHA-256 digests in the backend-neutral manifest. Protected storage is
included only when the plan requests it explicitly.

Restore first copies a finalized bundle into create-new files adjacent to the
live databases and verifies their exact digests, schema catalogs, SQLite
integrity, and foreign keys without changing live state. Finalization closes
the owned pools while retaining writer authority, persists a versioned recovery
marker, atomically replaces each policy-selected member, and requires callers
to reopen the closed backend. Writable open completes an interrupted marked
replacement before opening connections; read-only open fails closed until that
recovery is complete.

Legacy migration starts only from an explicit, forward-only import plan on an
open writable backend. Before any target mutation, the backend captures every
caller-identified predecessor database through SQLite's WAL-consistent online
snapshot operation into a create-new staging directory, verifies SQLite and
foreign-key integrity, records exact source provenance, lengths, and SHA-256
digests in a mode-`0600` manifest, synchronizes the evidence, and atomically
finalizes the immutable bundle. Import identities and timestamps are supplied
by the host; collisions, owned-database aliases, and unsupported paths fail
closed.

Classification revalidates the finalized manifest, member inventory, hashes,
SQLite integrity, and foreign keys before inspecting any schema. Event-store
versions 1 through 4 require the exact governed catalog and, when present, the
exact contiguous migration ledger; outbox, private, and Studio predecessors
require their exact catalog and application version. Unknown objects, mixed
source families, checksum drift, and newer schemas fail closed. Studio records
are explicitly classified for host handoff and are never imported into SDK
storage.

Beginning an import writes only governed recovery metadata. Runtime schema v6
binds one retained import journal to the target generation, finalized manifest,
and ordered classification digest, plus one exact member row per predecessor
source. SQLite guards immutable identity, one-shot conflicts, legal monotonic
state transitions, timestamps, staged counts, and retained audit history.
Repeated begin calls resume the exact journal; conflicting attempts fail
closed.

Runtime schema v7 adds isolated, append-only event-import staging. Each bounded
page revalidates the immutable evidence, decodes and identifier-checks the
legacy signed events, preserves their exact JSON and untrusted predecessor
admission evidence, and atomically advances an eight-byte legacy sequence
cursor with its durable row count. Restart resumes after that exact cursor and
a completed retry is a no-op. Staging never mutates the live canonical event
table or upgrades predecessor verification claims.

Runtime schema v8 stages the predecessor outbox as an ordered five-table
graph. A table-discriminated cursor advances through operations, events,
delivery plans, targets, and attempts; every child is accepted only after its
exact parent, and attempts must bind a target from the same plan. Governed
column-order JSON-array records preserve nullable and scalar predecessor data
without writing the live operation journal, outbox, or delivery evidence.

Private schema v2 stages secret-bearing predecessor records only inside
`private.sqlite`. Runtime and private commits use an explicit replay protocol:
enter runtime staging, idempotently commit and byte-verify one private page,
then compare-and-swap the runtime table cursor and count. A process loss after
the private commit therefore replays the exact page from the old runtime cursor
without duplicating counts or exposing secret-bearing staging in
`runtime.sqlite`.

Classified Studio state is never imported into either owned database. The
backend instead returns an immutable evidence descriptor bound to the import,
target generation, manifest, source digest, schema catalog, and byte length.
Only an exact host receipt carrying that handoff identity and a non-zero opaque
host-store commitment advances the Studio journal member to ready; exact retry
is idempotent and conflicting acknowledgement fails closed.

Before finalization, `validate_legacy_import` revalidates the immutable evidence,
requires every member to be ready, matches each staged count to its exact source
count, and hashes framed member cursors plus every runtime/private staging row
under write-stable snapshots. The returned digest is the deterministic commit
identity; validation itself mutates nothing.

Finalization seals that identity private-first, then completes the runtime
journal and all members in one transaction. A crash after the private marker
replays and verifies it while the runtime journal remains ready; a lost success
response reconstructs the exact receipt from both commit markers. Immutable
legacy staging remains retained as owned migration evidence, while live product
tables are not dual-written and predecessor evidence is not deleted.

The qualification matrix includes every supported predecessor family, mixed
four-source operation, bounded page interruption and reopen, invalid-row
rollback, unsupported-schema and identity conflicts, private-first recovery,
lost-success retry, source retention, and live-table isolation.

```rust,no_run
use radroots_storage::event::SourceGeneration;
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths, SqliteStorage};

# async fn open(directory: &std::path::Path) -> Result<(), radroots_storage_sqlite::Error> {
let paths = Paths::from_directory(directory)?;
let generation = SourceGeneration::new([7; 32]).expect("non-zero generation");
let storage = SqliteStorage::open(
    OpenOptions::new(paths, OpenMode::Create)
        .with_source_generation(generation, 1_700_000_000_000)?,
).await?;
# drop(storage);
# Ok(())
# }
```

Atomic paired authored revisions use the existing runtime revision tables in one
`BEGIN IMMEDIATE` transaction. Both insertions commit together; mismatched or
partial replay rolls back. Complete replay returns immutable historical rows and
does not establish current application authority. No migration or second storage
owner is introduced. Callers may recover an unknown commit result by replaying
the exact pair.

Open inspects both existing database schemas through read-only connections before
configuring writable connections or applying either pending migration. A future
version, foreign namespace, invalid catalog, ineligible authored migration or
incompatible source generation therefore leaves both original files intact. Each accepted migration still
rechecks metadata and applies its pending suffix atomically in its own database;
this does not claim a cross-database atomic commit. An interrupted compatible
upgrade resumes from its committed version on the next open.

On Unix, `workspace::NativeWorkspace` owns an explicit separate local buyer
outbox namespace. Initialization generates random workspace/executor/grant
identities and a protected 32-byte credential; it cannot restore an existing
order or create seller stock authority. Existing open requires the independently
retained selection and never enrolls a new grant. The grant and credential
verifier use the existing context-bound authenticated envelope and an explicitly
selected persistent secret provider. Plaintext credentials are not stored.

One bounded native worker owns the SQLite connection and persistent namespace
lock. Dropped authentication observers cannot cancel its work or release writer
authority. Close permanently stops admission, drains admitted requests, checks
the WAL checkpoint and awaits connection closure before releasing the lock.
Cancelled close can be awaited again. Dropping the handle is not close evidence.
Revocation commits a protected grant revision and survives reopening. This
attachment alone supplies no network pairing, firm stock, transfer, restore or
browser qualification.


The separate `OperationJournal` implementation durably admits non-stock chat
preparations and binds complete immutable signing plans before dispatch. It
rechecks the current protected grant, original preparation and all indexed facts
under `BEGIN IMMEDIATE`, with provider work outside that transaction. A fixed
first-issue window never restarts on replay. Already-bound bytes remain queryable
and replayable after expiry under current access; this grants no new issuance.

The current workspace schema is version 7. Older namespaces are preserved and
refused for explicit cutover, without an automatic state rewrite. Complete plans
use a protected manifest and at most twenty-four 16 KiB protected data parts,
with context binding the full workspace/operation/plan identity and part index.
Existing secret-material/envelope bounds remain intact. Queued and in-flight
request reservations share a 4 MiB byte cap alongside the 32-slot queue limit.
Known rollback, confirmed commit and unknown completion remain distinct.
Plan binding performs no signer dispatch, business installation or delivery.


Native first claims retain a full-width attempt identity and exact command,
plan, recipient, intent, original deadline and transaction bindings before any
signer invocation. A retained claim reports potential issuance and cannot permit
another invocation. Signing remains outside this storage owner.

Obtained seal capture independently verifies the signature and every signed
field against the actual protected claim. Its original scoped credential may
append already obtained matching bytes after revocation or expiry without an
observation clock; this grants no new issuance or read rights. The first raw
representation and optional observation are immutable. Borrowed capture input
keeps the caller's original through error or unknown completion. Evidence uses
a protected manifest and at most four 16 KiB data parts, bounded before reads.


The native operation snapshot reads and authenticates the original preparation,
optional plan and complete expected peer/self outcomes before taking a write
lock. It rechecks current access, all retained and absent facts and whole-graph
counts under one `BEGIN IMMEDIATE`, rejecting extra or orphan rows. It performs
no signing or graph mutation and confirms rollback before returning. Window
expiry does not erase retained facts. The existing owner drains accepted reads
after a lost observer and closes explicitly. Schema 7 has twenty exact objects
and preserves/refuses predecessors without automatic rewriting.

Native T2 uses that same connection owner. It authenticates the complete original
graph, protects fixed installation/work records before the transaction, then
rechecks current access and every retained/absent fact under `BEGIN IMMEDIATE`.
It atomically retains one `chat_messages` entry and distinct recipient work in
`chat_publication_work`, referencing exact retained seals and original routes.
The original private body stays protected in T0 instead of being duplicated.
Later recipient capture may append missing work without replacing the original
message installation. Already obtained evidence may install after the original
first-issue window, under current access; no new signature is requested.

Each T2 protected plaintext is at most 16 KiB. One message plus at most two work
records cap protected materialization below 1 MiB; excess sentinels reject more
than one message or two work rows without unbounded scans. Exact replay preserves
original ciphertext and transaction identities. Known rollback, confirmed commit
and anchored unknown completion remain distinct, and accepted work survives
observer loss.

Native D uses the same owner. Before retention it authenticates the original
work, selected signed DM routes and outer signature, derives the temporary key's
public identity and decrypts the outer against exact original retained seal
bytes. Original actor, peer and selected-recipient keys are refused as temporary
authors, including for a self copy. The temporary wrapping key is never
persisted. Each protected frame is
bounded to 393216 bytes in at most twenty-four 16 KiB parts, with a manifest,
full original work linkage, exact representation digest, complete target intent
and immutable endpoint attempt numbers. Its protected aggregate is at most
2 MiB. Twelve potential invocations per recipient/canonical endpoint and at most
384 operation target slots bound retention; replay spends no additional slot.

The first full target set and completion policy remain fixed per recipient work.
Each exact attempt retains a separate subset, and only that subset spends slots
or may be passed to selected delivery. Original acceptance facts accumulate
across retries under the full policy, without optional exhausted targets blocking
required targets. A distinct current-access attempt may use a fresh delivery
deadline; it changes neither an older attempt nor participant issuance.

Only those exact retained bytes can become transport input. Original per-target
observations append independently in at most six classes per slot, each in a
bounded protected record. Missing ACKs and uncertain transport outcomes never
reverse installation or release participant claims. Late capture authenticates
the original credential and protected attempt without acquiring a fresh deadline,
clock observation, scheduling lease or read permission. The first original time,
including null, remains immutable. Current-access snapshots fence the complete
T0/T1/S/T2/D graph and all target observations before projecting bounded summaries.
Reopen performs no publication. Ordinary library-owned inbox orchestration,
browser/bindings, consumer adoption and final qualification remain separate work.

## Fresh native seller stock authority

`StockEnrollment::prepare` creates a one-shot unrelated fresh seller workspace selection,
protected credential and original creation transaction ID without touching the
namespace. Retain those values through protected host handling before awaiting
`NativeWorkspace::initialize_seller_stock`. This explicit factory uses
`stock_execution_v1.sqlite` and `.stock_execution_v1.lock` in a new owner-only
directory. Its exact version-1 catalog has separate protected authority and
grant records. Grant scope, route, genesis, credential and AEAD context are
separate from ordinary outboxes. The same bounded native worker owns the
connection, admitted work, close completion and persistent writer lock.

Creation uses WAL, FULL synchronization, foreign keys and `BEGIN IMMEDIATE`.
Provider work completes outside the transaction. Use a local filesystem that
honors SQLite locking and synchronization; network filesystems and copied live
WAL sets are not qualified. Missing or malformed namespaces are never reset or
adopted. A failed or cancelled constructor is not evidence of absence: reopen
with `open_seller_stock` and query `stock_authority` with the original retained
selection and credential. The confirmed receipt preserves the original
transaction ID. Creation consumes the sole prepared grant before IO; that enrollment cannot
create a second authority namespace, including after caller cancellation.

Authentication rechecks the protected current authority and grant. Revocation
atomically compares and replaces both protected snapshots and advances the
revocation generation. An uncertain commit fences further access on that
worker until close and explicit reopen. This enrollment boundary provides no
pool allocation, old-order recovery, transfer, signing or business confirmation;
ordinary outbox journal jobs are refused on the stock namespace.

## Owner-private pool creation

The current stock factory creates exact catalog version2 and protected authority
payload version2. Retained enrollment-only version1 still authenticates and
revokes with its original codec; its inventory ports refuse unsupported work.
No namespace is automatically migrated. Ordinary outbox catalogs are separate.

`InventoryStore` admits `CreatePoolRequest` with the exact captured command,
current stock-owner binding, credential and independently retained minimum
inventory checkpoint. The backend verifies that minimum against its protected
root and authenticated retained history before reads, replay or new writes.
Protected operation links must connect that minimum to the current root; SQL
index fields alone cannot establish ancestry. A constant-memory fingerprint
rechecks the exact validated rows inside the transaction after provider IO.
The trusted host
must preserve acknowledged checkpoints through protected custody; metadata
construction alone does not establish checkpoint provenance. Bootstrap and
transfer qualification remain separate requirements.

`stock.create_pool` captures an explicit `tick_scale` canonical integer string
from0 through6 alongside its unit and exact `initial_quantity`. Each counts
require scale0. The command digest includes precision and every semantic context
field; correlation remains diagnostic. No implicit conversion or rounding occurs.
A confirmed owner-private receipt identifies the original transaction, initial
pool effect and acknowledged checkpoint. Its pool snapshot describes that
original effect; use the current pool query for a later projection.

One `BEGIN IMMEDIATE` transaction CAS-checks protected authority and grant,
current clock, minimum checkpoint, idempotency and pool identity, then writes
protected pool/operation/movement records and advances the protected root.
No signer or network work occurs. An empty pool has zero stock and no physical
movement. Replay returns the original effect before fresh deadline checks.
Known failures confirm rollback; unknown completion fences the owner until
reopen and reconciliation by its original operation and digest. Firm holds,
allocation, correction/deficit policy, commerce and transfer are still pending.
