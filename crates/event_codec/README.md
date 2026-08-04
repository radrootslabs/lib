# radroots_event_codec

`radroots_event_codec` is the portable, deterministic algorithm layer for
Radroots events. It converts between bounded wire representations and native
`radroots_event` values, computes canonical NIP-01 identifiers, verifies event
identifiers and signatures, validates event contracts, admits typed event
profiles, and generates governed contract manifests.

The crate is pre-release and its Cargo version is frozen at `0.1.0-alpha`
until explicitly changed. Serialized contract generations such as registry v7
are versioned independently from the Cargo package.

## Canonical surface

New code should enter through these modules:

| Module | Responsibility |
| --- | --- |
| `canonical` | Compute canonical NIP-01 preimages and event identifiers without asserting trust. |
| `decode` | Parse bounded wire data and domain projections without silently verifying later stages. |
| `encode` | Produce deterministic JSON, tags, and unsigned wire parts from validated native inputs. |
| `verify` | Advance explicit identifier, signature, and contract-validation typestates. |
| `admission` | Apply typed or registry admission to an already verified event; available with `json`. |
| `manifest` | Generate and validate registry and knowledge inventories; available with `manifests`. |

The Release V1 canonical root consists of `Codec`, `DecodeError`,
`EncodeError`, and `VerificationError`. Domain algorithms stay beneath the
canonical modules so each import states whether it encodes, decodes, verifies,
or admits data. Legacy top-level domain routes are not exposed. The canonical
surface is recorded in the
[public API baseline](../../docs/api/radroots_event_codec.txt).

## Verification pipeline

Wire parsing and cryptographic trust are separate operations. A successful
decode returns a structurally valid `RawEvent`; it does not make the declared
event ID, signature, contract, referenced events, relay hints, or remote media
trustworthy.

```rust
use radroots_event_codec::{admission, decode, verify};

const PROFILE_EVENT: &str = r#"{"id":"762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1800000100,"kind":0,"tags":[],"content":"{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}","sig":"4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109"}"#;

let raw = decode::event(PROFILE_EVENT)?;
let verified = verify::verify_nip01_event(raw.into_event())?;
let admitted = admission::admit_verified_event(verified)?;

assert_eq!(admitted.event().kind_u32(), 0);
# Ok::<(), Box<dyn std::error::Error>>(())
```

A runnable version is available at
[`examples/verify_profile.rs`](examples/verify_profile.rs).

For capability-injected verification, use `verify::id`, then
`verify::signature`, then `verify::contract`. Each function consumes its input
typestate and returns the next, so callers cannot obtain a later state by
accidentally skipping an earlier check.

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | Standard-library error integration for the portable surface. |
| `serde` | no | Serde support for native values used by codec contracts. |
| `json` | yes | Bounded JSON parsing/encoding and JSON-backed typed profiles; enables `serde`. |
| `knowledge` | no | Knowledge event codecs and verified decoding; enables `json`. |
| `manifests` | no | Typed registry and knowledge manifest generation; enables `knowledge`. |

`--no-default-features` keeps the portable allocation-backed canonical,
encoding, decoding, and verification core. Feature-specific APIs disappear
when their feature is disabled rather than installing a fallback with weaker
guarantees.

## Serialization and canonicalization

- `decode::event` accepts compact NIP-01 JSON only within the shared bounded
  wire limits. Unknown extension structure is bounded before it can consume
  unbounded memory or parser depth.
- `encode::event` emits deterministic compact JSON from an existing event
  envelope. Encoding does not verify or alter the envelope.
- `canonical::id_preimage` and `canonical::id` compute canonical bytes and the
  corresponding identifier. They do not compare the result with the event's
  declared ID.
- Domain encoders accept checked `radroots_event` inputs and emit unsigned
  wire parts. Signing and publication belong to the owning runtime.
- Manifest JSON and digests are generated from versioned contract authority;
  a manifest feature does not grant storage or publication authority.

Serialized output is stable only where its event profile or manifest
generation says it is stable. Rust data layout and the pre-release public API
are not serialized contracts.

## Security and trust boundaries

All public parsers treat their inputs as untrusted. They return structured
errors for malformed or over-budget data and do not intentionally panic on
untrusted input. Verification distinguishes these claims:

1. structural decoding proves only bounded event shape;
2. ID verification proves the declared identifier matches canonical bytes;
3. signature verification proves the BIP-340 signature for that event and
   author;
4. contract validation proves the event matches a registered shape;
5. typed admission proves the selected Radroots profile.

No stage proves referenced-event existence, relay availability, media upload
or retrievability, actor authorization, business-policy approval, persistence,
or publication. Inbound URLs and relay hints remain structural observations
unless a higher layer explicitly establishes a stronger state.

The crate never owns secret keys and does not sign events. Signature
verification is deterministic and public-key-only. Callers must not interpret
a successfully encoded unsigned event as signed or published.

## Side effects, cancellation, and commit points

This crate performs no network access, filesystem access, database access,
background work, executor installation, timer management, signing, or event
publication. Its algorithms are synchronous and deterministic for the same
inputs.

There is therefore no asynchronous cancellation or deadline boundary and no
durable commit point. Dropping a computation only discards in-memory work.
Storage, signing, transport, SDK, and daemon callers own cancellation and must
report success only after their own explicit commit boundary succeeds.

## Intended consumers

Direct consumers are `radroots_nostr`, `radroots_storage_sqlite`,
`radroots_transport_nostr`, `radroots_sync`, `radroots_sdk`, generated
bindings, indexers, and conformance tooling. Applications should normally use
the `radroots` or `radroots_sdk` front door and depend on this crate directly
only when implementing a deterministic event boundary.

This package must not acquire live relay clients, persistence, background
workers, host configuration, SDK state, or upstream client error types. Those
responsibilities belong to adapter and runtime crates.

## Package charter

The authoritative Release V1 responsibility, dependency, feature, module, and
forbidden-scope contract is the
[Radroots crates Release V1 specification](../../docs/specs/radroots_crates_release_v1.md).
The baseline generation procedure and toolchain are documented in
[`docs/api/README.md`](../../docs/api/README.md).

## Copyright

Except as otherwise noted, all files in the `radroots_event_codec`
distribution are copyright (c) 2025 Tyson Lupul. See `LICENSE` for usage,
redistribution, and warranty terms.
