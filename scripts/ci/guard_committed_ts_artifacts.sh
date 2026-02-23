#!/usr/bin/env bash
set -euo pipefail

tracked_artifacts="$(git ls-files 'crates/*/bindings/**')"

if [[ -n "$tracked_artifacts" ]]; then
  echo "committed ts artifacts are not allowed under crates/*/bindings/**"
  echo "$tracked_artifacts"
  exit 1
fi

echo "no committed ts artifacts found under crates/*/bindings/**"
