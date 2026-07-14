# Public Validation Model

This repository treats public source validation as a fail-closed contract. A change is ready for
public release consideration only when source formatting, crate tests, contract validation,
conformance vectors, and release preflight all agree with the checked-in contracts.

## Validation surfaces

The public validation surface is source-controlled in this repo:

- `Cargo.toml` defines the workspace members and crate dependency posture.
- `contracts/**` defines operation metadata, conformance vectors, event contracts, and release
  contract inputs.
- `crates/**/tests/**` exercises crate-local public APIs and malformed-input behavior.
- `tools/xtask/**` owns repo-local contract validation and release preflight commands.

The standard source validation entrypoints are:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-features
cargo xtask contract validate
cargo xtask release preflight
```

Focused hardening slices may run narrower crate commands first, but final release consideration
must return to the workspace and xtask validation surface.

## Conformance vectors

Stable public vectors live under `contracts/conformance/vectors/**`. These files are test inputs,
not generated output. They are deliberately small and inspectable so malformed input expectations
remain reviewable in source control.

The malformed corpus currently covers:

- event wire JSON in `contracts/conformance/vectors/event/nip01_wire.v1.json`
- transport target URI parsing in `contracts/conformance/vectors/transport/target_uri.v1.json`
- mesh frame CBOR in `contracts/conformance/vectors/mesh/frame_cbor.v1.json`
- replica schema JSON in `contracts/conformance/vectors/replica_schema/json_models.v1.json`

Each corpus has at least one canonical valid case and malformed cases that must fail through the
same public parser or model type used by downstream consumers.

## Failure posture

Validation failures are release blockers. The expected DTO crates.io blocker documented in
`docs/release/crates_io_policy.md` is still a blocker; it is recorded distinctly so release tooling
does not silently substitute git dependencies, local path dependencies, vendored copies,
retired-name reexports, or publication-only workarounds.
