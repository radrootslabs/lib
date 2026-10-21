# Native binding qualification host

This standalone test library composes the real shared client, protected native
outbox, file-secret provider and signer with an isolated loopback relay. It
reexports the production `radroots_ffi` component and exports test setup and
measurement controls in its own component. It contains no product commands,
validators, outbox or recovery implementation.

Generated Swift and generic Kotlin callers must invoke the production objects.
Rust host setup, measured effects and language execution remain separate proof
scopes. Synthetic keys, test grants, pause controls and the deliberate panic
probe belong only to this fixture. They are not product API or custody,
hardware, app-migration, signing or release qualification.

Build and test this independent manifest with the `native_binding_fixture`
Cargo profile, after the root producer/generator build. Keep every command
inside `cargo extbuild run --`. The named profile prevents fixed-name FFI
rlib collisions without overriding extbuild's target-directory routing.

After building the fixture and `radroots_ffi_bindgen`, execute the actual
language lane from the Lib root:

```sh
cargo extbuild run -- cargo build -p xtask --bin radroots_ffi_bindgen --locked
cargo extbuild run -- node crates/radroots_ffi/tests/native/qualify.mjs \
  --out /absolute/new/external/evidence-directory \
  --kotlinc /absolute/kotlin/bin/kotlinc \
  --jna /absolute/jna.jar \
  --java /absolute/java
```

The lane records actual tool versions, source and binary hashes, deterministic
generation, compiler output and real workflow results. Kotlin's coroutines
runtime is resolved from the specified Kotlin installation. This lane requires
native macOS with Swift6, Kotlin2.2 and Java21. Plain nonthrowing Swift panic
conversion is a deliberately isolated process-failure probe; Result-shaped
panics must be caught as unexpected errors. No panic probe belongs to product
API. Original failures remain in their original evidence directories.
