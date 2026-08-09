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

- Read `docs/specs/README.md` and
  `docs/specs/radroots_crates_release_v1.md` before changing a public package,
  package identity, dependency, feature, or release control.
- The Markdown specification is normative. Its TOML catalog is the executable
  package and dependency representation; the CSV and DOT files are review
  aids.
- Current source and tests are implementation evidence. They do not silently
  override `radroots.crates.release.v1`.
- Record any evidence-based plan deviation in
  `docs/implementation/deviations.toml`, following
  `docs/implementation/DEVIATIONS.md`, before proceeding. Validate it with
  `cargo xtask architecture`.

## 3. Repository operating model

- This is a public open-source library workspace; optimize for durable library design, portability, determinism, and explicit contracts.
- Keep release and validation automation forge-agnostic; repo-owned xtask commands, Nix apps, tags, and contract metadata are canonical, while committed provider-specific workflow automation is not.
- `.github/**` and capsule-local CI workflows are forbidden. Any required monorepo orchestration belongs exclusively to the parent repository's root `.act/**` authority and must not be copied into this standalone capsule.
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
- When touching Nix behavior, read `flake.nix` and the active Nix implementation files under `build/nix/`.
- Enter the canonical environment with `nix develop` or `direnv allow` before targeted cargo work.
- Discover commands from checked-in repo surfaces; do not invent ad hoc workflows.
- Read the current implementation and nearby tests before designing a change.
- Inspect `git status --short` before broad edits or refactors.
- Fail early when the task is blocked by missing prerequisites, contaminated scope, or unresolved public contract questions.

## 5. Canonical command surface

- `nix flake check`
- `nix run .#contract`
- `nix run .#release-preflight`
- `cargo xtask architecture` for controlled deviation records and local spec
  anchors
- targeted `cargo check -p <crate>` and `cargo test -p <crate>` only inside the Nix shell
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
- Keep pure flake checks and repo-aware command apps aligned with the documented Nix command map.
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
  authorized. Follow `docs/implementation/PUBLICATION_FREEZE.md`.

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
  `docs/implementation/deviations.toml`, following
  `docs/implementation/DEVIATIONS.md`. A normative architecture change also
  requires an approved decision record. Never silently skip or reorder work.

## 11. Definition of done

- The requested change is implemented.
- Affected code, tests, docs, and contract surfaces are updated together.
- Relevant canonical validation ran, or a concrete blocker is reported.
- The handoff states what changed, what validations ran, and any follow-up risks or assumptions.
