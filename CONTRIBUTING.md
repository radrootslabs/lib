# Contributing

Radroots core-library changes are contract-driven and independently
reviewable. Before editing, read these files in order:

1. `AGENTS.md`
2. `AGENT_INSTRUCTIONS.md`
3. `contracts/crates/release.v2.toml`
4. `contracts/crates/release_v1/radroots_crates_release_v1.toml` for
   crate-surface work
5. the affected manifests, implementation, contracts, and tests

The release-v1 architecture identifier is `radroots.crates.release.v1`. This
repository owns its first 17 public packages, from `radroots_core` through
`radroots_geonames`; the standalone SDK repository owns `radroots_sdk` and
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

Run `cargo extbuild doctor` before governed verification. Canonical
repository-wide lanes are the extbuild-routed workspace format, check, test,
Clippy, Rustdoc, contract, release-preflight, coverage, and
generated-freshness commands. Nix evaluation and Nix-derived outputs are
currently deferred and unclaimed; they are not prerequisites for native
qualification.

## Commits and deviations

Use this commit form:

```text
<scope>: <lower-case imperative summary>
```

Keep commits focused and keep public commit language independent of any
private checkout. Do not publish, tag, merge, or change registry ownership
without explicit authorization.

When current evidence proves a planned step obsolete or unsafe, update
`contracts/architecture/deviations.toml` and validate it with
`cargo xtask architecture`. Record the evidence and affected machine-contract
anchor before changing the plan. Anchors must use a validated Release V1 TOML
selector (`repositories.<name>`, `repository_policy`, `release_policy`,
`quality_policy.coverage`, or `package.<name>`); Markdown heading fragments are
not machine anchors. A normative change also requires the applicable machine
decision. Do not silently redefine the architecture.
