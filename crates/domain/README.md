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

Licensed under MIT OR Apache-2.0; see LICENSE-MIT and LICENSE-APACHE.
