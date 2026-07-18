# Blossom Media Foundation

Status: active implementation contract

Scope: public, runtime-independent Radroots primitives for Blossom BUD-01 hash paths, BUD-02
descriptors, and local descriptor-versus-byte verification.

## Protocol Basis

This contract is based on the Blossom repository at commit
`b5bd2801d1763aa635fc8fea7a76597e0eb18990`:

- [BUD-01: Server requirements and blob retrieval](https://github.com/hzrd149/blossom/blob/b5bd2801d1763aa635fc8fea7a76597e0eb18990/buds/01.md)
- [BUD-02: Blob upload and descriptor](https://github.com/hzrd149/blossom/blob/b5bd2801d1763aa635fc8fea7a76597e0eb18990/buds/02.md)

The upstream BUD rules and the Radroots application profile are distinct layers. A URL can be a
structurally valid Blossom URL without being an approved Radroots reference. Callers must not treat
structural parsing as approval, byte verification, network reachability, or proof that a server
implements Blossom.

## Ownership And Dependency Boundary

`radroots_blossom` owns these primitives as a public `no_std + alloc` protocol leaf. It may use pure
hashing and serialization dependencies, but it must not depend on Radroots event models, event
codecs, Nostr, an HTTP client or server runtime, platform policy, or private code.

This contract does not implement BUD-03 server discovery, BUD-11 authorization, BUD-12 listing,
upload entitlement, object-storage policy, network I/O, redirects, retries, or availability checks.
BUD-11 is added as a separate foundation slice.

## SHA-256

A `RadrootsBlossomSha256` is exactly 32 bytes. Its wire and display form is exactly 64 lowercase
ASCII hexadecimal characters.

- Parsing rejects uppercase hexadecimal, prefixes, separators, whitespace, non-hexadecimal bytes,
  and every length other than 64 characters.
- Serialization and display always emit lowercase hexadecimal.
- Digest construction hashes the exact supplied bytes without text conversion or normalization.
- The empty byte sequence is valid and has its ordinary SHA-256 digest.

## Root Hash Paths

A `RadrootsBlossomHashPath` represents one BUD-01 root path:

```text
/<64-lowercase-hex-sha256>[.<extension>]
```

The leading slash is required by the root-path parser. The path has exactly one segment. It has no
query, fragment, percent decoding, dot segment, or trailing slash.

An extension is returned without its leading period. It consists of one or more nonempty
dot-separated components. Each component contains only ASCII letters, digits, `-`, or `_`.
Extension case is preserved because the remote path may be case-sensitive. Examples include `png`,
`JPG`, and `tar.gz`; empty components, whitespace, path separators, percent escapes, and `..` are
invalid.

BUD-01 retrieval paths may omit the extension. A URL carried by a BUD-02 blob descriptor must
include one. The extension is not used to infer or validate the descriptor MIME type.

## Structural Blob URLs

A `RadrootsBlossomBlobUrl` is an absolute structural BUD-01 URL with these invariants:

- the scheme is `http` or `https`, compared case-insensitively and exposed canonically in lowercase
- the authority contains one nonempty DNS hostname, canonical dotted-decimal IPv4 address, or
  bracketed IPv6 address and may contain a valid decimal port
- user information is forbidden
- the path is exactly one root hash path
- query and fragment components are forbidden
- the path digest is exposed as a typed `RadrootsBlossomSha256`
- an optional safe extension is parsed independently from the digest

Scheme and DNS host case are canonicalized to lowercase by the URL parser. Extension case is
preserved. Noncanonical, shortened, octal, hexadecimal, or otherwise ambiguous IPv4 spellings are
rejected before a typed URL is returned; this prevents approval behavior from changing after a
serialization round trip. Raw user-information delimiters, whitespace, control characters,
percent-encoded path data, malformed or zero ports, unbracketed IPv6 addresses, and additional path
segments are also rejected before URL-parser normalization can discard or reinterpret them.

Structural validity proves only that the URL has a BUD-01-compatible shape. It does not approve the
transport scheme, perform DNS resolution, follow a redirect, issue `HEAD` or `GET`, or establish
that bytes exist at the URL.

## Approved Reference Policy

Radroots reference approval is a second, explicit validation step over a structurally valid blob
URL:

- every structurally valid `https` URL is transport-approved
- `http` is approved only for a syntactic loopback host
- the loopback hostname set is case-insensitive exact `localhost`, or a hostname with at least one
  nonempty label ending in `.localhost`
- the loopback IPv4 set is canonical dotted-decimal `127.0.0.0/8`
- the loopback IPv6 set is the address `::1`, including equivalent expanded IPv6 spelling

The policy does not use DNS resolution. A public hostname that resolves to loopback is not accepted
as loopback. `localhost.`, `localhost.example`, IPv4-mapped IPv6 addresses, private or link-local
addresses, unspecified addresses, and arbitrary public hosts are not approved for `http`. Expanded
IPv6 spellings that parse exactly to `::1` remain structurally valid and approved.

HTTPS approval is a reference-transport rule, not an SSRF or egress policy. Network runtimes remain
responsible for their own destination, redirect, DNS-rebinding, response-size, and timeout controls.

## Media Types

A `RadrootsBlossomMediaType` is a syntactically valid MIME media type parsed by the repository's
`mediatype` dependency. MIME parameters are supported. The canonical representation lowercases the
type, subtype, suffix, and parameter names, sorts parameters by name, and preserves parameter
values. Duplicate parameter names are rejected. Semantic equality is case-insensitive for those
names, while parameter values retain the media-type library's case-sensitive semantics.

Empty values, missing type/subtype structure, wildcards, invalid parameter syntax, leading or
parser-truncated trailing whitespace, control characters, and non-ASCII token characters are
rejected.

Parsing never substitutes a default for empty or malformed input. A caller that intends
`application/octet-stream` must provide it explicitly.

The media type used for byte verification is selected and approved by the caller. This foundation
does not sniff or infer content type from file bytes or the URL extension.

## BUD-02 Descriptors

A structural `RadrootsBlossomBlobDescriptor` contains the five required BUD-02 fields:

| Field | Contract |
| --- | --- |
| `url` | structurally valid absolute root blob URL with an extension |
| `sha256` | typed lowercase SHA-256 equal to the digest in `url` |
| `size` | unsigned 64-bit byte count |
| `type` | parsed `RadrootsBlossomMediaType` |
| `uploaded` | unsigned 64-bit Unix timestamp |

Unknown descriptor fields are tolerated on input for forward compatibility. They need not be
retained when the typed descriptor is serialized. Required fields, field types, URL structure, URL
extension presence, URL/hash equality, and MIME syntax are validated during construction and
deserialization; public fields must not permit bypassing those invariants.

Descriptor parsing is structural. It does not apply the approved-reference scheme policy. An
authored publication flow must explicitly require both a structurally valid descriptor and an
approved descriptor URL.

The `uploaded` value is parsed as protocol data. Structural parsing does not compare it to a wall
clock or treat it as server attestation.

## Byte Commitments And Byte-Verified Descriptors

A structural descriptor advances to `RadrootsBlossomApprovedDescriptor` only after its URL passes
the explicit HTTPS-or-loopback reference policy. `RadrootsBlossomByteCommitment::from_bytes`
computes the SHA-256 and exact `u64` size of a final byte slice and binds them to a caller-approved
`RadrootsBlossomMediaType`.

An approved descriptor advances to `RadrootsBlossomByteVerifiedDescriptor` only through
`verify_commitment`, or through the `verify_bytes` convenience path that constructs the same byte
commitment. Verification succeeds only when all three values match:

1. computed byte SHA-256 equals descriptor `sha256`
2. exact byte length equals descriptor `size`
3. caller-approved media type is semantically equal to descriptor `type`

These comparisons are independent. URL extension does not participate in MIME verification, and
no byte sniffing occurs.

The resulting byte-verified state proves only that the reference policy approved the descriptor URL,
the supplied descriptor and approved media type describe the supplied bytes, and the descriptor URL
carries the same digest. It is deliberately not named or modeled as an upload receipt. It does not
prove that an upload occurred, the descriptor was issued by the named server, the server is
authorized or reachable, the blob is currently available, a redirect preserves the hash, or a later
retrieval returns those bytes. An HTTP-capable runtime must separately require a successful BUD-02
upload response before publication and must perform bounded retrieval checks at their owning layer.

## Stable Conformance Operations

`contracts/conformance/vectors/blossom/hash_path_and_descriptor.v1.json` is executable evidence for
this contract. Integration tests dispatch its vectors by `kind`:

- `blossom.sha256.digest`
- `blossom.sha256.parse.valid` and `blossom.sha256.parse.invalid`
- `blossom.hash_path.parse.valid` and `blossom.hash_path.parse.invalid`
- `blossom.blob_url.parse.valid` and `blossom.blob_url.parse.invalid`
- `blossom.reference_policy.valid` and `blossom.reference_policy.invalid`
- `blossom.media_type.parse.valid` and `blossom.media_type.parse.invalid`
- `blossom.descriptor.parse.valid` and `blossom.descriptor.parse.invalid`
- `blossom.descriptor.approve_reference.valid` and
  `blossom.descriptor.approve_reference.invalid`
- `blossom.descriptor.verify_bytes.valid` and
  `blossom.descriptor.verify_bytes.invalid`

`bytes_hex` values are exact bytes encoded as lowercase hexadecimal. A `null` extension or port
means absence. Invalid typed-operation vectors use the exact stable identifier returned by
`RadrootsBlossomError::code()`.

## Stable Error Identifiers

The public typed API exposes these stable semantic identifiers:

- `invalid_sha256`
- `invalid_hash_path`
- `invalid_file_extension`
- `invalid_blob_url`
- `unsupported_blob_url_scheme`
- `blob_url_credentials_forbidden`
- `blob_url_query_forbidden`
- `blob_url_fragment_forbidden`
- `insecure_blob_url`
- `invalid_media_type`
- `descriptor_extension_required`
- `descriptor_hash_mismatch`
- `blob_hash_mismatch`
- `blob_size_mismatch`
- `blob_media_type_mismatch`

Malformed descriptor JSON can fail in serde before a `RadrootsBlossomError` exists. The executable
descriptor harness uses three additional wire-shape classifications for those cases:

- `missing_descriptor_field`, with the missing field named separately
- `invalid_descriptor_size`
- `invalid_descriptor_uploaded`

Nested descriptor values that reach a Radroots parser, including invalid SHA-256, URL, MIME, URL
extension, and URL/hash combinations, retain the applicable `RadrootsBlossomError::code()` value.

The canonical vector file is mirrored under `crates/blossom/tests/fixtures/` so the published crate
can execute the same conformance suite. Source-workspace tests require the canonical file and assert
that the packaged mirror is byte-for-byte current.

The public error enum is non-exhaustive so later BUD slices can add typed failures without breaking
consumers. Adding error detail is allowed, but it must not collapse protocol structure, reference
approval, and byte verification into one indistinguishable state.
