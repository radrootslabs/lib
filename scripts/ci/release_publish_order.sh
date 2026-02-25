#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root_dir"

mode="${1:-publish}"
if [[ "$mode" != "publish" && "$mode" != "dry-run" ]]; then
  echo "usage: scripts/ci/release_publish_order.sh [publish|dry-run]"
  exit 2
fi

release_version="$(
  awk '
    /^\[release\]/ { in_release = 1; next }
    in_release && /^version = / {
      gsub(/"/, "", $3);
      print $3;
      exit
    }
  ' contract/release/publish-set.toml
)"

if [[ -z "$release_version" ]]; then
  echo "failed to resolve release.version from contract/release/publish-set.toml"
  exit 1
fi

order_file="$(mktemp)"
trap 'rm -f "$order_file"' EXIT

awk '
  /^\[publish_order\]/ { in_order = 1; next }
  /^\[/ && in_order { exit }
  in_order && /"/ {
    line = $0
    gsub(/[" ,]/, "", line)
    if (length(line) > 0) print line
  }
' contract/release/publish-set.toml > "$order_file"

if [[ ! -s "$order_file" ]]; then
  echo "publish_order.crates list is empty"
  exit 1
fi

while IFS= read -r crate; do
  [ -n "$crate" ] || continue
  if [[ "$mode" == "dry-run" ]]; then
    log_file="$(mktemp)"
    if cargo publish --dry-run --locked --allow-dirty -p "$crate" >"$log_file" 2>&1; then
      cat "$log_file"
      rm -f "$log_file"
      continue
    fi

    missing_dep="$(sed -n 's/.*no matching package named `\([^`]*\)`.*/\1/p' "$log_file" | head -n1)"
    if [[ -n "$missing_dep" ]] && grep -Fxq "$missing_dep" "$order_file"; then
      echo "dry-run defer for ${crate}: dependency ${missing_dep} is not yet published"
      rm -f "$log_file"
      continue
    fi

    cat "$log_file"
    rm -f "$log_file"
    exit 1
  fi

  cargo publish --locked -p "$crate"
  for attempt in $(seq 1 30); do
    if curl -fsSL "https://crates.io/api/v1/crates/${crate}/${release_version}" >/dev/null 2>&1; then
      break
    fi
    if [[ "$attempt" == "30" ]]; then
      echo "crate ${crate} version ${release_version} not visible on crates.io after publish"
      exit 1
    fi
    sleep 10
  done
done < "$order_file"

echo "publish sequence complete for release ${release_version}"
