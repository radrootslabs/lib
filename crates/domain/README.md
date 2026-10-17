# Radroots domain

Independently implemented exact values for the shared SDK successor. This
package is under construction and is not published or part of the current
public release closure. It does not yet implement listings, agreements or
command execution.

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

Licensed under MIT OR Apache-2.0; see LICENSE-MIT and LICENSE-APACHE.
