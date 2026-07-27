# Identity, signing, and secret ownership migration

`radroots_identity` now owns public values only: `PublicKey`, `IdentityId`,
`AccountId`, `PublicIdentity`, `Profile`, and `Username`. The removed
`RadrootsIdentity` API, raw secret bytes, key generation, nsec encoding, NIP-49
encryption/decryption, and encrypted identity files have no compatibility
aliases in this package.

The approved destination boundaries are:

- `radroots_nostr::key` for explicit Nostr key parsing, nsec/NIP-49 conversion,
  and host-requested local Nostr key creation;
- `radroots_signing` for the signer SPI, requests, receipts, authorization, and
  actor provenance, without owning raw secret bytes;
- `radroots_nostr::signing` for concrete local Nostr signing adapters;
- `radroots_secrets::{reference, provider, envelope, wrapping}` for secret
  references, providers, wrapping, and versioned encrypted envelopes;
- host storage adapters composed from `radroots_secrets` and
  `radroots_storage_sqlite` for durable secret persistence.

Those destination APIs are introduced by their ordered release checkpoints.
Until then, callers must not recreate secret ownership in `radroots_identity`
or add a compatibility shim. Public identity profile file helpers remain only
for the immediately following filesystem-extraction checkpoint.
