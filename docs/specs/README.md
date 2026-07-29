# Release specification index

This directory carries the coordinated `radroots.crates.release.v1` contract
for the two existing standalone Rust repositories.

The `radrootslabs/lib` repository owns packages 1-17, from `radroots_core`
through `radroots_geonames`. The `radrootslabs/sdk` repository owns packages
18-19, `radroots_sdk` and `radroots`. No third Rust repository is part of this
release architecture.

## Files

- `radroots_crates_release_v1.md` is the normative architecture and
  publication specification.
- `radroots_crates_release_v1.toml` is the machine-readable package,
  dependency, feature, and repository-allocation catalog.
- `radroots_crates_release_v1_inventory.csv` is the reviewable package
  inventory.
- `radroots_crates_release_v1.dot` is the reviewable dependency and repository
  ownership graph.
- `radroots_crates_release_v1.sha256` pins the synchronized contract artifact
  contents.

## Precedence and change control

Repository instruction files govern how work is performed. Within this
release contract, the Markdown specification is normative, the TOML catalog
is its executable representation, and the CSV and DOT files are review aids.
Current code and tests are implementation evidence, not authority to silently
change the package architecture.

The four contract artifacts and their hashes MUST match the copies in
`radrootslabs/sdk`. A change to package identity, ownership, dependency
direction, or release policy MUST update both repositories together and MUST
fail validation if the copies diverge.

During migration, package manifests remain non-publishable until their
package-realistic release gates pass. Cross-repository dependencies use
registry versions in release candidates; a sibling checkout is never a
production dependency.
