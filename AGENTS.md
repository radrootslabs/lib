# Radroots Core Libraries - Agent Specification

See [AGENT_INSTRUCTIONS.md](AGENT_INSTRUCTIONS.md) for full instructions.

This file exists for compatibility with tools that look for AGENTS.md.

## 1. Scope and hierarchy

- This file applies to the full repository.
- Keep this file concise and durable.
- Put detailed procedures, examples, and extended guidance in `AGENT_INSTRUCTIONS.md`.
- If a closer directory-level `AGENTS.md` is added later, it overrides this file for that subtree.

## 2. Repository operating model

- This is a public open-source library workspace; optimize for durable library design, portability, determinism, and explicit contracts.
- Keep release and validation automation forge-agnostic; repo-owned scripts, Nix apps, tags, and contract metadata are canonical, while committed provider-specific workflow automation is not.
- Prefer clean target-state changes over compatibility scaffolding unless compatibility is explicitly required.
- Stay within the requested scope and the smallest coherent file set.
- Do not fold unrelated cleanup, speculative refactors, or roadmap work into the same change.
- Do not create hidden task trackers in markdown checklists, source comments, or stray notes.
- Keep commits and handoff language standalone and open-source-readable; do not reference internal monorepo paths, internal mapping rationale, or private repository context.

## 3. Preflight before edits

Before editing code:

- Read this file, `AGENT_INSTRUCTIONS.md`, `README`, `docs/nix.md`, and `spec/README.md`.
- Enter the canonical environment with `nix develop` or `direnv allow` before targeted cargo work.
- Discover commands from checked-in repo surfaces; do not invent ad hoc workflows.
- Read the current implementation and nearby tests before designing a change.
- Inspect `git status --short` before broad edits or refactors.
- Fail early when the task is blocked by missing prerequisites, contaminated scope, or unresolved public contract questions.

## 4. Canonical command surface

- `nix flake check`
- `nix run .#contract`
- `nix run .#release-preflight`
- targeted `cargo check -p <crate>` and `cargo test -p <crate>` only inside the Nix shell
- targeted `cargo run -q -p xtask -- ...` only when narrowing a repo-owned contract or export workflow
- if Beads is active, read `.beads/PRIME.md`

## 5. Rust engineering rules

- Use Rust `1.92.0`, edition `2024`, and workspace dependency versions from the root `Cargo.toml`.
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

## 6. Contract and release discipline

- `spec/`, `conformance/`, and `crates/xtask` are authoritative for public SDK contract, export, and release governance.
- Behavior changes that affect public surfaces must update the relevant contract metadata, conformance vectors, export rules, or validation flows in the same change.
- Keep pure flake checks and repo-aware command apps aligned with the documented Nix command map.

## 7. Commit directives

- Format commits as `<scope>: <imperative summary>`.
- Use lowercase scopes that match the crate or subsystem being changed.
- Leave a blank line after the summary when writing a multi-line commit.
- Use `- ` bullets for notable changes, validations, or compatibility notes when a body is needed.
- Split unrelated changes into separate commits.

## 8. Definition of done

- The requested change is implemented.
- Affected code, tests, docs, and contract surfaces are updated together.
- Relevant canonical validation ran, or a concrete blocker is reported.
- The handoff states what changed, what validations ran, and any follow-up risks or assumptions.
