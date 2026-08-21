# Radroots Core Libraries - Agent Instructions

**For repository overview and setup, see [README](README). For repository rules, see [AGENTS.md](AGENTS.md).**

This document contains detailed operational instructions for contributors and coding agents working on development, testing, and releases in the Radroots Core Libraries repository.

`AGENTS.md` is the concise repo contract. Read it first, then use this file for execution detail.

## 1. How to use this file

- Treat `AGENTS.md` as the durable always-on contract.
- Use this file for interpretation, procedures, and detailed engineering expectations.
- If a closer subtree-specific `AGENTS.md` is added later, that file overrides root guidance for its scope.
- Keep durable rules short and proven; if a problem repeats, tighten the root contract instead of growing ad hoc prompt text.

## 2. Repository operating model

This repository is a public open-source Rust workspace. Optimize for:

- portable library design
- deterministic behavior
- explicit contracts
- cross-target consistency
- clean public APIs

Stay disciplined:

- keep scope tight
- avoid drive-by cleanup
- avoid speculative abstraction
- avoid compatibility scaffolding unless it is explicitly required
- do not leave dead paths, temporary adapters, or silent fallback behavior behind

This repo is a library workspace, not an app monolith. The right default is small, durable changes that preserve clean crate boundaries.
Release automation should stay forge-agnostic. Keep release truth in repo-owned
xtask commands, native Cargo lanes, tags, and contract metadata rather than
committed provider-specific workflow files. Checked-in Nix surfaces are
deferred compatibility inputs and are not current qualification authority.

## 3. Preflight workflow

Before editing code:

- Read `AGENTS.md`.
- Read this file.
- Read `README` when the change touches workflow or public surfaces.
- When preserving deferred Nix behavior, read `flake.nix` and the relevant
  implementation files under `build/nix/`, but do not install, invoke, or
  require Nix as part of current qualification.
- Read the relevant crate manifest, implementation files, and nearby tests before proposing a new structure.
- Check `git status --short`.

Before running governed build, test, check, generation, package, artifact, or
release-preflight commands:

- Run `cargo extbuild doctor` once for the working session.
- Route the command through `cargo extbuild run --`.
- Prefer the documented repo-owned command surface over improvised local commands.

Fail early when:

- the environment is missing required tooling
- the task materially changes a public contract without enough local context
- the working tree is contaminated in a way that changes the requested scope

## 4. Workspace interpretation

Use this mental model:

- `crates/`
  - library crates and workspace tooling crates
  - keep domain logic inside the correct crate rather than spreading it across the workspace
- `contracts/`
  - core-library contract metadata, release-candidate policy, coverage governance, and public conformance assets
- `contracts/api_baselines/`
  - reviewed generated public Rust API surfaces
- `contracts/architecture/`
  - machine-readable deviations, decisions, and compatibility-retirement authority
- `contracts/crates/release_v1/`
  - historical machine catalog, inventory, graph, and checksums retained by release V2
- `contracts/conformance/`
  - cross-language and cross-surface vector expectations
- `build/nix/`, `flake.nix`, `treefmt.nix`
  - deferred compatibility surfaces whose evaluation and outputs are not
    current qualification evidence
- `tools/xtask/`
  - typed repo-owned automation used by canonical lanes

Do not duplicate contract knowledge between crates when `contracts/`, `contracts/conformance/`, or `tools/xtask` already owns it.

Do not add or retain tracked `docs/**`, `.github/**`, or `.act/**`. Root
`README.md`, `AGENTS.md`, `AGENT_INSTRUCTIONS.md`, conventional public project
files, package READMEs, and Rustdoc carry concise standalone guidance. Extended
human authority is parent-owned and is never a standalone command input.

Deviation `spec_anchors` target the Release V1 TOML and must use one of the
machine selectors enforced by `cargo xtask architecture`:
`repositories.<name>`, `repository_policy`, `release_policy`,
`quality_policy.coverage`, or `package.<name>`. Markdown heading fragments and
unresolved free-form fragments are invalid.

## 5. Rust engineering standards

### Core design

- Prefer pure functions and explicit data flow in core logic.
- Keep IO, filesystem, network, clocks, randomness, and runtime glue at the edges.
- Prefer data transformation pipelines over stateful orchestration when the problem is fundamentally transformational.
- Prefer explicit state machines and enums over ad hoc flags or loosely related booleans.
- Keep mutation local and minimal.
- Avoid hidden shared mutable state and interior mutability unless the boundary truly requires it.

### API design

- Public APIs should make invalid states hard to represent.
- Prefer newtypes, enums, and dedicated structs when semantics matter.
- Avoid exposing dependency-specific types in public API surfaces unless that dependency is a deliberate part of the contract.
- Separate parsing, validation, normalization, and serialization instead of collapsing them into a single opaque function.
- Prefer exhaustive `match` behavior for semantic enums over wildcard-heavy control flow.

### Errors and invariants

- Library code should not panic on normal invalid input.
- Reserve `unwrap`, `expect`, and panic-based control flow for tests, build scripts, or tightly proven internal invariants.
- Use precise typed errors for public and semantically important boundaries.
- Keep opaque convenience errors inside binaries, narrow tooling layers, or internal glue when appropriate.
- When an invariant truly cannot be violated, document it close to the code.

### Portability and feature discipline

- Preserve `no_std` intent where the crate is designed for it.
- Gate `std` behavior, wasm behavior, and runtime-specific behavior explicitly and predictably.
- Keep feature interactions simple and testable.
- When a change affects native, wasm, or `std`/`no_std` parity, update the affected tests or validation flow in the same change.

### Performance and allocation

- Borrow before cloning.
- Prefer `&str`, `&[u8]`, slices, and iterators when ownership is not required.
- Avoid unnecessary intermediate allocations.
- Preallocate only when the size is known or bounded meaningfully.
- Do not trade away clarity for micro-optimizations unless profiling or the hot-path nature of the code justifies it.

### Module layout

- Keep `lib.rs` thin.
- Put heavy logic in focused modules.
- Avoid giant files that mix models, parsing, validation, transformations, and integration glue.
- Introduce traits only when they remove real duplication or encode a stable abstraction boundary.
- Avoid generic abstraction that makes the code harder to reason about without clear reuse value.

### Documentation and source comments

- Do not add explanatory comments by habit.
- Add concise Rustdoc for non-obvious public APIs, invariants, and cross-target behavior.
- Keep docs aligned with the actual code and contract surface.

## 6. Contract, conformance, and release workflow

`contracts/`, `contracts/conformance/`, and `tools/xtask` are first-class parts of the product surface, not secondary metadata.

The package authority is `contracts/crates/catalog.v2.toml`. Imported entries
retain their exact immutable source repository, full revision, source path, and
source-tree digest. A newly created repository-native package instead uses
`provenance_kind = "native"` and records only its
`introduction_tree_sha256`; native entries are active and unpublished. Stage
the complete new package path before running `cargo xtask catalog check` or
`cargo xtask catalog write`. Before the first commit, xtask verifies the digest
against canonical stage-zero index tree records. After that commit, xtask
derives the earliest adding commit from history and verifies the same digest
against that commit's package tree. Later source changes do not change the
introduction digest, and the catalog never stores the introducing commit OID.

When a change affects exported models, transforms, identifiers, or public runtime expectations:

- update the relevant contract metadata
- update or add conformance vectors
- update repo-aware validation flows if needed
- keep release and export rules aligned with the new behavior

Do not change public behavior in Rust and leave contract or conformance assets stale.

Public API baselines are generated with `cargo-public-api` `0.52.0` and
rustdoc JSON from `nightly-2026-07-16`; the workspace's pinned stable toolchain
still governs package verification. From the canonical development shell,
regenerate one package with:

```sh
RUSTC="$(rustup which --toolchain nightly-2026-07-16 rustc)" \
RUSTDOC="$(rustup which --toolchain nightly-2026-07-16 rustdoc)" \
cargo public-api --manifest-path crates/<crate>/Cargo.toml \
  --all-features -sss \
  > contracts/api_baselines/<package>.txt
```

Review each baseline change with the package's machine charter and intended
SemVer impact. Generated listings are evidence of the Rust surface, not
authority to expand it.

## 7. Canonical validation strategy

Use the smallest authoritative lane that proves the change green.

Repo-wide canonical lanes, all routed through `cargo extbuild run --`:

- `cargo check --workspace --all-targets --locked`
- `cargo test --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo doc --workspace --no-deps`
- `cargo xtask contract validate`
- `cargo xtask release preflight`

Targeted iteration, also routed through `cargo extbuild run --`:

- `cargo check -p <crate>`
- `cargo test -p <crate>`
- `cargo xtask dto-roots --check`
- `cargo xtask dto-roots --write` after changing configured DTO exports
- `cargo xtask hygiene forbidden-identifiers`
- `cargo xtask hygiene prototype-contracts` for the deterministic report-only
  service-prototype census; strict mode is enabled only after the owning
  cleanup sequence clears its findings

Validation rules:

- crate-local changes may iterate with targeted cargo commands
- contract, export, conformance, release, or multi-crate changes should close
  on the applicable extbuild-routed repository-wide lanes
- Nix evaluation and Nix-derived package, app, check, development-shell,
  NixOS-module, and OCI outputs remain explicitly deferred and unclaimed
- deterministic tests are required for new behavior and edge cases
- do not rely on wall-clock time, random order, external network access, or ambient machine state in unit tests

Release discipline:

- create annotated release tags that match the current versioned release contract under `contracts/releases/`
- keep repo-owned release commands runnable without depending on GitHub-specific workflow files
- when documenting release flow here, document the local repo contract rather than forge-specific orchestration

## 8. Commit and handoff guidance

Commit messages in this repo are part of the public open-source surface.

That means:

- use `<scope>: <imperative summary>`
- keep the scope lowercase and meaningful
- keep the summary standalone and readable outside monorepo context
- do not reference internal repository paths, internal migration rationale, or private coordination context
- when using a body, leave a blank line after the summary and use `- ` bullets

Handoffs should state:

- what changed
- what validations ran
- any assumptions made
- any follow-up risks or missing work

## 9. Beads and Agent Mail

If Beads is active for the task:

- use `.beads/PRIME.md` as the Beads-specific operator layer
- keep live execution state in Beads rather than markdown task lists
- do not use `bd edit`
- use Beads for durable multi-commit work, not as a replacement for contract docs or repo docs

If Agent Mail is active for the task:

- use `.beads/PRIME.md` for the repository coordination conventions
- use the active Beads issue id as the Agent Mail thread id and reservation reason when both tools are active
- reserve files before the first write for coordinated multi-agent work
- use shared build slots for long-running singleton lanes such as contract or release-preflight runs

If Beads or Agent Mail is not active, the repo still follows the same coding and validation standards; only the task-state and coordination backend changes.
