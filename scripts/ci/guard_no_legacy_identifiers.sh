#!/usr/bin/env bash
set -euo pipefail

matches="$(
  git grep -nI 'tangle' -- . \
    ':(exclude)AGENTS.md' \
    ':(exclude)scripts/ci/guard_no_legacy_identifiers.sh' ||
    true
)"

if [[ -n $matches ]]; then
  echo "legacy identifier 'tangle' is forbidden in tracked oss files"
  echo "$matches"
  exit 1
fi

echo "no legacy 'tangle' identifiers found in tracked oss files"
