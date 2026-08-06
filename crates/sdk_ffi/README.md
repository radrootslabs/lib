# radroots_sdk_ffi

Private UniFFI bindings over the host-neutral `radroots_sdk` engine. The V1
surface exposes deterministic memory-client construction, versioned capability
DTOs, secret-safe errors, and explicit asynchronous close. It does not own
keychain prompts, background execution, application scheduling, or presentation
DTOs; those remain mobile-host responsibilities.

This Rust build crate is never published. Generated Swift and Kotlin artifacts
are qualified independently from the public 19-crate Rust inventory.
