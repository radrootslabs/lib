# Radroots Core Libraries Beads Prime

Read `AGENTS.md` first, then read `AGENT_INSTRUCTIONS.md`. `AGENTS.md` remains the authoritative concise contract for this repository.

## Workflow
- When Beads is active, use it as the live execution state.
- Keep active work scoped to the smallest coherent green checkpoint.
- For larger efforts, keep only the next few follow-up issues concrete.
- Every issue should have a clear scope, a definition of green, and one dominant verify lane.
- Do not create markdown TODO trackers when Beads is active.
- Do not use `bd edit`.

## Start Of Session
- `bd ready --json`
- `bd show <id>`
- `bd update <id> --claim --json`
- Trust cwd auto-filtering when working from a mapped crate or repo surface.
- When working from an external parent repo or worktree, switch into this repo root before running Beads or Agent Mail commands.

## Environment Contract
- Nix is the canonical environment contract for this workspace.
- Prefer `nix develop` or `direnv allow` before targeted cargo work.
- Prefer repo-owned Nix lanes over ad hoc raw commands for closeout validation.

## Planning Rules
- Keep work small enough that one dominant verify lane can prove it green.
- If a change needs unrelated lanes, split it into separate issues.
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
- any contract, export, conformance, release, flake, or multi-crate issue should close on a Nix lane, not on targeted cargo alone
- crate-local issues may iterate with targeted cargo commands, but should still finish with the narrowest canonical Nix lane that proves the change green

## Coordination
- If Agent Mail is active for the session, use the Beads issue id as the thread id.
- Use the same Beads issue id as the reservation reason.
- Reserve files before the first write when coordinating across agents.
- Use build slots for long-running singleton resources when needed:
  - `rr-contract`
  - `rr-release-preflight`
  - `rr-wasm-builds`

## End Of Session
- `bd close <id> --reason "..."`
- if a Beads remote or shared state target is configured for the session, run `bd dolt push`
