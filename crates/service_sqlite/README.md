# radroots_service_sqlite

`radroots_service_sqlite` is the unpublished, native SQLite-mechanism crate for
Radroots services. It provides narrow, reusable building blocks for exclusive
writer authority, instance locking, versioned schema mechanics, bounded
transactions, immutable service-instance database identity, integrity checks,
backup, restore, and passive storage status.

`ServiceSqliteHost` is the only public connection host. Its SQLx pool and raw
connections are sealed inside the crate. Services run typed SQLx queries through
the borrowed `ServiceSqliteTransaction` executor supplied by
`ServiceSqliteHost::transaction`; transaction begin, commit, rollback, policy
validation, attached-database exclusion, and cancellation quarantine remain
runner-owned. Writable host opening finishes every pending governed migration
before returning, and read-only inspection opens only current migration and
schema state.

Create-new state uses the borrowed `ServiceSqliteInitializer` executor rather
than a database path or raw connection. The runner reserves and retains the
exact canonical file, opens SQLite only through that retained descriptor, owns
`BEGIN IMMEDIATE`, and uses a private memory journal so descriptor-bound
initialization creates no path-derived SQLite sidecar. It commits the product
schema, shared `radroots_service_metadata`, empty v1 `schema_migrations`
ledger, and exact schema-catalog verification in one transaction. The
initializer screens the same closed transaction-control and attachment
inventory as host transactions; an ignored rejection still prevents commit.
Callback failure or cancellation cannot return a reusable connection or
publish a partial database.

Interactive callers may use `ServiceSqliteHost::open_or_initialize`. One held
writer authority and an exclusive create decide whether the sealed initializer
runs or the exact existing database is opened. The existing branch never runs
the initializer. Callers therefore do not use pathname probes, error-text
matching, recursive directory creation, permission repair, or direct SQLx
connections to choose the bootstrap branch. Success returns an
`OpenedServiceDatabase` that binds the retained host to the actual verified
metadata from the selected branch, including the persisted source generation
when state already existed.

Existing databases can be admitted without a caller guessing their stored
source generation. `ExistingServiceDatabaseIntent` seals the canonical
service and instance, supported schema ceiling, and SQLite application ID;
`ServiceSqliteHost::open_read_write_existing_with_intent` and
`ServiceSqliteHost::open_read_only_inspection_with_intent` discover and verify
the actual immutable metadata while retaining the corresponding writer or
inspection authority. Success returns an `OpenedServiceDatabase`,
which keeps the host and verified metadata inseparable until the caller
consumes them together. Recovery remains fail closed and may use this intent
only to discover the marker-bound generation; every other identity dimension,
artifact binding, migration prefix, and schema catalog remains governed.

Service-controlled SQL is screened before SQLite compilation through both the
borrowed transaction executor and migration callback executor. The closed
statement-control inventory is `PRAGMA`, `ATTACH`, `DETACH`, `BEGIN`, `COMMIT`,
`END`, `ROLLBACK`, `SAVEPOINT`, and `RELEASE`, regardless of case, whitespace,
comments, multiple statements, or prepared-query entry point. Rejection is
sticky for the transaction, so ignoring the immediate SQL error cannot permit
commit. Runner-owned setup remains private, and the complete connection policy
is revalidated before commit and before a connection can return to the pool.

Persisted SQLite text and blob values are admitted through bounded projections
before Rust decoding. Service and instance identifiers, source generations,
migration names, checksums, build identity, schema text, and database inventory
values carry an exact SQLite type, reported byte length, capped byte prefix,
and bounded row count. Integrity diagnostics use a borrowed-byte cap over the
exact `PRAGMA integrity_check(1)` operation and never convert an unbounded
diagnostic to UTF-8.

Cancelling a host transaction before the runner enables outer commit
quarantines its connection and leaves no authoritative transaction effect. A
service-operation error is returned only after rollback is confirmed; an
unconfirmed rollback is reported as `RollbackFailed`. Cancelling once outer
commit begins yields no result and must be treated as an unknown commit outcome.
Both that case and `CommitOutcomeUnknown` require rereading authoritative state
before an idempotent retry.

Every host must be closed explicitly with `ServiceSqliteHost::close`. Close
permanently stops new transaction admission, drains transactions that were
already admitted, and is safe to call sequentially or concurrently. Writable
close applies the fixed `PRAGMA wal_checkpoint(TRUNCATE)` policy, requires an
unblocked checkpoint, closes the private checkpoint connection, and explicitly
releases writer authority. Read-only inspection close drains its pool and
releases its shared inspection guard without checkpointing or mutating the
database or filesystem. Cancelling close before terminal completion leaves the
host non-admitting and retains authority; the private connect, checkpoint, and
explicit connection-close driver remains host-owned so a later call resumes
close without losing the SQLite handle or its close proof. Once authority
release is proven, the stable outer result is cached for every later call.
Dropping a host performs no asynchronous close work and is not proof that the
governed checkpoint and authority-release sequence completed.

`ServiceBackupManifest` is the stable model-only v1 backup identity. It admits
only compact canonical UTF-8 JSON in the frozen field order, capped at 1,024
bytes, and computes the external manifest SHA-256 over those exact bytes. The
v1 member array contains exactly one `state.sqlite` member with a nonzero byte
length and lowercase SHA-256; service, instance, nonzero source generation,
state schema, and injected creation time are explicit. SQLite and foreign-key
integrity are exactly `ok`, and protected material is always excluded.
Parsing proves only the strict structural and canonical contract. It rejects
unknown, duplicate, null, reordered, whitespace-altered, or version-drifted
input; member bytes, digest, SQLite identity, and actual integrity remain the
separate backup-verification boundary. Constructing or parsing the manifest
model performs no filesystem or SQLite work.

Writable hosts provide `ServiceSqliteHost::capture_online_backup` for one
incremental, point-in-time SQLite capture at a time. The caller supplies an
injected creation time and the exact new absolute staging-directory path under
an existing owner-controlled parent; capture creates that directory with mode
`0700` and its sole `state.sqlite` member with mode `0600`. It uses SQLite's
online-backup API without checkpointing or copying the live source file, then
requires exact service metadata, bounded `integrity_check`, an empty
`foreign_key_check`, a singleton member inventory, SHA-256, and file, staging,
and parent synchronization before returning the canonical manifest in memory.
No manifest file, bundle identifier, credential, or protected material is
written to the staging directory.

Capture rejects read-only, closing, unsupported, colliding, or concurrent
admission before publishing a result. Dropping the capture future requests
cancellation; the blocking worker retains its checked-out pool admission,
writer authority, and exact staging identities until SQLite handles are closed
and cleanup completes. Host close therefore drains capture and cancellation
cleanup before it checkpoints or releases authority. Capture has no hidden
timeout: callers own any deadline by cancelling the future. A completed capture
is still untrusted backup input until the separate verifier binds its manifest,
member bytes, expected intent, application metadata, and integrity; capture
does not provide restore or replacement behavior.

`verify_backup_bundle` is the synchronous, task-free boundary for an untrusted
manifest and bundle. The caller supplies the independently protected manifest
SHA-256, expected service database identity, and a positive maximum state-file
size. Verification requires canonical manifest bytes, exact service, instance,
source-generation, schema, and application intent, a restrictive owner-only
directory containing only `state.sqlite`, the exact bounded member length and
digest, immutable read-only/query-only SQLite access, main-only attachment,
bounded application metadata, `integrity_check(1)`, and an empty foreign-key
check. It performs no filesystem mutation and does not create a task or hidden
deadline.

Success returns a non-forgeable `VerifiedServiceBackup` that retains the exact
verified directory and member descriptors while exposing only the canonical
manifest and actual database metadata. It is not restore or replacement
authority, and it exposes no path or raw handle. Later restore work must copy
from the retained member and reverify the staged copy under its own supervised
blocking worker and deadline; pathname verification alone is insufficient.

Restore crash recovery uses a private sealed v1 marker stored beside canonical
service state. Its fixed layout names the live `state.sqlite`, staged
`state.restore-staged.sqlite`, retained `state.restore-backup.sqlite`, durable
`state.restore-marker.v1`, and create-new update scratch
`state.restore-marker.v1.next`. The compact canonical JSON is capped at 2,048
bytes and binds typed database intent, the protected source-manifest digest,
and exact live, staged, and retained-backup device, inode, length, and SHA-256
expectations. A domain-separated checksum binds the canonical fields and
detects corruption; it is not an authenticity credential.

The only legal durable sequence is `prepared` to `live_retained` to
`replacement_installed`; repeating the current phase is byte-idempotent and
every skip, reversal, or post-install transition fails closed. Marker files are
descriptor-relative, no-follow, single-link, owner-owned regular files with
mode `0600`. Creation synchronizes the file and state directory. Advancement
compare-and-reloads the current bytes, writes and synchronizes the fixed
create-new scratch, atomically replaces the marker, synchronizes the directory,
and reopens the exact new bytes. Stale scratch, tamper, collision, binding
replacement, insecure directory, or malformed marker remains evidence and
fails closed; reads do not repair or remove it.

This marker checkpoint does not stage, copy, open, rename, replace, or delete a
database. Later restore staging must consume a retained verified backup;
replacement must invoke the marker sequence around its governed renames; and
open-time recovery must reconcile durable marker and artifact identities before
removing any recovery evidence. No marker type, path, raw descriptor, or store
operation is public API.

`stage_verified_restore` is the offline boundary between retained backup proof
and live-state replacement. It first validates the expected identity plus exact
migration and schema catalogs, then acquires exclusive writer authority. A live
writable or read-only host, a live WAL/shared-memory/journal sidecar, an existing
stage, or any marker/retained-backup evidence fails closed. The only created
artifact is the fixed adjacent `state.restore-staged.sqlite`, opened create-new,
no-follow, owner-only, and single-link with mode `0600`.

Staging copies the exact manifest-bound bytes from the verifier's retained
member descriptor with a fixed-size buffer and digest, synchronizes the staged
file, and opens SQLite only through the retained staged descriptor. It then
rechecks immutable application metadata, the exact applied migration prefix and
schema-object catalog at the backup's actual supported version, main-only
read-only/query-only connection policy, bounded `integrity_check(1)`, and empty
`foreign_key_check`. A final retained-descriptor hash and file plus state-
directory synchronization precede success. Live database bytes, identity,
permissions, and timestamps remain untouched.

Success returns a sealed non-cloneable `StagedServiceRestore` that retains
writer authority and the exact staging identities. Dropping it attempts an
identity-checked unlink and state-directory synchronization before releasing
authority; cleanup failure leaves staging or recovery evidence that later
admission rejects. Cancelling the async operation requests bounded copy
cancellation; any detached work retains authority and exact cleanup ownership
until it ends.
The operation has no hidden timeout. It does not create or advance a recovery
marker, rename or retain live state, install a replacement, or authorize reopen;
those operations remain the finalization and recovery checkpoints.

`finalize_staged_restore` consumes that sealed stage in an owned blocking
worker. The stage has already bound the exact live inode, length, and digest
that will be retained. Finalization revalidates both retained descriptors,
creates and synchronizes the `prepared` marker, and only then disarms automatic
stage cleanup. It renames live to `state.restore-backup.sqlite` and staged to
live with descriptor-relative no-replace operations. Each rename is followed
by exact inode and hash verification, state-directory synchronization, and the
corresponding marker advance to `live_retained` or
`replacement_installed`.

Cancellation observed before the worker's atomic commit-ownership handoff
leaves live state untouched and attempts exact stage cleanup. Caller-task loss
after that handoff has an unknown immediate outcome, including the short
interval before `prepared` becomes durable; the worker retains writer
authority until it either fails before durability or establishes recovery
evidence and continues. Once `prepared` is durable, staged-artifact cleanup is
disarmed and the bound stage remains available after every later error.
Success returns no database host, retains the old live database and final
marker, and requires a new open. Read-write-existing open is the sole recovery
path. Under exclusive writer authority and before opening SQLite, it validates
the marker's exact service, instance, source generation, application ID, schema
ceiling, artifact identities, lengths, digests, restrictive modes, and the
absence of database sidecars. Read-only inspection, initialization, and an
initialized open never recover; they reject any stage, backup, marker, or marker
scratch as `Recovery` without mutation.

Recovery uses exact topology as the durable authority. `prepared` with the old
live database still installed rolls back by removing only the exact stage and
then the marker. Once the exact old live inode has reached the backup name,
recovery advances and rolls forward. A lagging `live_retained` phase installs or
recognizes the exact replacement, advances to `replacement_installed`, then
removes the exact old backup before retiring the marker. Interrupted rollback
and final cleanup accept only the corresponding already-absent exact artifact,
so repeated recovery is idempotent. Every other topology, sidecar, replacement,
link, mode, owner, length, digest, identity, or directory-authority mismatch
fails closed and preserves the evidence.

A marker scratch is admitted only when it is the canonical one-edge successor
of the current marker and the artifact topology already proves that successor.
Recovery removes only the exact bound scratch inode, synchronizes that removal,
then reproduces the transition through the governed marker-advance path. This
preserves the valid current marker if the scratch pathname was replaced.
Orphaned, malformed, skipped, same-phase, terminal, mismatched, or
topology-inconsistent scratch is never deleted or reinterpreted. Recovery has no
await point or hidden task: once a writable open is polled, each synchronous
filesystem step and its authority checks complete before the open can be
cancelled. A later cancelled SQLite open is retried by rereading the already
durable, marker-free state. Finalization itself does not reconcile or reopen the
database.

`ServiceSqliteHost::inspect_integrity` is the explicit active operator check.
It is available on initialized, writable-existing, and read-only inspection
hosts, admits at most one check per host, and uses one deferred read transaction
as the SQLite snapshot. The caller injects a positive wall-clock
`IntegrityCheckedAtUnixMs`; the library does not read an ambient clock or
create a timer. The completed report contains only `verified` or `failed` for
SQLite integrity and foreign keys, plus at most the fixed
`sqlite_integrity_failed` and `foreign_key_violation` diagnostic codes in that
canonical order. It can be projected to the passive `StorageIntegrity`
vocabulary, but the library does not persist or cache the report.

The check never publishes raw SQLite diagnostics, table or row identity,
filesystem paths, SQL, or dependency errors. Inability to execute, decode, or
finish either bounded check is an `Integrity` error rather than a fabricated
completed result. Authority is revalidated after every await and has precedence
over integrity classification. The operation has no hidden timeout or task.
Callers own a positive monotonic deadline by dropping the future; cancellation
returns no report, writes nothing, quarantines the checked-out connection, and
leaves it in a host-owned close driver. Retry or host close explicitly awaits
that retained close future until the prior SQLite worker terminates before any
new check or authority release. A retry uses a newly injected wall-clock time.
The strict backup and restore integrity verifier remains a separate fail-closed
boundary.

State-filesystem capacity inspection is an explicit synchronous input for
doctor checks and authoritative admission. `MinimumFreeBytes` must be supplied
and is constrained to `1..=i64::MAX`; it has no default. The value
`268435456` is the exact governed configuration and test vector, not an
implicit universal threshold. On Linux and macOS the platform adapter opens the
owner-owned state directory that is not group/other writable without following
links, retains and revalidates its identity, and uses `fstatvfs` to measure
bytes available to the unprivileged service user. The current native
qualification matrix is macOS aarch64 and Linux x86_64; other platforms fail
closed and successful compilation outside that matrix is not support evidence.

A successful immutable snapshot is `ready` when available bytes are greater
than or equal to the configured minimum and `low_disk` when they are below it.
Low disk rejects or pauses new authoritative admission; measurement failure is
a typed unavailable error and is never fabricated as low-disk evidence. A
consumer may cache the successful snapshot and later project low disk to the
stable `database_low_disk` readiness reason. `/readyz` remains passive and must
read only that caller-owned cached state; it never invokes the capacity
adapter. The measurement is advisory rather than a space reservation and does
not guarantee a later write.

Capacity inspection is host-independent and performs no database open, pool
operation, SQLite query, filesystem mutation, ambient time read, timer, task,
or hidden sampling. Service configuration, threshold defaults, cache refresh,
status persistence, admission wiring, and route projection remain consumer
responsibilities.

Durability fault injection is a private test-only mechanism. A closed
instance-scoped controller can arm exactly one named before/after boundary and
returns one injected error the first time that boundary is reached; later hits
are no-ops. The complete inventory covers database initialization, runner-owned
transaction begin and commit, online-backup creation/copy/synchronization,
restore-marker creation and advancement, both restore rename/synchronization
steps, and explicit host drain/checkpoint/connection-close/authority-release.
An ordinary controller has zero behavior, and no failpoint type or selector is
exported from the crate root. There is no process-global failpoint state,
environment or configuration selector, Cargo feature, hidden task, timer,
panic, or process-exit behavior. These deterministic in-process edges qualify
error ordering, rollback, cleanup, recovery evidence, and one-shot retry
semantics.

Process-crash qualification remains test-only and reuses Cargo's private
library and integration-test binaries; the crate ships no helper executable or
signal handler. A parent sends one bounded temporary root over stdin, waits for
a fixed stdout readiness token from an occurrence-aware failpoint barrier, and
then issues `SIGKILL`. The suite proves cross-process writer contention and
lock release plus five restore boundaries: orphan-stage refusal before a
durable marker, prepared rollback, interrupted marker-scratch promotion,
installed-replacement recovery, and terminal-marker cleanup. A permissive
child umask cannot broaden the fixed `0700` state directory or `0600` lock,
database, stage, backup, marker, and marker-scratch artifacts. Linux execution
on x86_64 is required for OS-level qualification; macOS aarch64 execution on
the current machine is developer evidence. No other platform or architecture
is an active qualification gate. These tests exercise process death at named
durable edges and do not claim abrupt power-loss or storage-device durability
behavior.

The crate owns mechanics only. Service-specific tables, SQL, repositories,
backup content policy, identity material, process lifecycle, and readiness
policy remain with the consuming service. The crate does not provide callers
with raw database authority.

Publication is disabled. The package is not part of the public Radroots crate
release closure.

## Public API and package boundary

This unpublished `private_runtime`, `package_private` crate owns only
service-neutral SQLite mechanics. Its built-in persistent objects are the
shared `radroots_service_metadata` and `schema_migrations` tables plus their
governed immutability triggers. Every service-owned table, index, trigger,
migration statement, and schema policy is supplied through the caller-owned
migration and schema catalogs; no product identifier or product table belongs
in this package.

The crate-root exports are frozen in the reviewed
[service-SQLite API baseline](../../contracts/api_baselines/radroots_service_sqlite.txt).
Raw pools, pooled or direct connections, transaction-control handles, and
dependency re-exports are forbidden. The deliberate narrow exception is the
`sqlx::Executor` implementation for borrowed
`&mut ServiceSqliteInitializer<'_>` and `&mut ServiceSqliteTransaction<'_>`
values. They permit compile-time typed queries. The crate retains connection
ownership and sole begin, commit, rollback, policy, and cancellation authority.
