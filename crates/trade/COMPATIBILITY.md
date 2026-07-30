# Temporary compatibility quarantine

`radroots_trade` is one approved Release V1 package and remains
`publish = false` during migration. This quarantine does not create or
preserve a second trade package identity.

The only canonical Release V1 modules are `evidence`, `model`, `reducer`,
`validation`, and `workflow`. Step 098 removed the unused `identity` and
`prelude` modules plus the versioned reducer reexports formerly exposed from
`workflow`.

The remaining documentation-hidden surfaces have verified first-party
consumers:

- `operational_listing` is consumed by the standalone SDK listing runtime,
  its integration tests, the operations registry, and generated-binding
  inventories. SDK listing operations migrate in Step 238 and binding
  ownership migrates in Steps 261-262.
- `validation_receipt` is consumed by the private SP1 trade host. Its
  cross-language and generated-contract ownership is resolved in Steps
  261-262.
- the `dto-bindgen` feature and hidden `dto` module support the same temporary
  generated inventories until Steps 261-262.
- the `serde_json` feature name is still selected by the standalone SDK and
  private SP1 host. Their final feature and contract migrations occur in
  Steps 242 and 261-262.
- the otherwise non-normative `radroots_event_codec` dependency exists only
  beneath `operational_listing` and leaves the package with that quarantine.

These routes are source-compatible access to the same package while it is
non-publishable. They are not supported Release V1 APIs, and no new consumer
may adopt them.

Step 313 is the exact final-removal checkpoint for every declaration,
feature, and dependency in this quarantine. That step must repeat the
all-first-party search, remove the compatibility declarations and this file,
regenerate the API baseline, and pass the complete downstream matrix before
Release V1 approval.
