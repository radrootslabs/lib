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
It performs no I/O, clock, randomness, storage or network work. Currency and
economic rules remain separate implementation requirements.

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

Licensed under MIT OR Apache-2.0; see LICENSE-MIT and LICENSE-APACHE.
