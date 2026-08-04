# Release V1 breaking migration

Release V1 is an intentional breaking cut to the 17-package public library
surface. All packages remain in `oss/lib`, use version `0.1.0-alpha`, and are
qualified independently from the two front-door packages in `oss/sdk`.

| Retired surface | Release V1 owner |
| --- | --- |
| authority and Nostr signer packages | `radroots_signing` and `radroots_nostr_connect` |
| vault and protected-store packages | `radroots_secrets` plus `radroots_storage` / `radroots_storage_sqlite` |
| event store, event index, outbox, and runtime store packages | `radroots_storage`, `radroots_storage_sqlite`, and `radroots_sync` |
| runtime and broad network packages | explicit host composition over `radroots_transport`, `radroots_transport_nostr`, storage, and sync |
| geocoder package | `radroots_geonames` |
| event-codec domain roots | `radroots_event_codec::{decode,encode,verify,admission}` |
| trade operational-listing and validation-receipt modules | `radroots_sdk::listing` for product operations; `radroots_event` retains the canonical listing model |
| prefixed transport target aliases and Reticulum helpers | `TransportId`, `Target`, `TargetSet`, and `target::{TargetScope,TargetLabel,TargetFingerprint}` |
| Nostr Connect prelude and prefixed client bridge | explicit `radroots_nostr_connect` root types and modules |

No compatibility package, hidden prelude, type alias, dual schema, or sibling
source path remains. Consumers must migrate atomically to the final owner; the
release does not provide a deprecated intermediate API.

The 17 packages are enabled only for package-realistic validation. Actual
crates.io publication remains blocked until the approval packet is complete
and a separate operator action is explicitly authorized.
