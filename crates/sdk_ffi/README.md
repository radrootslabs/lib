# radroots_sdk_ffi

Private UniFFI bindings over the host-neutral `radroots_sdk` engine. The V1
surface exposes deterministic memory-client construction, versioned capability
DTOs, secret-safe errors, and explicit asynchronous close. It does not own
keychain prompts, background execution, application scheduling, or presentation
DTOs; those remain mobile-host responsibilities.

This Rust build crate is never published. Generated Swift and Kotlin artifacts
are qualified independently from the public 19-crate Rust inventory.

The SDK artifact producer uses the source-owned `radroots_ffi_bindgen` binary
in `xtask`. It preserves the actual async close method, keeps native string
error messages as Kotlin's exception message, and gives Swift observer
cancellation an explicit safe lifetime. Generated files must never be patched
by consumers. Source-lock identity includes the generator implementation.

`tests/native/qualify.mjs` accepts explicit library, generator, output, Kotlin,
JNA and Java paths. It checks repeated generation, strict Swift6/Kotlin
compilation, real native error-field round trips and actual memory-client
closure. That retained bridge test does not qualify participant custody or
application behavior.
