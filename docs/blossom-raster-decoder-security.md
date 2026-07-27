# Blossom Raster Decoder Security

This document records the defensive security and cost boundary for the
publication raster decoder in `radroots_blossom`. The decoder parses
attacker-controlled JPEG, PNG, and WebP bytes at the publication-readiness
boundary, so the crate treats container inspection and full decoding as a
hardened parser surface: no input may panic, overflow, loop without a work
budget, allocate beyond its declared ceiling, or produce animated or
multiframe output.

## Normative limits

| Bound | Value | Enforcement |
| --- | --- | --- |
| Encoded raster bytes | `10,485,760` | public constant, checked before decode |
| Width/height per axis | `16,384` | dimension gate before allocation |
| Decoded pixels | `20,000,000` | pixel gate before allocation |
| Decoded output bytes | `80,000,000` | public constant, checked before allocation |
| Container records per format | `65,536` | checked record counter in each container walk |
| Sequential JPEG scans | `4` | work budget |
| Sequential JPEG blocks | `3,200,000` | work budget |
| Sequential JPEG coefficient steps | `204,800,000` | work budget |
| JPEG entropy bit reads | `8 x input bytes` | entropy reader budget |
| Decoder lane peak RSS | `131,072` KiB | governed lane hard failure |

Accepted processes are static 8-bit sequential JPEG (SOF0/SOF1), static
8-bit PNG with approved color types, one ordered palette, and no unknown
critical chunks, and static WebP whose extended header carries no reserved
flags and exactly one primary chunk. Animation, multiframe, higher bit
depths, and every other process fail before product authority is created;
unsupported PNG and WebP processes fail with the stable
`publication_raster_process_forbidden` error.

## Native WebP boundary

Production WebP decoding uses the safe `libwebp` `0.1.2` wrapper with features
`1_1` and `static` over `libwebp-sys2` `0.1.11`. Both crates and the vendored
libwebp source are BSD-3-Clause. The sys crate compiles its pinned C sources
with `cc`; static mode bypasses `pkg-config`, vcpkg, system-dylib discovery, and
all production subprocess or network behavior. Public `radroots_blossom` Rust
remains under `#![forbid(unsafe_code)]`; native and unsafe implementation is
confined to the pinned dependency boundary. The production operation obtains
the checked dimensions, reserves one bounded RGBA buffer fallibly, and decodes
directly into it. The governed Darwin lane also emits an
`aarch64-apple-ios` static archive and proves that the vendored decoder symbol
is linked into that archive.

## Ordinary verification gates

The regression corpus, independent-decoder differential, and peak-RSS probe
execute through the real public `verify_publication_readiness` API:

- `crates/blossom/tests/fixtures/raster_decoder_security.v1.json` mirrors the
  canonical `contracts/conformance/vectors/blossom/raster_decoder_security.v1.json`
  corpus of 30 cases covering every supported format/process variant, including
  lossy and lossless WebP alpha, plus
  malformed, truncated, animation, precision, Huffman, restart, CRC,
  deflate, dimension, pixel, and duplicate-primary failures. Each `bytes_hex`
  value is the final byte sequence passed to the operation; mutation labels are
  provenance only and the executor performs no runtime transformation.
- `decoder_differential_matches_independent_backend` compares accepted
  dimensions, format, frame count, and decoded byte count against the
  Nix-pinned ImageMagick `7.1.2-27` backend, which shares no decoder code
  with the production path. Disagreements fail and become corpus cases.
- `maximum_resource_probe` selects exactly one already-prepared 5,000 x 4,000
  fixture in a fresh child. The closed matrix covers JPEG grayscale, RGB,
  CMYK, and SOF1; PNG RGB, palette, RGBA, and Adam7; and WebP VP8 RGB, VP8
  alpha, VP8L RGB, and VP8L alpha. ImageMagick prepares the compact fixtures
  outside every measured child, while the child authenticates the actual
  process and executes `verify_publication_readiness` exactly once. Every case
  runs three times; any observation above `131,072` KiB fails, and the highest
  observation per case is retained in a TSV under the extbuild-owned Cargo
  target root. Separate prepared PNGs execute the exact `16,384 x 1` and
  `1 x 16,384` axis boundaries. Wall time remains informational only.

Run the whole gate surface through the governed app:

```bash
nix run .#decoder-security
```

On Darwin, compile and statically link the same production feature profile for
the device target with:

```bash
nix run .#blossom-raster-ios-compile-link
```

## Deterministic fuzz smoke

The workspace-excluded, non-publishable `fuzz` crate pins
`nightly-2026-07-15` and drives the same public API through three
`cargo-fuzz` targets: `publication_jpeg`, `publication_png`, and
`publication_webp`. Checked-in seed corpora live under
`fuzz/corpus/<target>/` as exactly one raw binary file per conformance case,
under its single format target. The common harness passes those bytes directly
to `verify_publication_readiness`; it has no textual-hex decoder or ASCII
fallback. Generated corpora and crash artifacts stay under the extbuild-owned
Cargo target root (or the isolated Nix build sandbox) and are disposable
evidence only.

The offline sandboxed smoke check vendors `fuzz/Cargo.lock` and runs each
target for 256 executions with seed `424242`, a 65,536-byte maximum input,
a 5-second per-input timeout, and a 2,048 MiB libFuzzer engine limit (the
engine limit is fuzzer headroom and is distinct from the 128 MiB decoder
lane gate above):

```bash
cargo extbuild run -- nix build \
  .#checks.aarch64-darwin.blossom-decoder-fuzz-smoke
```

## Extended campaign record

A longer AddressSanitizer campaign completed on 2026-07-27 with zero crash
artifacts:

- engine: libFuzzer via `cargo-fuzz`, nightly `2026-07-15`, `-fsanitize=address`
- seed: `20260726`
- duration: 60 seconds of fuzzing per format (informational, not contractual)
- maximum input: `10,485,760` bytes
- resulting corpora: 482 JPEG, 657 PNG, and 96 WebP inputs under an isolated
  `decoder-security-extended-20260726.*` directory in the extbuild Cargo target
  root
- crash artifacts: none in any format loop

Reproduce the extended campaign from the governed shell (the corpus and
artifact paths are disposable):

```bash
cargo extbuild run -- nix develop .#decoder-security --command sh -c '
set -eu
root="$(mktemp -d "${CARGO_TARGET_DIR:?}/decoder-security-extended.XXXXXX")"
mkdir -p "$root/corpus" "$root/artifacts"
cp -R fuzz/corpus/. "$root/corpus/"
for target in publication_jpeg publication_png publication_webp; do
  mkdir -p "$root/artifacts/$target"
  cargo fuzz run --fuzz-dir fuzz --sanitizer address "$target" \
    "$root/corpus/$target" -- \
    -max_total_time=60 \
    -seed=20260726 \
    -max_len=10485760 \
    -timeout=5 \
    -rss_limit_mb=2048 \
    -artifact_prefix="$root/artifacts/$target/"
done
test -z "$(find "$root/artifacts" -type f -print -quit)"
'
```

Any crash artifact must be reproduced and minimized with the same pinned
`decoder-security` Nix shell. Its minimized final bytes must then be promoted
to the canonical vector and regenerated raw corpus before the fix ships; an
unresolved artifact fails the campaign.

## Contract authority

The machine-readable successor contract
`radroots_blossom.raster_decoder_security_v1`
(`crates/blossom/contracts/raster_decoder_security_v1.manifest.json`)
authenticates the limits above, the decoder profile, the fuzz toolchain and
targets, the governed Nix lanes, the vector corpus and its executors, and
the immutable `radroots_blossom.publication_readiness_v1` predecessor.
Validate it with:

```bash
cargo xtask contract blossom-raster-decoder-security-manifest
```
