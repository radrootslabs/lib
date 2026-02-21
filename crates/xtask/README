# xtask sdk commands

## validate

```bash
cargo run -q -p xtask -- sdk validate
```

Validates the sdk contract manifest, version policy, export mappings, and required artifacts.

## export

```bash
cargo run -q -p xtask -- sdk export-ts
```

Runs the full export pipeline:

- generates ts-rs model sources from contract crates
- exports models/constants/wasm outputs to `target/sdk-export/ts/packages`
- writes deterministic checksums to `target/sdk-export/ts/export-manifest.json`

## granular commands

```bash
cargo run -q -p xtask -- sdk export-ts-models
cargo run -q -p xtask -- sdk export-ts-constants
cargo run -q -p xtask -- sdk export-ts-wasm
cargo run -q -p xtask -- sdk export-manifest
```

Use `--out <dir>` with any export command to write artifacts to a custom directory.
