# Blossom BUD-11 Authorization Foundation

Status: active implementation contract

Scope: public, runtime-independent parsing, endpoint validation, and strict upload-claim authoring
for Blossom BUD-11 authorization claims.

## Protocol Basis

This contract is based on
[Blossom BUD-11](https://github.com/hzrd149/blossom/blob/b5bd2801d1763aa635fc8fea7a76597e0eb18990/buds/11.md)
at Blossom commit `b5bd2801d1763aa635fc8fea7a76597e0eb18990`.

The example event in that upstream revision is not conformance evidence: its empty content violates
the same revision's human-readable-content requirement, its `created_at` is later than its
`expiration`, and the displayed signature therefore cannot establish a valid claim. Radroots uses
independent vectors and does not copy that example as a positive fixture.

BUD-11 claims are Nostr kind `24242` HTTP authorization tokens. They are ephemeral request
credentials. They are never relay-published, persisted in canonical relay fixtures, or projected as
product events.

## Ownership And Dependency Boundary

`radroots_blossom` owns pure BUD-11 vocabulary, tag parsing, endpoint scope validation, and strict
upload-claim construction as a public `no_std + alloc` protocol leaf. It accepts content,
`created_at`, and raw string tag arrays rather than depending on Nostr event types.

The outward `radroots_nostr` adapter owns the signed kind-`24242` event envelope, event-id and
signature verification, JSON encoding, and canonical Base64url-without-padding `Authorization:
Nostr` header. It emits the canonical `Nostr ` scheme spelling and one space. Inbound decoding
matches the HTTP authentication scheme case-insensitively and accepts the RFC 9110 `1*SP`
separator while retaining strict Base64url credential validation. HTTP clients and servers own
transport. Entitlement, rate limits, replay storage, production-domain policy, and any private
all-server-match rule remain outside this public crate.

## Actions And Endpoint Targets

`RadrootsBlossomAuthorizationAction` has exactly the five lowercase BUD-11 wire values `get`,
`upload`, `list`, `delete`, and `media`. Case variants, whitespace, unknown verbs, and the empty
string are invalid.

Endpoint validation uses an explicit `RadrootsBlossomAuthorizationTarget`:

| Endpoint | Target | Required action | Implied hash | `x` policy |
| --- | --- | --- | --- | --- |
| `GET /<sha256>` or `HEAD /<sha256>` | `GetBlob(hash)` | `get` | URL hash | optional when absent; any-match when present |
| `PUT /upload` or `HEAD /upload` | `Upload(hash)` | `upload` | `X-SHA-256` | required, any-match |
| `GET /list/<pubkey>` | `List` | `list` | none | not applicable |
| `DELETE /<sha256>` | `DeleteBlob(hash)` | `delete` | URL hash | required, any-match |
| `PUT /mirror` | `Mirror(hash)` | `upload` | mirrored blob hash | required, any-match |
| `PUT /media` or `HEAD /media` | `Media(hash)` | `media` | `X-SHA-256` | required, any-match |

The target model intentionally collapses HTTP methods that have identical BUD-11 requirements. It
does not authorize arbitrary endpoints and does not perform HTTP parsing.

## Human-Readable Content

`RadrootsBlossomAuthorizationContent` is nonempty, contains no Unicode control character, and is
already equal to its Unicode-whitespace-trimmed form. This rejects empty and whitespace-only input,
leading or trailing whitespace, line breaks, tabs, and embedded controls. Interior ordinary spaces
and non-ASCII human-readable text are retained. Parsing does not silently trim or rewrite signed
content.

This requirement is structural and deterministic. It cannot prove that wording accurately
describes a request, so product and runtime layers remain responsible for choosing clear text such
as `Upload farm photo`.

## Server Domains

A `RadrootsBlossomServerDomain` is an ASCII lowercase domain name of at most 253 bytes without
scheme, user information, port, path, query, fragment, IP-literal brackets, or trailing dot. Every
dot-separated label is from 1 through 63 bytes, begins and ends with an ASCII lowercase letter or
digit, and otherwise contains only ASCII lowercase letters, digits, or `-`. `localhost` is valid.
An all-numeric host spelling is valid only when it is the exact canonical four-octet dotted-decimal
IPv4 form; shortened, leading-zero, and out-of-range forms are rejected. Input is validated exactly
and is not lowercased or otherwise normalized.

`server` tags are optional in the BUD-11 protocol. With `OptionalAnyMatch`, no `server` tags means
the claim is valid for any server. When one or more are present, at least one must equal the
validation server. `RequiredAnyMatch` additionally rejects absence. Multiple valid server tags are
allowed, and a matching tag makes a mixed set valid. A private deployment may require every tag to
match, but that stricter policy is deliberately not part of this public protocol contract.

Strict authored Radroots upload claims require exactly one caller-supplied server domain. This
reduces replay scope without changing tolerant inbound BUD-11 semantics.

## Blob Hash Scope

Every known `x` tag value is a `RadrootsBlossomSha256`: exactly 64 lowercase hexadecimal
characters. Multiple valid `x` tags are allowed.

For targets requiring a hash scope, at least one `x` tag must equal the target's implied hash. Other
valid hashes may coexist because BUD-11 specifies any-match behavior. A `GetBlob` claim may omit
`x`; if any `x` tags are present, at least one must match. `List` has no implied hash and does not
apply hash matching, although every present known `x` tag must still be structurally valid.

Strict authored upload claims contain exactly one `x` tag equal to the upload hash.

## Claim Tag Parsing

`RadrootsBlossomParsedAuthorizationClaim::parse` consumes signed content, the event `created_at`
timestamp, and raw Nostr tag arrays. Structural parsing occurs before endpoint policy validation.

Known tags use exact lowercase names and require at least a key and value:

- exactly one `["t", "<action>", ...]` is required
- exactly one `["expiration", "<unsigned-decimal-unix-seconds>", ...]` is required
- zero or more `["server", "<domain>", ...]` tags are allowed
- zero or more `["x", "<lowercase-sha256>", ...]` tags are allowed

A missing, duplicate, malformed, or empty-valued `t` or `expiration` tag fails parsing. Malformed
or empty-valued `server` and `x` tags fail parsing even when another value would satisfy endpoint
policy. Consistent with NIP-01, trailing elements after a known tag's value are tolerated and do not
change its meaning. Repeated `server` and `x` tags are protocol scopes rather than duplicate
singleton fields and are retained in wire order.

Unknown tag names are ignored, including future tags with additional fields. They cannot satisfy a
known requirement. Case variants such as `T`, `Expiration`, `Server`, or `X` are unknown Nostr tags
and do not alias the lowercase BUD-11 names.

Parsing does not validate the Nostr kind, event id, signature, HTTP header, clock, endpoint action,
or target scope. Those checks belong to the appropriate outward adapter or validation transition.

## Time Validation

Endpoint validation receives an explicit `now` and maximum creation age so it is deterministic and
does not read a wall clock. Radroots uses
`RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS = 300`.

The requested maximum creation age may be from 0 through 300 seconds inclusive. A value above the
public cap is rejected when the validation policy is constructed rather than weakening the replay
window by accident.

- `created_at == now` is accepted to tolerate integer-second signing and validation in the same
  second
- `created_at > now` is rejected as future-dated
- `now - created_at <= max_created_age` is accepted
- an older value is rejected as stale, using checked comparisons without unsigned wraparound
- `expiration > now` is required; `expiration == now` is expired
- `expiration - created_at` must be from 1 through
  `RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS = 300` seconds inclusive

The 300-second horizon is the Radroots replay-limiting application profile layered on BUD-11.
Strict authored upload construction applies the same bound: lifetime must be from 1 through 300
seconds inclusive, and `created_at + lifetime` must not overflow `u64`.

## Strict Authored Upload Claims

`RadrootsBlossomAuthoredUploadClaim::new` requires typed human-readable content, one typed server
domain, one exact typed SHA-256, `created_at`, and a valid authored lifetime. It produces canonical
wire parts in this exact order:

```text
["t", "upload"]
["expiration", "<created_at-plus-lifetime>"]
["x", "<lowercase-sha256>"]
["server", "<lowercase-domain>"]
```

The constructor neither signs nor serializes a Nostr event. Callers must use the named Nostr
adapter, which must sign kind `24242` and must not offer relay publication as an authorization path.
The signed and verified authorization typestates keep the raw event private; they expose only its
event id, author, creation timestamp, and validated claim data.

Fresh Schnorr signing intentionally uses auxiliary randomness and is not modeled or registered as a
deterministic machine operation. Canonical header encoding is deterministic for an already signed
opaque authorization value, but that opaque typestate cannot be reconstructed from a fixed vector
without weakening the API. The operation registry intentionally enumerates reproducible machine
operations rather than every public helper. Fresh signing and encoding therefore remain directly
tested but outside deterministic machine-operation registration. Decode/verify is registered
directly, and fixed checked-in signed-event vectors make header decoding, id verification,
signature verification, and pure-claim validation reproducible without asserting dynamic signature
equality.

## Stable Conformance Operations

`contracts/conformance/vectors/blossom/bud11_claims.v1.json` is executable evidence for this
contract. Integration tests dispatch every vector through the public API using these kinds:

- `blossom.bud11.action.parse.valid` and `blossom.bud11.action.parse.invalid`
- `blossom.bud11.server_domain.parse.valid` and
  `blossom.bud11.server_domain.parse.invalid`
- `blossom.bud11.content.parse.valid` and `blossom.bud11.content.parse.invalid`
- `blossom.bud11.claim.parse.valid` and `blossom.bud11.claim.parse.invalid`
- `blossom.bud11.validation.new.valid` and `blossom.bud11.validation.new.invalid`
- `blossom.bud11.claim.validate.valid` and `blossom.bud11.claim.validate.invalid`
- `blossom.bud11.authored_upload.valid` and `blossom.bud11.authored_upload.invalid`

`contracts/conformance/vectors/blossom/bud11_nostr_adapter.v1.json` supplies fixed signed-event and
header evidence for the outward adapter. Its
`blossom.bud11.nostr.decode_verify.valid` and
`blossom.bud11.nostr.decode_verify.invalid` kinds execute canonical Base64url decoding, exact raw
event bytes, kind checks, event-id checks, signature checks, and pure claim validation in that
order. Syntax-only vectors cover whitespace, a wrong scheme, empty payload, padding, alphabet,
canonical tail bits, UTF-8, and JSON failures. Dynamic adapter tests additionally prove
case-insensitive `Nostr` matching and a one-or-more-space separator with a valid signed credential.
Cryptographic vectors contain immutable signed material; tests do not mint a fresh signature and
compare it for equality.

The adapter rejects unknown or duplicate top-level event fields rather than accepting ambiguous
credential JSON representations; a generic JSON map must not make a duplicated field acceptable by
collapsing it. The adapter validates the JSON kind as a non-truncated integer before converting to a
Nostr kind, so a value greater than `u16::MAX` cannot wrap into `24242`.

Invalid vectors use the exact stable identifier returned by the owning Blossom or Nostr adapter
error's `code()` method. The pure suite is mirrored at
`crates/blossom/tests/fixtures/bud11_claims.v1.json`; the signed adapter suite is mirrored at
`crates/nostr/tests/fixtures/bud11_nostr_adapter.v1.json`. Source-workspace tests require each
canonical file and assert byte-for-byte equality with its packaged mirror. A missing canonical file
falls back to the applicable mirror only when the positive Radroots workspace contract marker is
also absent, which prevents a broken source checkout from silently testing stale packaged data.

## Stable Error Identifiers

The public typed API exposes these stable BUD-11 identifiers:

- `invalid_authorization_content`
- `invalid_authorization_action`
- `invalid_authorization_server_domain`
- `missing_authorization_action_tag`
- `duplicate_authorization_action_tag`
- `malformed_authorization_action_tag`
- `missing_authorization_expiration_tag`
- `duplicate_authorization_expiration_tag`
- `malformed_authorization_expiration_tag`
- `malformed_authorization_server_tag`
- `malformed_authorization_hash_tag`
- `invalid_authorization_created_age`
- `invalid_authorization_lifetime`
- `authorization_timestamp_overflow`
- `authorization_created_in_future`
- `authorization_stale`
- `authorization_expired`
- `authorization_action_mismatch`
- `authorization_server_required`
- `authorization_server_mismatch`
- `authorization_hash_required`
- `authorization_hash_mismatch`

The outward Nostr adapter additionally exposes these stable boundary identifiers:

- `invalid_header_whitespace`
- `invalid_header_scheme`
- `empty_header_payload`
- `header_padding_forbidden`
- `invalid_header_base64`
- `noncanonical_header_base64`
- `invalid_header_utf8`
- `invalid_event_json`
- `invalid_event_kind`
- `invalid_event_id`
- `invalid_event_signature`
- `event_signing`

An authenticated pure-claim failure retains its `RadrootsBlossomError::code()` identifier rather
than being collapsed into a generic adapter error.

Direct typed action, server-domain, and hash parsing uses `invalid_authorization_action`,
`invalid_authorization_server_domain`, and the existing `invalid_sha256`. Inside a claim, a known
tag with a missing or semantically invalid value uses its contextual malformed-tag identifier.
Trailing elements remain tolerated.

## Security Boundary

A validated pure claim proves only that structurally parsed claim data satisfies one declared
target, server, hash-scope, and clock policy. It does not prove the kind, id, signature, signer
identity, header encoding, network destination, entitlement, possession of bytes, upload success,
single use, or absence of replay. The Nostr adapter must verify the signed envelope before exposing
the claim, and the owning HTTP service must enforce authorization and replay-sensitive runtime
policy.
