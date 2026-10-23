# Radroots domain

The generated `NOTICE` retains the pinned currency facts, original data
attribution and license texts, and selected MIT timezone dependency material.
Keep it with derived artifacts. Regenerate it from the source-owned contract
with `cargo extbuild run -- cargo xtask generate data-notices --write`;
`--check` and canonical contract validation enforce byte freshness.

Independently implemented exact values for the shared SDK successor. This
pre-release package provides validated values and public sale-option offer
terms. It does not execute commands, install agreements or establish stock.

`Decimal` has a nonnegative coefficient up to 999999999999 and scale 0–6.
Construction normalizes fractional trailing zeros. String admission requires
that canonical representation; JSON uses strings exclusively. `PositiveDecimal`
additionally rejects zero. Checked addition and subtraction are exact and
reject results outside the supported representation. No floating conversion
or implicit rounding is available.

The package supports `no_std`; `serde` and `std` are independent features.
It performs no I/O, clock, randomness, storage or network work.

`Currency` admits the 165 fixed supported currency/fund codes and their exact
minor-unit exponents. Unknown codes and unavailable minor units never receive
a guessed exponent. `MinorAmount` admits canonical integer strings up to
999999999999999. `Money` checks the explicit wire exponent against that registry
and performs checked signed 128-bit addition/subtraction within one currency.
JSON minor amounts remain strings; private monetary diagnostics are redacted.
Explicit zero money does not imply a price mode.

`FixedPrice` validates at most eight strictly increasing same-currency tiers
and one discount in basis points 0–10000. It selects the greatest applicable
threshold, applies the discount before rounding, and rounds once per net line
using half-even checked signed 128-bit rational arithmetic. `checked_order_total`
sums one through 32 rounded lines and an explicit same-currency fulfillment
charge within the money cap. The calculation uses one selected sale-unit basis
and performs no implicit physical conversion. Sale options validate purchase
rules before using this engine. Complete bilateral terms, agreement installation
and command effects remain separate requirements; calculations establish no
authorization or stock.

`Quantity` uses the existing measurement identity and exact decimal bounds.
Conversions between g/kg and mL/L are explicit and reject precision loss;
arithmetic requires matching units. `each` counts remain integral.
`ContentMeasure` descriptors distinguish fixed, estimated and unspecified
contents; estimates do not provide fixed conversions or stock backing. Existing
sale options validate selected amounts against their minimum, increment and
integral package/count rules. Advertised supply establishes no quantity ceiling
or actual availability. Sale-unit packages retain all three content modes;
complete bilateral-term integration remains separate.

`VariableWeight` retains a positive inclusive measurement range, exact rate and
basis, and an explicit same-currency maximum total. Its minimum is feasible
within the cap. Final measurements must use the declared unit, remain in range,
and recompute within the cap using the same half-even engine without a discount.
An in-range measurement can still exceed the cap and require refusal; this does
not amend the terms or consume stock. Offer/line scaling and installation remain
the responsibility of complete enclosing validated terms and execution.

Order, thread, workspace, executor and operation IDs have distinct Rust types.
Each stores 32 bytes and uses exactly 64 lowercase hexadecimal characters in
strings and JSON. Byte/string construction validates representation; generation
belongs to the execution owner's qualified CSPRNG boundary.

`listing::ListingId` is the distinct 128-bit stable coordinate discriminator;
`ListingOptionId` is a 256-bit sale-choice identity. Public listing terms bound
and deduplicate options, retain an exact selected default, and validate positive
increments/minimums without floating conversion. Package and `each` quantities
remain integral; measured units imply no conversion. Quote-required prices
contain no invented zero amount or currency. Fixed options retain their complete
rate/basis, discount and tiers; `fixed_line` validates the selected purchase
amount before calculating it. Variable option bounds and caps describe one
sale unit and require complete absolute agreement terms before installation.
Optional opaque authority and
producer metadata establish no permission or endpoint trust. These immutable
terms serialize public data explicitly and redact ordinary diagnostics.

`listing::Media` retains bounded, nonempty URL/MIME claims, exact alternative
text and a declared SHA-256 digest. Construction does not normalize claims,
approve an endpoint, fetch a file, verify its contents or promise a decoder.
`ListingTerms::new` requires the explicit media list, with at most eight
references and preserved order. Empty media is explicit. Media descriptors
serialize complete exact strings and have no unchecked deserialization route.

`Supply::Estimated` retains an exact nonnegative amount and an explicit nullable
`Window`. Package and `each` estimates remain integral. Estimates are neither
stock balances nor reservations, ceilings or current-availability guarantees.
`Window` requires strictly ordered absolute Unix-second instants and an exact
canonical `TimeZoneId` from the already pinned no_std `jiff-tzdb` 0.1.8 / IANA
2026c data (MIT OR Unlicense). No system zone, clock, local wall-time, offset or
fold is inferred. The timezone dependency retains its own upstream notices.
Private fulfillment data retains a required window, explicit pickup/delivery
mode and bounded zeroizing address/instructions/contact. It has no public wire
serialization and grants no agreement, destination or delivery authority.

`Terms` retains complete immutable bilateral values: one through32 uniquely
identified lines, one through8 uniquely identified private fulfillment groups,
explicit distinct buyer/seller participants, original listing revision/product/option snapshots,
selected positive purchase amounts, fulfillment fee, aggregate cap and supply
commitment. Every line names a declared group and a source coordinate for the
named seller. Coordinate/revision bytes and snapshots remain unverified claims
until independent authenticated source admission.

`AgreementPrice` admits resolved fixed or absolute variable measurement terms;
quote-required has no agreed-price representation. `AgreedFixedPrice` retains
the selected rate/threshold, basis and discount independently of the unchanged
public option. It uses the shared half-even primitive and refuses a selected
threshold above the purchased amount. Lines validate original purchase rules
and recompute the declared fixed total or explicit variable cap. `Terms` sums
those checked maxima and the explicit fee in one currency within the money cap.
Variable line bounds are explicit absolute values; construction performs no
per-sale-unit scaling. Future promises retain a required expected window and
bounded nonempty risk disclosure, with no physical hold. Cancellation and
rounding identities are fixed by the supported value contract.

Private terms, line/group data and supply promises expose borrowed accessors
and redacted diagnostics, with no automatic serialization/deserialization.
Their constructors grant no participant consent, installed agreement, signer
permission or stock effect. Private wire/hash admission remains separate.

Line and group identities cannot be substituted:

```compile_fail
use radroots_domain::{AgreementLineId, FulfillmentGroupId};
let group = FulfillmentGroupId::from_bytes([0; 32]);
let _: AgreementLineId = group;
```

Private terms have no automatic public wire encoder:

```compile_fail
fn encode(terms: &radroots_domain::Terms) {
    let _ = serde_json::to_string(terms);
}
```

`CommerceContent` retains the19 supported logical record families as immutable
private data before protocol-owned canonical record-ID computation. Its header
requires distinct participants, a participant author, a positive sequence with
the explicit first/later previous-link shape, and at most8 strictly sorted
unique full parent digests. Fixed seller/buyer record roles are checked; all
non-quote families require an order ID. Terms-bearing records match header
participants and declare their exact listing sources. Each payload's required
source references and the author's previous record must appear in the parent
set. Source-array, line, text and parent bounds apply jointly; construction
refuses missing references and never silently omits them to fit a limit.

Complete payloads preserve typed terms digests, positive seller sequences,
explicit authority claims, bounded line/measurement assertions, issue data and
resolution proposals. Measurements cannot repeat or name a line outside their
handover claim. Private text is zeroizing and diagnostics are redacted. There is
no automatic serialization/deserialization, numeric Nostr rumor kind, ambient
clock, signature or installed effect. An exact zero observation remains data;
it never grants a movement, release or trusted measurement.

Declared parent membership is not authenticated coverage. Actual parent
author/thread/order/type/sequence continuity, counterparty consent, source and
terms hashes, deadline readiness, measurement economics and cumulative business
state remain separate admission/execution requirements. Record and terms
digests remain distinct from unsigned chat rumors and signed carrier IDs.

```compile_fail
use radroots_domain::{RecordId, TermsDigest};
let _: TermsDigest = RecordId::from_bytes([0; 32]);
```

```compile_fail
fn encode(content: &radroots_domain::CommerceContent) {
    let _ = serde_json::to_string(content);
}
```

Namespaces remain separate across native calls:

```compile_fail
use radroots_domain::{OperationId, WorkspaceId};
let operation = OperationId::from_bytes([0; 32]);
let _: WorkspaceId = operation;
```

Byte inputs have a fixed width:

```compile_fail
use radroots_domain::OperationId;
let _ = OperationId::from_bytes([0; 31]);
```

Actor, authority, checkpoint, approval, expected-order and complete captured
context descriptors validate their representations without authenticating a
principal. Sequence, generation and wall time use exact canonical decimal
strings. Required nullable fields remain explicit JSON nulls. Checkpoint missing
sources retain full digests independently of structured-record identity.
`CommandContextData` is untrusted data; only an authenticated execution session
can construct the opaque executable context.

`ChatMessage` owns a bounded logical participant view, with separate message,
thread, author, peer, creation time and inquiry/source or reply relation. Plain
text has the shared 4,096 UTF-8 byte limit before JSON/encryption expansion;
empty text and Unicode contents are preserved exactly. Private text uses
zeroizing owned storage and redacted diagnostics, with no automatic payload
serialization. Constructing a value proves no signature, history or installation.

Crate-owned Rust is licensed under MIT OR Apache-2.0; see LICENSE-MIT and
LICENSE-APACHE. The minimal derived currency data retains its upstream
Apache-2.0 license, copyright and attribution in
`data/currency_registry_v1.license` and `data/currency_registry_v1.notice`.
No upstream Rust/generator, country/name data or official XML is included.
