#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root_dir"

mode="${1:-publish}"
if [[ $mode != "publish" && $mode != "dry-run" ]]; then
  echo "usage: scripts/ci/release_publish_order.sh [publish|dry-run] [crate names]"
  exit 2
fi

requested_raw="${2:-}"
requested_raw="${requested_raw//,/ }"

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

if [[ -z $release_version ]]; then
  echo "failed to resolve release.version from contract/release/publish-set.toml"
  exit 1
fi

order_file="$(mktemp)"

awk '
  /^\[publish_order\]/ { in_order = 1; next }
  /^\[/ && in_order { exit }
  in_order && /"/ {
    line = $0
    gsub(/[" ,]/, "", line)
    if (length(line) > 0) print line
  }
' contract/release/publish-set.toml >"$order_file"

if [[ ! -s $order_file ]]; then
  echo "publish_order.crates list is empty"
  exit 1
fi

selected_file="$(mktemp)"
requested_file="$(mktemp)"
trap 'rm -f "$order_file" "$selected_file" "$requested_file"' EXIT

crate_version_visible() {
  local crate="$1"
  curl -fsSL "https://crates.io/api/v1/crates/${crate}/${release_version}" >/dev/null 2>&1
}

seconds_until_http_date() {
  local retry_after="$1"
  python3 - "$retry_after" <<'PY'
import datetime
import email.utils
import sys

retry_after = sys.argv[1].strip()
try:
    target = email.utils.parsedate_to_datetime(retry_after)
except Exception:
    print(0)
    raise SystemExit(0)

if target.tzinfo is None:
    target = target.replace(tzinfo=datetime.timezone.utc)

now = datetime.datetime.now(datetime.timezone.utc)
remaining = (target - now).total_seconds()
print(max(1, int(remaining) + 1))
PY
}

publish_with_retry() {
  local crate="$1"
  local attempt=1
  while true; do
    local log_file
    log_file="$(mktemp)"
    if cargo publish --locked -p "$crate" >"$log_file" 2>&1; then
      cat "$log_file"
      rm -f "$log_file"
      return 0
    fi

    cat "$log_file"

    if grep -Fq "already uploaded" "$log_file"; then
      echo "crate ${crate} version ${release_version} is already uploaded"
      rm -f "$log_file"
      return 0
    fi

    if grep -Fq "429 Too Many Requests" "$log_file"; then
      local retry_after
      retry_after="$(sed -n 's/.*Please try again after \(.*GMT\).*/\1/p' "$log_file" | head -n1)"
      local sleep_secs=0
      if [[ -n $retry_after ]]; then
        sleep_secs="$(seconds_until_http_date "$retry_after")"
      fi
      if [[ $sleep_secs -le 0 ]]; then
        sleep_secs=$((30 + attempt * 15))
      fi
      echo "publish rate-limited for ${crate}; retry ${attempt} in ${sleep_secs}s"
      rm -f "$log_file"
      sleep "$sleep_secs"
      attempt=$((attempt + 1))
      continue
    fi

    rm -f "$log_file"
    return 1
  done
}

if [[ -n $requested_raw ]]; then
  for token in $requested_raw; do
    [[ -n $token ]] || continue
    echo "$token" >>"$requested_file"
  done
  sort -u "$requested_file" -o "$requested_file"

  while IFS= read -r token; do
    if ! grep -Fxq "$token" "$order_file"; then
      echo "requested crate is not in publish_order.crates: ${token}"
      exit 1
    fi
  done <"$requested_file"

  while IFS= read -r crate; do
    [[ -n $crate ]] || continue
    if grep -Fxq "$crate" "$requested_file"; then
      echo "$crate" >>"$selected_file"
    fi
  done <"$order_file"
else
  cp "$order_file" "$selected_file"
fi

while IFS= read -r crate; do
  [ -n "$crate" ] || continue
  if [[ $mode == "dry-run" ]]; then
    log_file="$(mktemp)"
    if cargo publish --dry-run --locked --allow-dirty -p "$crate" >"$log_file" 2>&1; then
      cat "$log_file"
      rm -f "$log_file"
      continue
    fi

    missing_dep="$(sed -n 's/.*no matching package named `\([^`]*\)`.*/\1/p' "$log_file" | head -n1)"
    if [[ -n $missing_dep ]] && grep -Fxq "$missing_dep" "$order_file"; then
      echo "dry-run defer for ${crate}: dependency ${missing_dep} is not yet published"
      rm -f "$log_file"
      continue
    fi

    cat "$log_file"
    rm -f "$log_file"
    exit 1
  fi

  if crate_version_visible "$crate"; then
    echo "skip ${crate}: version ${release_version} already visible on crates.io"
    continue
  fi

  publish_with_retry "$crate"
  for attempt in $(seq 1 30); do
    if crate_version_visible "$crate"; then
      break
    fi
    if [[ $attempt == "30" ]]; then
      echo "crate ${crate} version ${release_version} not visible on crates.io after publish"
      exit 1
    fi
    sleep 10
  done
done <"$selected_file"

echo "publish sequence complete for release ${release_version}"
