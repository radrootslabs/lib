# Contributing

Radroots core-library changes are contract-driven and independently
reviewable. Before editing, read these files in order:

1. `AGENTS.md`
2. `docs/specs/README.md`
3. `docs/specs/radroots_crates_release_v1.md` for crate-surface work
4. `AGENT_INSTRUCTIONS.md`
5. the affected manifests, implementation, contracts, and tests

The release-v1 architecture identifier is `radroots.crates.release.v1`. This
repository owns its first 17 public packages, from `radroots-core` through
`radroots-geonames`; the standalone SDK repository owns `radroots-sdk` and
`radroots`.

## Workflow

1. Inspect repository status and the current source authority.
2. Make one coherent, commit-sized change.
3. Update public contracts, tests, fixtures, generated authorities, and docs
   with the implementation they govern.
4. Run the narrowest repository-owned validation that proves the change, then
   the broader contract or release lane required by its scope.
5. Review the staged diff for API leakage, private dependencies, generated
   drift, secrets, hidden side effects, and unrelated changes.

Canonical repository-wide lanes are `nix flake check`,
`nix run .#contract`, and `nix run .#release-preflight`. Targeted Rust work is
performed in the repository's Nix environment with the applicable format,
check, test, Clippy, contract, coverage, and generated-freshness commands.

## Commits and deviations

Use this commit form:

```text
<scope>: <lower-case imperative summary>
```

Keep commits focused and keep public commit language independent of any
private checkout. Do not publish, tag, merge, or change registry ownership
without explicit authorization.

When current evidence proves a planned step obsolete or unsafe, follow
`docs/implementation/DEVIATIONS.md`. Record the evidence and affected spec
anchor before changing the plan; do not silently redefine the architecture.
