#!/usr/bin/env bash
set -euo pipefail

tracked_artifacts="$(git ls-files 'target/ts-rs/**' 'target/sdk-export-ci/**')"

if [[ -n $tracked_artifacts ]]; then
  echo "committed generated typescript artifacts are not allowed under target/ts-rs or target/sdk-export-ci"
  echo "$tracked_artifacts"
  exit 1
fi

echo "no committed generated typescript artifacts found under target/ts-rs or target/sdk-export-ci"
