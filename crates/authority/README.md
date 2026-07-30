# `radroots_authority` transition package

`radroots_authority` is a private, non-publishable transition package. New
code must use `radroots_signing` for actor context, sign requests, sign
receipts, and signer injection.

The package remains only because the standalone `oss/cli` and
`oss/studio_app` repositories still consume its source API. Those consumers
are assigned to the first-party downstream migration phase (Steps 269-293).
The package must be removed at Step 313 after the Step 294 compatibility
matrix proves every consumer has completed its cutover.

This package is not an approved public crate, must keep `publish = false`, and
must not acquire new consumers, features, or behavior while it is retained.
