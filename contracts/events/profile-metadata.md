# Profile metadata contract

Status: canonical

This contract defines the public kind-`0` Profile metadata boundary used by
Radroots strict authoring and tolerant inbound projection. It is based on the
pinned NIP-01, NIP-05, and NIP-24 documents at NIPs commit
`bdfa7e62ef87fcfcb992b1a27aee49d36b0b4f91` and the Blossom protocol at commit
`b5bd2801d1763aa635fc8fea7a76597e0eb18990`.

## Operation authority

| Operation | Boundary | Signing | Transport |
| --- | --- | --- | --- |
| `event.verify_nip01` | exact identifier and Schnorr verification to a non-forgeable wrapper | NIP-01 | none |
| `event.select_head` | NIP-01 replaceable/addressable timestamp and lowest-id selection | none | none |
| `profile.build_authored_draft` | strict authored metadata to kind-`0` wire parts | none | none |
| `profile.parse_inbound_metadata` | JSON object to tolerant inbound metadata | none | none |
| `profile.verify_and_admit_event` | verified exact kind-`0` envelope to tolerant metadata bound to that envelope | NIP-01 | none |

`profile.parse_inbound_metadata` is a content parser, not an event-acceptance
boundary. A caller must supply content from a kind-`0` event only after the
event identifier and signature have been verified. The authoritative combined
boundary is `profile.verify_and_admit_event`: it recomputes the identifier,
verifies the Schnorr signature, requires exact kind `0`, and only then invokes
the same tolerant parser. It accepts standard tagless Profile events and does
not require a Radroots marker tag.

`event.verify_nip01` and its `RadrootsSignatureVerifiedEvent` result are
available through the codec's `nostr` feature without enabling `knowledge`.
Envelope conversion requires the author to be a valid secp256k1 x-only public
key; a canonical-length hex value that is not a curve point returns
`malformed_envelope`, not `signature_invalid`. Knowledge contract validation and
decoding are a later optional stage and cannot be substituted for general event
verification.

The direct legacy `RadrootsProfile` codec, Profile-specific Nostr/network
publish helpers, and replica Profile draft emission were removed in the
`1.0.0-alpha.1` breaking contract. A replica Profile row is a lossy inbound
projection and cannot prove author intent for a complete kind-`0` replacement.
New authored callers must use `profile.build_authored_draft`. Generic raw event
builders and send APIs remain protocol escape hatches; their ability to emit
kind `0` does not make them Profile operation authority.

`RadrootsNostrClient` no longer dereferences implicitly to the upstream SDK
client, so upstream Profile conveniences are not exposed through ordinary
method resolution. `from_inner` and `into_inner` remain explicit low-level
interoperability boundaries; once a caller takes that boundary, upstream APIs
are outside the Radroots authored-operation contract.

Legacy `profile::decode`, Nostr Profile adapters/fetchers, the network Profile
fetch methods, and replica Profile ingest remain read-side compatibility paths.
They may require or coerce legacy fields, discard unprojected metadata, or rely
on legacy marker tags. They are not `profile.parse_inbound_metadata`, do not
establish verified event admission, and must not be used as its substitute.

## Strict authored boundary

`RadrootsAuthoredProfile` has private fields and requires a non-whitespace,
control-free `name`, consistent with the NIP-24 recommendation that `name`
remain present when `display_name` is used. The scoped optional fields are
`display_name`, `about`, `nip05`, `bot`, `picture`, and `banner`; `bot` is a
Boolean. NIP-05 values enter only through `RadrootsNip05Identifier`. Picture and
banner values enter through the shared `RadrootsAuthoredImage`, which can wrap
only an `image/*` `RadrootsBlossomByteVerifiedDescriptor`. There is no raw-string
media setter or unchecked deserialization path. Generic `website`, `lud06`, and
`lud16` strings remain outside this strict authored operation.

Kind `0` is whole-object replaceable. Strict authored output is therefore a
complete replacement snapshot, never a patch: every omitted existing standard,
residual, or custom field is removed by the replacement. This operation does
not merge the tolerant inbound raw object. It is safe for initial Profile
creation or an explicitly confirmed full replacement; it must not power a
silent partial edit. Retaining an existing picture or banner requires the
runtime to re-fetch or retain the bytes, re-establish the byte-verified image
descriptor, and satisfy BUD-02 before signing. Until such a full-snapshot edit
pipeline exists, clients must keep existing Profile editing read-only.

The authored codec emits:

- kind `0`
- no tags
- one JSON object with fields in this deterministic order when present:
  `name`, `display_name`, `about`, `picture`, `banner`, `nip05`, `bot`
- only the descriptor URL for each media field
- at most 131072 UTF-8 bytes of metadata content

The descriptor state proves that approved descriptor hash, size, and media type
match supplied bytes. It does not prove that BUD-02 upload completed or that a
blob is network-retrievable, and it does not inspect or sanitize the image
format. A publication runtime must require successful BUD-02 completion before
signing media-bearing output. A consuming media runtime remains responsible for
decode and format-safety policy.

For two verified kind-`0` events from the same author, the head is the event
with the greater `created_at`. Equal timestamps select the lexicographically
lowest canonical event id, independent of relay or ingestion arrival order.

## Tolerant inbound boundary

`profile.parse_inbound_metadata` accepts a JSON object of at most 131072 UTF-8
bytes without requiring `name`. Correctly typed `name`, `display_name`, `about`,
`picture`, `banner`, `nip05`, and `bot` fields are projected without JSON type
coercion; accepted NIP-05 domains are canonicalized as described below. In
particular, `bot` must be a JSON Boolean. Every string picture or banner is
returned as `RadrootsUnverifiedProfileMediaReference`, including strings that
look like valid Blossom hash-path URLs.

The result retains:

- the exact input content
- the complete parsed object
- a residual map containing every unknown field, wrong-typed known field, and
  syntactically invalid NIP-05 string

Oversized content is rejected before parsing. Malformed JSON and non-object
roots are parse errors. Duplicate top-level metadata keys are rejected because
they have ambiguous cross-parser semantics. Optional metadata that this
contract does not project, including NIP-24 `website` and birthday data plus
`lud06` and `lud16`, remains in the residual and complete raw views. Exact
nested JSON text, including any duplicate nested names, remains available
through the raw content view.

## NIP-05 boundary

`RadrootsNip05Identifier` requires exactly one `@`, a non-empty local part using
only lowercase `a-z`, digits, `-`, `_`, or `.`, and an ASCII DNS domain.
Domain matching is case-insensitive, so accepted domains are canonicalized to
lowercase. Parsing is syntax-only. It performs no HTTPS lookup and never
represents verified ownership or identity trust.

A syntax-checked identifier is not a safe network-fetch target by itself. A
future resolver must separately govern HTTPS, redirects, address resolution,
loopback/private/link-local targets, response size, and timeouts.

## Stable error codes

The public error enums are non-exhaustive so future codes can be added without
making downstream matches exhaustive. Current stable codes are:

| Boundary | Codes |
| --- | --- |
| NIP-05 syntax | `missing_separator`, `multiple_separators`, `invalid_local_part`, `invalid_domain` |
| strict Profile construction | `invalid_name` |
| shared authored image construction | `media_type_not_image` |
| strict Profile encoding | `content_too_large` |
| tolerant inbound parsing | `content_too_large`, `invalid_json`, `root_not_object`, `duplicate_field` |
| NIP-01 event verification | `malformed_envelope`, `kind_out_of_range`, `id_mismatch`, `signature_invalid`, `signature_verification_unavailable` |
| verified Profile admission | NIP-01 codes plus `invalid_kind`, `content_too_large`, `invalid_json`, `root_not_object`, `duplicate_field` |

Inbound size validation occurs before JSON parsing. For content within the
limit, malformed JSON returns `invalid_json`; a well-formed non-object returns
`root_not_object`; and a well-formed object with a duplicate top-level field
returns `duplicate_field`. Wrong-typed projected fields are residual data, not
parse errors.

## Conformance

The canonical suite is
`contracts/conformance/vectors/profile/metadata.v1.json`. Verified event
admission and replacement use
`contracts/conformance/vectors/profile/verified_event.v1.json`. Both are
mirrored under `crates/event_codec/tests/fixtures/` for published-package
tests. The dispatchers execute every vector against the public APIs and require
canonical and packaged copies to be byte-for-byte equal when the workspace
contract is present. The verified-event suite includes a canonical-id raw event
whose author is not a valid secp256k1 curve point, proving the stable
`malformed_envelope` mapping. Consumers enable `serde_json` for metadata
operations and both `serde_json` and `nostr` for cryptographic Profile
admission.
