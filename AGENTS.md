# Radroots Core Libraries - Agent Specification

See [CONTRIBUTING.md](CONTRIBUTING.md) for the contributor workflow and
[AGENT_INSTRUCTIONS.md](AGENT_INSTRUCTIONS.md) for extended execution detail.

This file exists for compatibility with tools that look for AGENTS.md.

## 1. Scope and hierarchy

- This file applies to the full repository.
- Keep this file concise and durable.
- Put detailed procedures, examples, and extended guidance in `AGENT_INSTRUCTIONS.md`.
- If a closer directory-level `AGENTS.md` is added later, it overrides this file for that subtree.

## 2. Source of intent

- Read `contracts/crates/release.v2.toml`,
  `contracts/crates/release_v1/radroots_crates_release_v1.toml`, and
  `contracts/crates/catalog.v2.toml` before changing a public package, package
  identity, dependency, feature, or release control.
- Machine contracts under `contracts/**` are the standalone authority. Human
  specifications, decisions, runbooks, and qualification evidence belong
  under the parent monorepo's `docs/oss/lib/**` authority and must never become
  a standalone build, test, package, or release input.
- The pre-implementation service-event reservation is
  `contracts/architecture/decisions/services_hardening_events.v1.json`.
  Service-event source, registry, generated, and consumer work must implement
  that exact kind, tag, cardinality, query, and supersession contract; it may
  not reinterpret the reservation from current prototype wire behavior.
- The pre-implementation local-admin, process-exit, doctor, readiness,
  peer-credential, systemd, and bare-Rust host decisions are reserved by
  `contracts/architecture/decisions/services_hardening_host.v1.json`.
  Service-host and service-owned operator contracts must implement or narrow
  that boundary without adding a second transport, exit map, or readiness
  authority.
- Source-lock consumer identities include `sdk`, `mobile`, `myc`, and `rhi`.
  Only the first two are generated-artifact product identities;
  accepting a service consumer marker must not expose an artifact route.
- The canonical service source lock is the bounded, canonical
  `radroots.service.source-lock.v2.toml` model. It binds the exact active public
  Lib repository and full revision, Lib source-archive and workspace-catalog
  digests, the service `Cargo.lock` digest, Rust `1.97.1`, the `service-host`
  feature profile, positive config, state, admin, status, and provider contract
  versions, and an exact closed Nix-material state. `absent` requires both Nix
  files to be absent. `deferred` independently binds an exact mutually
  consistent `flake.nix` and `flake.lock` revision and digest without claiming
  Nix qualification or active-revision alignment. Keep the model and
  diagnostics private to repo tooling, reject noncanonical or extra fields,
  and never put credentials, local paths, floating refs, or private repository
  identity in it.
- Generate or verify that lock with `cargo xtask service-source-lock --mode
  write|check --service-root <absolute-directory> --source-archive
  <absolute-bundle>`. The service root supplies the exact
  `workspace.metadata.radroots.service_source_lock` Cargo metadata. Every Lib
  dependency, the Cargo lock, source archive, and canonical public remote must
  agree on the active revision. Deferred Nix inputs must agree with each other
  and remain remotely reachable, but need not equal the active revision before
  terminal Nix alignment. The command rejects every source-tree change except
  the exact generated lock path.
- Generate or verify one immutable service release artifact set with `cargo
  xtask service-release-artifacts --mode write|check --service-root
  <absolute-directory> --input-root <absolute-directory> --output-root
  <absolute-directory> --target <rust-target> --source-date-epoch <seconds>`.
  The command consumes the exact fixed input inventory, validates clean and
  stable service and Lib source bundles, and emits the canonical binary
  archive, OCI/source metadata, CycloneDX SBOM, notices, manifest, unsigned
  provenance signing input, and checksums. Signing credentials and signatures
  remain external; generated artifacts must contain no protected material.
- The native shared-build qualification contract is
  `contracts/architecture/decisions/services_hardening_build_qualification.v2.json`.
  It freezes the supported Rust targets, standalone Cargo and xtask commands,
  native release evidence, and the fixture agreement among Cargo metadata,
  the source lock, and release metadata. Nix package/app/check, development
  shell, NixOS-module, and Nix-produced OCI outputs are explicitly deferred
  and are not qualified by that contract.
- Current source and tests are implementation evidence. They do not silently
  override `radroots.crates.release.v1`.
- Record any evidence-based plan deviation in
  `contracts/architecture/deviations.toml` before proceeding. Validate it with
  `cargo xtask architecture`; a normative architecture exception also requires
  the applicable machine decision under `contracts/architecture/decisions/**`.
  Deviation anchors must resolve the Release V1 TOML through a validated
  selector: `repositories.<name>`, `repository_policy`, `release_policy`,
  `quality_policy.coverage`, or `package.<name>`.

## 3. Repository operating model

- This is a public open-source library workspace; optimize for durable library design, portability, determinism, and explicit contracts.
- Keep release and validation automation forge-agnostic; repo-owned xtask
  commands, native Cargo lanes, tags, and contract metadata are canonical,
  while committed provider-specific workflow automation is not. Checked-in
  Nix surfaces are deferred compatibility inputs, not current qualification
  authority.
- Do not add or retain tracked `docs/**`, `.github/**`, or `.act/**` content.
  Keep validation forge-agnostic. Any required monorepo orchestration belongs
  exclusively to the parent repository's root `.act/**` authority and must not
  be copied into this standalone capsule.
- Prefer clean target-state changes over compatibility scaffolding unless compatibility is explicitly required.
- Stay within the requested scope and the smallest coherent file set.
- Do not fold unrelated cleanup, speculative refactors, or roadmap work into the same change.
- Do not create hidden task trackers in markdown checklists, source comments, or stray notes.
- Keep commits and handoff language standalone and open-source-readable; do
  not reference non-public repository paths, internal mapping rationale, or
  private repository context.

## 4. Preflight before edits

Before editing code:

- Read this file, `AGENT_INSTRUCTIONS.md`, and `README`.
- When preserving deferred Nix behavior, read `flake.nix` and the relevant
  implementation files under `build/nix/`, but do not install, invoke, or
  require Nix as part of current qualification.
- Run `cargo extbuild doctor` before the first governed build, test, check,
  generation, package, artifact, or release-preflight command, then route the
  command through `cargo extbuild run --`.
- Discover commands from checked-in repo surfaces; do not invent ad hoc workflows.
- Read the current implementation and nearby tests before designing a change.
- Inspect `git status --short` before broad edits or refactors.
- Fail early when the task is blocked by missing prerequisites, contaminated scope, or unresolved public contract questions.

## 5. Canonical command surface

- `cargo extbuild run -- cargo check --workspace --all-targets --locked`
- `cargo extbuild run -- cargo test --workspace --all-targets --locked`
- `cargo extbuild run -- cargo xtask contract validate`
- `cargo extbuild run -- cargo xtask release preflight`
- `cargo extbuild run -- cargo xtask architecture` for controlled deviation records and local spec
  anchors
- Public API baselines live in `contracts/api_baselines/**`. Regenerate one
  with `cargo-public-api` `0.52.0` and rustdoc JSON from
  `nightly-2026-07-16`, writing the reviewed output back to that directory.
- targeted `cargo check -p <crate>` and `cargo test -p <crate>` through
  `cargo extbuild run --`
- `cargo xtask dto-roots --write` after changing configured DTO exports and
  `cargo xtask dto-roots --check` for exact generated-root freshness
- targeted `cargo xtask contract ...`, `cargo xtask coverage ...`, `cargo xtask release ...`, or `cargo xtask hygiene ...` only when narrowing a repo-owned workflow
- `cargo xtask hygiene prototype-contracts` for the governed report-only
  service-prototype census; use `--strict` only when the cleanup sequence has
  made every non-allowlisted finding release-blocking
- if Beads is active, read `.beads/PRIME.md`

## 6. Rust engineering rules

- Use Rust `1.97.1`, edition `2024`, resolver `3`, and workspace dependency
  versions from the root `Cargo.toml` after the release-v1 workspace cutover.
- Preserve intended `no_std` portability; gate `std`, wasm, and runtime-specific behavior explicitly.
- Keep core logic functional and composable: prefer pure transformations, explicit state, and narrow side-effect boundaries.
- Prefer enums, newtypes, and typed domain models over stringly APIs, boolean mode switches, or loosely typed maps.
- Avoid hidden panics in library code; reserve `unwrap` and `expect` for tests, build tooling, or proven internal invariants.
- Prefer typed public error surfaces; do not expose opaque convenience errors as stable library contracts.
- Avoid `unsafe` unless it is strictly necessary and documented by invariants close to the code.
- Borrow first, clone late, and allocate intentionally.
- Keep `lib.rs` thin as a module manifest and public re-export surface.
- Treat generated bindings and generated type artifacts as generated; do not hand-edit them.
- Add or update deterministic tests for new behavior, invariants, parsing, conversions, feature gates, and cross-target behavior where relevant.

## 7. Architecture, contract, and release discipline

- `contracts/` and `tools/xtask` are authoritative for core-library contracts, conformance, coverage, hygiene, and release-candidate governance.
- `contracts/crates/catalog.v2.toml` is the package-catalog authority. Preserve
  imported packages as `provenance_kind = "imported"` with their immutable
  repository, revision, path, and tree digest. New repository-native packages
  must be active, unpublished `provenance_kind = "native"` entries and must
  store only `introduction_tree_sha256`; never embed a self-referential
  introducing commit OID.
- Before validating a new native catalog entry, stage the complete package path
  and run `cargo xtask catalog check` or `cargo xtask catalog write`. The
  pre-commit digest is derived from stage-zero index records, not the mutable
  worktree. After the introducing commit, the same command derives the first
  adding commit from repository history and verifies its immutable tree. Do
  not rewrite that digest for later source changes.
- Behavior changes that affect public surfaces must update the relevant contract metadata, conformance vectors, export rules, or validation flows in the same change.
- Preserve deferred flake expressions as unqualified compatibility inputs;
  do not use their evaluation or outputs as evidence until an accepted
  contract explicitly reactivates them.
- This repository owns packages 1-17 in `radroots.crates.release.v1`, from
  `radroots_core` through `radroots_geonames`. `radroots_sdk` and `radroots`
  remain owned by the standalone SDK repository.
- Public packages have no dependency on private Radroots packages. Every
  Radroots dependency edge points downward in the approved graph.
- Domain and protocol packages do not own storage, live networking, host UI,
  executors, schedulers, or process-global behavior.
- Generic SPIs do not expose concrete SQLx, Tokio, Reqwest, Nostr SDK,
  keyring, or operating-system implementation types.
- Preview, code-generation, fixture, binding-generator, coverage, xtask, and
  implementation-assembly packages remain private and absent from published
  feature closures.
- During the migration, every package remains non-publishable until its
  package-realistic release gates pass and publication is explicitly
  authorized. `contracts/releases/publish_policy.toml` is the machine
  authority; validation metadata does not authorize upload.

## 8. Service hardening boundaries

- Service hardening is clean-slate: do not add or preserve prototype
  configuration readers, environment-file configuration, prototype state
  importers, JSON/JSONL mutable state, fallback path searches, compatibility
  aliases, deprecated modules/APIs/re-exports, dual wire encodings, or old/new
  feature switches. Update affected consumers directly.
- `radroots_service_host` owns reusable host mechanics only, and
  `radroots_service_sqlite` owns reusable SQLite mechanics only. Neither crate
  may contain Myc or RHI domain configuration, tables, policy, or business
  rules, and neither may become a broad lifecycle framework.
- Each service instance has one live SQLite database. Keep its pool private to
  the owning store, keep live mutable state daemon-owned, and route live-state
  mutations from local tools through the typed, permissioned Unix-socket
  local-admin boundary.
- `radroots_service_sqlite::ServiceSqliteHost` is the sole public owner of the
  private SQLx pool. Service code may execute typed SQLx queries only through
  the sealed `&mut ServiceSqliteTransaction` executor passed to
  `ServiceSqliteHost::transaction`; do not expose or reconstruct raw pools,
  pooled connections, SQLx transactions, commit/rollback handles, or inner
  accessors. Do not attach or detach secondary SQLite databases through the
  transaction executor. Writable host construction must finish governed
  migrations before returning, while read-only inspection must require current
  migration and schema state. Every host owner must explicitly await
  `ServiceSqliteHost::close`: close permanently stops admission, drains admitted
  work, applies the fixed unblocked `TRUNCATE` WAL checkpoint for writable
  hosts only, closes its private checkpoint connection, and explicitly releases
  writer or inspection authority. A cancelled close retains authority and must
  be resumed through the host-owned connect/checkpoint/connection-close driver;
  Drop is not an asynchronous close or completion proof. Do not add public
  checkpoint knobs, background close tasks, or Drop-based async cleanup.
- `ServiceBackupManifest` is the sole v1 backup-manifest model. Preserve its
  exact 1,024-byte compact canonical JSON, raw canonical-byte SHA-256, typed
  service/instance/source-generation/schema/time binding, singleton
  `state.sqlite` inventory, exact `ok` integrity projection, and mandatory
  protected-material exclusion. Parsing is structural only and must not perform
  filesystem or SQLite work. Online capture belongs only to the writable
  `ServiceSqliteHost`: admit one capture at a time, use SQLite's incremental
  online-backup API, create a caller-selected new owner-only staging directory,
  return the manifest in memory, and retain host authority until success or
  exact-artifact cancellation cleanup completes. Do not expose raw backup
  handles, capture credentials, invent a manifest filename, read an ambient
  clock, or fold untrusted verification or restore behavior into capture.
- Untrusted backup verification is the synchronous, task-free
  `verify_backup_bundle` boundary. Require an independently protected manifest
  digest, exact `ServiceDatabaseIdentity`, and caller-supplied positive member
  limit; retain the verified directory and member descriptors in the sealed
  non-cloneable proof. Do not expose paths or raw handles, treat pathname-only
  verification as restore authority, create an internal task/deadline, mutate
  the bundle, or introduce restore markers, staging, replacement, or recovery
  into verification. Later restore work must consume the retained member and
  reverify its staged copy.
- Restore recovery markers are private `radroots_service_sqlite` mechanics.
  Preserve the fixed sibling names, exact 2,048-byte canonical v1 JSON,
  domain-separated self-checksum, typed database and backup intent, and the
  only legal durable sequence `prepared -> live_retained ->
  replacement_installed`. Marker creation and advancement require retained
  writer authority, descriptor-relative owner-only files, exact inode
  revalidation, file and parent synchronization, and create-new scratch plus
  atomic replacement. Reads never repair or remove evidence. Do not expose
  marker types or paths, truncate markers in place, accept caller-selected
  names, or move, copy, open, or delete a database in the marker checkpoint;
  restore staging, replacement, and open-time recovery remain separate steps.
- Offline restore staging consumes a sealed `VerifiedServiceBackup`, acquires
  exclusive writer authority after every governed host has closed, and creates
  only the fixed adjacent `state.restore-staged.sqlite` file. It must copy from
  the retained source descriptor, reverify exact metadata, migration prefix,
  schema catalog, integrity, foreign keys, length, and digest through retained
  descriptors, and keep authority plus exact cleanup ownership across caller
  cancellation. The returned sealed capability owns the staged inode until
  finalization or an identity-checked drop cleanup attempt; failed cleanup must
  remain evidence that later admission rejects. Staging must not create a
  marker, rename live state, retain an old live database, or install a
  replacement; those are later finalization and recovery boundaries.
- Atomic restore finalization consumes only a sealed `StagedServiceRestore`.
  Staging must bind the exact live inode, length, and digest that finalization
  will retain. The owned blocking worker creates and synchronizes `prepared`
  before disarming stage cleanup, then uses descriptor-relative no-replace
  renames and parent synchronization for live-to-backup and staged-to-live,
  advancing the marker only after each durable rename. Cancellation observed
  before the worker atomically claims commit ownership may cleanly stop;
  caller loss after that handoff is an unknown immediate outcome, including
  the interval before `prepared` is durable. Once `prepared` is durable, stage
  cleanup must remain disarmed after every later error so the marker never
  loses a bound artifact.
  Successful finalization returns no host and leaves the old live database and
  `replacement_installed` marker for the next writable open to recover. Other
  open modes reject that evidence as `Recovery`. Finalization itself must not
  roll back, delete recovery evidence, reopen SQLite, or expose paths,
  descriptors, marker controls, or rename controls.
- Interrupted restore recovery is private and automatic only for
  read-write-existing open under exclusive `WriterAuthority`, before any
  SQLite connection or await point. Initialize, initialized-open, and
  read-only inspection must reject every fixed stage, backup, marker, or
  marker-scratch artifact without mutation. Recovery must bind the marker to
  the requested database identity, hash and revalidate exact owner-only
  single-link artifacts, reject sidecars, and let exact topology decide the
  sole action: roll back `prepared` while old live is still installed, then
  roll forward once old live is durably retained. Persist every inferred phase
  before the next destructive step; retire exact backup before marker; and
  admit marker scratch only as a canonical topology-consistent one-edge
  successor whose exact bound inode is removed and durably reproduced through
  the governed marker-advance path without overwriting the valid marker.
  Repeated recovery may
  finish already-absent stage or backup cleanup, but every other missing,
  replaced, linked, malformed, mismatched, or ambiguous artifact remains
  `Recovery` evidence. Do not expose recovery controls, add a background task
  or hidden timeout, repair without writer authority, or fold Step 070
  integrity/status APIs and the later process failpoint harness into recovery.
- Explicit active integrity inspection belongs only to
  `ServiceSqliteHost::inspect_integrity`. It admits at most one check per host,
  uses one governed read snapshot, accepts an injected positive wall-clock
  timestamp, and returns only the closed SQLite/foreign-key outcomes plus at
  most two stable diagnostic codes in canonical order. Preserve authority
  precedence after every await. Do not expose raw SQLite diagnostics, paths,
  SQL, pool handles, or dependency errors; persist or cache the result; read an
  ambient clock; create a timer/task; or weaken the strict restore/backup
  integrity verifier. Callers own the monotonic deadline by cancelling the
  future. The host-owned integrity driver must retain a cancelled in-flight
  connection and its explicit close future until the SQLx worker terminates;
  retry and host close resume that cleanup before proceeding. A retry must
  inject a new timestamp.
- State-filesystem capacity inspection is an explicit, synchronous,
  host-independent doctor and admission input. Callers must supply a positive
  `MinimumFreeBytes`; there is no default threshold. The platform adapter
  measures unprivileged available bytes through a retained owner-owned state
  directory descriptor that is not group/other writable, and the immutable
  result classifies exact equality as
  ready and anything below the policy as low disk. Measurement failure is a
  typed unavailable result, never fabricated low-disk evidence. Consumers may
  cache a successful snapshot and project low disk to the stable
  `database_low_disk` reason, but passive readiness handlers must never invoke
  the adapter. Keep inspection advisory: do not add a reservation, host/pool or
  SQLite dependency, ambient timer, background sampler, service default, or
  status persistence to this crate.
- Durability failpoints are private, instance-scoped test mechanics only. Keep
  a closed before/after inventory across initialization, transaction commit,
  online backup, restore-marker persistence, restore rename/synchronization,
  and explicit close. One armed controller may fail one selected edge once;
  ordinary controllers have zero behavior. Never export failpoint types, use
  process-global failpoint state, select a point from environment or service
  configuration, or add a Cargo feature that alters production behavior.
  Process-level crash and signal qualification must remain in private Cargo
  test binaries. Pass only one bounded temporary root over stdin, require a
  fixed stdout token from an occurrence-aware failpoint barrier before
  `SIGKILL`, and retain a parent kill-on-drop watchdog. Cover writer-lock death
  and the exact pre-marker, prepared, marker-scratch, installed-replacement,
  and terminal-marker restore topologies under a permissive child umask.
  Require Linux x86_64 execution for OS-level qualification; macOS aarch64 on
  the current machine is developer evidence only. No other platform or
  architecture is an active qualification gate. Do not ship a helper binary,
  add production signal/process behavior, poll filesystem state for crash
  timing, or claim abrupt power-loss durability from process-death tests.
- Runtime-management flows consume a sealed `RuntimeContext` for every service
  instance. They must not reconstruct service paths from raw identifiers,
  ambient selectors, or manager-owned roots, and registries must not persist
  duplicate config, state, logs, run, secrets, or binary paths. Manager-owned
  install and process-tracking artifacts remain separate; uninstall and
  cleanup must never recursively delete canonical service state or secrets.
  The manager has no credential read/write authority, executable artifact
  names are validated single path components, and ordinary manager errors and
  `Debug` output must not expose filesystem paths, file contents, or raw
  dependency-owned causes.
- Runtime-path consumers must use the sole typed
  `services/<service>/<instance>` model through `RuntimeContext`. The generic
  app/service/worker/shared namespace, public raw root containers, path
  overrides, ambient process-environment selectors, bootstrap helpers, and
  duplicate service-instance path constructors are removed breaking surfaces;
  do not restore them or add compatibility aliases.
- Runtime-distribution and runtime-management service metadata is the sealed
  exact Myc/RHI v1 inventory. Both services support multiple validated
  instances, one TOML config, explicit initialization with existing-only run,
  detailed HTTP/1.1-over-Unix local administration, cached
  `/livez`/`readyz`/`metrics`, and only Linux x86_64/aarch64 Tier-1 eligibility
  in `target` posture. This metadata does not authorize service registration,
  PID/config/log probing, lifecycle actions, artifact names, channels, archive
  resolution, or a `qualified` support claim.
- Library code must not initialize a tracing subscriber, parse a process CLI,
  read service configuration from environment variables, install signal
  handlers, create a Tokio runtime, call `process::exit`, or spawn arbitrary
  signer executables.
- Inject time, entropy, transport, providers, and failpoints. Bound queues,
  pools, retries, requests, responses, and collections; redact sensitive data
  from logs, status, metrics, fixtures, errors, and ordinary `Debug` output.
- Preserve public Nostr interoperability while removing Radroots-owned
  prototype behavior; clean-slate rules never authorize protocol drift.

## 9. Irreversible actions

Do not publish crates, create release tags, change crates.io ownership, merge
or rename repositories, merge pull requests, rotate credentials, or mutate
trusted-publisher configuration without explicit authorization.

## 10. Commit and deviation directives

- Format commits as `<scope>: <imperative summary>`.
- Use lowercase scopes that match the crate or subsystem being changed.
- Leave a blank line after the summary when writing a multi-line commit.
- Use `- ` bullets for notable changes, validations, or compatibility notes when a body is needed.
- Split unrelated changes into separate commits.
- If repository evidence proves a planned step obsolete or unsafe, record the
  evidence, affected specification anchor, disposition, and validation in
  `contracts/architecture/deviations.toml`. A normative architecture change
  also requires an approved machine decision under
  `contracts/architecture/decisions/**`. Never silently skip or reorder work.

## 11. Definition of done

- The requested change is implemented.
- Affected code, tests, docs, and contract surfaces are updated together.
- Relevant canonical validation ran, or a concrete blocker is reported.
- The handoff states what changed, what validations ran, and any follow-up risks or assumptions.
