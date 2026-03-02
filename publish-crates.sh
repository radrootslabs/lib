#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$root_dir"

mode="publish"
case "${1:-}" in
  --dry-run)
    mode="dry-run"
    shift
    ;;
  --publish)
    mode="publish"
    shift
    ;;
  "" )
    ;;
  *)
    ;;
esac

requested="${*:-}"

if [[ "$mode" == "publish" ]] && [[ -z "${CARGO_REGISTRY_TOKEN:-}" ]] && [[ -n "${CRATES_IO_TOKEN:-}" ]]; then
  export CARGO_REGISTRY_TOKEN="${CRATES_IO_TOKEN}"
fi

if [[ "$mode" == "publish" ]] && [[ -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
  echo "set CARGO_REGISTRY_TOKEN or CRATES_IO_TOKEN before publish"
  exit 1
fi

exec ./scripts/ci/release_publish_order.sh "$mode" "$requested"
