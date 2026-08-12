# radroots_core

`radroots_core` provides the portable, deterministic value model shared by
Radroots domain packages: decimals, currencies, money, percentages,
quantities, units, and pricing.

The crate supports `no_std` environments with `alloc`. It performs no I/O,
starts no tasks or threads, reads no clocks or process state, and owns no
networking or persistence behavior.

## Example

Checked constructors and checked pricing operations are the supported trust
boundary for data received from users, files, databases, or networks:

```rust
use radroots_core::pricing::QuantityPriceOps;
use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};

let price = QuantityPrice::try_new(
    Money::try_new("6.00".parse::<Decimal>()?, Currency::USD)?,
    Quantity::try_new(Decimal::from(2_u32), Unit::MassKg)?,
)?;
let requested = Quantity::try_new(Decimal::from(3_u32), Unit::MassKg)?;
let total = price.try_cost_for_rounded(&requested)?;

assert_eq!(total.amount().to_string(), "9");
# Ok::<(), radroots_core::Error>(())
```

The same program is available as the checked
[`pricing`](examples/checked_pricing.rs) example.

## Public API

The crate root intentionally exports the common value types and aggregate
[`Error`](https://docs.rs/radroots_core/latest/radroots_core/enum.Error.html).
Focused errors and operations remain in these modules:

- `currency` — canonical three-letter ASCII currency codes;
- `decimal` — fixed-precision decimal parsing, conversion, and checked math;
- `money` — non-negative monetary values and currency-aware quantization;
- `percent` — signed percentage values and percentage calculations;
- `pricing` — quantity prices, discounts, and checked pricing operations;
- `quantity` — non-negative amounts associated with units;
- `unit` — unit parsing, dimensions, and deterministic conversions.

The normative responsibility and dependency boundary are defined by the
[`radroots_core` package charter](../../contracts/crates/release_v1/radroots_crates_release_v1.toml).
The reviewed pre-release surface is recorded in the
[`radroots_core` API baseline](../../contracts/api_baselines/radroots_core.txt).

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | Implements `std::error::Error`; value behavior remains portable. |
| `serde` | yes | Implements canonical serialization and deserialization for public values. |

Disabling default features leaves the `no_std` + `alloc` value model. Features
are additive; enabling one does not select a runtime, backend, global state, or
side effect.

## Invariants and untrusted input

Use `try_*`, `checked_*`, parsing, and exact-conversion APIs at trust
boundaries. They reject negative money or quantities, mismatched currencies or
units, division by zero, overflow, and lossy exact conversions as applicable.
Public composite values keep invariant-bearing fields private and expose only
checked construction and arithmetic, so invalid native state cannot be created
through the supported public API.

Currency and unit parsers normalize their documented textual spellings.
Decimal display and serialization normalize insignificant trailing zeroes.
Pricing quantization uses deterministic currency exponents and documents its
rounding strategy on the operation that performs the rounding.

The crate forbids unsafe Rust. It does not process credentials, authorize
actors, or provide confidentiality or authenticity. Callers must impose
application-specific limits before accepting unbounded collections or text;
this crate validates value semantics only.

## Serialization

With `serde`, decimal numeric components are encoded as strings so JSON and
other number-limited formats do not introduce floating-point loss. Currencies
and units use their canonical string codes; aggregate values use named fields.
Wire compatibility is governed by the repository conformance vectors.

Deserializing a composite value re-applies the same invariants as its checked
constructor. Callers remain responsible for application-specific policy and
resource limits before committing untrusted decoded data.

## Execution and commit semantics

Operations are synchronous and deterministic. There are no asynchronous
cancellation points, deadlines, retries, callbacks, or partial external side
effects. A successful call returns its complete value; an error returns before
any durable commit because the crate owns no durable state. Methods taking
`&mut self` document whether an error leaves the receiver unchanged.

## Intended consumers

`radroots_core` is intended for lower-level Radroots domain crates and for
integrators that need the portable value model directly. Applications should
normally begin with the curated `radroots` crate or the advanced
`radroots_sdk` composition surface.

The package is pre-1.0. Its durable responsibility and package identity are
fixed, while API-breaking changes follow the workspace's pre-1.0 versioning
policy.

## License

Licensed under either Apache-2.0 or MIT, at your option.
