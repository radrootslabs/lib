# Independent protocol fixture producer

This optional development tool uses pinned NostrTools/Noble implementations to
produce deterministic synthetic NIP-44/NIP-59 fixtures independently of the Rust
implementation. The lockfile records the exact full npm dependency closure and
tarball integrity. No upstream implementation is copied into Radroots source.
NostrTools is Unlicense; the remaining locked packages declare MIT. Provenance
records the primary source identity and license equality.
The unused transitive `nostr-wasm` package omits license text from its pinned
source tree and tarball. Its MIT declaration matches the primary manifest and
npm metadata; this is recorded explicitly without claiming redistribution
qualification. The producer imports only the pure/NIP44 entrypoints, and no
Node dependency is vendored or redistributed in Rust packages or artifacts.

From the Lib root, route installation and production through extbuild:

```sh
cargo extbuild run -- npm ci --prefix tools/protocol_oracle --ignore-scripts --no-audit --no-fund
cargo extbuild run -- node tools/protocol_oracle/commerce_envelopes.mjs --write
cargo extbuild run -- node tools/protocol_oracle/commerce_envelopes.mjs --check
```

The producer first matches the retained published NIP-44 reference vector.
Complete records come from the independent canonical record fixture. It creates
peer and self copies for all19 families, independently solves exact full-size
bounds and constructs nested corruption and unsupported-profile cases.

All private scalars, nonces, auxiliary bytes and timestamps are public synthetic
test inputs. Determinism supplies no production entropy or custody claim.
Rust runtime and standalone tests consume only checked-in fixture data; they
need neither Node, npm, a network connection nor the parent repository.
