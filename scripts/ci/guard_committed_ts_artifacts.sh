#!/usr/bin/env bash
set -euo pipefail

tracked_artifacts="$(git ls-files 'target/ts-rs/**')"

if [[ -n $tracked_artifacts ]]; then
  echo "committed generated typescript artifacts are not allowed under target/ts-rs"
  echo "$tracked_artifacts"
  exit 1
fi

echo "no committed generated typescript artifacts found under target/ts-rs"
