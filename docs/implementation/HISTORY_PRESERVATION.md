# Standalone history preservation

Step 015 is satisfied under `RCRV1-DEV-001` without importing or combining
repository history. This repository remains the independent source authority
for the 17 lower release-v1 packages.

## Verified checkpoint

- repository: `git@github.com:radrootslabs/lib.git`
- reviewed baseline: `466f3cc36739179bc17edb9db796530729ba5219`
- verified candidate parent: `fab75a9d3950b92ed70e7e3d8cec0d55d1caf34b`
- baseline relationship: the reviewed baseline is an ancestor of the verified
  candidate parent
- submodules: none
- import, subtree, filter-repo, history merge, repository rename, or archive:
  not required and not performed

`git log --follow` retains representative history for
`crates/core/Cargo.toml` and `tools/xtask/src/main.rs`. `git fsck --full`
completed successfully with no corrupt or missing reachable objects. It
reported only unreachable dangling objects retained by Git; those are not
part of the release candidate and were not pruned or modified.

The next workspace steps must preserve this repository, its lockfile, and its
release boundary independently from `radrootslabs/sdk`.
