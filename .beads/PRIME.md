# Radroots Core Libraries Beads Prime

Read `AGENTS.md` first. That file remains the authoritative code and commit-style contract for this repository.

## RCL
- This repo uses RCL for multi-commit work.
- When Beads is active, Beads is the live execution state.
- Every goal is an epic.
- Only the next 1-3 slices may be concrete at any time.
- Every slice needs a clear scope, a definition of green, and one dominant verify lane.
- Do not create markdown TODO trackers when Beads is active.
- Do not use `bd edit`.

## Start Of Session
- `bd ready --json`
- `bd show <id>`
- `bd update <id> --claim --json`
- Trust cwd auto-filtering when working from a mapped crate or repo surface.

## Environment Contract
- Nix is the canonical environment contract for this workspace.
- Prefer `nix develop` or `direnv allow` before targeted cargo work.
- Prefer repo-owned Nix lanes over ad hoc raw commands for closeout validation.

## Slice Rules
- Keep slices small enough that one dominant verify lane can prove them green.
- If a slice needs unrelated lanes, split it.
- If new work is discovered, create it immediately and link it with `discovered-from:<parent-id>`.
- Use stdin-based `bd create` or `bd update` forms when text contains backticks or quotes.

## Verify Lanes
- repo-wide pure formatting, workflow linting, and Rust check/test validation
  - `nix flake check`
- repo-aware SDK contract, export, and manifest validation
  - `nix run .#contract`
- release workflow or publish-surface changes
  - `nix run .#release-preflight`
- targeted crate iteration inside the Nix shell
  - `cargo check -p <crate>`
  - `cargo test -p <crate>`
- targeted xtask iteration inside the Nix shell
  - `cargo run -q -p xtask -- sdk validate`
  - `cargo run -q -p xtask -- sdk release preflight`

## Closeout Guidance
- any contract, export, conformance, release, flake, or multi-crate slice should close on a Nix lane, not on targeted cargo alone
- crate-local slices may iterate with targeted cargo commands, but should still finish with the narrowest canonical Nix lane that proves the change green

## RCL-Swarm
- In `rcl-swarm`, the Beads issue id is the Agent Mail thread id.
- Use the same Beads issue id as the reservation reason.
- Reserve files before the first write.
- Use build slots for long-running singleton resources when needed:
  - `rr-contract`
  - `rr-release-preflight`
  - `rr-wasm-builds`

## End Of Session
- `bd close <id> --reason "..."`
- `bd dolt push`
