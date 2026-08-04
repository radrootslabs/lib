# Compatibility shim retirement

Step 313 completed the coordinated compatibility retirement after every
first-party consumer cut over to the final Release V1 owners. No compatibility
package, hidden prelude, alternate crate identity, or second contract authority
remains in the release source graph.

| Retired bridge | Final owner | Cutover evidence | Final removal |
| --- | --- | --- | --- |
| `radroots_authority` | `radroots_signing` | downstream Steps 269-294 | Step 313 |
| `radroots_nostr_signer` | `radroots_signing`, `radroots_nostr_connect`, Myc-private state | crate Steps 109-143; SDK Step 248; downstream Steps 269-294 | Step 313 |
| hidden `radroots_nostr_connect::prelude` and prefixed client bridge | final `radroots_nostr_connect` client state machine | CLI Step 271; Myc Step 288; residual consumers Steps 293-294 | Step 313 |
| `radroots_geocoder` | `radroots_geonames` | SDK Steps 226 and 248 | Step 313 source census |

Release policy contains only the exact 19 publishable packages plus explicitly
deferred private/preview packages. A source census and architecture validation
must fail if any retired package name or compatibility route returns.

`radroots_nostr_runtime`, the private NostrDB runtime adapter, and
`radroots_net` were deleted during Step 301 qualification. The full workspace
matrix proved their assigned removal edits had been missed even though their
consumer gates were satisfied, and neither obsolete all-feature closure could
compile against the final transport boundary.
