# Radroots native bindings

This source owns the concrete UniFFI boundary over the current `radroots`
client. Native app startup supplies its actual configured client and actual
authenticated workspace through the Rust host constructors. Foreign input
cannot create a grant, signer claim or delivery permission.

Known input validation and command execution remain in shared Rust. The
boundary owns language conversion, with exact identifiers and integer/decimal
strings, explicit optional values and tagged business results. It does not
implement a second command engine, storage adapter, signer or retry policy.

Effectful command errors preserve the shared execution failure's actual code,
retry class, original operation and admission. Optional `SignerRejection` carries
a fixed reason and optional SHA-256 fingerprint with exact covered/total byte
counts as canonical decimal strings. A prefix digest does not claim full
coverage. No raw signer response or private input enters these diagnostics.
Future unsupported reasons remain explicit, and unsupported state projection
retains any bounded diagnostic. The generated error schema changes with this
contract; source-owned Swift and Kotlin fixtures regenerate and execute it.

Host injection is distinct from a general foreign-language open/enrollment
factory, platform custody and callback qualification. These additional surfaces
must be implemented and exercised before they are advertised. This package is
not a compatibility wrapper for `radroots_sdk_ffi`.

Lib's private `xtask` tooling owns the `radroots_ffi_bindgen` executable. It
pins UniFFI0.32.2 and applies
the `radroots-close-v1` Kotlin generation policy. When the actual native
metadata declares `close`, generated `close` calls that method; the conflicting
automatic AutoCloseable destruction alias is omitted. `destroy` remains the
explicit generated handle-release operation. It is not asynchronous resource
closure. Other generated objects retain their ordinary AutoCloseable behavior.
Generation fails if the expected upstream template shape changes. Outputs are
produced in staging and must use a new output directory and `--no-format`.

The `radroots-swift-cancellation-v1` policy also connects Swift task cancellation
to the existing Rust-future cancel ABI and converts its cancelled status into
`CancellationError`. A synchronized lifetime owner prevents cancellation from
using a released handle. Already-cancelled calls refuse before creating a Rust
future. Cancellation ends observation; the shared runtime retains admitted
work and obtained material. No command lifecycle is implemented in Swift.
