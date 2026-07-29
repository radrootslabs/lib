# Architecture continuous integration

The pull-request architecture lane is a thin GitHub adapter over the
forge-agnostic repository command:

```sh
nix run .#architecture
```

The command validates the synchronized release specification, workspace and
package metadata, production dependency paths, the Cargo-resolved package-tier
graph, public API implementation leakage, contract artifacts, DTO roots, and
generated-manifest freshness. The same command is exposed as the
`architecture` flake check, so `nix flake check` includes the lane.

The workflow grants only read access to repository contents. Action
dependencies are pinned to full commit identifiers. Its Nix store cache is
content-addressed from the current source and lock inputs; generated files are
read from the checkout on every run and are never restored from a workflow
cache.

Repository administrators may require the `Architecture / architecture`
status after this commit is publicly reachable. Changing branch protection or
other repository administration remains a separate authorized operation.
