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
marker, and requires a new open. Until open-time recovery is implemented,
every initialized, read-write-existing, or read-only-inspection pool open
refuses a marker or marker-scratch as `Recovery` before opening SQLite.
Finalization does not delete evidence, roll back, reconcile an interruption,
or reopen the database; those remain the next recovery checkpoint.

The crate owns mechanics only. Service-specific tables, SQL, repositories,
backup content policy, identity material, process lifecycle, and readiness
policy remain with the consuming service. The crate does not provide callers
with raw database authority.

Publication is disabled. The package is not part of the public Radroots crate
release closure.
