# radroots_blossom

`radroots_blossom` provides portable Blossom protocol primitives for Radroots:
canonical SHA-256 values and root hash paths, blob URLs, media types, BUD-02
descriptors, byte-verification typestates, and pure BUD-11 authorization
claims.

The crate supports `no_std` environments with `alloc`. It performs no I/O,
starts no tasks or threads, reads no clocks or process state, and owns no HTTP
client, upload scheduler, cache, filesystem, credential, signer, or global
authentication state.

## Example

Parse a BUD-02 descriptor, apply the Radroots reference policy, and prove that
its hash, size, and media type match locally available bytes:

```rust
use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};

let bytes = b"hello";
let hash = Sha256::digest(bytes);
let media_type = MediaType::parse("text/plain")?;
let url = BlobUrl::parse(&format!("https://media.example/{hash}.txt"))?;
let descriptor = BlobDescriptor::new(
    url,
    hash,
    bytes.len() as u64,
    media_type.clone(),
    1_725_105_921,
)?;

let verified = descriptor
    .approve_reference()?
    .verify_bytes(bytes, &media_type)?;

assert_eq!(verified.sha256(), hash);
assert_eq!(verified.size(), 5);
assert_eq!(verified.url().as_str(), format!("https://media.example/{hash}.txt"));
# Ok::<(), radroots_blossom::Error>(())
```

The same program is available as the
[`verified_descriptor`](examples/verified_descriptor.rs) example.

## Public API

The crate root intentionally exports `AuthorizationClaim`, `BlobDescriptor`,
`BlobUrl`, `ByteVerifiedDescriptor`, `MediaType`, `Sha256`, and the aggregate
[`Error`](https://docs.rs/radroots_blossom/latest/radroots_blossom/enum.Error.html).
Supporting states and constructors remain in these modules:

- `authorization` — bounded BUD-11 claim parsing, endpoint-scope validation,
  and upload-claim wire parts;
- `descriptor` — BUD-02 descriptors and the approved-reference,
  byte-commitment, and byte-verified states;
- `hash` — SHA-256 values, safe file extensions, and root hash paths;
- `media_type` — parsed and deterministically canonicalized media types;
- `url` — structural blob URLs and the stricter approved-reference state.

The normative responsibility and dependency boundary are defined by the
[`radroots_blossom` package charter](../../contracts/crates/release_v1/radroots_crates_release_v1.toml).
The reviewed pre-release surface is recorded in the
[`radroots_blossom` API baseline](../../contracts/api_baselines/radroots_blossom.txt).

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | Implements the standard error integration and enables standard-library support in dependencies; protocol behavior remains portable. |
| `serde` | yes | Implements checked string serialization for hashes, paths, URLs, and media types, plus checked BUD-02 descriptor serialization. |

Disabling default features leaves the `no_std` + `alloc` protocol model.
Features are additive; neither feature selects a runtime, transport, backend,
clock, signer, credential source, or side effect.

## Protocol and validation boundaries

Protocol behavior is pinned to Blossom commit
`b5bd2801d1763aa635fc8fea7a76597e0eb18990`:

- [BUD-01](https://github.com/hzrd149/blossom/blob/b5bd2801d1763aa635fc8fea7a76597e0eb18990/buds/01.md)
  defines root hash paths and basic blob retrieval;
- [BUD-02](https://github.com/hzrd149/blossom/blob/b5bd2801d1763aa635fc8fea7a76597e0eb18990/buds/02.md)
  defines blob descriptors;
- [BUD-11](https://github.com/hzrd149/blossom/blob/b5bd2801d1763aa635fc8fea7a76597e0eb18990/buds/11.md)
  defines Nostr authorization claims.

`Sha256`, `HashPath`, `BlobUrl`, and `MediaType` reject ambiguous encodings and
emit their documented canonical textual forms. A `BlobDescriptor` additionally
requires a URL file extension and the same hash in its URL and `sha256` field.
These checks establish structural protocol validity; they do not make a remote
reference trustworthy.

`BlobUrl::parse` accepts structurally valid public HTTP references so received
Blossom data can be represented faithfully. `BlobUrl::approve` advances only
HTTPS or loopback HTTP references to `ApprovedBlobUrl`. Callers must not fetch,
display, cache, or otherwise act on an unapproved URL. URL approval is a narrow
transport policy, not host reputation, content safety, malware scanning, or
application media policy.

`ByteVerifiedDescriptor` can only be produced after an approved descriptor's
hash, byte length, and approved media type match supplied bytes or a locally
computed `ByteCommitment`. It proves local descriptor-to-byte agreement. It is
not a server receipt, upload acknowledgement, authenticity proof, or safe-media
classification.

## Authorization and security

`AuthorizationClaim` parses the BUD-11 content and tags needed for endpoint
authorization. Validation checks caller-supplied current time, action, server,
hash, expiration, creation age, and lifetime policy. The crate never reads a
clock and never verifies a Nostr event signature or signer identity. The
composing Nostr layer must validate event kind `24242`, signature, author,
request association, and canonical `Authorization: Nostr` encoding before an
endpoint treats a validated claim as authenticated. Kind `24242` is ephemeral
HTTP authorization material and must not be published to relays.

`AuthoredUploadClaim` emits checked event wire parts; it does not sign them or
send a request. Authorization content is bounded to 4,096 bytes, server domains
use lowercase ASCII DNS or canonical IPv4 forms, timestamps are explicit
caller inputs, and authored lifetimes are limited to 300 seconds.

The crate forbids unsafe Rust. It does not provide confidentiality,
authentication, authorization storage, credential redaction, content scanning,
SSRF protection beyond its explicit URL approval rule, or resource limits for
the bytes callers choose to hash. Hosts must bound untrusted payload sizes
before hashing or retaining them and must enforce application policy at their
own trust boundaries.

## Serialization

With `serde`, SHA-256 values are lowercase hexadecimal strings; file extensions,
hash paths, blob URLs, and media types use their canonical string forms; and
BUD-02 descriptors use the protocol fields `url`, `sha256`, `size`, `type`, and
`uploaded`. Deserialization re-runs the same parsers and descriptor constructor,
so malformed or cross-field-inconsistent values cannot enter through the
supported serde surface.

Authorization states and approved or byte-verified typestates intentionally do
not implement serde. Reconstruct them by parsing untrusted wire parts and
reapplying the current request, time, URL, and byte-verification policies.
Serialization alone never preserves an approval or authentication decision.

## Execution and commit semantics

Operations are synchronous and deterministic. Hashing cost is linear in the
caller-supplied byte slice; parsing and authored authorization construction may
allocate in proportion to bounded input. There are no asynchronous cancellation
points, deadlines, retries, callbacks, network requests, partial external
effects, or durable commit points. A successful call returns its complete
in-memory value; an error returns without durable mutation because the crate
owns no external state.

HTTP-capable callers must define their own cancellation and commit boundary.
In particular, local byte verification does not authorize marking a BUD-02
upload complete: durable upload success may be committed only after the
composing transport has validated its successful server response.

## Intended consumers

`radroots_blossom` is intended directly for `radroots_event`,
`radroots_event_codec`, `radroots_nostr`, and media clients that need portable
Blossom values. Applications should normally begin with the curated `radroots`
crate or the advanced `radroots_sdk` composition surface, where transport,
signing, storage, and application media policy can be supplied explicitly.

The package is pre-1.0. Its durable responsibility and package identity are
fixed, while API-breaking changes follow the workspace's pre-1.0 versioning
policy.

## License

Licensed under either Apache-2.0 or MIT, at your option.
