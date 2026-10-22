# Radroots domain

Independently implemented exact values for the shared SDK successor. This
pre-release package provides validated values and public quote-required offer
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
and performs no implicit physical conversion. Complete option/terms admission,
fixed-price listing authoring, agreement installation and command effects remain
separate requirements; calculation values establish no authorization or stock.

`Quantity` uses the existing measurement identity and exact decimal bounds.
Conversions between g/kg and mL/L are explicit and reject precision loss;
arithmetic requires matching units. `each` counts remain integral. Standalone
`ContentMeasure` descriptors distinguish fixed, estimated and unspecified
contents; estimates do not provide fixed conversions or stock backing. Existing
sale options validate selected amounts against their minimum, increment and
integral package/count rules. Advertised supply establishes no quantity ceiling
or actual availability. Complete listing/term integration remains separate.

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
contain no invented zero amount or currency. Optional opaque authority and
producer metadata establish no permission or endpoint trust. These immutable
terms serialize public data explicitly and redact ordinary diagnostics.

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
