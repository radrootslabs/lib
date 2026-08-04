# Compatibility shim quarantine

First-party source and manifest censuses run before each retirement checkpoint.
Active consumers make immediate deletion unsafe for the bridges below, so only
private source compatibility remains. None is an approved public crate
identity or a second contract authority.

| Bridge | Final owner | Remaining first-party consumers | Assigned cutover | Exact final removal |
| --- | --- | --- | --- | --- |
| `radroots_authority` | `radroots_signing` | `oss/cli`, `oss/studio_app` | downstream Steps 269-293; matrix Step 294 | Step 313 |
| `radroots_nostr_signer` | `radroots_signing`, `radroots_nostr_connect`, Myc-private state | `radroots_nostr_accounts`, `oss/sdk`, `oss/myc`, `oss/cli` | crate Steps 109-143; SDK Step 248; downstream Steps 269-293; matrix Step 294 | Step 313 |
| hidden `radroots_nostr_connect::prelude` and prefixed client bridge | final `radroots_nostr_connect` modules and client state machine | `oss/cli`, `oss/myc`, enterprise NIP-46 adapters, integration harnesses | CLI Step 271; Myc Step 288; residual consumers Step 293; matrix Step 294 | Step 313 |
| `radroots_geocoder` | `radroots_geonames` | standalone `radroots_sdk` GeoNames feature and error adapter | SDK manifest cutover Step 226 | SDK quarantine removal Step 248 |

The remaining compatibility package manifests and `radroots_nostr_connect` keep
`publish = false`. Release policy classifies the shims as private and excludes
them from the exact 19-package public API inventory. The
Compatibility package manifests intentionally have no docs.rs URL. No new
consumer, feature, public contract, or behavior may be added before removal.

The Step 294 matrix must prove all listed consumers have migrated. Step 313
then removes the packages, their workspace/dependency entries, their source
names, and every remaining source pin.

`radroots_nostr_runtime`, the private NostrDB runtime adapter, and
`radroots_net` were deleted during Step 301 qualification. The full workspace
matrix proved their assigned removal edits had been missed even though their
consumer gates were satisfied, and neither obsolete all-feature closure could
compile against the final transport boundary.
