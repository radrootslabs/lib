# Public API baselines

This directory records reviewed pre-release public API surfaces for publishable
Radroots packages. Baselines are generated with `cargo-public-api` `0.52.0`,
`nightly-2026-07-16` for rustdoc JSON, and the package's complete public feature
set. Package verification remains governed by the workspace's pinned stable
toolchain.

Regenerate a package baseline from the workspace root inside the canonical
development shell with:

```sh
RUSTC="$(rustup which --toolchain nightly-2026-07-16 rustc)" \
RUSTDOC="$(rustup which --toolchain nightly-2026-07-16 rustdoc)" \
cargo public-api --manifest-path crates/<crate>/Cargo.toml \
  --all-features -sss \
  > docs/api/<package>.txt
```

Install the exact tool version, when it is not already available, with:

```sh
cargo install cargo-public-api --version 0.52.0 --locked
rustup toolchain install nightly-2026-07-16 --profile minimal
```

Review baseline changes together with the package charter and intended SemVer
impact. A generated listing is evidence of the Rust surface, not authority to
expand a package beyond its charter.

| Package | Baseline | Charter |
| --- | --- | --- |
| `radroots_core` | [`radroots_core.txt`](radroots_core.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_identity` | [`radroots_identity.txt`](radroots_identity.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_blossom` | [`radroots_blossom.txt`](radroots_blossom.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_protocol` | [`radroots_protocol.txt`](radroots_protocol.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_event` | [`radroots_event.txt`](radroots_event.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_event_codec` | [`radroots_event_codec.txt`](radroots_event_codec.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_trade` | [`radroots_trade.txt`](radroots_trade.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_signing` | [`radroots_signing.txt`](radroots_signing.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_transport` | [`radroots_transport.txt`](radroots_transport.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_nostr` | [`radroots_nostr.txt`](radroots_nostr.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_nostr_connect` | [`radroots_nostr_connect.txt`](radroots_nostr_connect.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_secrets` | [`radroots_secrets.txt`](radroots_secrets.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_storage` | [`radroots_storage.txt`](radroots_storage.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
| `radroots_transport_nostr` | [`radroots_transport_nostr.txt`](radroots_transport_nostr.txt) | [release V1 specification](../specs/radroots_crates_release_v1.md) |
