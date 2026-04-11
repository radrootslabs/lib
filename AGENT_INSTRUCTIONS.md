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
Release automation should stay forge-agnostic. Keep release truth in repo-owned scripts, Nix apps, tags, and contract metadata rather than committed provider-specific workflow files.

## 3. Preflight workflow

Before editing code:

- Read `AGENTS.md`.
- Read this file.
- Read `README`, `docs/nix.md`, and `contract/README.md` when the change touches workflow, exports, or public surfaces.
- Read the relevant crate manifest, implementation files, and nearby tests before proposing a new structure.
- Check `git status --short`.

Before running cargo commands:

- Prefer `nix develop` or `direnv allow`.
- Treat Nix as the canonical environment contract.
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
- `contract/`
  - public SDK contract metadata, export policy, release policy, and coverage governance
- `conformance/`
  - cross-language and cross-surface vector expectations
- `docs/`
  - durable workflow and environment documentation
- `nix/`, `flake.nix`, `treefmt.nix`
  - canonical environment and CI contract
- `scripts/`
  - repo-owned automation used by canonical lanes

Do not duplicate contract knowledge between crates when `contract/`, `conformance/`, or `xtask` already owns it.

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

`contract/`, `conformance/`, and `crates/xtask` are first-class parts of the product surface, not secondary metadata.

When a change affects exported models, transforms, identifiers, or public runtime expectations:

- update the relevant contract metadata
- update or add conformance vectors
- update repo-aware validation flows if needed
- keep release and export rules aligned with the new behavior

Do not change public behavior in Rust and leave contract or conformance assets stale.

## 7. Canonical validation strategy

Use the smallest authoritative lane that proves the change green.

Repo-wide canonical lanes:

- `nix flake check`
- `nix run .#contract`
- `nix run .#release-preflight`

Targeted iteration inside the Nix shell:

- `cargo check -p <crate>`
- `cargo test -p <crate>`
- `cargo run -q -p xtask -- sdk validate`
- `cargo run -q -p xtask -- sdk release preflight`

Validation rules:

- crate-local changes may iterate with targeted cargo commands
- contract, export, conformance, flake, release, or multi-crate changes should close on a canonical Nix lane
- deterministic tests are required for new behavior and edge cases
- do not rely on wall-clock time, random order, external network access, or ambient machine state in unit tests

Release discipline:

- create annotated release tags that match the root release policy at `contracts/release/mounted-rust-crates/publish-policy.toml` in the owning monorepo
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
- use shared build slots for long-running singleton lanes such as contract, release-preflight, or wasm-build runs

If Beads or Agent Mail is not active, the repo still follows the same coding and validation standards; only the task-state and coordination backend changes.
