# radroots_protocol

With `listing`, the public decoder validates a complete canonical profile and
its standard projections without assuming a particular tag order. With
`advertisement`, the closed capability decoder preserves exact bounded versions,
features and at-most-thirty-day author-time validity. These are passive claims;
signature admission, observed-head ranking and current-time policy belong to the
concrete adapter. Known values supply neither source freshness nor permission.

The chat profile's `GiftWrapBudget` measures the complete kind-1059 envelope
for the exact already obtained seal representation. Whitespace in retained
signed JSON still consumes encryption and event budget; canonical event
identity does not permit replacing those bytes during recovery.

Authored chat rumors carry the original full operation ID in a private
`radroots/operation` tag. Identical messages from distinct operations in one
second therefore have distinct NIP-01 message identities. Replaying the
original operation preserves its captured time, exact rumor and message ID;
the discriminator supplies no authorization.

`radroots_protocol` defines the passive, versioned wire contracts shared by
Radroots processes and language bindings. It is a `no_std + alloc` foundation
crate: it validates schema identities and contract catalogs, but performs no
network, storage, signing, scheduling, or process-lifecycle work.

The crate is pre-release and its Cargo version is frozen at `0.1.0-alpha` until
explicitly changed. Cargo package versions are independent from the version
numbers embedded in wire and schema contracts.

## Contract surface

| Module | Authority |
| --- | --- |
| `capability::v1` | transport capabilities, maturity, and availability |
| `error::v1` | stable, redaction-safe error reports and recovery guidance |
| `event::v1` | event-kind catalogs and the serialized trade-state vocabulary |
| `runtime::v1` | operation descriptors, risk, approval, idempotency, and effects |
| `radrootsd::transport_publish::v5` | daemon transport-publish request, job, outcome, and capability DTOs |
| `schema` | schema IDs, module generations, descriptors, and aggregate registries |
| `profile` | assigned regular commerce kind8890 and replaceable capability kind18890, with closed profile identities; authoring qualification remains pending |
| `terms` (`terms` feature) | bounded closed Terms v1 decoding, explicit protected encoding and domain-separated digest; no authenticated consent, stock or signing authority |
| `commerce` (`commerce` feature) | complete closed record data, canonical RecordId and exact final carrier-field matching; no authenticated ingress or issuing authority |
| `private_rumor` (`private_rumor` feature) | closed protected unsigned data, bounded decoded text/tags and original NIP-01 RumorEventId recomputation; no cryptographic carrier admission |
| `command` (`commands` feature) | bounded closed v1 inquiry/reply data codec and exact command/approval-intent hashes |
| `nip01` (`nip01` feature) | exact bounded canonical preimages, IDs and complete-event sizes |
| `intent` (`intent` feature) | exact v1 signing commitment data without execution authority |
| `chat` (`chat` feature) | bounded kind-14/13 data, complete immutable signing-plan codec and observed DM route descriptors |
| `listing` (`listing` feature) | closed sale-option inputs and bounded kind-30402/profile-v1 preimages without execution or publication authority |

Versioned DTOs live below an explicit generation module. The command codec
implements its fixed v1 profile without exposing automatic serialization of
captured private commands. Parsed command data supplies no authority or consent.
Command roots, inquiry/reply inputs, source references and all shared context
descriptors require objects; positional arrays are refused. The map-only typed
decoder preserves borrowed input and existing duplicate, unknown-field, explicit
null, byte/depth and exact canonical command/hash checks.
The crate root
does not reexport individual DTOs, so consumers must name the contract version
they use.

Public listing inputs retain fixed, quote-required or per-sale-unit variable
prices, with fixed/estimated/unspecified package metadata. Object fields reject
positional arrays, and the closed typed decoder rejects duplicate, missing and
unknown fields. Up to eight complete media-reference objects retain exact
URL/digest/MIME/alt claims; missing, malformed or excessive references are
refused. Media never activates fetching, uploading or a decoder. Estimated
supply retains its exact nonnegative amount and a required nullable absolute
window. Windows require strictly ordered Unix-second strings and an exact known
IANA zone from the pinned dataset; missing fields, positional arrays, duplicates,
noncanonical zones and extra offset/fold/wall-time claims are refused. Estimates
do not provide stock, reservation or freshness authority.
Only the fixed default contributes a three-field NIP-99 base-rate `price` tag;
the monetary summary identifies its basis, unit, discount and tier conditions.
Variable bounds/caps stay explicit in the profile and summary. No recurring
frequency field is used for units. Complete signed events retain the64KiB cap,
with at most nine tags and19 fields shared by signer and storage admission.

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | implements standard-library error integration and enables `serde?/std` |
| `serde` | yes | derives serialization and deserialization for wire-contract types |
| `commands` | no | enables the portable closed chat codec, domain values, SHA-256 and zeroizing private command buffers |
| `nip01` | no | enables pure exact NIP-01 serialization and complete byte counting |
| `private_rumor` | no | enables bounded closed protected unsigned data and original rumor identity checks |
| `intent` | no | enables pure domain-separated v1 signing commitment data |
| `chat` | no | enables commands, NIP-01 and the bounded known-kind chat/plan data profile |
| `listing` | no | enables portable complete listing input and exact public profile/event production |
| `terms` | no | enables the closed private Terms codec over validated domain data, without record authoring |
| `commerce` | no | enables all nineteen complete record families, their closed codec and canonical identity over Terms data |

Terms encoding preserves complete source snapshots, resolved prices, private
fulfillment and explicit supply commitments. It counts the escaped UTF-8 output
before allocating one zeroizing private buffer, retains exact array order and
computes `SHA256("radroots.terms.v1\0" || JCS(Terms))`. The16KiB Terms limit
also applies jointly with the header/payload when building a complete private
record; fitting Terms alone supplies no complete-record or nested-wrap proof.
`EncodedTerms::protected_wire` is an explicit private disclosure. Its digest and
redacted diagnostics provide no participant approval, authenticated source,
reservation or installed agreement.

`terms::decode` admits at most16KiB and depth16, requiring closed objects,
explicit nullable fields and bounded line/group arrays. It checks duplicates
before acceptance and runs the shared domain constructors for every value.
Private text is borrowed from the caller's original frame until explicitly
decoded into zeroizing buffers; only public snapshots enter temporary JSON
trees. Decoding a claim establishes no signature, consent or current source
availability. Callers retain responsibility for protecting original bytes.
Profile public keys must match the identity library's canonical lowercase
spelling. The general identity parser remains case insensitive; profile input
does not silently normalize an uppercase participant, organization, peer,
listing author or product producer.

The commerce codec counts the complete header, record ID and payload against
the16KiB private content limit. Its record identity is
`SHA256("radroots.record.v1\0" || JCS(complete content without record_id))`.
The temporary protected hash preimage is dropped before final wire allocation.
All nineteen families retain complete data and exact causal references; parsing
checks every domain constructor and recomputes the content identity. Unknown
profile/version/type is classified before interpreting a known payload.
`check_rumor_fields` compares the final kind, author and time claims. Callers
must separately authenticate the complete wrap/seal/rumor and recipient, prove
history/authority/consent and check enclosing byte budgets before issuing or
installing anything. These data APIs enable no structured send or stock effect.

The listing producer emits one canonical `radroots.listing` version-1 profile,
explicit default units and no fabricated price. Paused/withdrawn projections
keep the same addressable coordinate, state non-availability in summary and
content, and omit a standard status that cannot represent that state. Complete
signed-event framing is bounded to 64 KiB before the canonical preimage is
allocated. Canonical profile ordering remains explicit when a consumer enables
JSON insertion-order preservation. The producer retains all implemented price,
package-content and public-supply modes, including estimated supply windows,
and the explicit media list. Pure input/preimage construction supplies no workspace
permission, durable preparation, signature, installed record or delivery receipt.

Disabling default features leaves the catalog and structural validation APIs
available with `alloc` only. Enable `serde` without `std` for portable encoded
DTOs.

## Validate the aggregate contract

```rust
use radroots_protocol::{capability, event, runtime, schema};

capability::v1::validate_catalog(capability::v1::CATALOG).unwrap();
event::v1::validate_catalog(event::v1::CATALOG).unwrap();
event::v1::validate_trade_state_vocabulary(event::v1::TRADE_STATE_VOCABULARY)
    .unwrap();
runtime::v1::validate_catalog(runtime::v1::CATALOG).unwrap();

let registry = schema::protocol_v1_registry().unwrap();
assert!(!registry.is_empty());
```

For a runnable version, see
[`examples/inspect_contract.rs`](examples/inspect_contract.rs).

## Serialization and trust boundaries

Wire representations are stable only within their named generation. A caller
must deserialize into the intended version, run the module's structural
validation, and apply its own authorization and policy checks before acting on
the data. Successful deserialization alone does not make input trusted.

Stable error reports intentionally carry only safe messages and bounded detail
values. Use `error::v1::ErrorReport::redacted_from_source` at sensitive
boundaries; do not copy secrets, credentials, raw upstream errors, or private
payloads into protocol DTOs.

## Side effects, cancellation, and commit points

This crate has no side effects, asynchronous work, cancellation mechanism, or
commit point. Runtime operation descriptors describe those properties; they do
not execute operations. The owning SDK, daemon, storage, signing, or transport
implementation must define cancellation behavior and must not report a commit
until its own durable boundary has succeeded.

## Intended consumers

The direct consumers are Radroots domain crates, SDK and daemon runtimes,
transport/storage/signing boundaries, generated bindings, and conformance
tools. Applications should normally enter through `radroots` or
`radroots_sdk`, using this crate directly only when implementing or inspecting
a versioned boundary.

The package charter is the
[Release V1 specification](../../contracts/crates/release_v1/radroots_crates_release_v1.toml).
The reviewed Rust surface is recorded in the
[public API baseline](../../contracts/api_baselines/radroots_protocol.txt).

## Complete private signing-plan data

`command::CapturedCommand` preserves the original chat bytes and commitments
and admits a separate typed `listing::CapturedListingCommand` when `listing`
is enabled. Public capture uses the same context and correlation/approval hash
projections. Its closed parser validates the supported quote-required profile
through domain constructors; unsupported price, supply, package contents or
media features are refused explicitly. Public commands have no chat peer or
private message. Capture supplies no current grant, signature or durable effect.

`chat::ChatSigningPlan` retains the exact sorted unique peer/self recipient set,
seal author/kind/time/empty tags/content/event ID/purpose, original signed DM
route-source bytes, selected targets, logical message ID, command commitment and
provider replay declaration. The closed binary profile is capped at 384 KiB;
its decoded recipient collections discard caller spare capacity. Original route
sources are each at most 64 KiB, selected URIs at most 2048 bytes, and the
combined session selection at most sixteen distinct relays.

Construction and decoding validate descriptors only. They verify no signature,
MAC, grant, consent, route readiness or provider capability and create no durable
commit. The executor must verify source evidence, approved targets and current
access, then retain this complete plan through its qualified protected journal
before any business signing invocation. A stored replay declaration alone never
permits uncertain reissue.
